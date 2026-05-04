# Building zeitlupius — A Hands-On Tutorial

This tutorial walks you through implementing `zeitlupius` from the empty Cargo crate to a working terminal time tracker with a Ratatui TUI and a feature-equivalent CLI. It is the friendly counterpart to the agent plan at `docs/superpowers/plans/2026-05-04-zeitlupius.md` — that plan lists every step verbatim with full code; this tutorial explains the *why* and walks you through the architecture chapter by chapter.

If you ever feel lost, the spec at `docs/superpowers/specs/2026-05-04-zeitlupius-design.md` is the source of truth, and the agent plan has the complete code for every file. Use them as references.

**Audience:** A working Rust developer who wants to understand each design decision while implementing them. You should be comfortable with traits, modules, and `cargo test`. You don't need to know `ratatui` or `jiff` — we introduce them.

**Time estimate:** A focused day for a comfortable Rust developer; longer if you want to take detours into `ratatui` or `jiff`. Plan to commit after each numbered section.

**What you'll have at the end:** A binary called `zeitlupius` that:

- Stores projects under `~/.zeitlupius/projects/<name>.csv`
- Lets you `start`, `stop`, `list`, `create`, `delete`, `status`, and `report` from the CLI
- Has a TUI dashboard (split list + detail panel) with live-ticking timers and pageable Day/Week/Month/Year/Custom intervals
- Treats CLI and TUI as peers — both can do everything

---

## Chapter 0 — Conventions and habits

Before we touch code, internalize three habits that will save you pain:

1. **TDD per change.** Every behaviour we add starts with a failing test. The compiler error or assertion failure tells you what to write next. Don't write tests after the fact — you'll miss edge cases and ship bugs.
2. **Commit after every chapter.** This tutorial is structured so each section ends with a clean commit. Use `feat:`, `test:`, `refactor:`, `docs:`, or `chore:` prefixes — the existing repo uses that style.
3. **Keep the build green.** Run `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test` before each commit. This is what the spec's testing strategy is built to support.

We won't repeat these reminders. Just always do them.

---

## Chapter 1 — Project scaffold

The repo already contains a fresh Cargo crate (`Cargo.toml`, `src/main.rs` printing "Hello, world!"). Our first job is to convert it into a **library + binary** crate. This is a common Rust pattern for tools with multiple front-ends: the library is where the testable logic lives, and the binary just parses arguments and dispatches.

### 1.1 Add the dependencies

Use `cargo add` so you get current stable versions instead of guessing.

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

cargo add --dev tempfile
cargo add --dev assert_cmd
cargo add --dev predicates
```

What each is for:

- **clap** — CLI parser. We use the derive feature for a clean `#[derive(Parser)]` style.
- **ratatui** + **crossterm** — TUI framework and its terminal backend. `ratatui` does layout and widgets; `crossterm` reads key events and toggles raw mode.
- **jiff** — modern date/time library by BurntSushi. We use `jiff::Zoned` (a moment in a specific time zone), `jiff::civil::Date` (a calendar date with no time), and `jiff::Span` (a duration). Critically, jiff's arithmetic is DST-aware — a Berlin "day" on the 25th of October has 25 hours, and `Interval::contains` will get that right.
- **csv** + **serde** — CSV codec. We use serde derive on the row struct.
- **fd-lock** — small wrapper over `flock`. We use it for one advisory lock file at `~/.zeitlupius/.lock` so two processes mutating the same project can't trample each other.
- **thiserror** + **anyhow** — the conventional library/binary error split. The library defines a typed `Error` enum; the binary adds runtime context with `anyhow::Context`.

### 1.2 Declare lib + bin in `Cargo.toml`

Append:

```toml
[lib]
name = "zeitlupius"
path = "src/lib.rs"

[[bin]]
name = "zeitlupius"
path = "src/main.rs"
```

### 1.3 Create `src/lib.rs` and shrink `src/main.rs`

```rust
// src/lib.rs
//! Library core for zeitlupius. UI front-ends call into this crate.
```

```rust
// src/main.rs
fn main() {
    println!("zeitlupius — placeholder; replaced in later chapters");
}
```

Run `cargo build`. It should compile cleanly. Commit:

> `chore: scaffold lib+bin layout and core dependencies`

