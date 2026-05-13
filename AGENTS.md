# AGENTS.md

Single-crate Rust project (lib + bin) — terminal time tracker with a Ratatui TUI and feature-equivalent CLI.

## Commands

```bash
cargo build
cargo run                          # TUI
cargo run -- <subcommand>          # CLI (create/start/stop/list/status/delete/report)
cargo test                         # all tests
cargo test --test cli              # CLI integration suite only
cargo clippy --all-targets
cargo fmt
```

CLI and TUI honor `--data-dir <path>` / `ZEITLUPIUS_HOME`; default is `~/.zeitlupius/`.

## Structure

```txt
src/
  main.rs        - thin entry: resolvers + dispatch
  lib.rs          - re-exports
  cli/            - clap parser + handlers + JSON/text formatting
  tui/            - ratatui + crossterm + modal stack + event loop
  storage/        - ProjectStore trait; FsStore (CSV on disk) + MemStore (test)
  ops.rs          - orchestration (create/start/stop/report) generic over S: ProjectStore
  time/           - interval math (aggregate, intersect)
  model.rs        - ProjectName (validated), Session, Project, Interval
  error.rs        - Error enum with ErrorKind::User (exit code 1) vs Internal (exit code 2)
tests/cli.rs      - assert_cmd + TempDir integration suite
```

Direction: `model` → `time` → `storage` → `ops` → `cli|tui`. No cross-wiring between front-ends.

## Gotchas

- `main.rs` exit codes: `ErrorKind::User` → exit 1, `ErrorKind::Internal` → exit 2. Do not change.
- `Delete` subcommand has a non-interactive prompt in `main`; handlers themselves never prompt. Pass `force: true` to skip.
- `ProjectName::parse` rejects: empties, >64 chars, leading `.`, reserved CLI verbs (`list`, `start`, `stop`, `status`, `create`, `delete`), and any non-`[A-Za-z0-9._-]` characters. Never bypass.
- Sessions are CSV rows in `<data_dir>/projects/<name>.csv` — one file per project.
- Dates parsed from CLI use `DD.MM.YYYY`. Reports intersect sessions in the given `TimeZone` (defaults to system).
- `time::aggregate` splits open sessions at `now` automatically.
- `ops::` functions are generic over `ProjectStore` — test against `MemStore`; integration cases go in `tests/cli.rs`.

## Existing instructions

Deeper architecture and conventions are in `CLAUDE.md`.
