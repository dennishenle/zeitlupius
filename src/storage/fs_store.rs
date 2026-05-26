use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::model::{Project, ProjectName, Session, SessionId};
use crate::storage::ProjectStore;
use crate::storage::csv_io::{read_project, write_project};

pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn projects_dir(&self) -> PathBuf {
        self.root.join("projects")
    }
    pub fn lock_path(&self) -> PathBuf {
        self.root.join(".lock")
    }
    pub fn project_path(&self, name: &ProjectName) -> PathBuf {
        self.projects_dir().join(format!("{name}.csv"))
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.projects_dir())?;
        if !self.lock_path().exists() {
            File::create(self.lock_path())?;
        }
        Ok(())
    }

    fn with_lock<R>(&self, f: impl FnOnce() -> Result<R>) -> Result<R> {
        self.ensure_dirs()?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.lock_path())?;
        let mut lock = fd_lock::RwLock::new(file);
        let _guard = lock.write()?;
        f()
    }

    fn atomic_write(&self, target: &Path, bytes: &[u8]) -> Result<()> {
        let dir = target.parent().expect("project path has parent");
        fs::create_dir_all(dir)?;
        let pid = std::process::id();
        let tmp = dir.join(format!(
            "{}.tmp.{pid}",
            target.file_name().unwrap().to_string_lossy()
        ));
        {
            let mut f = File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, target)?;
        Ok(())
    }

    fn load_inner(
        &self,
        name: &ProjectName,
    ) -> Result<(Project, crate::storage::csv_io::Migrated)> {
        let path = self.project_path(name);
        if !path.exists() {
            return Err(Error::ProjectNotFound(name.to_string()));
        }
        let bytes = fs::read(&path)?;
        read_project(name, bytes.as_slice())
    }

    fn save_inner(&self, project: &Project) -> Result<()> {
        let mut buf = Vec::new();
        write_project(project, &mut buf)?;
        self.atomic_write(&self.project_path(&project.name), &buf)
    }
}

