# zeitlupius — Design Spec

- Date: 2026-05-04
- Status: approved (pending user review of this spec doc)
- Source requirements: `INSTRUCTIONS+REQUIREMENTS.md`

`zeitlupius` is a Rust time-tracking tool with a Ratatui terminal UI and a feature-equivalent CLI. Each project's sessions live in a per-project CSV file under `~/.zeitlupius/`. The CSV is the single source of truth — a row with an empty `stop` column denotes a currently running session. The TUI and CLI are two front-ends over the same library core.

## Goals & non-goals

**Goals (v1):**

- Create, list, and delete projects.
- Start and stop tracking from CLI or TUI; multiple projects may run concurrently.
- Aggregate tracked time over Day, Week (ISO), Month, Year, and custom intervals; pageable backward and forward.
- TUI dashboard (split list + detail panel) showing per-project totals and a live-ticking active session.
- CLI parity for every TUI feature.

**Non-goals (v1):**

- Editing or deleting individual past sessions (the user can edit CSVs directly if needed).
- Tags, hierarchies, billing rates, or external sync.
- Per-session logs / detailed history view in the TUI beyond the last 5 sessions.
- Auto-repair of corrupted CSV files; we surface a clear error and let the user fix manually.
- Snapshot testing of TUI rendering.

## Decisions (settled in brainstorming)

| Decision | Choice |
|---|---|
| Storage location | `~/.zeitlupius/` (no XDG complication) |
| Data format | One CSV per project, columns `start,stop,note` |
| Active timer representation | Open row in the CSV (`stop` empty) — single source of truth |
| Timestamp format | RFC 3339 with local UTC offset captured at write time |
| Cross-process refresh | TUI polls the data dir (~1 s) — also drives the running-timer tick |
| Project-name handling on `start` | Strict — error if missing |
| CLI command shape | Verb-first (`zeitlupius start <project>`) |
| Code organization | Library + binary split (idiomatic Rust pattern) |
| Date library | `jiff` (DST-aware civil/zoned types) |

## Storage layout

```text
~/.zeitlupius/
├── projects/
│   ├── rust-zlp.csv
│   ├── side-project.csv
│   └── admin.csv
└── .lock          # advisory lock file used during writes
```

- One CSV per project. Filename without `.csv` is the canonical project name.
- Listing projects = listing `*.csv` files under `projects/`, sorted alphabetically. Other files are ignored.
- Creating a project = writing an empty CSV with the header row.
- Deleting a project = deleting the CSV (after confirmation in the CLI/TUI). If the project has an open session, deletion still proceeds — the open session is lost; the user is warned in the confirmation prompt and `--force` skips the prompt.
- The data dir is auto-created on first run.
- The location can be overridden with the `ZEITLUPIUS_HOME` env var or the `--data-dir <path>` global CLI flag (the flag wins). This is required for tests and useful for non-standard setups.

### CSV schema

```text
start,stop,note
2026-05-04T09:12:34+02:00,2026-05-04T11:30:00+02:00,refactored auth module
2026-05-04T13:00:00+02:00,,
```

- Header is mandatory: `start,stop,note`.
- `start` always present.
- `stop` empty for the currently running session; at most one open row per file, and it must be the **last** row.
- `note` is optional free text. Quoting/escaping per RFC 4180 (the `csv` crate handles this).
- All timestamps RFC 3339 with the local UTC offset captured at write time.

### Project name rules

- Allowed: ASCII letters, digits, `-`, `_`, `.`. Length 1–64.
- Rejected: spaces, slashes, leading dots, reserved CLI verbs (`list`, `start`, `stop`, `status`, `create`, `delete`, `report`).
- Validation lives in `model::ProjectName::parse(&str) -> Result<Self, Error>`.

### Concurrency

- Any **mutation** (start/stop/create/delete) acquires an exclusive advisory lock on `~/.zeitlupius/.lock` via `fd-lock`. Reads do not lock.
- Each mutation reads the CSV, applies the change in memory, writes a temp file in the same directory (e.g. `rust-zlp.csv.tmp.<pid>`), then `fs::rename`s over the original. `fs::rename` is atomic within a directory on POSIX and NTFS.
- A crash mid-write leaves the original intact; a stale `.tmp.*` may remain (cleaned up opportunistically on the next write).

## Domain model

```rust
pub struct ProjectName(String);

pub struct Session {
    pub start: jiff::Zoned,
    pub stop:  Option<jiff::Zoned>,   // None = running
    pub note:  Option<String>,
}

pub struct Project {
    pub name: ProjectName,
    pub sessions: Vec<Session>,        // chronological; at most one running
}

pub enum Interval {
    Day(jiff::civil::Date),
    Week(jiff::civil::Date),                  // ISO week containing this date
    Month(jiff::civil::Date),
    Year(i16),
    Custom { from: jiff::civil::Date, to: jiff::civil::Date },
}
```

