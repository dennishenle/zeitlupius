# Building zeitlupius — A Hands-On Tutorial

This tutorial walks you through building `zeitlupius`, a terminal time-tracking tool written in Rust. By the end you will have a working program that tracks time on projects, stores data in simple CSV files, and offers both a command-line interface (CLI) and a graphical terminal dashboard (TUI).

**No prior programming experience is required.** Every file you need to create is shown in full. Every command you need to run is written out. If something goes wrong, the design spec at `docs/superpowers/specs/2026-05-04-zeitlupius-design.md` is the source of truth, and the implementation plan at `docs/superpowers/plans/2026-05-04-zeitlupius.md` has the complete code for every file.

**What you will build:**

- A program called `zeitlupius` that stores projects under `~/.zeitlupius/projects/<name>.csv`
- CLI commands: `start`, `stop`, `list`, `create`, `delete`, `status`, and `report`
- A TUI dashboard with a split list/detail panel, live-ticking timers, and pageable time intervals
- Both interfaces can do everything — they are equal peers over the same data

**Time estimate:** Several hours if you type carefully and read each explanation. Longer if you explore.

---

## Prerequisites

Before starting, you need three tools installed on your computer.

### 1. Rust

Rust is the programming language we use. Install it by opening a terminal and running:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the prompts (the defaults are fine). When it finishes, close and re-open your terminal, then verify:

```bash
rustc --version
cargo --version
```

Both should print a version number. `cargo` is Rust's build tool — it compiles code, runs tests, and manages dependencies (external libraries).

### 2. A text editor

You need a program to create and edit code files. Any editor works: VS Code, Sublime Text, Zed, nano, or even Notepad. When this tutorial says "create a file", open your editor, paste the content shown, and save it at the specified path.

### 3. Git

Git tracks changes to your code. It is likely already installed. Check:

```bash
git --version
```

If not installed, see <https://git-scm.com/downloads>.

### Terminology

A few terms used throughout:

- **Terminal** — the text-based window where you type commands
- **Directory / folder** — a container for files; these terms are interchangeable
- **Crate** — Rust's name for a library or program package
- **Compile / build** — turning source code into a runnable program
- **Test** — an automated check that verifies code works correctly
- **Commit** — saving a snapshot of your changes in git

---

## Chapter 0 — Conventions

Before touching code, adopt three habits:

1. **Test after every change.** Every chapter ends with a command to run the automated tests. Always run it before committing.
2. **Commit after every chapter.** Each chapter ends with a clean commit. The tutorial gives you the exact commit message.
3. **Keep the build clean.** Run this command before each commit:

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

- `cargo fmt` — automatically formats your code to follow Rust style
- `cargo clippy` — checks for common mistakes and warns you
- `cargo test` — runs all automated tests

We will not repeat this reminder in every chapter. Just always do it.

---

## Chapter 1 — Project scaffold

The repository already contains a bare Rust project with `Cargo.toml` (the project configuration file) and `src/main.rs` (the program entry point). Our first job is to add the libraries we need and set up the project structure.

### Step 1: Add dependencies

Dependencies are external libraries that our code uses. Run each of these commands in your terminal from the project root directory:

```bash
cargo add clap --features derive
cargo add ratatui
cargo add crossterm
cargo add jiff --features serde
cargo add csv
cargo add serde --features derive
cargo add fd-lock
cargo add thiserror
cargo add anyhow
```

What each library does:

- **clap** — parses command-line arguments (e.g. `zeitlupius start my-project`)
- **ratatui** + **crossterm** — draws the terminal dashboard (TUI) and reads keyboard input
- **jiff** — handles dates, times, and time zones correctly (including daylight saving time)
- **csv** + **serde** — reads and writes CSV files
- **fd-lock** — prevents two processes from writing the same file at the same time
- **thiserror** + **anyhow** — makes error handling clean and readable

Now add the test-only dependencies (libraries used only during testing, not in the final program):

```bash
cargo add --dev tempfile
cargo add --dev assert_cmd
cargo add --dev predicates
```

- **tempfile** — creates temporary directories for tests (so tests never touch your real data)
- **assert_cmd** + **predicates** — lets tests run the program and check its output

### Step 2: Configure the project layout

Open `Cargo.toml` in your editor. After the `[dependencies]` section that `cargo add` created, add these lines at the end of the file:

```toml
[lib]
name = "zeitlupius"
path = "src/lib.rs"

[[bin]]
name = "zeitlupius"
path = "src/main.rs"
```

This tells Rust our project has two parts: a **library** (`src/lib.rs`) containing all the logic, and a **binary** (`src/main.rs`) that is the actual program users run. The library is where we put testable code; the binary just parses arguments and calls into the library.

### Step 3: Create the library root

Create a new file `src/lib.rs` with this content:

```rust
//! Library core for zeitlupius. UI front-ends call into this crate.
```

The `//!` line is a documentation comment — it describes what this file is for. We will add more to this file in later chapters.

### Step 4: Update the main program

Replace the contents of `src/main.rs` with:

```rust
fn main() {
    println!("zeitlupius — placeholder; replaced in later chapters");
}
```

This is a temporary placeholder. We will replace it with the real program in Chapter 7.

### Step 5: Verify it compiles

```bash
cargo build
```

This should complete without errors. If you see errors, double-check that the `[lib]` and `[[bin]]` sections were added to `Cargo.toml` and that `src/lib.rs` exists.

