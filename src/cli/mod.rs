pub mod format;
pub mod handlers;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "zeitlupius", about = "Time tracking with a TUI and a CLI")]
pub struct Cli {
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    List,
    Create {
        project: String,
    },
    Delete {
        project: String,
        #[arg(long)]
        force: bool,
    },
    Start {
        project: String,
        #[arg(long)]
        note: Option<String>,
    },
    Stop {
        project: String,
    },
    Status {
        project: Option<String>,
    },
    #[command(subcommand)]
    Session(SessionCmd),
    Report(ReportArgs),
}

#[derive(Args, Debug)]
pub struct ReportArgs {
    pub project: Option<String>,

    #[command(flatten)]
    pub interval: IntervalArgs,

    #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
    pub page: i32,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct IntervalArgs {
    #[arg(long, group = "iv")]
    pub day: bool,
    #[arg(long, group = "iv")]
    pub week: bool,
    #[arg(long, group = "iv")]
    pub month: bool,
    #[arg(long, group = "iv")]
    pub year: bool,
    #[arg(long, group = "iv", value_name = "DD.MM.YYYY", requires = "to")]
    pub from: Option<String>,
    #[arg(long, value_name = "DD.MM.YYYY", requires = "from")]
    pub to: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum SessionCmd {
    List {
        project: String,
        #[arg(long)]
        json: bool,
    },
    Delete {
        project: String,
        id: String,
        #[arg(long, short = 'f')]
        force: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn debug_assert() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_start_with_note() {
        let cli = Cli::try_parse_from(["zeitlupius", "start", "rust-zlp", "--note", "hi"]).unwrap();
        match cli.command {
            Some(Command::Start { project, note }) => {
                assert_eq!(project, "rust-zlp");
                assert_eq!(note.as_deref(), Some("hi"));
            }
            _ => panic!("wrong subcommand"),
        }
    }

    #[test]
    fn parses_report_week_with_page() {
        let cli = Cli::try_parse_from(["zeitlupius", "report", "--week", "--page", "-1"]).unwrap();
        match cli.command {
            Some(Command::Report(a)) => {
                assert!(a.interval.week);
                assert_eq!(a.page, -1);
            }
            _ => panic!("wrong subcommand"),
        }
    }

    #[test]
    fn parses_report_custom() {
        let cli = Cli::try_parse_from([
            "zeitlupius",
            "report",
            "--from",
            "18.04.2024",
            "--to",
            "16.07.2024",
        ])
        .unwrap();
        match cli.command {
            Some(Command::Report(a)) => {
                assert_eq!(a.interval.from.as_deref(), Some("18.04.2024"));
                assert_eq!(a.interval.to.as_deref(), Some("16.07.2024"));
            }
            _ => panic!("wrong subcommand"),
        }
    }

    #[test]
    fn no_subcommand_means_tui() {
        let cli = Cli::try_parse_from(["zeitlupius"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_session_list() {
        let cli = Cli::try_parse_from(["zeitlupius", "session", "list", "p"]).unwrap();
        match cli.command {
            Some(Command::Session(SessionCmd::List { project, json })) => {
                assert_eq!(project, "p");
                assert!(!json);
            }
            _ => panic!("wrong subcommand"),
        }
    }

    #[test]
    fn parses_session_delete_force() {
        let cli = Cli::try_parse_from([
            "zeitlupius",
            "session",
            "delete",
            "p",
            "abcdef23",
            "--force",
        ])
        .unwrap();
        match cli.command {
            Some(Command::Session(SessionCmd::Delete { project, id, force })) => {
                assert_eq!(project, "p");
                assert_eq!(id, "abcdef23");
                assert!(force);
            }
            _ => panic!("wrong subcommand"),
        }
    }
}