---

## Chapter 2 — The Error type

We define one `Error` enum at the library boundary. The CLI prints `error: <Display>` and chooses an exit code based on the error kind; the TUI shows a transient red status line. The enum is designed so each variant captures everything the user needs to know — no opaque "something went wrong".

### 2.1 The shape

Create `src/error.rs`:

```rust
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
```

The two `#[from]` annotations let `?` convert `std::io::Error` and `csv::Error` automatically. The string interpolations in `#[error("...")]` are how `thiserror` builds the `Display` impl.

### 2.2 User vs internal

Add a small classifier so the binary can pick the right exit code:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind { User, Internal }

impl Error {
    pub fn kind(&self) -> ErrorKind {
        use Error::*;
        match self {
            ProjectNotFound(_) | ProjectAlreadyExists(_) | InvalidProjectName(_, _)
            | AlreadyRunning(_, _) | NotRunning(_) | InvalidDate(_) | InvalidRange { .. }
                => ErrorKind::User,
            Corrupt(_) | Io(_) | Csv(_) => ErrorKind::Internal,
        }
    }
}
```

### 2.3 Tests

Add `#[cfg(test)] mod tests` at the bottom and write a few assertions: a user error has `ErrorKind::User`, `Corrupt` has `ErrorKind::Internal`, the `Display` text matches the format string. These are tiny but they nail down the contract.

### 2.4 Re-export

In `src/lib.rs`:

```rust
pub mod error;
pub use error::{Error, ErrorKind, Result};
```

Run the tests, lint, commit:

> `feat: add Error enum and Result alias`

---

## Chapter 3 — Domain types

This chapter introduces three small files that together carry every fact `zeitlupius` knows about the world:

- `ProjectName` — a validated string for filesystem-safe project names
- `Session` — one `[start, stop)` interval, possibly open
- `Project` — a name and a chronological list of sessions
- `Interval` — Day/Week/Month/Year/Custom for reports and TUI views

We put all of them in `src/model.rs`. The library spec on this is short and the types are tightly related.

### 3.1 `ProjectName`

A naked `String` won't do — we'll use this as a filename, in CLI arguments, and in CSV headers. Trying to validate at every callsite is how dirty data leaks in. So we introduce a "newtype" with a single `parse(&str) -> Result<Self>` constructor that enforces:

- 1..=64 ASCII characters
- letters, digits, `-`, `_`, `.` only
- no leading `.`
- not equal to a reserved CLI verb (`list`, `start`, `stop`, `status`, `create`, `delete`, `report`)

The reserved-verb rule is there so `zeitlupius start list` always means "start a project literally named `list`" → wait, it doesn't — we *reject* `list` as a project name. This is intentional: it removes the ambiguity that would force users to read documentation.

Write the failing tests first (one per rule), then the impl. Implement `Display` so format strings just work.

> Tip: `ProjectName::as_str(&self) -> &str` plus `Display` is enough — don't expose the inner `String` directly. That keeps the validation airtight.

### 3.2 `Session` and `Project`

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub start: jiff::Zoned,
    pub stop: Option<jiff::Zoned>,   // None = running
    pub note: Option<String>,
}