### Step 6: Commit

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "chore: scaffold lib+bin layout and core dependencies"
```

---

## Chapter 2 — The Error type

Every program needs to handle things that go wrong: a project that does not exist, a file that cannot be read, an invalid date. We define a single `Error` type that lists every kind of problem the program can encounter. This makes error messages clear and consistent.

The error type has two categories:

- **User errors** — mistakes the user made (wrong project name, invalid date). The program exits with code `1`.
- **Internal errors** — things outside the user's control (disk failure, corrupted file). The program exits with code `2`.

### Step 1: Create the error module

Create `src/error.rs` with this content:

```rust
use std::fmt;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("project '{0}' does not exist")]
    ProjectNotFound(String),
    #[error("project '{0}' already exists")]
    ProjectAlreadyExists(String),
    #[error("invalid project name '{0}': {1}")]
    InvalidProjectName(String, &'static str),
    #[error("project '{0}' is already running (session started at {1})")]
    AlreadyRunning(String, String),
    #[error("project '{0}' is not running")]
    NotRunning(String),
    #[error("invalid date '{0}': expected DD.MM.YYYY")]
    InvalidDate(String),
    #[error("invalid range: {from} is after {to}")]
    InvalidRange { from: String, to: String },
    #[error("data dir is corrupted: {0}")]
    Corrupt(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    User,
    Internal,
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::ProjectNotFound(_)
            | Error::ProjectAlreadyExists(_)
            | Error::InvalidProjectName(_, _)
            | Error::AlreadyRunning(_, _)
            | Error::NotRunning(_)
            | Error::InvalidDate(_)
            | Error::InvalidRange { .. } => ErrorKind::User,
            Error::Corrupt(_) | Error::Io(_) | Error::Csv(_) => ErrorKind::Internal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_errors_have_user_kind() {
        let e = Error::ProjectNotFound("x".into());
        assert_eq!(e.kind(), ErrorKind::User);
    }

    #[test]
    fn corrupt_is_internal() {
        let e = Error::Corrupt("bad row".into());
        assert_eq!(e.kind(), ErrorKind::Internal);
    }

    #[test]
    fn display_matches_thiserror_format() {
        let e = Error::ProjectNotFound("foo".into());
        assert_eq!(format!("{e}"), "project 'foo' does not exist");
    }
}

#[allow(dead_code)]
fn _force_fmt_use(_: &dyn fmt::Display) {}
```

Here is what each section does:

- The `enum Error` lists every kind of problem. Each line starting with `#[error("...")]` defines the human-readable message for that error. For example, if you try to start a project that does not exist, the user sees `project 'my-project' does not exist`.
- `pub type Result<T, E = Error>` creates a shorthand so we can write `Result<Something>` instead of `std::result::Result<Something, Error>` everywhere.
- `ErrorKind` and the `kind()` method classify errors as `User` (exit code 1) or `Internal` (exit code 2).
- The `#[cfg(test)] mod tests` section contains automated tests. The `#[test]` marker tells Rust "this function is a test". Each test creates an error and checks that it behaves correctly.

### Step 2: Register the module

Replace the contents of `src/lib.rs` with:

```rust
//! Library core for zeitlupius. UI front-ends call into this crate.

pub mod error;

pub use error::{Error, ErrorKind, Result};
```

`pub mod error;` tells Rust "there is a file called `error.rs` — include it as part of the library". `pub use` re-exports the types so other code can use them directly.

### Step 3: Run the tests

```bash
cargo test --lib error::tests
```

You should see `3 passed`. If any test fails, compare your `src/error.rs` with the code above.

### Step 4: Commit

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "feat: add Error enum and Result alias"
```

---

## Chapter 3 — Domain types

This chapter defines the core data structures — the "nouns" of the program. There are four:

1. **ProjectName** — a validated string for project names (safe for filenames and CLI arguments)
2. **Session** — one time-tracking interval with a start time and optional stop time
3. **Project** — a named collection of sessions
4. **Interval** — a time range (Day, Week, Month, Year, or Custom) for reports

All four live in a single file `src/model.rs`. The file also includes methods on `Interval` for computing time boundaries, navigating between intervals, and generating labels.

### What each piece does

**ProjectName** only allows names made of ASCII letters, digits, hyphens, underscores, and dots. It rejects empty names, names longer than 64 characters, names starting with a dot, and names that collide with CLI commands like `list` or `start`. This prevents confusing situations where a project name looks like a command.

**Session** represents one block of tracked time. It has a `start` timestamp, an optional `stop` timestamp (empty means the timer is still running), and an optional `note`. The `duration` method calculates how long the session lasted — if the timer is still running, it uses the current time.

**Project** is simply a name and a list of sessions. The `running_session` method returns the currently active session, if any.

**Interval** represents a time range for reports. `Day(date)` means "from midnight to midnight on that date". `Week(date)` is the ISO week containing that date (Monday to Sunday). `Month(date)` is the entire month. `Year(year)` is the entire year. `Custom { from, to }` is an arbitrary date range.

The methods on `Interval`:

- `bounds(tz)` — returns the exact start and end timestamps for the interval in the given time zone
- `contains(timestamp, tz)` — checks if a timestamp falls within the interval
- `previous()` / `next()` — shifts the interval backward or forward by one unit (one day, one week, etc.)
- `label()` — returns a human-readable string like `"CW 19 · 04.05–10.05.2026"` or `"May 2026"`

### Step 1: Create the model module

Create `src/model.rs` with this content:

```rust
use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectName(String);

const RESERVED: &[&str] = &[
    "list", "start", "stop", "status", "create", "delete", "report",
];
const MAX_LEN: usize = 64;

impl ProjectName {
    pub fn parse(raw: &str) -> Result<Self> {
        if raw.is_empty() {
            return Err(Error::InvalidProjectName(raw.into(), "name is empty"));
        }
        if raw.len() > MAX_LEN {
            return Err(Error::InvalidProjectName(raw.into(), "name longer than 64 chars"));
        }
        if raw.starts_with('.') {
            return Err(Error::InvalidProjectName(raw.into(), "name cannot start with '.'"));
        }
        if RESERVED.contains(&raw) {
            return Err(Error::InvalidProjectName(
                raw.into(),
                "name collides with a reserved CLI verb",
            ));
        }
        for ch in raw.chars() {
            let ok = ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.');
            if !ok {
                return Err(Error::InvalidProjectName(
                    raw.into(),
                    "only ASCII letters, digits, '-', '_', '.' allowed",
                ));
            }
        }
        Ok(Self(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProjectName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub start: jiff::Zoned,
    pub stop: Option<jiff::Zoned>,
    pub note: Option<String>,
}

impl Session {
    pub fn is_running(&self) -> bool {
        self.stop.is_none()
    }

    pub fn duration(&self, now: &jiff::Zoned) -> jiff::Span {
        let end = self.stop.as_ref().unwrap_or(now);
        if end < &self.start {
            return jiff::Span::new();
        }
        end.since(&self.start)
            .unwrap_or_else(|_| jiff::Span::new())
    }
}

#[derive(Clone, Debug)]
pub struct Project {
    pub name: ProjectName,
    pub sessions: Vec<Session>,
}

impl Project {
    pub fn running_session(&self) -> Option<&Session> {
        self.sessions.last().filter(|s| s.is_running())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Interval {
    Day(jiff::civil::Date),
    Week(jiff::civil::Date),
    Month(jiff::civil::Date),
    Year(i16),
    Custom {
        from: jiff::civil::Date,
        to: jiff::civil::Date,
    },
}

impl Interval {
    pub fn bounds(&self, tz: &jiff::tz::TimeZone) -> (jiff::Zoned, jiff::Zoned) {
        use jiff::civil::{date, Weekday};
        let (lo_d, hi_d) = match self {
            Interval::Day(d) => (*d, d.tomorrow().expect("date in range")),
            Interval::Week(d) => {
                let dow: i64 = match d.weekday() {
                    Weekday::Monday => 0,
                    Weekday::Tuesday => 1,
                    Weekday::Wednesday => 2,
                    Weekday::Thursday => 3,
                    Weekday::Friday => 4,
                    Weekday::Saturday => 5,
                    Weekday::Sunday => 6,
                };
                let mon = d.checked_sub(jiff::Span::new().days(dow)).expect("monday");
                let next_mon = mon
                    .checked_add(jiff::Span::new().days(7))
                    .expect("next monday");
                (mon, next_mon)
            }
            Interval::Month(d) => {
                let first = date(d.year(), d.month(), 1);
                let next_first = if d.month() == 12 {
                    date(d.year() + 1, 1, 1)
                } else {
                    date(d.year(), d.month() + 1, 1)
                };
                (first, next_first)
            }
            Interval::Year(y) => (date(*y, 1, 1), date(*y + 1, 1, 1)),
            Interval::Custom { from, to } => {
                let next = to.tomorrow().expect("date in range");
                (*from, next)
            }
        };
        let lo = lo_d
            .at(0, 0, 0, 0)
            .to_zoned(tz.clone())
            .expect("midnight");
        let hi = hi_d
            .at(0, 0, 0, 0)
            .to_zoned(tz.clone())
            .expect("midnight");
        (lo, hi)
    }

    pub fn contains(&self, t: &jiff::Zoned, tz: &jiff::tz::TimeZone) -> bool {
        let (lo, hi) = self.bounds(tz);
        t >= &lo && t < &hi
    }

    pub fn previous(&self) -> Self {
        match self {
            Interval::Day(d) => Interval::Day(d.yesterday().expect("date")),
            Interval::Week(d) => {
                Interval::Week(d.checked_sub(jiff::Span::new().days(7)).expect("date"))
            }
            Interval::Month(d) => {
                let (y, m) = if d.month() == 1 {
                    (d.year() - 1, 12)
                } else {
                    (d.year(), d.month() - 1)
                };
                Interval::Month(jiff::civil::date(y, m, 1))
            }
            Interval::Year(y) => Interval::Year(*y - 1),
            Interval::Custom { from, to } => {
                let len_days = to.since(*from).expect("range").get_days() + 1;
                let span = jiff::Span::new().days(len_days);
                Interval::Custom {
                    from: from.checked_sub(span).expect("date"),
                    to: to.checked_sub(span).expect("date"),
                }
            }
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Interval::Day(d) => Interval::Day(d.tomorrow().expect("date")),
            Interval::Week(d) => {
                Interval::Week(d.checked_add(jiff::Span::new().days(7)).expect("date"))
            }
            Interval::Month(d) => {
                let (y, m) = if d.month() == 12 {
                    (d.year() + 1, 1)
                } else {
                    (d.year(), d.month() + 1)
                };
                Interval::Month(jiff::civil::date(y, m, 1))
            }
            Interval::Year(y) => Interval::Year(*y + 1),
            Interval::Custom { from, to } => {
                let len_days = to.since(*from).expect("range").get_days() + 1;
                let span = jiff::Span::new().days(len_days);
                Interval::Custom {
                    from: from.checked_add(span).expect("date"),
                    to: to.checked_add(span).expect("date"),
                }
            }
        }
    }

    pub fn label(&self) -> String {
        match self {
            Interval::Day(d) => format!("{:02}.{:02}.{}", d.day(), d.month(), d.year()),
            Interval::Week(d) => {
                use jiff::civil::Weekday;
                let dow: i64 = match d.weekday() {
                    Weekday::Monday => 0,
                    Weekday::Tuesday => 1,
                    Weekday::Wednesday => 2,
                    Weekday::Thursday => 3,
                    Weekday::Friday => 4,
                    Weekday::Saturday => 5,
                    Weekday::Sunday => 6,
                };
                let mon = d.checked_sub(jiff::Span::new().days(dow)).expect("monday");
                let sun = mon.checked_add(jiff::Span::new().days(6)).expect("sun");
                let iso = d.iso_week_date();
                format!(
                    "CW {} · {:02}.{:02}–{:02}.{:02}.{}",
                    iso.week(),
                    mon.day(),
                    mon.month(),
                    sun.day(),
                    sun.month(),
                    sun.year(),
                )
            }
            Interval::Month(d) => {
                let names = [
                    "January",
                    "February",
                    "March",
                    "April",
                    "May",
                    "June",
                    "July",
                    "August",
                    "September",
                    "October",
                    "November",
                    "December",
                ];
                format!("{} {}", names[(d.month() - 1) as usize], d.year())
            }
            Interval::Year(y) => y.to_string(),
            Interval::Custom { from, to } => format!(
                "Custom · {:02}.{:02}.{} – {:02}.{:02}.{}",
                from.day(),
                from.month(),
                from.year(),
                to.day(),
                to.month(),
                to.year(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::{civil::date, tz::TimeZone};

    #[test]
    fn accepts_valid_names() {
        for ok in ["rust-zlp", "side_project", "admin", "v1.0", "a"] {
            assert!(ProjectName::parse(ok).is_ok(), "{ok} should be valid");
        }
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(
            ProjectName::parse(""),
            Err(Error::InvalidProjectName(_, _))
        ));
    }

    #[test]
    fn rejects_too_long() {
        let n = "a".repeat(65);
        assert!(ProjectName::parse(&n).is_err());
    }

    #[test]
    fn rejects_leading_dot() {
        assert!(ProjectName::parse(".hidden").is_err());
    }

    #[test]
    fn rejects_reserved_verbs() {
        for v in ["list", "start", "stop", "status", "create", "delete", "report"] {
            assert!(ProjectName::parse(v).is_err(), "{v} should be reserved");
        }
    }

    #[test]
    fn rejects_invalid_chars() {
        for bad in ["with space", "slash/name", "ümlaut", "name!"] {
            assert!(ProjectName::parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    fn z(year: i16, m: i8, d: i8, h: i8, min: i8) -> jiff::Zoned {
        date(year, m, d)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn session_running_means_no_stop() {
        let s = Session {
            start: z(2026, 5, 4, 9, 0),
            stop: None,
            note: None,
        };
        assert!(s.is_running());
    }

    #[test]
    fn session_duration_uses_now_when_running() {
        let s = Session {
            start: z(2026, 5, 4, 9, 0),
            stop: None,
            note: None,
        };
        let now = z(2026, 5, 4, 10, 30);
        let span = s.duration(&now);
        assert_eq!(span.get_minutes(), 90);
    }

    #[test]
    fn session_duration_uses_stop_when_closed() {
        let s = Session {
            start: z(2026, 5, 4, 9, 0),
            stop: Some(z(2026, 5, 4, 9, 45)),
            note: None,
        };
        let now = z(2026, 5, 4, 23, 0);
        assert_eq!(s.duration(&now).get_minutes(), 45);
    }

    #[test]
    fn project_running_session_picks_open_one() {
        let mut p = Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![],
        };
        p.sessions.push(Session {
            start: z(2026, 5, 4, 9, 0),
            stop: Some(z(2026, 5, 4, 9, 30)),
            note: None,
        });
        assert!(p.running_session().is_none());
        p.sessions.push(Session {
            start: z(2026, 5, 4, 10, 0),
            stop: None,
            note: None,
        });
        assert!(p.running_session().is_some());
    }

    #[test]
    fn day_bounds_are_midnight_to_midnight() {
        let tz = TimeZone::get("Europe/Berlin").unwrap();
        let i = Interval::Day(date(2026, 5, 4));
        let (lo, hi) = i.bounds(&tz);
        assert_eq!(lo.year(), 2026);
        assert_eq!(lo.month(), 5);
        assert_eq!(lo.day(), 4);
        assert_eq!(lo.hour(), 0);
        assert_eq!(lo.minute(), 0);
        assert_eq!(hi.year(), 2026);
        assert_eq!(hi.month(), 5);
        assert_eq!(hi.day(), 5);
        assert_eq!(hi.hour(), 0);
    }

    #[test]
    fn iso_week_bounds_monday_to_next_monday() {
        let tz = TimeZone::UTC;
        let i = Interval::Week(date(2026, 5, 6));
        let (lo, hi) = i.bounds(&tz);
        assert_eq!((lo.year(), lo.month(), lo.day()), (2026, 5, 4));
        assert_eq!((hi.year(), hi.month(), hi.day()), (2026, 5, 11));
    }

    #[test]
    fn month_bounds_first_to_first() {
        let tz = TimeZone::UTC;
        let i = Interval::Month(date(2026, 2, 15));
        let (lo, hi) = i.bounds(&tz);
        assert_eq!((lo.year(), lo.month(), lo.day()), (2026, 2, 1));
        assert_eq!((hi.year(), hi.month(), hi.day()), (2026, 3, 1));
    }

    #[test]
    fn year_bounds_jan1_to_jan1() {
        let tz = TimeZone::UTC;
        let i = Interval::Year(2024);
        let (lo, hi) = i.bounds(&tz);
        assert_eq!((lo.year(), lo.month(), lo.day()), (2024, 1, 1));
        assert_eq!((hi.year(), hi.month(), hi.day()), (2025, 1, 1));
    }

    #[test]
    fn custom_bounds_inclusive_to_day() {
        let tz = TimeZone::UTC;
        let i = Interval::Custom {
            from: date(2024, 4, 18),
            to: date(2024, 7, 16),
        };
        let (lo, hi) = i.bounds(&tz);
        assert_eq!((lo.year(), lo.month(), lo.day()), (2024, 4, 18));
        assert_eq!((hi.year(), hi.month(), hi.day()), (2024, 7, 17));
    }

    #[test]
    fn contains_uses_local_bounds() {
        let tz = TimeZone::UTC;
        let i = Interval::Day(date(2026, 5, 4));
        let inside = z(2026, 5, 4, 12, 0);
        let outside = z(2026, 5, 5, 0, 0);
        assert!(i.contains(&inside, &tz));
        assert!(!i.contains(&outside, &tz));
    }

    #[test]
    fn previous_next_day() {
        let i = Interval::Day(date(2026, 5, 4));
        assert_eq!(i.previous(), Interval::Day(date(2026, 5, 3)));
        assert_eq!(i.next(), Interval::Day(date(2026, 5, 5)));
    }

    #[test]
    fn previous_next_week() {
        let i = Interval::Week(date(2026, 5, 4));
        let p = i.previous();
        let n = i.next();
        assert!(matches!(p, Interval::Week(_)));
        let tz = TimeZone::UTC;
        let (lo, _) = i.bounds(&tz);
        let (lop, _) = p.bounds(&tz);
        assert_eq!(
            (lo.timestamp().as_second() - lop.timestamp().as_second()) / 86400,
            7
        );
        let (lon, _) = n.bounds(&tz);
        assert_eq!(
            (lon.timestamp().as_second() - lo.timestamp().as_second()) / 86400,
            7
        );
    }

    #[test]
    fn previous_next_month_handles_year_wrap() {
        let i = Interval::Month(date(2026, 1, 15));
        assert_eq!(i.previous(), Interval::Month(date(2025, 12, 1)));
        let i2 = Interval::Month(date(2026, 12, 1));
        assert_eq!(i2.next(), Interval::Month(date(2027, 1, 1)));
    }

    #[test]
    fn previous_next_year() {
        assert_eq!(Interval::Year(2026).previous(), Interval::Year(2025));
        assert_eq!(Interval::Year(2026).next(), Interval::Year(2027));
    }

    #[test]
    fn custom_previous_next_shifts_by_range_length_in_days() {
        let i = Interval::Custom {
            from: date(2024, 4, 18),
            to: date(2024, 4, 20),
        };
        let p = i.previous();
        match p {
            Interval::Custom { from, to } => {
                assert_eq!((from.year(), from.month(), from.day()), (2024, 4, 15));
                assert_eq!((to.year(), to.month(), to.day()), (2024, 4, 17));
            }
            _ => panic!("expected Custom"),
        }
    }

    #[test]
    fn labels_are_human_readable() {
        let i_day = Interval::Day(date(2026, 5, 4));
        assert!(i_day.label().contains("04.05.2026"));
        let i_year = Interval::Year(2026);
        assert_eq!(i_year.label(), "2026");
    }
}
```

### Step 2: Register the module

Replace the contents of `src/lib.rs` with:

```rust
//! Library core for zeitlupius. UI front-ends call into this crate.

pub mod error;
pub mod model;

pub use error::{Error, ErrorKind, Result};
pub use model::ProjectName;
```

### Step 3: Run the tests

```bash
cargo test --lib model::tests
```

You should see `24 passed` (6 for ProjectName, 4 for Session/Project, 14 for Interval).

### Step 4: Commit

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "feat: add domain types ProjectName, Session, Project, Interval"
```

---

## Chapter 4 — Time math

This chapter adds the functions that calculate how much tracked time falls within a given interval. The logic is simple: clip a session's `[start, end)` range to the interval's `[lo, hi)` range and measure the overlap in seconds.

We create three files inside a `src/time/` directory:

- `mod.rs` — declares the module and re-exports the public functions
- `intersect.rs` — the core overlap calculation for a single session
- `aggregate.rs` — sums overlaps across all sessions in a project

### Step 1: Create the directory and module file

Create the directory `src/time/` and then create `src/time/mod.rs` with:

```rust
pub mod aggregate;
pub mod intersect;

pub use aggregate::project_total_in;
pub use intersect::session_overlap_seconds;
```

### Step 2: Create the intersection function

Create `src/time/intersect.rs` with:

```rust
use crate::model::{Interval, Session};

pub fn session_overlap_seconds(
    session: &Session,
    interval: &Interval,
    now: &jiff::Zoned,
    tz: &jiff::tz::TimeZone,
) -> i64 {
    let s = &session.start;
    let e = session.stop.as_ref().unwrap_or(now);
    if e <= s {
        return 0;
    }
    let (lo, hi) = interval.bounds(tz);
    let start = if s > &lo { s.clone() } else { lo };
    let end = if e < &hi { e.clone() } else { hi };
    if end <= start {
        return 0;
    }
    end.timestamp().as_second() - start.timestamp().as_second()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ProjectName;
    use jiff::{civil::date, tz::TimeZone};

    fn z(year: i16, m: i8, d: i8, h: i8, min: i8) -> jiff::Zoned {
        date(year, m, d)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }
    fn closed(start: jiff::Zoned, stop: jiff::Zoned) -> Session {
        Session {
            start,
            stop: Some(stop),
            note: None,
        }
    }
    fn open(start: jiff::Zoned) -> Session {
        Session {
            start,
            stop: None,
            note: None,
        }
    }

    #[test]
    fn entirely_inside() {
        let i = Interval::Day(date(2026, 5, 4));
        let s = closed(z(2026, 5, 4, 9, 0), z(2026, 5, 4, 11, 0));
        assert_eq!(
            session_overlap_seconds(&s, &i, &z(2026, 5, 4, 23, 0), &TimeZone::UTC),
            2 * 3600,
        );
    }

    #[test]
    fn no_overlap_returns_zero() {
        let i = Interval::Day(date(2026, 5, 4));
        let s = closed(z(2026, 5, 5, 0, 0), z(2026, 5, 5, 1, 0));
        assert_eq!(
            session_overlap_seconds(&s, &i, &z(2026, 5, 5, 23, 0), &TimeZone::UTC),
            0,
        );
    }

    #[test]
    fn partial_at_start() {
        let i = Interval::Day(date(2026, 5, 4));
        let s = closed(z(2026, 5, 3, 23, 0), z(2026, 5, 4, 1, 30));
        assert_eq!(
            session_overlap_seconds(&s, &i, &z(2026, 5, 4, 23, 0), &TimeZone::UTC),
            5400,
        );
    }

    #[test]
    fn partial_at_end() {
        let i = Interval::Day(date(2026, 5, 4));
        let s = closed(z(2026, 5, 4, 23, 30), z(2026, 5, 5, 0, 30));
        assert_eq!(
            session_overlap_seconds(&s, &i, &z(2026, 5, 5, 23, 0), &TimeZone::UTC),
            1800,
        );
    }

    #[test]
    fn open_session_uses_now() {
        let i = Interval::Day(date(2026, 5, 4));
        let s = open(z(2026, 5, 4, 22, 0));
        assert_eq!(
            session_overlap_seconds(&s, &i, &z(2026, 5, 4, 22, 30), &TimeZone::UTC),
            1800,
        );
    }

    #[test]
    fn dst_aware_25_hour_day() {
        let tz = TimeZone::get("Europe/Berlin").unwrap();
        let i = Interval::Day(date(2026, 10, 25));
        let s = Session {
            start: date(2026, 10, 25)
                .at(0, 0, 0, 0)
                .to_zoned(tz.clone())
                .unwrap(),
            stop: Some(
                date(2026, 10, 26)
                    .at(0, 0, 0, 0)
                    .to_zoned(tz.clone())
                    .unwrap(),
            ),
            note: None,
        };
        let now = date(2026, 10, 26)
            .at(12, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap();
        assert_eq!(session_overlap_seconds(&s, &i, &now, &tz), 25 * 3600);
        assert!(ProjectName::parse("a").is_ok());
    }
}
```

This function clips a session to an interval and returns the overlap in seconds. The DST test at the bottom verifies that when clocks fall back (October 25 in Berlin), a "day" is correctly calculated as 25 hours.

### Step 3: Create the aggregation function

Create `src/time/aggregate.rs` with:

```rust
use crate::model::{Interval, Project};
use crate::time::session_overlap_seconds;

pub fn project_total_in(
    project: &Project,
    interval: &Interval,
    now: &jiff::Zoned,
    tz: &jiff::tz::TimeZone,
) -> i64 {
    project
        .sessions
        .iter()
        .map(|s| session_overlap_seconds(s, interval, now, tz))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProjectName, Session};
    use jiff::{civil::date, tz::TimeZone};

    fn z(year: i16, m: i8, d: i8, h: i8, min: i8) -> jiff::Zoned {
        date(year, m, d)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn sums_sessions_within_interval() {
        let p = Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![
                Session {
                    start: z(2026, 5, 4, 9, 0),
                    stop: Some(z(2026, 5, 4, 10, 0)),
                    note: None,
                },
                Session {
                    start: z(2026, 5, 4, 14, 0),
                    stop: Some(z(2026, 5, 4, 15, 30)),
                    note: None,
                },
            ],
        };
        let total = project_total_in(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 23, 0),
            &TimeZone::UTC,
        );
        assert_eq!(total, 3600 + 5400);
    }

    #[test]
    fn ignores_sessions_outside_interval() {
        let p = Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![Session {
                start: z(2026, 5, 3, 9, 0),
                stop: Some(z(2026, 5, 3, 10, 0)),
                note: None,
            }],
        };
        let total = project_total_in(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 23, 0),
            &TimeZone::UTC,
        );
        assert_eq!(total, 0);
    }
}
```

This simply adds up the overlap of every session in a project. Sessions outside the interval contribute zero.

### Step 4: Register the module

Replace the contents of `src/lib.rs` with:

```rust
//! Library core for zeitlupius. UI front-ends call into this crate.

pub mod error;
pub mod model;
pub mod time;

pub use error::{Error, ErrorKind, Result};
pub use model::ProjectName;
```

### Step 5: Run the tests

```bash
cargo test --lib time::
```

You should see `8 passed` (6 for intersect, 2 for aggregate).

### Step 6: Commit

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "feat: add time math (bounds, intersect, aggregate)"
```

---

## Chapter 5 — Storage

Storage is the layer that reads and writes project data to disk. We define a `ProjectStore` trait (an interface that describes what storage operations are available) and provide two implementations:

- **MemStore** — an in-memory store used only in tests (fast, no disk access)
- **FsStore** — the real filesystem store used by the program

The data format is CSV: one file per project under `~/.zeitlupius/projects/`. An open row (empty `stop` column) means a timer is running.

We also implement safety measures:

- **Locking** — only one process can write at a time (prevents data corruption)
- **Atomic writes** — data is written to a temporary file first, then renamed over the original (prevents partial writes if the program crashes)

Create the directory `src/storage/` and then create these three files.

### Step 1: Create the trait and in-memory store

Create `src/storage/mod.rs` with:

```rust
use crate::error::{Error, Result};
use crate::model::{Project, ProjectName, Session};

pub mod csv_io;
pub mod fs_store;

pub use fs_store::FsStore;

pub trait ProjectStore: Send + Sync {
    fn list(&self) -> Result<Vec<ProjectName>>;
    fn load(&self, name: &ProjectName) -> Result<Project>;
    fn create(&self, name: &ProjectName) -> Result<()>;
    fn delete(&self, name: &ProjectName) -> Result<()>;
    fn append_start(&self, name: &ProjectName, start: &jiff::Zoned, note: Option<&str>)
        -> Result<()>;
    fn close_open(&self, name: &ProjectName, stop: &jiff::Zoned) -> Result<()>;
}

#[cfg(any(test, feature = "test-helpers"))]
pub mod mem {
    use super::*;
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
            if let Some(last) = v.last() {
                if last.is_running() {
                    return Err(Error::AlreadyRunning(
                        name.to_string(),
                        last.start.to_string(),
                    ));
                }
            }
            v.push(Session {
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
            let t0 = date(2026, 5, 4).at(9, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap();
            let t1 = date(2026, 5, 4).at(10, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap();
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
            let t0 = date(2026, 5, 4).at(9, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap();
            s.append_start(&n, &t0, None).unwrap();
            let err = s.append_start(&n, &t0, None).unwrap_err();
            assert!(matches!(err, Error::AlreadyRunning(_, _)));
        }

        #[test]
        fn not_running_errors() {
            let s = MemStore::new();
            let n = ProjectName::parse("p").unwrap();
            s.create(&n).unwrap();
            let t = date(2026, 5, 4).at(9, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap();
            let err = s.close_open(&n, &t).unwrap_err();
            assert!(matches!(err, Error::NotRunning(_)));
        }
    }
}
```

The `ProjectStore` trait has six methods — `list`, `load`, `create`, `delete`, `append_start` (begin tracking), and `close_open` (stop tracking). `MemStore` implements all six using a simple in-memory dictionary, protected by a mutex (a lock that prevents simultaneous access). It exists only for testing.

### Step 2: Create the CSV codec

Create `src/storage/csv_io.rs` with:

```rust
use std::io::{Read, Write};

use jiff::Zoned;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{Project, ProjectName, Session};

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
            Error::Corrupt(format!("project '{name}' row {}: invalid start: {e}", i + 2))
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
        let session = Session { start, stop, note };
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
        let stop = s
            .stop
            .as_ref()
            .map(|z| z.to_string())
            .unwrap_or_default();
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
                    start: z(2026, 5, 4, 9, 0),
                    stop: Some(z(2026, 5, 4, 10, 0)),
                    note: Some("foo".into()),
                },
                Session {
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
        let csv = "start,stop,note\n2026-05-04T09:00:00Z,,\n2026-05-04T10:00:00Z,2026-05-04T11:00:00Z,\n";
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
```

`read_project` reads a CSV file and validates it: timestamps must be valid, sessions must be in order, and at most one row can be open (missing `stop`). `write_project` produces a CSV with the header `start,stop,note`.

### Step 3: Create the filesystem store

Create `src/storage/fs_store.rs` with:

```rust
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::model::{Project, ProjectName, Session};
use crate::storage::csv_io::{read_project, write_project};
use crate::storage::ProjectStore;

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

    fn load_inner(&self, name: &ProjectName) -> Result<Project> {
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
        self.ensure_dirs()?;
        self.load_inner(name)
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
            let mut p = self.load_inner(name)?;
            if let Some(last) = p.sessions.last() {
                if last.is_running() {
                    return Err(Error::AlreadyRunning(
                        name.to_string(),
                        last.start.to_string(),
                    ));
                }
            }
            p.sessions.push(Session {
                start: start.clone(),
                stop: None,
                note: note.map(str::to_string),
            });
            self.save_inner(&p)
        })
    }

    fn close_open(&self, name: &ProjectName, stop: &jiff::Zoned) -> Result<()> {
        self.with_lock(|| {
            let mut p = self.load_inner(name)?;
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
        assert_eq!(oks, 1, "exactly one starter wins; the other must error cleanly");
        let p = s.load(&n).unwrap();
        assert_eq!(p.sessions.len(), 1);
    }
}
```

`FsStore` is the real storage that reads/writes CSV files on disk. Key details:

- `ensure_dirs` creates the `projects/` directory and `.lock` file on first use.
- `with_lock` acquires an exclusive file lock before any mutation, preventing two processes from corrupting data.
- `atomic_write` writes to a temporary file then renames it — if the program crashes mid-write, the original file is untouched.
- `load` does not lock because reads tolerate a slightly stale (but always complete) file.
- The concurrency test verifies that two threads trying to start the same project don't corrupt the file.

### Step 4: Register the module

Replace the contents of `src/lib.rs` with:

```rust
//! Library core for zeitlupius. UI front-ends call into this crate.

pub mod error;
pub mod model;
pub mod storage;
pub mod time;

pub use error::{Error, ErrorKind, Result};
pub use model::ProjectName;
pub use storage::ProjectStore;
```

### Step 5: Run the tests

```bash
cargo test --lib storage::
```

You should see `16 passed` (4 for MemStore, 6 for csv_io, 6 for fs_store).

### Step 6: Commit

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "feat: add ProjectStore, MemStore, CSV codec, and FsStore"
```

---

## Chapter 6 — The operations layer

The `ops` module is a thin layer of functions that the CLI and TUI both call. Each function takes a store and some arguments, and returns a result. This is the single place where "business logic" lives — creating projects, starting/stopping timers, generating reports.

Two design choices make this layer easy to test:

- It accepts `now: &Zoned` (the current time) as a parameter, so tests can pass a fixed time instead of using the real clock.
- It accepts any `ProjectStore`, so tests use the fast `MemStore` instead of touching the filesystem.

### Step 1: Create the ops module

Create `src/ops.rs` with:

```rust
use jiff::tz::TimeZone;

use crate::error::Result;
use crate::model::{Interval, Project, ProjectName};
use crate::storage::ProjectStore;
use crate::time::project_total_in;

pub fn create<S: ProjectStore>(store: &S, name: &ProjectName) -> Result<()> {
    store.create(name)
}

pub fn delete<S: ProjectStore>(store: &S, name: &ProjectName) -> Result<()> {
    store.delete(name)
}

pub fn list<S: ProjectStore>(store: &S) -> Result<Vec<ProjectName>> {
    store.list()
}

pub fn start<S: ProjectStore>(
    store: &S,
    name: &ProjectName,
    now: &jiff::Zoned,
    note: Option<&str>,
) -> Result<()> {
    store.append_start(name, now, note)
}

pub fn stop<S: ProjectStore>(store: &S, name: &ProjectName, now: &jiff::Zoned) -> Result<()> {
    store.close_open(name, now)
}

pub fn load<S: ProjectStore>(store: &S, name: &ProjectName) -> Result<Project> {
    store.load(name)
}

#[derive(Debug, Clone)]
pub struct Row {
    pub name: ProjectName,
    pub seconds: i64,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub interval: Interval,
    pub rows: Vec<Row>,
    pub total_seconds: i64,
}

pub fn report<S: ProjectStore>(
    store: &S,
    only: Option<&ProjectName>,
    interval: Interval,
    now: &jiff::Zoned,
    tz: &TimeZone,
) -> Result<Report> {
    let names: Vec<ProjectName> = match only {
        Some(n) => vec![n.clone()],
        None => store.list()?,
    };
    let mut rows = Vec::new();
    let mut total = 0i64;
    for n in names {
        let p = store.load(&n)?;
        let s = project_total_in(&p, &interval, now, tz);
        total += s;
        rows.push(Row { name: n, seconds: s });
    }
    Ok(Report {
        interval,
        rows,
        total_seconds: total,
    })
}

#[derive(Debug, Clone)]
pub struct StatusEntry {
    pub name: ProjectName,
    pub start: jiff::Zoned,
    pub running_seconds: i64,
}

pub fn status<S: ProjectStore>(
    store: &S,
    only: Option<&ProjectName>,
    now: &jiff::Zoned,
) -> Result<Vec<StatusEntry>> {
    let names: Vec<ProjectName> = match only {
        Some(n) => vec![n.clone()],
        None => store.list()?,
    };
    let mut out = Vec::new();
    for n in names {
        let p = store.load(&n)?;
        if let Some(s) = p.running_session() {
            let secs = if now > &s.start {
                now.timestamp().as_second() - s.start.timestamp().as_second()
            } else {
                0
            };
            out.push(StatusEntry {
                name: p.name,
                start: s.start.clone(),
                running_seconds: secs,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::mem::MemStore;
    use jiff::{civil::date, tz::TimeZone};

    fn z(h: i8, min: i8) -> jiff::Zoned {
        date(2026, 5, 4)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn full_lifecycle() {
        let s = MemStore::new();
        let n = ProjectName::parse("rust-zlp").unwrap();
        create(&s, &n).unwrap();
        start(&s, &n, &z(9, 0), Some("note")).unwrap();
        stop(&s, &n, &z(10, 30)).unwrap();
        let r = report(
            &s,
            Some(&n),
            Interval::Day(date(2026, 5, 4)),
            &z(23, 0),
            &TimeZone::UTC,
        )
        .unwrap();
        assert_eq!(r.total_seconds, 5400);
    }

    #[test]
    fn status_lists_only_running() {
        let s = MemStore::new();
        let a = ProjectName::parse("a").unwrap();
        let b = ProjectName::parse("b").unwrap();
        create(&s, &a).unwrap();
        create(&s, &b).unwrap();
        start(&s, &a, &z(9, 0), None).unwrap();
        let st = status(&s, None, &z(9, 30)).unwrap();
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].name.as_str(), "a");
        assert_eq!(st[0].running_seconds, 1800);
    }
}
```

The `report` function loads every project (or one specific project), sums each project's tracked time within the given interval, and returns a `Report` with rows and a total. The `status` function finds all currently running timers and computes how long each has been running.

### Step 2: Register the module

Replace the contents of `src/lib.rs` with:

```rust
//! Library core for zeitlupius. UI front-ends call into this crate.

pub mod error;
pub mod model;
pub mod ops;
pub mod storage;
pub mod time;

pub use error::{Error, ErrorKind, Result};
pub use model::ProjectName;
pub use storage::ProjectStore;
```

### Step 3: Run the tests

```bash
cargo test --lib ops::tests
```

You should see `2 passed`.

### Step 4: Commit

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "feat: add ops module wrapping the store with reports"
```

---

## Chapter 7 — The CLI

The CLI is the command-line interface — the way users interact with zeitlupius by typing commands like `zeitlupius start my-project`. This chapter creates the argument parser, the command handlers, the main program entry point, and integration tests.

Create the directory `src/cli/`.

### Step 1: Create duration formatters

Create `src/cli/format.rs` with:

```rust
pub fn fmt_hms(seconds: i64) -> String {
    let s = seconds.max(0);
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

pub fn fmt_hm(seconds: i64) -> String {
    let s = seconds.max(0);
    format!("{:>3}:{:02}", s / 3600, (s % 3600) / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hms_pads_correctly() {
        assert_eq!(fmt_hms(0), "00:00:00");
        assert_eq!(fmt_hms(3661), "01:01:01");
        assert_eq!(fmt_hms(-5), "00:00:00");
    }

    #[test]
    fn hm_aligns() {
        assert_eq!(fmt_hm(3600), "  1:00");
        assert_eq!(fmt_hm(45 * 3600 + 30 * 60), " 45:30");
    }
}
```

`fmt_hms` formats seconds as `HH:MM:SS` (used for live timers). `fmt_hm` formats as `HH:MM` (used in reports). Negative values are clamped to zero.

### Step 2: Create the argument parser

Create `src/cli/mod.rs` with:

```rust
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
#[group(multiple = false)]
pub struct IntervalArgs {
    #[arg(long, group = "iv")]
    pub day: bool,
    #[arg(long, group = "iv")]
    pub week: bool,
    #[arg(long, group = "iv")]
    pub month: bool,
    #[arg(long, group = "iv")]
    pub year: bool,
    #[arg(long, value_name = "DD.MM.YYYY", requires = "to")]
    pub from: Option<String>,
    #[arg(long, value_name = "DD.MM.YYYY", requires = "from")]
    pub to: Option<String>,
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
        let cli =
            Cli::try_parse_from(["zeitlupius", "start", "rust-zlp", "--note", "hi"]).unwrap();
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
        let cli =
            Cli::try_parse_from(["zeitlupius", "report", "--week", "--page", "-1"]).unwrap();
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
}
```

The `Cli` struct uses `clap`'s derive feature to parse command-line arguments automatically. `Command` lists every subcommand. `ReportArgs` holds the report-specific options. `IntervalArgs` uses a group to ensure `--day`, `--week`, `--month`, and `--year` are mutually exclusive. `--from` and `--to` must appear together.

### Step 3: Create the command handlers

Create `src/cli/handlers.rs` with:

```rust
use std::io::Write;

use jiff::tz::TimeZone;

use crate::cli::format::{fmt_hm, fmt_hms};
use crate::cli::{Command, IntervalArgs, ReportArgs};
use crate::error::{Error, Result};
use crate::model::{Interval, ProjectName};
use crate::ops;
use crate::storage::ProjectStore;

pub fn run<S: ProjectStore, W: Write>(
    store: &S,
    cmd: Command,
    out: &mut W,
    now: &jiff::Zoned,
    tz: &TimeZone,
) -> Result<()> {
    match cmd {
        Command::List => list(store, out),
        Command::Create { project } => {
            ops::create(store, &ProjectName::parse(&project)?)?;
            writeln!(out, "created '{project}'")?;
            Ok(())
        }
        Command::Delete { project, force: _ } => {
            ops::delete(store, &ProjectName::parse(&project)?)?;
            writeln!(out, "deleted '{project}'")?;
            Ok(())
        }
        Command::Start { project, note } => {
            ops::start(store, &ProjectName::parse(&project)?, now, note.as_deref())?;
            writeln!(out, "started '{project}' at {now}")?;
            Ok(())
        }
        Command::Stop { project } => {
            ops::stop(store, &ProjectName::parse(&project)?, now)?;
            writeln!(out, "stopped '{project}' at {now}")?;
            Ok(())
        }
        Command::Status { project } => {
            let only = match project {
                Some(p) => Some(ProjectName::parse(&p)?),
                None => None,
            };
            let entries = ops::status(store, only.as_ref(), now)?;
            if entries.is_empty() {
                writeln!(out, "no running timers")?;
            } else {
                for e in entries {
                    writeln!(
                        out,
                        "● {:<24} started {}   running {}",
                        e.name,
                        e.start,
                        fmt_hms(e.running_seconds)
                    )?;
                }
            }
            Ok(())
        }
        Command::Report(args) => report(store, args, out, now, tz),
    }
}

fn list<S: ProjectStore, W: Write>(store: &S, out: &mut W) -> Result<()> {
    let names = ops::list(store)?;
    if names.is_empty() {
        return Ok(());
    }
    let now = jiff::Zoned::now();
    for n in names {
        let project = ops::load(store, &n)?;
        match project.running_session() {
            Some(s) => {
                let secs =
                    (now.timestamp().as_second() - s.start.timestamp().as_second()).max(0);
                writeln!(out, "* {:<24} (running for {})", n, fmt_hms(secs))?;
            }
            None => writeln!(out, "  {n}")?,
        }
    }
    Ok(())
}

fn report<S: ProjectStore, W: Write>(
    store: &S,
    args: ReportArgs,
    out: &mut W,
    now: &jiff::Zoned,
    tz: &TimeZone,
) -> Result<()> {
    let only = match args.project {
        Some(p) => Some(ProjectName::parse(&p)?),
        None => None,
    };
    let interval = build_interval(&args.interval, args.page, now)?;
    let rep = ops::report(store, only.as_ref(), interval, now, tz)?;
    if args.json {
        write!(out, "{{\"interval\":{:?},\"rows\":[", rep.interval.label())?;
        for (i, r) in rep.rows.iter().enumerate() {
            if i > 0 {
                write!(out, ",")?;
            }
            write!(
                out,
                "{{\"project\":{:?},\"seconds\":{}}}",
                r.name.as_str(),
                r.seconds
            )?;
        }
        writeln!(out, "],\"total_seconds\":{}}}", rep.total_seconds)?;
    } else {
        writeln!(out, "{}", rep.interval.label())?;
        for r in &rep.rows {
            writeln!(out, "  {:<20}  {}", r.name, fmt_hm(r.seconds))?;
        }
        writeln!(out, "  {:<20}  {}", "TOTAL", fmt_hm(rep.total_seconds))?;
    }
    Ok(())
}

fn build_interval(args: &IntervalArgs, page: i32, now: &jiff::Zoned) -> Result<Interval> {
    let today = now.date();
    if let (Some(f), Some(t)) = (args.from.as_deref(), args.to.as_deref()) {
        let from = parse_date(f)?;
        let to = parse_date(t)?;
        if from > to {
            return Err(Error::InvalidRange {
                from: f.to_string(),
                to: t.to_string(),
            });
        }
        return Ok(Interval::Custom { from, to });
    }
    let mut iv = if args.week {
        Interval::Week(today)
    } else if args.month {
        Interval::Month(today)
    } else if args.year {
        Interval::Year(today.year())
    } else {
        Interval::Day(today)
    };
    if page < 0 {
        for _ in 0..(-page) {
            iv = iv.previous();
        }
    } else if page > 0 {
        for _ in 0..page {
            iv = iv.next();
        }
    }
    Ok(iv)
}

fn parse_date(s: &str) -> Result<jiff::civil::Date> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return Err(Error::InvalidDate(s.into()));
    }
    let dd: i8 = parts[0].parse().map_err(|_| Error::InvalidDate(s.into()))?;
    let mm: i8 = parts[1].parse().map_err(|_| Error::InvalidDate(s.into()))?;
    let yyyy: i16 = parts[2].parse().map_err(|_| Error::InvalidDate(s.into()))?;
    jiff::civil::Date::new(yyyy, mm, dd).map_err(|_| Error::InvalidDate(s.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::mem::MemStore;
    use jiff::{civil::date, tz::TimeZone};

    fn z(h: i8) -> jiff::Zoned {
        date(2026, 5, 4)
            .at(h, 0, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn list_empty_outputs_nothing() {
        let s = MemStore::new();
        let mut buf = Vec::new();
        run(&s, Command::List, &mut buf, &z(12), &TimeZone::UTC).unwrap();
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn create_then_list_shows_project() {
        let s = MemStore::new();
        let mut buf = Vec::new();
        run(
            &s,
            Command::Create {
                project: "p".into(),
            },
            &mut buf,
            &z(12),
            &TimeZone::UTC,
        )
        .unwrap();
        buf.clear();
        run(&s, Command::List, &mut buf, &z(12), &TimeZone::UTC).unwrap();
        let txt = String::from_utf8(buf).unwrap();
        assert!(txt.contains("p"));
    }

    #[test]
    fn report_day_sums() {
        let s = MemStore::new();
        let mut sink = Vec::new();
        run(
            &s,
            Command::Create {
                project: "p".into(),
            },
            &mut sink,
            &z(8),
            &TimeZone::UTC,
        )
        .unwrap();
        run(
            &s,
            Command::Start {
                project: "p".into(),
                note: None,
            },
            &mut sink,
            &z(9),
            &TimeZone::UTC,
        )
        .unwrap();
        run(
            &s,
            Command::Stop {
                project: "p".into(),
            },
            &mut sink,
            &z(10),
            &TimeZone::UTC,
        )
        .unwrap();
        let mut buf = Vec::new();
        run(
            &s,
            Command::Report(ReportArgs {
                project: None,
                interval: IntervalArgs {
                    day: true,
                    week: false,
                    month: false,
                    year: false,
                    from: None,
                    to: None,
                },
                page: 0,
                json: false,
            }),
            &mut buf,
            &z(23),
            &TimeZone::UTC,
        )
        .unwrap();
        let txt = String::from_utf8(buf).unwrap();
        assert!(txt.contains("1:00"));
        assert!(txt.contains("TOTAL"));
    }

    #[test]
    fn parse_date_rejects_garbage() {
        assert!(parse_date("nope").is_err());
        assert!(parse_date("31.02.2026").is_err());
        assert_eq!(parse_date("18.04.2024").unwrap().day(), 18);
    }
}
```

Each CLI subcommand is handled by a branch in the `run` function. The handlers call into `ops::*` and print results to a writer (so tests can capture the output). `build_interval` translates the CLI flags into an `Interval`, and `parse_date` parses the `DD.MM.YYYY` format.

### Step 4: Create the TUI stub

The TUI (terminal user interface) is built in Chapter 8. For now, create a placeholder so the program compiles.

Create the directory `src/tui/` and then create `src/tui/mod.rs` with:

```rust
pub mod app;
pub mod input;
pub mod modals;
pub mod view;

use jiff::tz::TimeZone;

use crate::error::Result;
use crate::storage::ProjectStore;

pub fn run<S: ProjectStore>(_store: &S, _tz: &TimeZone) -> Result<()> {
    eprintln!("(TUI not implemented yet)");
    Ok(())
}
```

Also create these placeholder files so the module compiles:

Create `src/tui/app.rs` with:

```rust
//! Filled in by Chapter 8.
```

Create `src/tui/input.rs` with:

```rust
//! Filled in by Chapter 8.
```

Create `src/tui/view.rs` with:

```rust
//! Filled in by Chapter 8.
```

Create `src/tui/modals.rs` with:

```rust
//! Filled in by Chapter 8.
```

### Step 5: Register the modules and create the main program

Replace the contents of `src/lib.rs` with:

```rust
//! Library core for zeitlupius. UI front-ends call into this crate.

pub mod cli;
pub mod error;
pub mod model;
pub mod ops;
pub mod storage;
pub mod time;
pub mod tui;

pub use error::{Error, ErrorKind, Result};
pub use model::ProjectName;
pub use storage::ProjectStore;
```

Replace the contents of `src/main.rs` with:

```rust
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
```

The `main` function:

1. Parses command-line arguments
2. Determines the data directory (CLI flag > `ZEITLUPIUS_HOME` env var > `~/.zeitlupius`)
3. If no subcommand, launches the TUI
4. For `delete` without `--force`, asks for confirmation
5. Runs the appropriate handler
6. Prints errors and returns the correct exit code

### Step 6: Smoke test

```bash
cargo run -- --data-dir /tmp/zlp-smoke create demo
cargo run -- --data-dir /tmp/zlp-smoke list
cargo run -- --data-dir /tmp/zlp-smoke start demo
cargo run -- --data-dir /tmp/zlp-smoke status
cargo run -- --data-dir /tmp/zlp-smoke stop demo
cargo run -- --data-dir /tmp/zlp-smoke report --day
rm -rf /tmp/zlp-smoke
```

### Step 7: Create integration tests

Create the directory `tests/` at the project root (next to `src/`) and create `tests/cli.rs` with:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(td: &TempDir) -> Command {
    let mut c = Command::cargo_bin("zeitlupius").unwrap();
    c.arg("--data-dir").arg(td.path());
    c
}

#[test]
fn create_list_delete_cycle() {
    let td = TempDir::new().unwrap();

    cmd(&td).args(["create", "rust-zlp"]).assert().success();
    cmd(&td)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("rust-zlp"));
    cmd(&td)
        .args(["delete", "rust-zlp", "--force"])
        .assert()
        .success();
    cmd(&td)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn start_then_status() {
    let td = TempDir::new().unwrap();
    cmd(&td).args(["create", "p"]).assert().success();
    cmd(&td).args(["start", "p"]).assert().success();
    cmd(&td)
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("p"));
}

#[test]
fn missing_project_user_error() {
    let td = TempDir::new().unwrap();
    cmd(&td).args(["start", "nope"]).assert().code(1);
}

#[test]
fn invalid_date_user_error() {
    let td = TempDir::new().unwrap();
    cmd(&td)
        .args(["report", "--from", "31.02.2026", "--to", "01.03.2026"])
        .assert()
        .code(1);
}

#[test]
fn report_week_runs() {
    let td = TempDir::new().unwrap();
    cmd(&td).args(["create", "p"]).assert().success();
    cmd(&td)
        .args(["report", "--week"])
        .assert()
        .success()
        .stdout(predicate::str::contains("TOTAL"));
}
```

These integration tests run the actual compiled program with a temporary data directory. They verify: the full create/list/delete cycle, starting a timer and checking status, proper error exit codes, and that reports work.

### Step 8: Run all tests

```bash
cargo test
```

All tests should pass — unit tests and integration tests alike.

### Step 9: Commit

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "feat: implement CLI surface end-to-end with integration tests"
```

---

## Chapter 8 — The TUI

The TUI (Terminal User Interface) is the graphical dashboard that appears when you run `zeitlupius` with no subcommand. It has four source files:

- `app.rs` — the state of the dashboard (selected project, current interval, modal dialogs)
- `input.rs` — translates key presses into actions
- `view.rs` — draws the dashboard using `ratatui`
- `modals.rs` — draws popup dialogs (new project, confirm delete, custom interval, help)
- `mod.rs` — the main event loop that ties everything together

### Step 1: Create the app state

Replace the contents of `src/tui/app.rs` with:

```rust
use crate::model::{Interval, ProjectName};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalKind {
    Day,
    Week,
    Month,
    Year,
    Custom,
}

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    NewProject { input: String },
    ConfirmDelete { project: ProjectName, running: bool },
    CustomInterval { from: String, to: String, focus_to: bool, error: Option<String> },
    Help,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub projects: Vec<ProjectName>,
    pub selected: usize,
    pub interval: Interval,
    pub modal: Modal,
    pub status_line: Option<(String, std::time::Instant)>,
    pub should_quit: bool,
}

impl AppState {
    pub fn new(today: jiff::civil::Date, projects: Vec<ProjectName>) -> Self {
        Self {
            projects,
            selected: 0,
            interval: Interval::Day(today),
            modal: Modal::None,
            status_line: None,
            should_quit: false,
        }
    }

    pub fn selected_project(&self) -> Option<&ProjectName> {
        self.projects.get(self.selected)
    }

    pub fn ensure_selection_valid(&mut self) {
        if self.projects.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.projects.len() {
            self.selected = self.projects.len() - 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
    pub fn move_down(&mut self) {
        if self.selected + 1 < self.projects.len() {
            self.selected += 1;
        }
    }

    pub fn switch_interval(&mut self, kind: IntervalKind, today: jiff::civil::Date) {
        self.interval = match kind {
            IntervalKind::Day => Interval::Day(today),
            IntervalKind::Week => Interval::Week(today),
            IntervalKind::Month => Interval::Month(today),
            IntervalKind::Year => Interval::Year(today.year()),
            IntervalKind::Custom => self.interval.clone(),
        };
    }

    pub fn page_prev(&mut self) {
        self.interval = self.interval.previous();
    }
    pub fn page_next(&mut self) {
        self.interval = self.interval.next();
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_line = Some((msg.into(), std::time::Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn names(xs: &[&str]) -> Vec<ProjectName> {
        xs.iter()
            .map(|x| ProjectName::parse(x).unwrap())
            .collect()
    }

    #[test]
    fn navigation_clamps_at_edges() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a", "b", "c"]));
        a.move_up();
        assert_eq!(a.selected, 0);
        a.move_down();
        a.move_down();
        a.move_down();
        assert_eq!(a.selected, 2);
    }

    #[test]
    fn switch_interval_resets_to_today() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a"]));
        a.page_prev();
        a.switch_interval(IntervalKind::Week, date(2026, 5, 4));
        assert!(matches!(a.interval, Interval::Week(_)));
    }

    #[test]
    fn ensure_selection_valid_after_deletion() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a", "b"]));
        a.selected = 1;
        a.projects.pop();
        a.ensure_selection_valid();
        assert_eq!(a.selected, 0);
    }
}
```

`AppState` holds everything the TUI needs to know: which projects exist, which one is selected, what interval is shown, whether a modal dialog is open, and any status message. The helper methods handle navigation (clamped so you cannot go past the edge) and interval switching.

### Step 2: Create the key dispatcher

Replace the contents of `src/tui/input.rs` with:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::{AppState, IntervalKind, Modal};

pub enum Action {
    None,
    StartSelected,
    StopSelected,
    DeleteSelected,
    NewProject(String),
    DeleteConfirmed,
    ApplyCustomInterval(jiff::civil::Date, jiff::civil::Date),
    Reload,
}

pub fn dispatch(key: KeyEvent, state: &mut AppState, today: jiff::civil::Date) -> Action {
    let modal = state.modal.clone();
    match modal {
        Modal::None => dispatch_dashboard(key, state, today),
        Modal::NewProject { input } => dispatch_new_project(key, input, state),
        Modal::ConfirmDelete { .. } => dispatch_confirm_delete(key, state),
        Modal::CustomInterval {
            from,
            to,
            focus_to,
            ..
        } => dispatch_custom(key, from, to, focus_to, state),
        Modal::Help => {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
            ) {
                state.modal = Modal::None;
            }
            Action::None
        }
    }
}

fn dispatch_dashboard(key: KeyEvent, state: &mut AppState, today: jiff::civil::Date) -> Action {
    if matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL) {
        state.should_quit = true;
        return Action::None;
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.should_quit = true;
            Action::None
        }
        KeyCode::Char('?') => {
            state.modal = Modal::Help;
            Action::None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            state.move_down();
            Action::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.move_up();
            Action::None
        }
        KeyCode::Char('d') => {
            state.switch_interval(IntervalKind::Day, today);
            Action::None
        }
        KeyCode::Char('w') => {
            state.switch_interval(IntervalKind::Week, today);
            Action::None
        }
        KeyCode::Char('m') => {
            state.switch_interval(IntervalKind::Month, today);
            Action::None
        }
        KeyCode::Char('y') => {
            state.switch_interval(IntervalKind::Year, today);
            Action::None
        }
        KeyCode::Left => {
            state.page_prev();
            Action::None
        }
        KeyCode::Right => {
            state.page_next();
            Action::None
        }
        KeyCode::Home | KeyCode::Char('g') => {
            use crate::model::Interval;
            state.interval = match state.interval {
                Interval::Day(_) => Interval::Day(today),
                Interval::Week(_) => Interval::Week(today),
                Interval::Month(_) => Interval::Month(today),
                Interval::Year(_) => Interval::Year(today.year()),
                Interval::Custom { .. } => Interval::Day(today),
            };
            Action::None
        }
        KeyCode::Char('c') => {
            state.modal = Modal::CustomInterval {
                from: String::new(),
                to: String::new(),
                focus_to: false,
                error: None,
            };
            Action::None
        }
        KeyCode::Char('n') => {
            state.modal = Modal::NewProject {
                input: String::new(),
            };
            Action::None
        }
        KeyCode::Char('s') => Action::StartSelected,
        KeyCode::Char('S') => Action::StopSelected,
        KeyCode::Char('D') => Action::DeleteSelected,
        KeyCode::Char('r') => Action::Reload,
        _ => Action::None,
    }
}

fn dispatch_new_project(key: KeyEvent, mut input: String, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.modal = Modal::None;
            Action::None
        }
        KeyCode::Enter => {
            state.modal = Modal::None;
            Action::NewProject(input)
        }
        KeyCode::Backspace => {
            input.pop();
            state.modal = Modal::NewProject { input };
            Action::None
        }
        KeyCode::Char(c) => {
            input.push(c);
            state.modal = Modal::NewProject { input };
            Action::None
        }
        _ => Action::None,
    }
}

fn dispatch_confirm_delete(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => Action::DeleteConfirmed,
        _ => {
            state.modal = Modal::None;
            Action::None
        }
    }
}

fn dispatch_custom(
    key: KeyEvent,
    mut from: String,
    mut to: String,
    mut focus_to: bool,
    state: &mut AppState,
) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.modal = Modal::None;
            Action::None
        }
        KeyCode::Tab => {
            focus_to = !focus_to;
            state.modal = Modal::CustomInterval {
                from,
                to,
                focus_to,
                error: None,
            };
            Action::None
        }
        KeyCode::Backspace => {
            if focus_to {
                to.pop();
            } else {
                from.pop();
            }
            state.modal = Modal::CustomInterval {
                from,
                to,
                focus_to,
                error: None,
            };
            Action::None
        }
        KeyCode::Char(c) => {
            if focus_to {
                to.push(c);
            } else {
                from.push(c);
            }
            state.modal = Modal::CustomInterval {
                from,
                to,
                focus_to,
                error: None,
            };
            Action::None
        }
        KeyCode::Enter => match (parse_dot_date(&from), parse_dot_date(&to)) {
            (Some(f), Some(t)) if f <= t => {
                state.modal = Modal::None;
                Action::ApplyCustomInterval(f, t)
            }
            _ => {
                state.modal = Modal::CustomInterval {
                    from,
                    to,
                    focus_to,
                    error: Some("invalid dates".into()),
                };
                Action::None
            }
        },
        _ => Action::None,
    }
}

fn parse_dot_date(s: &str) -> Option<jiff::civil::Date> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let dd: i8 = parts[0].parse().ok()?;
    let mm: i8 = parts[1].parse().ok()?;
    let yyyy: i16 = parts[2].parse().ok()?;
    jiff::civil::Date::new(yyyy, mm, dd).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEventKind, KeyEventState};
    use jiff::civil::date;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent {
            code: c,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }
    fn names(xs: &[&str]) -> Vec<crate::model::ProjectName> {
        xs.iter()
            .map(|x| crate::model::ProjectName::parse(x).unwrap())
            .collect()
    }

    #[test]
    fn q_quits() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        let _ = dispatch(key(KeyCode::Char('q')), &mut s, date(2026, 5, 4));
        assert!(s.should_quit);
    }

    #[test]
    fn s_returns_start_action() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        let a = dispatch(key(KeyCode::Char('s')), &mut s, date(2026, 5, 4));
        assert!(matches!(a, Action::StartSelected));
    }

    #[test]
    fn n_opens_modal_and_typing_appends() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        dispatch(key(KeyCode::Char('n')), &mut s, date(2026, 5, 4));
        dispatch(key(KeyCode::Char('p')), &mut s, date(2026, 5, 4));
        let act = dispatch(key(KeyCode::Enter), &mut s, date(2026, 5, 4));
        match act {
            Action::NewProject(name) => assert_eq!(name, "p"),
            _ => panic!(),
        }
    }

    #[test]
    fn arrow_left_pages_back() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        dispatch(key(KeyCode::Left), &mut s, date(2026, 5, 4));
        match s.interval {
            crate::model::Interval::Day(d) => assert_eq!(d, date(2026, 5, 3)),
            _ => panic!(),
        }
    }
}
```

The dispatcher translates key events into `Action` values. The event loop (in `mod.rs`) will execute each action against the store. This separation means we can test the key-handling logic without a real terminal. Key bindings: `j`/`k` or arrows to navigate, `d`/`w`/`m`/`y` to switch intervals, `s`/`S` to start/stop, `n` for new project, `D` for delete, `q` to quit.

### Step 3: Create the dashboard renderer

Replace the contents of `src/tui/view.rs` with:

```rust
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::cli::format::{fmt_hm, fmt_hms};
use crate::model::{Interval, Project, ProjectName};
use crate::tui::app::{AppState, Modal};

pub struct DashboardData<'a> {
    pub state: &'a AppState,
    pub projects: &'a [Project],
    pub now: &'a jiff::Zoned,
    pub tz: &'a jiff::tz::TimeZone,
}

pub fn draw(f: &mut Frame, data: &DashboardData) {
    let area = f.area();
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);
    draw_header(f, outer[0], data);
    if data.projects.is_empty() {
        draw_empty(f, outer[1]);
    } else {
        draw_body(f, outer[1], data);
    }
    draw_footer(f, outer[2], data);
    if !matches!(data.state.modal, Modal::None) {
        super::modals::draw(f, area, &data.state.modal);
    }
}

fn draw_header(f: &mut Frame, area: Rect, data: &DashboardData) {
    let kind = match data.state.interval {
        Interval::Day(_) => "Day",
        Interval::Week(_) => "Week",
        Interval::Month(_) => "Month",
        Interval::Year(_) => "Year",
        Interval::Custom { .. } => "Custom",
    };
    let label = data.state.interval.label();
    let txt = format!("zeitlupius · {kind} · {label}");
    let p = Paragraph::new(Line::from(Span::styled(
        txt,
        Style::default().add_modifier(Modifier::BOLD),
    )));
    f.render_widget(p, area);
}

fn draw_empty(f: &mut Frame, area: Rect) {
    let p = Paragraph::new("No projects yet — press n to create one.")
        .alignment(ratatui::layout::Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn draw_body(f: &mut Frame, area: Rect, data: &DashboardData) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);
    draw_project_list(f, cols[0], data);
    draw_detail(f, cols[1], data);
}

fn draw_project_list(f: &mut Frame, area: Rect, data: &DashboardData) {
    use crate::time::project_total_in;
    let items: Vec<ListItem> = data
        .projects
        .iter()
        .map(|p| {
            let secs = project_total_in(p, &data.state.interval, data.now, data.tz);
            let running = p.running_session().map(|s| {
                let r = data.now.timestamp().as_second() - s.start.timestamp().as_second();
                r.max(0)
            });
            let mut spans = Vec::new();
            if let Some(r) = running {
                spans.push(Span::styled("● ", Style::default().fg(Color::Green)));
                spans.push(Span::raw(format!("{:<24}", p.name)));
                spans.push(Span::styled(fmt_hms(r), Style::default().fg(Color::Green)));
            } else {
                spans.push(Span::raw("  "));
                spans.push(Span::raw(format!("{:<24}", p.name)));
                spans.push(Span::raw(fmt_hm(secs)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let mut ls = ListState::default();
    ls.select(Some(data.state.selected));
    let list = List::new(items)
        .block(Block::default().borders(Borders::RIGHT).title("Projects"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, area, &mut ls);
}

fn draw_detail(f: &mut Frame, area: Rect, data: &DashboardData) {
    use crate::time::project_total_in;
    let Some(p) = data.projects.get(data.state.selected) else {
        return;
    };
    let today = data.now.date();
    let intervals = [
        ("Day  ", Interval::Day(today)),
        ("Week ", Interval::Week(today)),
        ("Month", Interval::Month(today)),
        ("Year ", Interval::Year(today.year())),
    ];
    let mut lines = vec![
        Line::from(Span::styled(
            p.name.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for (label, iv) in intervals {
        let secs = project_total_in(p, &iv, data.now, data.tz);
        lines.push(Line::from(format!(
            "{label}  {}  ({})",
            fmt_hm(secs),
            iv.label()
        )));
    }
    lines.push(Line::from(""));
    if let Some(s) = p.running_session() {
        let r = (data.now.timestamp().as_second() - s.start.timestamp().as_second()).max(0);
        lines.push(Line::from(Span::styled(
            "Active session",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(format!(
            " started {} for {}",
            s.start,
            fmt_hms(r)
        )));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "Recent sessions (last 5)",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for s in p.sessions.iter().rev().take(5) {
        let stop = match &s.stop {
            Some(z) => z.to_string(),
            None => "running".to_string(),
        };
        let secs = match &s.stop {
            Some(stop) => stop.timestamp().as_second() - s.start.timestamp().as_second(),
            None => {
                (data.now.timestamp().as_second() - s.start.timestamp().as_second()).max(0)
            }
        };
        lines.push(Line::from(format!(
            " {}  →  {}    {}",
            s.start,
            stop,
            fmt_hms(secs)
        )));
    }
    let par = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Detail"));
    f.render_widget(par, area);
}

fn draw_footer(f: &mut Frame, area: Rect, data: &DashboardData) {
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);
    let line1 = "d/w/m/y interval · ←/→ page · c custom · j/k select";
    let line2 = "s start · S stop · n new · D delete · r reload · ? help · q quit";
    f.render_widget(Paragraph::new(line1), split[0]);
    let l2 = if let Some((msg, _)) = &data.state.status_line {
        Paragraph::new(Line::from(Span::styled(
            msg.clone(),
            Style::default().fg(Color::Red),
        )))
    } else {
        Paragraph::new(line2)
    };
    f.render_widget(l2, split[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Project, ProjectName, Session};
    use jiff::{civil::date, tz::TimeZone};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn z(h: i8) -> jiff::Zoned {
        date(2026, 5, 4)
            .at(h, 0, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn renders_empty_dashboard_without_panic() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let state = AppState::new(date(2026, 5, 4), vec![]);
        let now = z(12);
        let tz = TimeZone::UTC;
        term.draw(|f| {
            draw(
                f,
                &DashboardData {
                    state: &state,
                    projects: &[],
                    now: &now,
                    tz: &tz,
                },
            )
        })
        .unwrap();
    }

    #[test]
    fn renders_with_projects_without_panic() {
        let backend = TestBackend::new(100, 25);
        let mut term = Terminal::new(backend).unwrap();
        let projects = vec![Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![Session {
                start: z(9),
                stop: Some(z(10)),
                note: None,
            }],
        }];
        let names = vec![ProjectName::parse("p").unwrap()];
        let state = AppState::new(date(2026, 5, 4), names);
        let now = z(11);
        let tz = TimeZone::UTC;
        term.draw(|f| {
            draw(
                f,
                &DashboardData {
                    state: &state,
                    projects: &projects,
                    now: &now,
                    tz: &tz,
                },
            )
        })
        .unwrap();
    }
}
```

The dashboard layout has three vertical sections: a header bar (showing the interval), the body (project list on the left, detail panel on the right), and a footer (key hints and status messages). The left panel shows each project with a green dot and live timer if running. The detail panel shows the selected project's totals across day/week/month/year, the active session, and the last 5 sessions.

### Step 4: Create the modal renderer

Replace the contents of `src/tui/modals.rs` with:

```rust
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::app::Modal;

pub fn draw(f: &mut Frame, area: Rect, modal: &Modal) {
    let popup = centered(area, 50, 30);
    f.render_widget(Clear, popup);
    match modal {
        Modal::None => {}
        Modal::NewProject { input } => {
            let p = Paragraph::new(vec![
                Line::from("Create project — name:"),
                Line::from(""),
                Line::from(Span::styled(
                    input.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Enter to create · Esc to cancel"),
            ])
            .block(Block::default().borders(Borders::ALL).title("New project"))
            .alignment(Alignment::Left);
            f.render_widget(p, popup);
        }
        Modal::ConfirmDelete { project, running } => {
            let mut lines = vec![
                Line::from(format!("Delete '{project}'?")),
                Line::from(""),
                Line::from("All sessions will be erased."),
            ];
            if *running {
                lines.push(Line::from(Span::styled(
                    "This project is running — the open session will be lost.",
                    Style::default().fg(Color::Yellow),
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from("y to confirm · N/Esc to cancel"));
            f.render_widget(
                Paragraph::new(lines)
                    .block(Block::default().borders(Borders::ALL).title("Confirm delete")),
                popup,
            );
        }
        Modal::CustomInterval {
            from,
            to,
            focus_to,
            error,
        } => {
            let from_line = if !*focus_to {
                format!("> from: {from}")
            } else {
                format!("  from: {from}")
            };
            let to_line = if *focus_to {
                format!("> to:   {to}")
            } else {
                format!("  to:   {to}")
            };
            let mut lines = vec![
                Line::from("Custom interval (DD.MM.YYYY)"),
                Line::from(""),
                Line::from(from_line),
                Line::from(to_line),
                Line::from(""),
                Line::from("Tab to switch · Enter to apply · Esc to cancel"),
            ];
            if let Some(err) = error {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    err.clone(),
                    Style::default().fg(Color::Red),
                )));
            }
            f.render_widget(
                Paragraph::new(lines)
                    .block(Block::default().borders(Borders::ALL).title("Custom interval")),
                popup,
            );
        }
        Modal::Help => {
            let lines = vec![
                Line::from("Keys"),
                Line::from(""),
                Line::from("j/k or ↓/↑   move selection"),
                Line::from("d/w/m/y      switch interval"),
                Line::from("← / →        page back / forward"),
                Line::from("g / Home     reset to current"),
                Line::from("c            custom interval"),
                Line::from("s / S        start / stop selected project"),
                Line::from("n            new project"),
                Line::from("D            delete selected project"),
                Line::from("r            reload from disk"),
                Line::from("? / Esc      close help · q quit"),
            ];
            f.render_widget(
                Paragraph::new(lines)
                    .block(Block::default().borders(Borders::ALL).title("Help")),
                popup,
            );
        }
    }
}

fn centered(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_pct) / 2),
            Constraint::Percentage(height_pct),
            Constraint::Percentage((100 - height_pct) / 2),
        ])
        .split(area)[1];
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(v)[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn renders_each_modal_without_panic() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        for m in [
            Modal::NewProject {
                input: "abc".into(),
            },
            Modal::Help,
            Modal::CustomInterval {
                from: "01.01.2026".into(),
                to: "02.01.2026".into(),
                focus_to: true,
                error: None,
            },
        ] {
            term.draw(|f| draw(f, f.area(), &m)).unwrap();
        }
    }
}
```

Each modal draws a centered popup. The `centered` helper splits the screen into thirds horizontally and vertically and takes the center cell. The `ConfirmDelete` modal adds a yellow warning if the project has a running timer.

### Step 5: Create the event loop

Replace the contents of `src/tui/mod.rs` with:

```rust
pub mod app;
pub mod input;
pub mod modals;
pub mod view;

use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use jiff::tz::TimeZone;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::error::Result;
use crate::model::{Interval, Project, ProjectName};
use crate::ops;
use crate::storage::ProjectStore;
use crate::tui::app::{AppState, Modal};
use crate::tui::input::{dispatch, Action};
use crate::tui::view::{draw as draw_dashboard, DashboardData};

const TICK: Duration = Duration::from_millis(250);
const RELOAD_EVERY: Duration = Duration::from_millis(1000);
const STATUS_TTL: Duration = Duration::from_secs(3);

pub fn run<S: ProjectStore>(store: &S, tz: &TimeZone) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let result = event_loop(store, tz, &mut term);

    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;
    result
}

fn event_loop<S: ProjectStore>(
    store: &S,
    tz: &TimeZone,
    term: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<()> {
    let now = jiff::Zoned::now();
    let names = ops::list(store)?;
    let mut state = AppState::new(now.date(), names.clone());
    let mut projects = load_all(store, &names)?;
    let mut last_reload = Instant::now();

    while !state.should_quit {
        let now = jiff::Zoned::now();
        if let Some((_, started)) = &state.status_line {
            if started.elapsed() > STATUS_TTL {
                state.status_line = None;
            }
        }
        term.draw(|f| {
            draw_dashboard(
                f,
                &DashboardData {
                    state: &state,
                    projects: &projects,
                    now: &now,
                    tz,
                },
            );
        })?;

        if event::poll(TICK)? {
            if let Event::Key(k) = event::read()? {
                let today = now.date();
                let action = dispatch(k, &mut state, today);
                match handle_action(store, action, &mut state, &now) {
                    Ok(true) => {
                        let names = ops::list(store)?;
                        state.projects = names.clone();
                        state.ensure_selection_valid();
                        projects = load_all(store, &names)?;
                    }
                    Ok(false) => {}
                    Err(e) => state.set_status(format!("error: {e}")),
                }
            }
        }

        if last_reload.elapsed() >= RELOAD_EVERY {
            let names = ops::list(store)?;
            state.projects = names.clone();
            state.ensure_selection_valid();
            projects = load_all(store, &names)?;
            last_reload = Instant::now();
        }
    }
    Ok(())
}

fn load_all<S: ProjectStore>(store: &S, names: &[ProjectName]) -> Result<Vec<Project>> {
    names.iter().map(|n| ops::load(store, n)).collect()
}

fn handle_action<S: ProjectStore>(
    store: &S,
    action: Action,
    state: &mut AppState,
    now: &jiff::Zoned,
) -> Result<bool> {
    match action {
        Action::None => Ok(false),
        Action::Reload => Ok(true),
        Action::StartSelected => {
            let Some(n) = state.selected_project().cloned() else {
                return Ok(false);
            };
            ops::start(store, &n, now, None)?;
            state.set_status(format!("started {n}"));
            Ok(true)
        }
        Action::StopSelected => {
            let Some(n) = state.selected_project().cloned() else {
                return Ok(false);
            };
            ops::stop(store, &n, now)?;
            state.set_status(format!("stopped {n}"));
            Ok(true)
        }
        Action::DeleteSelected => {
            let Some(n) = state.selected_project().cloned() else {
                return Ok(false);
            };
            let p = ops::load(store, &n)?;
            state.modal = Modal::ConfirmDelete {
                project: n,
                running: p.running_session().is_some(),
            };
            Ok(false)
        }
        Action::DeleteConfirmed => {
            let Modal::ConfirmDelete { project, .. } =
                std::mem::replace(&mut state.modal, Modal::None)
            else {
                return Ok(false);
            };
            ops::delete(store, &project)?;
            state.set_status(format!("deleted {project}"));
            Ok(true)
        }
        Action::NewProject(raw) => {
            let n = ProjectName::parse(&raw)?;
            ops::create(store, &n)?;
            state.set_status(format!("created {n}"));
            Ok(true)
        }
        Action::ApplyCustomInterval(from, to) => {
            state.interval = Interval::Custom { from, to };
            Ok(false)
        }
    }
}
```

The event loop:

1. Draws the dashboard every 250 ms (so live timers update smoothly).
2. Reads keyboard input and dispatches it to the key handler.
3. Executes the resulting action (start/stop/create/delete) against the store.
4. Reloads data from disk every second (so changes from another terminal appear automatically).
5. Errors are caught and shown in a red status line instead of crashing.

### Step 6: Build and smoke test

```bash
cargo build
```

If the build succeeds, try it out:

```bash
cargo run -- --data-dir /tmp/zlp-tui
```

You should see the TUI dashboard. Press `n`, type a project name, and press Enter to create a project. Press `s` to start the timer. Press `S` to stop it. Press `q` to quit. Then clean up:

```bash
rm -rf /tmp/zlp-tui
```

### Step 7: Run all tests

```bash
cargo test
```

All tests should pass.

### Step 8: Commit

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "feat: TUI with Layout B, modals, and event loop"
```

---

## Chapter 9 — Wrapping up

### Step 1: Create a README

Create `README.md` at the project root with:

```markdown
# zeitlupius

Terminal time tracking — a Ratatui TUI plus a feature-equivalent CLI. Each project's sessions live in a CSV under `~/.zeitlupius/projects/`.

## Install

    cargo install --path .

## Usage

Launch the TUI:

    zeitlupius

Or use the CLI:

    zeitlupius create rust-zlp
    zeitlupius start  rust-zlp
    zeitlupius stop   rust-zlp
    zeitlupius status
    zeitlupius report --week
    zeitlupius report --from 18.04.2024 --to 16.07.2024

Override the data directory:

    zeitlupius --data-dir /path/to/dir <subcommand>
    ZEITLUPIUS_HOME=/path/to/dir zeitlupius

## Design

See `docs/superpowers/specs/2026-05-04-zeitlupius-design.md` for the architecture and storage format.
```

### Step 2: Final verification

Run the full test suite and lints:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All green? Good. Now verify manually:

1. **CSV format:** `zeitlupius --data-dir /tmp/zlp-verify create demo && zeitlupius --data-dir /tmp/zlp-verify start demo`. Open `/tmp/zlp-verify/projects/demo.csv` in your editor — you should see the header `start,stop,note` and one row with an empty `stop`.
2. **Stop:** `zeitlupius --data-dir /tmp/zlp-verify stop demo`. Check the CSV again — the `stop` column is now filled.
3. **Exit codes:** `zeitlupius --data-dir /tmp/zlp-verify start nope; echo $?` — should print `1`.
4. **TUI:** `zeitlupius --data-dir /tmp/zlp-verify` — you should see the dashboard with the `demo` project.
5. **Cross-process:** Open two terminals. Run the TUI in one. In the other, run `zeitlupius --data-dir /tmp/zlp-verify start demo`. The TUI should update within ~1 second.
6. **Clean up:** `rm -rf /tmp/zlp-verify`

### Step 3: Final commit

```bash
git add -A
git commit -m "docs: add README"
```

Congratulations — you have built a complete terminal time tracker from scratch.

---

## Where to find more depth

- The full design spec is at `docs/superpowers/specs/2026-05-04-zeitlupius-design.md`.
- The implementation plan with step-by-step code is at `docs/superpowers/plans/2026-05-04-zeitlupius.md`.
- `jiff` documentation: <https://docs.rs/jiff>
- `ratatui` documentation: <https://ratatui.rs>