### Invariants (enforced by the library)

- A project file has at most one row with empty `stop`, and if it exists it's the last row.
- `start <= stop` for closed sessions.
- Sessions sorted chronologically by `start`. The loader validates ordering and returns `Error::Corrupt` if a row is out of order.
- `start` is set to "now" at the moment `start` is invoked; same for `stop`.

### Aggregation rule

For a session `[start, end)` (`end` is `stop`, or "now" for the open one), its contribution to an interval `I` is `intersect([start, end), bounds_of(I))`. Sessions spanning midnight contribute proportionally to each day. This makes Day/Week/Month/Year sums add up correctly and behaves correctly across DST transitions because all bounds are constructed in the user's local TZ via `jiff::Zoned`.

## Code architecture

```text
zeitlupius/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # public API of the library (re-exports)
│   ├── error.rs                  # Error enum (thiserror) + Result alias
│   ├── model.rs                  # ProjectName, Session, Project, Interval
│   ├── time/
│   │   ├── mod.rs                # interval math (contains/previous/next/label)
│   │   ├── intersect.rs          # session ∩ interval → Span
│   │   └── aggregate.rs          # totals per project per interval
│   ├── storage/
│   │   ├── mod.rs                # ProjectStore trait + FsStore impl
│   │   ├── csv_io.rs             # read/write a single project file
│   │   ├── lock.rs               # fd-lock wrapper for ~/.zeitlupius/.lock
│   │   └── atomic.rs             # write-temp-then-rename helper
│   ├── ops.rs                    # high-level commands: start, stop, list,
│   │                             # create, delete, report, status
│   ├── cli/
│   │   ├── mod.rs                # clap structs (#[derive(Parser)])
│   │   ├── format.rs             # human-readable rendering of times/durations
│   │   └── handlers.rs           # one fn per subcommand, all call into ops::
│   ├── tui/
│   │   ├── mod.rs                # event loop entry point
│   │   ├── app.rs                # AppState, navigation, focus
│   │   ├── view.rs               # ratatui render: split layout, list, details
│   │   ├── input.rs              # key dispatch
│   │   └── modals.rs             # custom-interval input, new-project, confirm-delete
│   └── main.rs                   # parse args; run TUI if no subcommand else dispatch
└── tests/
    ├── cli.rs                    # integration tests using assert_cmd + tempfile
    └── ops.rs                    # library-level integration tests with FsStore on tempdir
```

### Boundaries

- `model`, `time`, `storage`, `ops` are the **library core** (`lib.rs` re-exports). They depend on neither `clap`, `ratatui`, nor `crossterm`.
- `cli` and `tui` are front-ends that call into `ops::*`.
- `main.rs` is small (~30 lines): parse `clap::Command`, resolve `~/.zeitlupius/` (or `ZEITLUPIUS_HOME` / `--data-dir`), construct an `FsStore`, dispatch.

### `ProjectStore` trait

```rust
pub trait ProjectStore {
    fn list(&self) -> Result<Vec<ProjectName>>;
    fn load(&self, name: &ProjectName) -> Result<Project>;
    fn create(&self, name: &ProjectName) -> Result<()>;
    fn delete(&self, name: &ProjectName) -> Result<()>;
    fn append_start(&self, name: &ProjectName, start: &Zoned, note: Option<&str>) -> Result<()>;
    fn close_open(&self, name: &ProjectName, stop: &Zoned) -> Result<()>;
}
```

`FsStore` is the production impl. An in-memory `MemStore` lives behind `#[cfg(test)]` for fast unit tests.

## Library dependencies

| Crate | Purpose |
|---|---|
| `clap` (v4, `derive`) | CLI parsing |
| `ratatui` | TUI widgets and layout |
| `crossterm` | terminal backend for `ratatui`, key events |
| `jiff` | dates, times, time zones, ISO weeks, DST-correct arithmetic |
| `csv` | CSV read/write per RFC 4180 |
| `fd-lock` | advisory file lock on `.lock` |
| `thiserror` | library error enum |
| `anyhow` | binary-side error context |
| `serde` (+ derive) | (de)serializing CSV rows |

Test-only:

| Crate | Purpose |
|---|---|
| `tempfile` | sandboxed `~/.zeitlupius/` |
| `assert_cmd`, `predicates` | CLI integration tests |

Specific versions are pinned by the implementation plan; this spec deliberately leaves them open.

## CLI surface