impl Session {
    pub fn is_running(&self) -> bool { self.stop.is_none() }
    pub fn duration(&self, now: &jiff::Zoned) -> jiff::Span {
        let end = self.stop.as_ref().unwrap_or(now);
        if end < &self.start { return jiff::Span::new(); }
        end.since(&self.start).unwrap_or_else(|_| jiff::Span::new())
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
```

Two design points worth pausing on:

- **`stop: Option<Zoned>`, not a separate `RunningSession` type.** The CSV literally has an empty `stop` cell, so the in-memory shape mirrors the storage shape. We then enforce the invariant ("at most one open row, and if it exists it's the last") at the storage layer, not the type level. This trades type safety for simplicity in the codec — a worthwhile trade for v1.
- **`duration` saturates at zero on backwards clocks.** If someone runs `start` and then their NTP daemon yanks the clock backwards, we don't want the TUI to crash with a negative-duration overflow.

Tests: build a few `Zoned` values with `jiff::civil::date(...).at(...).to_zoned(TimeZone::UTC)` and assert: an open session uses `now`, a closed session ignores `now`, `running_session` returns `None` when nothing is running, etc.

### 3.3 `Interval`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Interval {
    Day(jiff::civil::Date),
    Week(jiff::civil::Date),                  // any date inside the ISO week
    Month(jiff::civil::Date),
    Year(i16),
    Custom { from: jiff::civil::Date, to: jiff::civil::Date },
}
```

`Interval` is everything we need to ask "give me the bounds in the user's local TZ", "is this timestamp inside?", "what's the previous/next page?", and "what does the header say?". We'll add four methods next chapter; the type itself is just a tagged union.

Commit at the end of the chapter:

> `feat: add domain types ProjectName, Session, Project, Interval`

---

## Chapter 4 — Time math (bounds, contains, paging, aggregation)

This is the chapter where DST correctness pays off. Everything we do here is in the user's local time zone, but `jiff` knows about DST so we don't have to think hard.

### 4.1 `Interval::bounds(&self, tz: &TimeZone) -> (Zoned, Zoned)`

Returns a half-open `[lo, hi)` pair:

- `Day(d)` → `[d at 00:00, d+1day at 00:00)`
- `Week(d)` → ISO week containing `d`, Monday → next Monday. Use a `match d.weekday()` to compute days back to Monday explicitly (some jiff versions have `to_monday_zero_offset`, but matching is portable).
- `Month(d)` → first of month → first of next month (mind the December-to-January wrap)
- `Year(y)` → Jan 1 of `y` → Jan 1 of `y+1`
- `Custom { from, to }` → `[from at 00:00, to+1day at 00:00)` — `to` is inclusive on the day; we make the upper bound exclusive by going to the next day.

To convert a `Date` to a `Zoned` at midnight: `date.at(0, 0, 0, 0).to_zoned(tz.clone())`. The DST magic happens here — if midnight doesn't exist on a "spring forward" day in some TZ, `jiff` resolves it correctly per its policy.

Tests: pick a few representative cases (today, an ISO week wrap, February for month, leap year for year, an arbitrary custom range) and assert the year/month/day of `lo` and `hi`. Don't bother asserting hours — they're always 0.

### 4.2 `contains`, `previous`, `next`, `label`

```rust
pub fn contains(&self, t: &jiff::Zoned, tz: &TimeZone) -> bool {
    let (lo, hi) = self.bounds(tz);
    t >= &lo && t < &hi
}
```

`previous` and `next` shift by one unit of the variant — a day, a week (7 days), a month (mind December/January), a year, or the length of the custom range. Beware of two subtle traps:

- **Month arithmetic isn't days.** `Month(2026-01)` → `previous()` is `Month(2025-12)`, not "30 days earlier". Always go through `(year, month)` integers and reconstruct the date.
- **Custom range length is `to - from + 1` days.** `from = 2024-04-18`, `to = 2024-04-20` is 3 days, so `previous()` shifts the range by 3 days. Use `to.since(*from).unwrap().get_days() + 1`.

`label()` returns a human string for the header bar — `04.05.2026`, `CW 19 · 04.05–10.05.2026`, `May 2026`, `2026`, or `Custom · 18.04.2024 – 16.07.2024`. Pick a date, write the expected string, then make it pass.

### 4.3 The aggregation primitive

Create `src/time/intersect.rs`. The whole module is one function:

```rust
pub fn session_overlap_seconds(
    session: &Session,
    interval: &Interval,
    now: &jiff::Zoned,
    tz: &jiff::tz::TimeZone,
) -> i64 {
    let s = &session.start;
    let e = session.stop.as_ref().unwrap_or(now);
    if e <= s { return 0; }
    let (lo, hi) = interval.bounds(tz);
    let start = if s > &lo { s.clone() } else { lo };
    let end   = if e < &hi { e.clone() } else { hi };
    if end <= start { return 0; }
    end.timestamp().as_second() - start.timestamp().as_second()
}
```

That's the whole aggregation rule from the spec, in eight lines: clip `[start, end)` to `[lo, hi)` and return the length in seconds. Open sessions use `now` as their end. Two reasons it's important to use seconds (not Span):

1. We're going to sum many of these — `i64` addition is trivial; `Span` arithmetic is not.
2. The TUI displays `HH:MM:SS`, which is just integer math from seconds.

The DST test is the most interesting: pick `2026-10-25` in `Europe/Berlin` (DST end), record a session from local midnight to local next-midnight, and assert the result is `25 * 3600`. If your bounds are right and your seconds-extraction is right, this just works.

### 4.4 `project_total_in`

Trivial fold: for each session, add its `session_overlap_seconds`. Lives in `src/time/aggregate.rs`. Test that closed sessions sum correctly and out-of-interval sessions contribute zero.

Module structure:

```rust
// src/time/mod.rs
pub mod aggregate;
pub mod intersect;

pub use aggregate::project_total_in;
pub use intersect::session_overlap_seconds;
```

Commit:

> `feat: add time math (bounds, intersect, aggregate)`

---

## Chapter 5 — Storage

Storage is where the architecture earns its keep. We define a `ProjectStore` trait and provide two implementations: `MemStore` (fast, used in tests) and `FsStore` (real filesystem, used in production). Every higher-level operation goes through this trait, which is why the rest of the codebase is so easy to test.

### 5.1 The trait

```rust
pub trait ProjectStore: Send + Sync {
    fn list(&self) -> Result<Vec<ProjectName>>;
    fn load(&self, name: &ProjectName) -> Result<Project>;
    fn create(&self, name: &ProjectName) -> Result<()>;
    fn delete(&self, name: &ProjectName) -> Result<()>;
    fn append_start(&self, name: &ProjectName, start: &Zoned, note: Option<&str>) -> Result<()>;
    fn close_open(&self, name: &ProjectName, stop: &Zoned) -> Result<()>;
}
```

Six methods. `append_start` and `close_open` are the only mutators that touch the running-session invariant; we centralize that logic so the upper layers don't have to think about it.

### 5.2 `MemStore`

Lives behind `#[cfg(any(test, feature = "test-helpers"))]` (we only need it in tests). It's a `BTreeMap<String, Vec<Session>>` behind a `Mutex`. Implementing the trait is mechanical — write four small tests that cover the happy path (create → start → stop → load), the duplicate-create error, the already-running error, and the not-running error.

This pays for itself within a chapter: every `ops::*` test, every CLI handler test, every TUI dispatcher test will run against `MemStore` in microseconds.

### 5.3 The CSV codec

Create `src/storage/csv_io.rs` with two free functions:

```rust
pub fn read_project<R: Read>(name: &ProjectName, reader: R) -> Result<Project>;
pub fn write_project<W: Write>(project: &Project, writer: W) -> Result<()>;
```

The row struct is a serde-derived `{ start: String, stop: String, note: String }`. We could fight to make `start` a `Zoned` directly with `jiff`'s serde feature, but the empty-`stop` case is simpler with strings — we parse them ourselves and explicitly check ordering invariants on read.

The five things `read_project` validates as it parses:

1. Every `start` parses as RFC 3339.
2. Every non-empty `stop` parses, and `stop >= start`.
3. Sessions are sorted by `start`.
4. At most one open row, and it must be last.
5. CSV-level errors propagate as `Error::Csv` (the `?` operator does this automatically).

When any of (1–4) fails, we return `Error::Corrupt` with the project name and the offending CSV row number (header is row 1, so the first data row is row 2). This is the corruption-recovery story from the spec.

Tests: round-trip empty project, round-trip closed + open sessions, three corruption cases (out of order, open-not-last, invalid timestamp), and a "note with comma" case to verify CSV quoting works (the `csv` crate handles RFC 4180 for us).

### 5.4 `FsStore`

The real implementation. Three concerns it pulls together:

- **Atomic write.** `~/.zeitlupius/projects/<name>.csv.tmp.<pid>` → write CSV bytes → `fs::sync_all` → `fs::rename` over the original. `fs::rename` is atomic on POSIX and NTFS *within the same directory*. A crash mid-write leaves the original file intact.
- **Lockfile.** Every mutation grabs an exclusive lock on `~/.zeitlupius/.lock` via `fd_lock::RwLock::write()`. Reads don't lock — they tolerate seeing a stale-but-coherent file (because the codec only sees fully-written rows; partial rows don't exist on disk because of the rename).
- **Directory bootstrap.** On first call, we ensure `projects/` and `.lock` exist. The whole tree is created lazily.

The structure of every mutating method is the same: take the lock, load the file, mutate in memory, write atomically, drop the lock.

```rust
fn append_start(&self, name: &ProjectName, start: &jiff::Zoned, note: Option<&str>) -> Result<()> {
    self.with_lock(|| {
        let mut p = self.load_locked(name)?;
        if let Some(last) = p.sessions.last() {
            if last.is_running() {
                return Err(Error::AlreadyRunning(name.to_string(), last.start.to_string()));
            }
        }
        p.sessions.push(Session { start: start.clone(), stop: None, note: note.map(str::to_string) });
        self.save_locked(&p)
    })
}
```

Tests use `tempfile::TempDir` so the real filesystem code paths are exercised but no user data is touched. Cover create-then-list, list-ignores-non-csv, start-stop-persists, delete-missing-errors, duplicate-create-errors. Add a concurrency smoke test: spawn two threads that both try to `append_start` the same project — exactly one should succeed (the other gets `AlreadyRunning`); both must finish cleanly with no corrupt CSV.

> Pitfall: don't try to take `Vec<&Project>` from the store and mutate them in place. The store's API is read-only at the type level; mutations go through the dedicated methods. This is what keeps the locking simple.

Commit:

> `feat: add ProjectStore, MemStore, CSV codec, and FsStore`

---

## Chapter 6 — `ops` — the application service layer

`ops` is a thin, stateless module of free functions that take a `&S: ProjectStore` plus arguments and produce `Result<...>`. It's the contract that the CLI and TUI both call.

```rust
pub fn create<S: ProjectStore>(store: &S, name: &ProjectName) -> Result<()>;
pub fn delete<S: ProjectStore>(store: &S, name: &ProjectName) -> Result<()>;
pub fn list<S: ProjectStore>(store: &S) -> Result<Vec<ProjectName>>;
pub fn start<S: ProjectStore>(store: &S, name: &ProjectName, now: &Zoned, note: Option<&str>) -> Result<()>;
pub fn stop<S: ProjectStore>(store: &S, name: &ProjectName, now: &Zoned) -> Result<()>;
pub fn load<S: ProjectStore>(store: &S, name: &ProjectName) -> Result<Project>;

pub struct Report { pub interval: Interval, pub rows: Vec<Row>, pub total_seconds: i64 }
pub struct Row    { pub name: ProjectName, pub seconds: i64 }
pub fn report<S: ProjectStore>(store: &S, only: Option<&ProjectName>, interval: Interval,
                                now: &Zoned, tz: &TimeZone) -> Result<Report>;

pub struct StatusEntry { pub name: ProjectName, pub start: Zoned, pub running_seconds: i64 }
pub fn status<S: ProjectStore>(store: &S, only: Option<&ProjectName>, now: &Zoned)
    -> Result<Vec<StatusEntry>>;
```

Two reasons `ops` exists as a layer:

1. **It accepts `now: &Zoned`.** This makes time deterministic: tests pass a fixed `Zoned`; the binary passes `Zoned::now()`. No more "this test is flaky because it crossed a second boundary".
2. **It accepts `tz: &TimeZone`.** Same story — the binary passes `TimeZone::system()`; tests pass `TimeZone::UTC` or whatever they need.

Tests use `MemStore` and exercise: full lifecycle (create → start → stop → report sums correctly) and the `status` filter (only running projects appear).

Commit:

> `feat: add ops module wrapping the store with reports`

---

## Chapter 7 — The CLI

We're now done with the library core. The next two chapters are about putting front-ends on top of it.

### 7.1 `cli/format.rs` — display helpers

Two helpers: `fmt_hms(seconds) -> String` for `HH:MM:SS` (used in the live timer) and `fmt_hm(seconds) -> String` for `HH:MM` (used in reports). Saturate negatives at zero; pad widths so columns align.

### 7.2 `cli/mod.rs` — clap derive

Define a `Cli` parser with the structure from the spec:

```rust
#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    List,
    Create  { project: String },
    Delete  { project: String, #[arg(long)] force: bool },
    Start   { project: String, #[arg(long)] note: Option<String> },
    Stop    { project: String },
    Status  { project: Option<String> },
    Report(ReportArgs),
}
```

The trickiest part is `report`. Its interval flags (`--day`, `--week`, `--month`, `--year`) are mutually exclusive, but `--page` and `--json` are modifiers. clap's `#[group(multiple = false)]` handles the exclusivity. Use `#[arg(allow_negative_numbers = true)]` on `--page` so `--page -1` works. Use `requires =` on `--from`/`--to` so they always appear together.

Two tests catch most clap mistakes:

```rust
#[test]
fn debug_assert() { Cli::command().debug_assert(); }   // clap's own self-check

#[test]
fn parses_report_week_with_page() {
    let cli = Cli::try_parse_from(["zeitlupius", "report", "--week", "--page", "-1"]).unwrap();
    // ...
}
```

### 7.3 `cli/handlers.rs` — what each subcommand does

One function:

```rust
pub fn run<S: ProjectStore, W: Write>(
    store: &S,
    cmd: Command,
    out: &mut W,
    now: &Zoned,
    tz: &TimeZone,
) -> Result<()>
```

Branch on `cmd` and call into `ops::*`. Print human-readable results to `out` (we write to a `Write` so tests can capture output into a `Vec<u8>`). For `report --json`, hand-roll a JSON object — using `serde_json` for one writer would pull in an extra crate for one feature.

The `build_interval(args, page, now)` helper inside this module is responsible for translating the clap struct into an `Interval`:

```rust
fn build_interval(args: &IntervalArgs, page: i32, now: &Zoned) -> Result<Interval> {
    let today = now.date();
    if let (Some(f), Some(t)) = (args.from.as_deref(), args.to.as_deref()) {
        let from = parse_date(f)?;
        let to   = parse_date(t)?;
        if from > to { return Err(Error::InvalidRange { from: f.into(), to: t.into() }); }
        return Ok(Interval::Custom { from, to });
    }
    let mut iv = if      args.week  { Interval::Week(today)  }
                 else if args.month { Interval::Month(today) }
                 else if args.year  { Interval::Year(today.year()) }
                 else               { Interval::Day(today)   };
    for _ in 0..(page.unsigned_abs() as usize) {
        iv = if page < 0 { iv.previous() } else { iv.next() };
    }
    Ok(iv)
}
```

`parse_date` splits on `.` and expects three pieces, then calls `jiff::civil::Date::new(yyyy, mm, dd)`. If anything fails, return `Error::InvalidDate`.

Tests use `MemStore` and exercise: list-empty-outputs-nothing, create-then-list-shows-project, report-day-sums, parse_date-rejects-garbage.

### 7.4 `main.rs` — the binary entry

```rust
fn main() -> ExitCode {
    let cli = Cli::parse();
    let dir = data_dir(cli.data_dir);   // CLI flag > $ZEITLUPIUS_HOME > ~/.zeitlupius
    let store = FsStore::new(dir);
    if let Err(e) = store.ensure_dirs() { eprintln!("error: {e}"); return ExitCode::from(2); }

    let now = jiff::Zoned::now();
    let tz  = jiff::tz::TimeZone::system();

    let result = match cli.command {
        None => zeitlupius::tui::run(&store, &tz),
        Some(Command::Delete { project, force }) if !force => confirm_then_delete(&store, project, &now, &tz),
        Some(cmd) => run_cli(&store, cmd, &mut io::stdout(), &now, &tz),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            match e.kind() { ErrorKind::User => ExitCode::from(1), ErrorKind::Internal => ExitCode::from(2) }
        }
    }
}
```

Two notes:

- **Delete confirmation lives in `main`, not the handler.** This keeps the handler pure (no stdin) and easy to test. The handler always deletes; only the binary asks "y/N".
- **`zeitlupius::tui::run` is a stub for now.** Create `src/tui/mod.rs` with `pub fn run<S: ProjectStore>(_: &S, _: &TimeZone) -> Result<()> { eprintln!("(TUI not implemented yet)"); Ok(()) }`. We replace it in chapter 8.

Smoke-test:

```bash
cargo run -- --data-dir /tmp/zlp-smoke create demo
cargo run -- --data-dir /tmp/zlp-smoke list
cargo run -- --data-dir /tmp/zlp-smoke start demo
cargo run -- --data-dir /tmp/zlp-smoke status
cargo run -- --data-dir /tmp/zlp-smoke stop demo
cargo run -- --data-dir /tmp/zlp-smoke report --day
rm -rf /tmp/zlp-smoke
```

### 7.5 `tests/cli.rs` — integration tests

Use `assert_cmd::Command::cargo_bin("zeitlupius")` plus a fresh `TempDir` per test. Cover the create/list/delete cycle, start-then-status, missing-project gives exit 1, invalid date gives exit 1, `report --week` runs and prints "TOTAL".

Commit at the end of the chapter:

> `feat: implement CLI surface end-to-end`
> `test: cover the CLI with integration tests`

You can split that into two commits if you want — the spec recommends frequent commits.

---

## Chapter 8 — The TUI

The TUI has four files: `app.rs` (state), `input.rs` (key dispatch), `view.rs` (renderer), `modals.rs` (popups), and `mod.rs` (event loop). The split is intentional — each file has one job, and the dispatcher is unit-testable without spinning up a terminal.

### 8.1 `app.rs` — `AppState`

```rust
pub struct AppState {
    pub projects: Vec<ProjectName>,
    pub selected: usize,
    pub interval: Interval,
    pub modal: Modal,
    pub status_line: Option<(String, std::time::Instant)>,
    pub should_quit: bool,
}

pub enum Modal { None, NewProject { input: String }, ConfirmDelete { project: ProjectName, running: bool },
                 CustomInterval { from: String, to: String, focus_to: bool, error: Option<String> }, Help }
```

Tiny helpers: `move_up`/`move_down` that clamp at edges, `switch_interval(kind, today)` that resets the interval to today's anchor, `page_prev`/`page_next` that delegate to `Interval::previous`/`next`, `set_status(msg)` that stamps the current time so the renderer can fade it out, `ensure_selection_valid()` for after a delete.

Tests are pure state-in/state-out: simulate a sequence of method calls and assert the final state. No terminal, no async — fast and deterministic.

### 8.2 `input.rs` — the key dispatcher

```rust
pub enum Action {
    None,
    StartSelected, StopSelected, DeleteSelected, DeleteConfirmed,
    NewProject(String),
    ApplyCustomInterval(jiff::civil::Date, jiff::civil::Date),
    Reload,
}

pub fn dispatch(key: KeyEvent, state: &mut AppState, today: jiff::civil::Date) -> Action;
```

The `dispatch` function:

1. Cheap-clones `state.modal` (the type is small).
2. Branches on the modal kind: dashboard, NewProject, ConfirmDelete, CustomInterval, or Help.
3. For each case, mutates `state` in place and returns an `Action` describing any side effect the event loop needs to perform on storage.

The clone is a deliberate choice: pattern-matching on `&mut state.modal` and then mutating `state` inside the arms creates borrow-checker friction. Cloning a small enum is a worthwhile simplification.

Why return `Action` instead of doing the storage call inside `dispatch`? Because `dispatch` is unit-testable without a `ProjectStore`. The event loop maps `Action` → `ops::*` calls. Tests assert: `q` sets `should_quit`, `s` returns `StartSelected`, `n` opens NewProject and Enter returns `NewProject(input)`, `←` pages back.

Catch the Ctrl-C case before the unguarded `'c'` arm:

```rust
if matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL) {
    state.should_quit = true;
    return Action::None;
}
```

### 8.3 `view.rs` — Layout B

Layout B is a vertical split: a thin header (interval + page label), a horizontal split body (project list left, detail panel right), and a two-line footer (key hints on top, status line on bottom).

Use `ratatui::layout::Layout` with `Direction::Vertical` for the outer split and `Direction::Horizontal` for the body. Constraints:

- Header: `Length(1)`
- Body: `Min(0)` (takes everything left over)
- Footer: `Length(2)`

Inside the body:

- Left: `Percentage(40)` — project list with running marker `●` and a live `HH:MM:SS` for active projects, or a right-aligned `HH:MM` for closed.
- Right: `Percentage(60)` — selected project's totals across all four intervals + active session block + last 5 sessions.

Two specific ratatui details:

- `Frame::area()` returns the drawable rect (this replaced `size()` in newer versions).
- A `List` with selection uses a `ListState` — `let mut ls = ListState::default(); ls.select(Some(i)); f.render_stateful_widget(list, area, &mut ls);`.
- Modals draw on top by clearing a centered sub-rect (`Clear` widget) and rendering a `Paragraph` inside a bordered `Block`.

Tests use `ratatui::backend::TestBackend` to render a `Terminal` to memory and assert that no panic happened. We don't snapshot the cells — that's fragile and the spec says we don't need to.

### 8.4 `modals.rs`

One `pub fn draw(f: &mut Frame, area: Rect, modal: &Modal)` function with a match on `Modal`. NewProject shows the typed name and "Enter to create · Esc to cancel". ConfirmDelete adds a yellow-text warning when the project is currently running. CustomInterval has two text fields with a `> ` cursor on the focused one and a red error if the dates didn't parse. Help is a static cheat sheet.

`Centered<Rect>` is a small helper — split the rect vertically with three constraints, take the middle, then split horizontally the same way and take the middle. That's the conventional way to position a popup.

### 8.5 `mod.rs` — the event loop

```rust
pub fn run<S: ProjectStore>(store: &S, tz: &TimeZone) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(stdout))?;

    let result = event_loop(store, tz, &mut term);

    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;
    result
}
```

The wrapper enables and disables raw mode + the alternate screen unconditionally, even on error. (For production-grade panic safety you'd wrap this in a guard struct using `Drop`; for v1 the linear flow is fine because we don't have unwinding paths that bypass it.)

`event_loop` does:

1. Compute `now`. Fade out an old status line if it's older than 3 seconds.
2. Render with `term.draw(|f| view::draw(f, &DashboardData { ... }))`.
3. `event::poll(TICK)` for 250 ms — that's our redraw cadence.
4. If a key arrives, `dispatch` it and `handle_action` it.
5. Every 1 second, re-list and re-load projects so changes from another `zeitlupius` process are picked up.
6. Loop until `state.should_quit`.

`handle_action(store, action, state, now)` pattern-matches the `Action` enum and calls the matching `ops::*` function. It returns `Result<bool>` where `true` means "storage changed, reload now". Errors set the red status line via `state.set_status(format!("error: {e}"))` instead of crashing the TUI.

Smoke test it manually: `cargo run -- --data-dir /tmp/zlp-tui` then press `n`, type a name, Enter; press `s` to start, watch the timer tick; press `S` to stop; `D` and `y` to delete; `q` to quit. Then `rm -rf /tmp/zlp-tui`.

Commit:

> `feat: TUI with Layout B, modals, and event loop`

---

## Chapter 9 — Wrapping up

A short README at the project root explains how to install (`cargo install --path .`), run the TUI (`zeitlupius`), and the CLI surface (a few one-liners is enough — the CLI is self-documenting via `--help`). Point readers to `docs/superpowers/specs/` for the design.

Final check before declaring victory:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All green? Good. Now eyeball the spec one more time:

- `~/.zeitlupius/projects/<name>.csv` — `cat` one and verify the format and offset.
- Open row = running — start a project, `cat` the CSV, see the empty `stop`.
- `.lock` exists.
- All eight CLI subcommands work.
- Exit codes: `zeitlupius start nope; echo $?` should print `1`.
- TUI shows live timer, navigates intervals, opens modals.
- Two terminals: TUI in one, `zeitlupius start demo` in the other — change appears in TUI within ~1 second.
- DST: the 25-hour-day test passes.
- Corrupt: hand-edit a CSV to insert duplicate open rows; TUI shows the error; CLI exits with code 2.

If everything checks out, you've shipped.

---

## Where to find more depth

- The full design is at `docs/superpowers/specs/2026-05-04-zeitlupius-design.md` — error handling, edge cases, deferred features.
- Exact code for every chapter (every test, every line) is at `docs/superpowers/plans/2026-05-04-zeitlupius.md`. Start there if you'd rather follow a recipe than a narrative.
- `jiff` is well-documented at <https://docs.rs/jiff>; the `tz`, `civil`, and `Span` modules are the ones you'll touch.
- `ratatui` has a great book at <https://ratatui.rs>; the `Layout`, `Paragraph`, `List`, and `Block` types cover 90 % of what we use.

Good luck and happy tracking.
