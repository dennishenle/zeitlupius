use crate::error::Result;
use crate::model::{Project, ProjectName};

pub mod csv_io;
pub mod fs_store;

pub use fs_store::FsStore;

pub trait ProjectStore: Send + Sync {
    fn list(&self) -> Result<Vec<ProjectName>>;
    fn load(&self, name: &ProjectName) -> Result<Project>;
    fn create(&self, name: &ProjectName) -> Result<()>;
    fn delete(&self, name: &ProjectName) -> Result<()>;
    fn append_start(
        &self,
        name: &ProjectName,
        start: &jiff::Zoned,
        note: Option<&str>,
    ) -> Result<()>;
    fn close_open(&self, name: &ProjectName, stop: &jiff::Zoned) -> Result<()>;
}

#[cfg(test)]
pub mod mem {
    use super::*;
    use crate::error::Error;
    use crate::model::{Session, SessionId};
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    pub struct MemStore {
        inner: Mutex<BTreeMap<String, Vec<Session>>>,
    }

    impl MemStore {
        pub fn new() -> Self {
            Self {
                inner: Mutex::new(BTreeMap::new()),
            }
        }
    }

    impl Default for MemStore {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ProjectStore for MemStore {
        fn list(&self) -> Result<Vec<ProjectName>> {
            let g = self.inner.lock().unwrap();
            g.keys().map(|k| ProjectName::parse(k)).collect()
        }
        fn load(&self, name: &ProjectName) -> Result<Project> {
            let g = self.inner.lock().unwrap();
            let sessions = g
                .get(name.as_str())
                .ok_or_else(|| Error::ProjectNotFound(name.to_string()))?
                .clone();
            Ok(Project {
                name: name.clone(),
                sessions,
            })
        }
        fn create(&self, name: &ProjectName) -> Result<()> {
            let mut g = self.inner.lock().unwrap();
            if g.contains_key(name.as_str()) {
                return Err(Error::ProjectAlreadyExists(name.to_string()));
            }
            g.insert(name.to_string(), vec![]);
            Ok(())
        }
        fn delete(&self, name: &ProjectName) -> Result<()> {
            let mut g = self.inner.lock().unwrap();
            g.remove(name.as_str())
                .ok_or_else(|| Error::ProjectNotFound(name.to_string()))?;
            Ok(())
        }
        fn append_start(
            &self,
            name: &ProjectName,
            start: &jiff::Zoned,
            note: Option<&str>,
        ) -> Result<()> {
            let mut g = self.inner.lock().unwrap();
            let v = g
                .get_mut(name.as_str())
                .ok_or_else(|| Error::ProjectNotFound(name.to_string()))?;
            if let Some(last) = v.last()
                && last.is_running()
            {
                return Err(Error::AlreadyRunning(
                    name.to_string(),
                    last.start.to_string(),
                ));
            }
            v.push(Session {
                id: SessionId::generate(),
                start: start.clone(),
                stop: None,
                note: note.map(str::to_string),
            });
            Ok(())
        }
        fn close_open(&self, name: &ProjectName, stop: &jiff::Zoned) -> Result<()> {
            let mut g = self.inner.lock().unwrap();
            let v = g
                .get_mut(name.as_str())
                .ok_or_else(|| Error::ProjectNotFound(name.to_string()))?;
            let Some(last) = v.last_mut() else {
                return Err(Error::NotRunning(name.to_string()));
            };
            if !last.is_running() {
                return Err(Error::NotRunning(name.to_string()));
            }
            last.stop = Some(stop.clone());
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use jiff::{civil::date, tz::TimeZone};

        #[test]
        fn create_list_delete_roundtrip() {
            let s = MemStore::new();
            let n = ProjectName::parse("p").unwrap();
            s.create(&n).unwrap();
            assert_eq!(s.list().unwrap().len(), 1);
            s.delete(&n).unwrap();
            assert!(s.list().unwrap().is_empty());
        }

        #[test]
        fn start_then_stop() {
            let s = MemStore::new();
            let n = ProjectName::parse("p").unwrap();
            s.create(&n).unwrap();
            let t0 = date(2026, 5, 4)
                .at(9, 0, 0, 0)
                .to_zoned(TimeZone::UTC)
                .unwrap();
            let t1 = date(2026, 5, 4)
                .at(10, 0, 0, 0)
                .to_zoned(TimeZone::UTC)
                .unwrap();
            s.append_start(&n, &t0, None).unwrap();
            s.close_open(&n, &t1).unwrap();
            let p = s.load(&n).unwrap();
            assert_eq!(p.sessions.len(), 1);
            assert!(!p.sessions[0].is_running());
        }

        #[test]
        fn already_running_errors() {
            let s = MemStore::new();
            let n = ProjectName::parse("p").unwrap();
            s.create(&n).unwrap();
            let t0 = date(2026, 5, 4)
                .at(9, 0, 0, 0)
                .to_zoned(TimeZone::UTC)
                .unwrap();
            s.append_start(&n, &t0, None).unwrap();
            let err = s.append_start(&n, &t0, None).unwrap_err();
            assert!(matches!(err, Error::AlreadyRunning(_, _)));
        }

        #[test]
        fn not_running_errors() {
            let s = MemStore::new();
            let n = ProjectName::parse("p").unwrap();
            s.create(&n).unwrap();
            let t = date(2026, 5, 4)
                .at(9, 0, 0, 0)
                .to_zoned(TimeZone::UTC)
                .unwrap();
            let err = s.close_open(&n, &t).unwrap_err();
            assert!(matches!(err, Error::NotRunning(_)));
        }
    }
}