impl ProjectStore for FsStore {
    fn list(&self) -> Result<Vec<ProjectName>> {
        self.ensure_dirs()?;
        let mut names = Vec::new();
        for entry in fs::read_dir(self.projects_dir())? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("csv") {
                continue;
            }
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => continue,
            };
            if let Ok(n) = ProjectName::parse(stem) {
                names.push(n);
            }
        }
        names.sort();
        Ok(names)
    }

    fn load(&self, name: &ProjectName) -> Result<Project> {
        self.with_lock(|| {
            let (project, migrated) = self.load_inner(name)?;
            if matches!(migrated, crate::storage::csv_io::Migrated::Yes) {
                // Rewrite to persist generated IDs. Best-effort: on failure we still
                // return the in-memory project so callers can proceed.
                let _ = self.save_inner(&project);
            }
            Ok(project)
        })
    }

    fn create(&self, name: &ProjectName) -> Result<()> {
        self.with_lock(|| {
            let path = self.project_path(name);
            if path.exists() {
                return Err(Error::ProjectAlreadyExists(name.to_string()));
            }
            self.save_inner(&Project {
                name: name.clone(),
                sessions: vec![],
            })
        })
    }

    fn delete(&self, name: &ProjectName) -> Result<()> {
        self.with_lock(|| {
            let path = self.project_path(name);
            if !path.exists() {
                return Err(Error::ProjectNotFound(name.to_string()));
            }
            fs::remove_file(&path)?;
            Ok(())
        })
    }

    fn append_start(
        &self,
        name: &ProjectName,
        start: &jiff::Zoned,
        note: Option<&str>,
    ) -> Result<()> {
        self.with_lock(|| {
            let (mut p, _) = self.load_inner(name)?;
            if let Some(last) = p.sessions.last()
                && last.is_running()
            {
                return Err(Error::AlreadyRunning(
                    name.to_string(),
                    last.start.to_string(),
                ));
            }
            let mut id = SessionId::generate();
            for _ in 0..5 {
                if !p.sessions.iter().any(|s| s.id == id) {
                    break;
                }
                id = SessionId::generate();
            }
            if p.sessions.iter().any(|s| s.id == id) {
                return Err(Error::Corrupt(
                    "session id collision after 5 retries".into(),
                ));
            }
            p.sessions.push(Session {
                id,
                start: start.clone(),
                stop: None,
                note: note.map(str::to_string),
            });
            self.save_inner(&p)
        })
    }

    fn close_open(&self, name: &ProjectName, stop: &jiff::Zoned) -> Result<()> {
        self.with_lock(|| {
            let (mut p, _) = self.load_inner(name)?;
            let Some(last) = p.sessions.last_mut() else {
                return Err(Error::NotRunning(name.to_string()));
            };
            if !last.is_running() {
                return Err(Error::NotRunning(name.to_string()));
            }
            last.stop = Some(stop.clone());
            self.save_inner(&p)
        })
    }

    fn delete_session(&self, name: &ProjectName, id: &SessionId) -> Result<Session> {
        self.with_lock(|| {
            let (mut p, _) = self.load_inner(name)?;
            let pos = p
                .sessions
                .iter()
                .position(|s| s.id == *id)
                .ok_or_else(|| Error::SessionNotFound(name.to_string(), id.to_string()))?;
            let removed = p.sessions.remove(pos);
            self.save_inner(&p)?;
            Ok(removed)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::{civil::date, tz::TimeZone};
    use tempfile::TempDir;

    fn store() -> (TempDir, FsStore) {
        let td = TempDir::new().unwrap();
        let s = FsStore::new(td.path().to_path_buf());
        (td, s)
    }

    fn z(h: i8, min: i8) -> jiff::Zoned {
        date(2026, 5, 4)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn create_then_list() {
        let (_td, s) = store();
        let n = ProjectName::parse("rust-zlp").unwrap();
        s.create(&n).unwrap();
        assert_eq!(s.list().unwrap(), vec![n]);
    }

    #[test]
    fn list_ignores_non_csv() {
        let (_td, s) = store();
        s.ensure_dirs().unwrap();
        std::fs::write(s.projects_dir().join("README.txt"), "noise").unwrap();
        assert!(s.list().unwrap().is_empty());
    }

    #[test]
    fn start_stop_persists() {
        let (_td, s) = store();
        let n = ProjectName::parse("p").unwrap();
        s.create(&n).unwrap();
        s.append_start(&n, &z(9, 0), Some("hi")).unwrap();
        s.close_open(&n, &z(10, 0)).unwrap();
        let p = s.load(&n).unwrap();
        assert_eq!(p.sessions.len(), 1);
        assert_eq!(p.sessions[0].note.as_deref(), Some("hi"));
        assert!(!p.sessions[0].is_running());
    }

    #[test]
    fn delete_missing_errors() {
        let (_td, s) = store();
        let n = ProjectName::parse("nope").unwrap();
        let err = s.delete(&n).unwrap_err();
        assert!(matches!(err, Error::ProjectNotFound(_)));
    }

    #[test]
    fn duplicate_create_errors() {
        let (_td, s) = store();
        let n = ProjectName::parse("p").unwrap();
        s.create(&n).unwrap();
        assert!(matches!(
            s.create(&n).unwrap_err(),
            Error::ProjectAlreadyExists(_)
        ));
    }

    #[test]
    fn legacy_file_is_rewritten_on_first_load() {
        let (_td, s) = store();
        s.ensure_dirs().unwrap();
        let path = s.project_path(&ProjectName::parse("p").unwrap());
        let legacy = "start,stop,note\n2026-05-04T09:00:00+00:00[UTC],2026-05-04T10:00:00+00:00[UTC],morning\n";
        std::fs::write(&path, legacy).unwrap();

        let p = s.load(&ProjectName::parse("p").unwrap()).unwrap();
        assert_eq!(p.sessions.len(), 1);

        // File on disk now has the `id` column.
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(
            after.contains("start,stop,note,id"),
            "header was not rewritten: {after}"
        );

        // Second load reads the persisted id (no further migration).
        let p2 = s.load(&ProjectName::parse("p").unwrap()).unwrap();
        assert_eq!(
            p2.sessions[0].id, p.sessions[0].id,
            "id changed across loads"
        );
    }

    #[test]
    fn delete_session_persists_to_disk() {
        let (_td, s) = store();
        let n = ProjectName::parse("p").unwrap();
        s.create(&n).unwrap();
        s.append_start(&n, &z(9, 0), None).unwrap();
        s.close_open(&n, &z(10, 0)).unwrap();
        s.append_start(&n, &z(11, 0), None).unwrap();
        let p = s.load(&n).unwrap();
        let id = p.sessions[0].id.clone();
        s.delete_session(&n, &id).unwrap();
        let p2 = s.load(&n).unwrap();
        assert_eq!(p2.sessions.len(), 1);
        assert!(p2.sessions[0].is_running());
    }

    #[test]
    fn concurrent_starts_serialize_via_lock() {
        use std::sync::Arc;
        use std::thread;
        let (_td, s) = store();
        let s = Arc::new(s);
        let n = ProjectName::parse("p").unwrap();
        s.create(&n).unwrap();
        let s1 = s.clone();
        let n1 = n.clone();
        let h1 = thread::spawn(move || s1.append_start(&n1, &z(9, 0), None));
        let s2 = s.clone();
        let n2 = n.clone();
        let h2 = thread::spawn(move || s2.append_start(&n2, &z(10, 0), None));
        let r1 = h1.join().unwrap();
        let r2 = h2.join().unwrap();
        let oks = [r1.is_ok(), r2.is_ok()].iter().filter(|x| **x).count();
        assert_eq!(
            oks, 1,
            "exactly one starter wins; the other must error cleanly"
        );
        let p = s.load(&n).unwrap();
        assert_eq!(p.sessions.len(), 1);
    }
}
