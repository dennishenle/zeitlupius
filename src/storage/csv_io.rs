use std::io::{Read, Write};

use jiff::Zoned;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{Project, ProjectName, Session, SessionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Migrated {
    No,
    Yes,
}

#[derive(Serialize, Deserialize)]
struct Row {
    start: String,
    stop: String,
    note: String,
    #[serde(default)]
    id: String,
}

pub fn read_project<R: Read>(name: &ProjectName, reader: R) -> Result<(Project, Migrated)> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(reader);
    // Detect whether the header has the `id` column.
    let has_id = rdr
        .headers()
        .map(|h| h.iter().any(|f| f == "id"))
        .unwrap_or(false);

    let mut sessions: Vec<Session> = Vec::new();
    let mut migrated = Migrated::No;
    let mut seen_ids: std::collections::HashSet<SessionId> = std::collections::HashSet::new();
    for (i, rec) in rdr.deserialize::<Row>().enumerate() {
        let row = rec?;
        let start: Zoned = row.start.parse().map_err(|e: jiff::Error| {
            Error::Corrupt(format!(
                "project '{name}' row {}: invalid start: {e}",
                i + 2
            ))
        })?;
        let stop = if row.stop.is_empty() {
            None
        } else {
            let s: Zoned = row.stop.parse().map_err(|e: jiff::Error| {
                Error::Corrupt(format!("project '{name}' row {}: invalid stop: {e}", i + 2))
            })?;
            if s < start {
                return Err(Error::Corrupt(format!(
                    "project '{name}' row {}: stop is before start",
                    i + 2
                )));
            }
            Some(s)
        };
        let note = if row.note.is_empty() {
            None
        } else {
            Some(row.note)
        };
        let id = if has_id {
            if row.id.is_empty() {
                return Err(Error::Corrupt(format!(
                    "project '{name}' row {}: empty id in id-bearing file",
                    i + 2
                )));
            }
            SessionId::parse(&row.id)
                .map_err(|e| Error::Corrupt(format!("project '{name}' row {}: {e}", i + 2)))?
        } else {
            migrated = Migrated::Yes;
            SessionId::generate()
        };
        if !seen_ids.insert(id.clone()) {
            return Err(Error::Corrupt(format!(
                "project '{name}' row {}: duplicate session id '{id}'",
                i + 2
            )));
        }
        let session = Session {
            id,
            start,
            stop,
            note,
        };
        if let Some(prev) = sessions.last() {
            if session.start < prev.start {
                return Err(Error::Corrupt(format!(
                    "project '{name}' row {}: out of chronological order",
                    i + 2
                )));
            }
            if prev.is_running() {
                return Err(Error::Corrupt(format!(
                    "project '{name}' row {}: open row not at end of file",
                    i + 1
                )));
            }
        }
        sessions.push(session);
    }
    Ok((
        Project {
            name: name.clone(),
            sessions,
        },
        migrated,
    ))
}

