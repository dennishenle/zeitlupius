use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use jiff::tz::TimeZone;

use zeitlupius::cli::handlers::run as run_cli;
use zeitlupius::cli::{Cli, Command};
use zeitlupius::error::ErrorKind;
use zeitlupius::storage::FsStore;

fn data_dir(cli_override: Option<PathBuf>) -> PathBuf {
    if let Some(p) = cli_override {
        return p;
    }
    if let Ok(env) = std::env::var("ZEITLUPIUS_HOME") {
        return PathBuf::from(env);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".zeitlupius")
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let dir = data_dir(cli.data_dir);
    let store = FsStore::new(dir);
    if let Err(e) = store.ensure_dirs() {
        eprintln!("error: {e}");
        return ExitCode::from(2);
    }

    let now = jiff::Zoned::now();
    let tz = TimeZone::system();

    let result = match cli.command {
        None => zeitlupius::tui::run(&store, &tz),
        Some(Command::Delete { project, force }) if !force => {
            print!("Delete '{project}'? [y/N] ");
            io::stdout().flush().ok();
            let mut ans = String::new();
            io::stdin().read_line(&mut ans).ok();
            if ans.trim().eq_ignore_ascii_case("y") {
                run_cli(
                    &store,
                    Command::Delete {
                        project,
                        force: true,
                    },
                    &mut io::stdout(),
                    &now,
                    &tz,
                )
            } else {
                println!("aborted");
                Ok(())
            }
        }
        Some(Command::Session(zeitlupius::cli::SessionCmd::Delete { project, id, force }))
            if !force =>
        {
            let store_ref = &store;
            let preview = (|| -> Result<String, zeitlupius::Error> {
                let name = zeitlupius::ProjectName::parse(&project)?;
                let sid = zeitlupius::model::SessionId::parse(&id)?;
                let sessions = zeitlupius::ops::list_sessions(store_ref, &name)?;
                let s = sessions
                    .into_iter()
                    .find(|s| s.id == sid)
                    .ok_or(zeitlupius::Error::SessionNotFound(
                        name.to_string(),
                        sid.to_string(),
                    ))?;
                let stop = s
                    .stop
                    .as_ref()
                    .map(|z| z.to_string())
                    .unwrap_or_else(|| "running".into());
                Ok(format!("{} → {}", s.start, stop))
            })();

            match preview {
                Ok(line) => {
                    print!("Delete session {id} of '{project}' ({line})? [y/N] ");
                    io::stdout().flush().ok();
                    let mut ans = String::new();
                    io::stdin().read_line(&mut ans).ok();
                    if ans.trim().eq_ignore_ascii_case("y") {
                        run_cli(
                            &store,
                            Command::Session(zeitlupius::cli::SessionCmd::Delete {
                                project,
                                id,
                                force: true,
                            }),
                            &mut io::stdout(),
                            &now,
                            &tz,
                        )
                    } else {
                        println!("aborted");
                        Ok(())
                    }
                }
                Err(e) => Err(e),
            }
        }
        Some(cmd) => run_cli(&store, cmd, &mut io::stdout(), &now, &tz),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            match e.kind() {
                ErrorKind::User => ExitCode::from(1),
                ErrorKind::Internal => ExitCode::from(2),
            }
        }
    }
}