```text
zeitlupius                                 # launches TUI (default)

# Project lifecycle
zeitlupius list                            # list projects, marks running ones
zeitlupius create <P>                      # create empty project
zeitlupius delete <P> [--force]            # confirm unless --force

# Tracking
zeitlupius start <P> [--note "<text>"]     # error if running
zeitlupius stop  <P>                       # error if not running
zeitlupius status [P]                      # running timer(s)

# Reporting
zeitlupius report [<P>] <interval-flags>
   Interval selection (mutually exclusive; default --day):
     --day | --week | --month | --year
     --from DD.MM.YYYY --to DD.MM.YYYY     # custom interval (inclusive)
   Modifiers:
     --page <N>                            # default 0; negative = past;
                                           # ignored when --from/--to is used
     --json                                # machine-readable

# Globals (apply to any subcommand)
   --data-dir <path>                       # overrides ~/.zeitlupius and $ZEITLUPIUS_HOME
```

### Output examples

```text
$ zeitlupius list
* rust-zlp        (running for 00:12:04)
  side-project
  admin

$ zeitlupius status
● rust-zlp     started 09:12:34   running 00:12:04

$ zeitlupius report --week
Week 19 · 04.05.2026 – 10.05.2026
  rust-zlp        12:08:00
  side-project     1:30:00
  admin            0:00:00
  ─────────       ────────
  TOTAL           13:38:00

$ zeitlupius report rust-zlp --month --page -1
Month · April 2026
  rust-zlp        47:21:00
```

### Exit codes

- `0` success
- `1` user error (project missing, already running, bad date, validation)
- `2` I/O or internal error (incl. `Corrupt`)

## TUI design (Layout B)

### App states

- `Dashboard` (default screen)
- `ConfirmDelete { project }` (modal)
- `NewProject { input }` (modal)
- `CustomInterval { from, to, focus }` (modal)
- `Quit`

### Dashboard layout

```text
┌─ zeitlupius ─ Day · Today · 04.05.2026 ────────────────────────────┐
│                            ┌──────────────────────────────────────┐│
│ Projects                   │  rust-zlp                            ││
│ ─────────                  │  ─────────────────────────────────   ││
│ > rust-zlp     ● 02:26:34  │  Day      02:26:34  (Today)          ││
│   side-project   00:45:00  │  Week     12:08:00  (CW 19)          ││
│   admin          00:00:00  │  Month    47:21:00  (May 2026)       ││
│                            │  Year    120:55:00  (2026)           ││
│                            │                                      ││
│                            │  Active session                      ││
│                            │   started 09:12:34  for 00:12:04     ││
│                            │                                      ││
│                            │  Recent sessions (last 5)            ││
│                            │   09:12 → running     00:12:04       ││
│                            │   yesterday 14:00 → 16:30  02:30:00  ││
│                            │   ...                                ││
│                            └──────────────────────────────────────┘│
├────────────────────────────────────────────────────────────────────┤
│ d/w/m/y interval · ←/→ page · c custom · j/k select                │
│ s start · S stop · n new · D delete · r reload · ? help · q quit   │
└────────────────────────────────────────────────────────────────────┘
```

### Behavior

- Left list: each project plus its total for the **current paged interval**. Running projects show a leading `●` and a live-ticking duration.
- Right detail: the **selected** project's totals across all four intervals, always anchored to "now" — paging the left does not change the right's anchors. Also shows the active session (if any) and the last 5 sessions.
- Header bar shows the current interval + page label, e.g.
  `Day · Today · 04.05.2026`,
  `Day · Yesterday · 03.05.2026`,
  `Week · CW 18 · 27.04 – 03.05.2026`,
  `Custom · 18.04.2024 – 16.07.2024`.
- Tick: 250 ms (smooth running-timer display).
- Disk re-read: ~1 s, and immediately after any TUI-initiated mutation.

### Keybindings

| Key | Action |
|---|---|
| `j`/`↓`, `k`/`↑` | move selection in project list |
| `d`, `w`, `m`, `y` | switch interval to Day/Week/Month/Year |
| `←`, `→` | page interval back/forward |
| `Home` / `g` | reset to current page (today / this week / etc.) |
| `c` | open custom-interval modal |
| `s` / `S` | start / stop the selected project |
| `n` | new-project modal |
| `D` | delete selected project (confirmation modal) |
| `r` | reload from disk |
| `?` | help overlay |
| `q` / `Esc` | quit (or close modal) |

### Modals

- **NewProject:** centered, single text field with project-name validation; Enter creates, Esc cancels.
- **ConfirmDelete:** centered, `"Delete <P>? All sessions will be erased."`. If the project is currently running, the prompt also notes `"This project is running — the open session will be lost."`. `y` confirms, `N`/Esc cancels.
- **CustomInterval:** two text fields (`from DD.MM.YYYY`, `to DD.MM.YYYY`); Tab toggles focus; Enter applies; Esc cancels. Header bar then shows `Custom · …`.

### Empty state