pub fn write_project<W: Write>(project: &Project, writer: W) -> Result<()> {
    let mut wtr = csv::Writer::from_writer(writer);
    wtr.write_record(["start", "stop", "note", "id"])?;
    for s in &project.sessions {
        let stop = s.stop.as_ref().map(|z| z.to_string()).unwrap_or_default();
        let note = s.note.clone().unwrap_or_default();
        wtr.write_record([s.start.to_string(), stop, note, s.id.to_string()])?;
    }
    wtr.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Project, ProjectName, Session, SessionId};
    use jiff::{civil::date, tz::TimeZone};

    fn pname() -> ProjectName {
        ProjectName::parse("p").unwrap()
    }

    fn z(y: i16, m: i8, d: i8, h: i8, min: i8) -> Zoned {
        date(y, m, d)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn roundtrip_empty_project() {
        let p = Project {
            name: pname(),
            sessions: vec![],
        };
        let mut buf = Vec::new();
        write_project(&p, &mut buf).unwrap();
        let (p2, _) = read_project(&pname(), buf.as_slice()).unwrap();
        assert_eq!(p2.sessions.len(), 0);
    }

    #[test]
    fn roundtrip_closed_and_open_sessions() {
        let p = Project {
            name: pname(),
            sessions: vec![
                Session {
                    id: SessionId::generate(),
                    start: z(2026, 5, 4, 9, 0),
                    stop: Some(z(2026, 5, 4, 10, 0)),
                    note: Some("foo".into()),
                },
                Session {
                    id: SessionId::generate(),
                    start: z(2026, 5, 4, 13, 0),
                    stop: None,
                    note: None,
                },
            ],
        };
        let mut buf = Vec::new();
        write_project(&p, &mut buf).unwrap();
        let (p2, _) = read_project(&pname(), buf.as_slice()).unwrap();
        assert_eq!(p2.sessions.len(), 2);
        assert_eq!(p2.sessions[0].note.as_deref(), Some("foo"));
        assert!(p2.sessions[1].is_running());
    }

    #[test]
    fn out_of_order_is_corrupt() {
        let csv = "start,stop,note\n2026-05-04T10:00:00Z,2026-05-04T11:00:00Z,\n2026-05-04T09:00:00Z,2026-05-04T09:30:00Z,\n";
        let err = read_project(&pname(), csv.as_bytes()).unwrap_err();
        assert!(matches!(err, Error::Corrupt(_)));
    }

    #[test]
    fn open_row_not_last_is_corrupt() {
        let csv =
            "start,stop,note\n2026-05-04T09:00:00Z,,\n2026-05-04T10:00:00Z,2026-05-04T11:00:00Z,\n";
        let err = read_project(&pname(), csv.as_bytes()).unwrap_err();
        assert!(matches!(err, Error::Corrupt(_)));
    }

    #[test]
    fn invalid_timestamp_is_corrupt() {
        let csv = "start,stop,note\nnope,2026-05-04T10:00:00Z,\n";
        let err = read_project(&pname(), csv.as_bytes()).unwrap_err();
        assert!(matches!(err, Error::Corrupt(_)));
    }

    #[test]
    fn note_with_comma_round_trips() {
        let p = Project {
            name: pname(),
            sessions: vec![Session {
                id: SessionId::generate(),
                start: z(2026, 5, 4, 9, 0),
                stop: Some(z(2026, 5, 4, 10, 0)),
                note: Some("a, b".into()),
            }],
        };
        let mut buf = Vec::new();
        write_project(&p, &mut buf).unwrap();
        let (p2, _) = read_project(&pname(), buf.as_slice()).unwrap();
        assert_eq!(p2.sessions[0].note.as_deref(), Some("a, b"));
    }

    #[test]
    fn roundtrip_preserves_session_id() {
        let id = SessionId::parse("abcdef23").unwrap();
        let p = Project {
            name: pname(),
            sessions: vec![Session {
                id: id.clone(),
                start: z(2026, 5, 4, 9, 0),
                stop: Some(z(2026, 5, 4, 10, 0)),
                note: None,
            }],
        };
        let mut buf = Vec::new();
        write_project(&p, &mut buf).unwrap();
        let (p2, migrated) = read_project(&pname(), buf.as_slice()).unwrap();
        assert_eq!(p2.sessions[0].id, id);
        assert!(matches!(migrated, Migrated::No));
    }

    #[test]
    fn legacy_csv_without_id_is_migrated() {
        let csv =
            "start,stop,note\n2026-05-04T09:00:00+00:00[UTC],2026-05-04T10:00:00+00:00[UTC],\n";
        let (p, migrated) = read_project(&pname(), csv.as_bytes()).unwrap();
        assert_eq!(p.sessions.len(), 1);
        assert!(matches!(migrated, Migrated::Yes));
        // Generated ID must be valid.
        assert!(SessionId::parse(p.sessions[0].id.as_str()).is_ok());
    }

    #[test]
    fn duplicate_session_id_is_corrupt() {
        let csv = "start,stop,note,id\n\
            2026-05-04T09:00:00+00:00[UTC],2026-05-04T10:00:00+00:00[UTC],,aaaaaaaa\n\
            2026-05-04T11:00:00+00:00[UTC],2026-05-04T12:00:00+00:00[UTC],,aaaaaaaa\n";
        let err = read_project(&pname(), csv.as_bytes()).unwrap_err();
        assert!(matches!(err, Error::Corrupt(_)));
    }

    #[test]
    fn invalid_session_id_in_file_is_corrupt() {
        let csv = "start,stop,note,id\n2026-05-04T09:00:00+00:00[UTC],2026-05-04T10:00:00+00:00[UTC],,short\n";
        let err = read_project(&pname(), csv.as_bytes()).unwrap_err();
        assert!(matches!(err, Error::Corrupt(_)));
    }
}
