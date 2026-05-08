# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

Build / run / test:

    cargo build
    cargo run -- <subcommand>          # CLI
    cargo run                          # TUI (no subcommand)
    cargo test                         # unit + integration tests
    cargo test --test cli              # only the CLI integration suite (tests/cli.rs)
    cargo test <name>                  # filter by test name substring
    cargo clippy --all-targets
    cargo fmt

The TUI and CLI both honor `--data-dir <path>` (CLI flag) and `ZEITLUPIUS_HOME` (env). When unset the data dir is `~/.zeitlupius/`. Integration tests use `tempfile` + `--data-dir` to avoid touching the real home.

## Architecture

Single crate exposing both a `lib` (`src/lib.rs`) and a `bin` (`src/main.rs`). Two front-ends — CLI (`src/cli/`) and TUI (`src/tui/`) — sit on top of one shared core. They never talk to the filesystem directly; both go through `ops` → `ProjectStore`.

Layered, with strict direction of dependencies:

- `model` — `ProjectName` (validated; rejects reserved CLI verbs), `Session`, `Project`, `Interval`. Pure types, no I/O.
- `time` — pure interval math (`aggregate`, `intersect`). Computes per-project totals across a half-open interval, splitting open sessions at `now`.
- `storage` — `ProjectStore` trait; `FsStore` (CSV-on-disk, fd-locked) is the production impl, `mem::MemStore` is a test impl behind `#[cfg(test)]`. One CSV per project at `<data_dir>/projects/<name>.csv`. `csv_io` handles parsing/writing; `fs_store` enforces invariants (open-row uniqueness, atomic close).
- `ops` — thin orchestration (`create`, `start`, `stop`, `report`, …) generic over `S: ProjectStore`. This is the seam tests target.
- `cli` — `clap` parser (`mod.rs`) + `handlers::run` dispatch + human/JSON formatting (`format.rs`). Writes to a generic `&mut impl Write` so it's testable.
- `tui` — `ratatui` + `crossterm`. `app::AppState` holds UI state + modals; `input::dispatch` maps key events to `Action`s; `view::draw` renders; `mod.rs` owns the event loop (250 ms tick, 1 s reload, 3 s status TTL).

`main.rs` is a thin shell: resolves data dir, constructs `FsStore`, builds `now: Zoned` + `tz: TimeZone`, and dispatches to either `tui::run` or `cli::handlers::run`. The `Delete` subcommand has an interactive `[y/N]` prompt in `main` that re-invokes the handler with `force: true` after confirmation — handlers themselves are non-interactive.

Time handling uses `jiff` end-to-end (`Zoned`, `TimeZone`, `Span`). Dates parsed from the CLI use `DD.MM.YYYY`. Reports are timezone-aware: `time::aggregate` intersects each session with the requested interval in the given `TimeZone`.

Errors flow through one `Error` enum (`src/error.rs`) classified into `ErrorKind::User` vs `ErrorKind::Internal` — `main` uses this to set the process exit code. When adding error variants, classify them in `Error::kind()`.

## Conventions

- `ProjectName::parse` is the only way to construct a project name; it rejects empties, names > 64 chars, names starting with `.`, names that collide with CLI verbs (`list`, `start`, `stop`, `status`, `create`, `delete`, `report`), and any non-`[A-Za-z0-9._-]` chars. Reuse it; do not bypass.
- New ops belong in `src/ops.rs` and stay generic over `S: ProjectStore` so they're exercised against `MemStore` in unit tests and `FsStore` in `tests/cli.rs`.
- CLI handler additions: extend `Command` in `src/cli/mod.rs`, dispatch in `handlers::run`, format in `cli::format`, and add an integration case in `tests/cli.rs` (which uses `assert_cmd` with a tempdir).
- TUI changes: keep input → `Action` → state mutation separated. Modals live in `tui::modals` and are owned by `AppState::modal`.
