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
