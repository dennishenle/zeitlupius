use std::io::{Read, Write};

use jiff::Zoned;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{Project, ProjectName, Session, SessionId};

#[derive(Serialize, Deserialize)]
struct Row {
    start: String,
    stop: String,
    note: String,
}

pub fn read_project<R: Read>(name: &ProjectName, reader: R) -> Result<Project> {
    let mut rdr = csv::Reader::from_reader(reader);
    let mut sessions: Vec<Session> = Vec::new();
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
        let session = Session { id: SessionId::generate(), start, stop, note };
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
    Ok(Project {
        name: name.clone(),
        sessions,
    })
}

pub fn write_project<W: Write>(project: &Project, writer: W) -> Result<()> {
    let mut wtr = csv::Writer::from_writer(writer);
    wtr.write_record(["start", "stop", "note"])?;
    for s in &project.sessions {
        let stop = s.stop.as_ref().map(|z| z.to_string()).unwrap_or_default();
        let note = s.note.clone().unwrap_or_default();
        wtr.write_record([s.start.to_string(), stop, note])?;
    }
    wtr.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let p2 = read_project(&pname(), buf.as_slice()).unwrap();
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
        let p2 = read_project(&pname(), buf.as_slice()).unwrap();
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
        let p2 = read_project(&pname(), buf.as_slice()).unwrap();
        assert_eq!(p2.sessions[0].note.as_deref(), Some("a, b"));
    }
}