If `projects/` contains no projects, the dashboard shows a centered message: `"No projects yet — press n to create one."` Reporting in the CLI on a missing project errors as usual; `zeitlupius list` prints nothing and exits `0`.

## Time interval semantics

### Anchor + page model

- The dashboard holds an `Interval`, e.g. `Day(2026-05-04)`.
- `←` calls `previous()`; `→` calls `next()`. `next()` is allowed past today (zero totals, with a small `(future)` label appended in the header).
- `g` / `Home` resets to the "now" anchor for the current interval.
- Switching interval (`d`/`w`/`m`/`y`) resets to the "now" anchor.

### Boundaries (in user's local TZ)

- **Day:** `[date 00:00, date+1day 00:00)`.
- **Week:** ISO 8601 week (Monday → Sunday). `Week(d)` is the ISO week containing `d`. Header shows `CW <N> · DD.MM – DD.MM.YYYY`.
- **Month:** `[first-of-month 00:00, first-of-next-month 00:00)`. Header shows `<MonthName> YYYY`.
- **Year:** `[Jan 1 00:00, next Jan 1 00:00)`. Header shows `YYYY`.
- **Custom:** `[from 00:00, to+1day 00:00)` — `to` inclusive on the day.

### DST safety

Because `jiff::Zoned` arithmetic is DST-aware and `Interval::contains()` compares zoned timestamps to civil-date-derived bounds in the local TZ, a 25-hour day really sums to 25 hours.

### Custom interval validation

- Both dates parse as `DD.MM.YYYY`.
- `from <= to`.
- Helpful error if a date is invalid (`"31.02.2026 is not a valid date"`).

## Error handling

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
```

### User-facing surface

- **CLI:** prints `error: <Display>` to stderr; exit `1` for user errors, `2` for I/O / `Corrupt`. `anyhow::Context` is added at the binary boundary for breadcrumbs.
- **TUI:** errors raised by `ops::*` are caught and shown in a transient red status line at the bottom for ~3 seconds; the TUI does not crash on a recoverable error.

### Corruption recovery

`load` returns `Error::Corrupt` (with project name and offending line numbers) when any of:

- More than one row has an empty `stop`.
- An open row exists but isn't the last row.
- Rows are not sorted chronologically by `start`.
- A row has `stop < start`.
- A timestamp fails RFC 3339 parse.

The TUI shows the error; the user fixes the CSV manually. Auto-repair is deliberately deferred.

### Crash mid-write

The write-temp-then-rename strategy guarantees the original file is intact until the rename. The lockfile prevents two simultaneous writers from racing.

## Testing strategy

We follow TDD: failing test first, then the implementation.

### Three test layers

1. **Unit tests** (`#[cfg(test)] mod tests` in each file):
   - `model::*`: name validation, `Session::duration_now`, invariants.
   - `time::intersect`: closed-interval ∩ session — entirely inside, partial start, partial end, crossing midnight, crossing DST.
   - `time::aggregate`: totals across multiple sessions and projects.
   - `time::interval`: `previous()`/`next()` for Day/Week/Month/Year incl. month-length and leap-year edges.

2. **Library integration tests** (`tests/ops.rs`):
   - Use `tempfile::TempDir` as a fake `~/.zeitlupius/`.
   - Full flows: create → start → stop → list → report → delete.
   - Concurrency smoke test: simultaneous `start`/`stop` across threads succeed via the lockfile.
   - Corruption test: hand-craft a bad CSV, assert `Error::Corrupt`.

3. **CLI integration tests** (`tests/cli.rs`):
   - `assert_cmd` + `predicates`.
   - `ZEITLUPIUS_HOME` (or `--data-dir`) points at a tempdir.
   - Cover every subcommand happy path; expected user errors and exit codes; date parsing.

### TUI testing

- `App::handle_key(&mut self, key)` is unit-testable: state in, state out. Tests cover navigation, modal lifecycle, and that `s`/`S` produce the right `ops::*` calls (verified via `MemStore`).
- We do **not** snapshot-test rendering. A smoke test confirms `view::draw(...)` doesn't panic on representative `AppState`s.

### Test data dir override

- `ZEITLUPIUS_HOME` env var.
- `--data-dir <path>` global flag (also useful for users with non-standard setups).

### Coverage target

All `ops::*` paths exercised, all `time::*` edge cases covered, every `Error` variant produced at least once.

## Out of scope (deferred)

- Editing or deleting individual past sessions through the UI (manual CSV editing remains the workaround).
- Auto-repair of corrupted CSVs.
- Tagging, project hierarchies, billing rates.
- Snapshot/visual TUI tests.
- Configurable week-start (locked to ISO Monday).
- Configurable storage location beyond `ZEITLUPIUS_HOME` / `--data-dir`.
- Notifications, idle detection, calendar sync.
