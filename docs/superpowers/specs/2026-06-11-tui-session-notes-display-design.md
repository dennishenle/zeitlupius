# Show session notes in the TUI

**Date:** 2026-06-11
**Status:** Approved

## Goal

Display each session's note in the TUI. Notes already exist on the model
(`Session::note: Option<String>`), are persisted by storage, and are editable
via the CLI (`session set-note`) and the TUI (`e` while the Sessions panel is
focused). The only gap is that they are never *shown* in the dashboard. This
spec closes that gap.

## Scope

Rendering only. No changes to `model`, `storage`, `ops`, or `cli`. All changes
live in `src/tui/view.rs` plus its test module.

Out of scope: editing/creating notes (already implemented), wrapping/scrolling
long notes, and any new key bindings.

## Design

### 1. Sessions panel (bottom-left, ~40% width)

Today each session renders as a single-line `ListItem` produced by
`right_aligned_row`: `id  start → stop` on the left, duration right-aligned.

Change: when a session's note is `Some` and non-empty, the `ListItem` becomes
two lines — the existing row, followed by a dimmed note line indented to align
under the timestamps, prefixed with `↳`, and truncated with `…` to the panel's
inner width. Sessions with no note (or an empty note) stay single-line.

```
abcdef23  04.05.2026 09:00:00 → 10:00:00   1:00:00
          ↳ fixed the parser bug
ghi23klm  04.05.2026 11:00:00 → 12:30:00   1:30:00
```

Selection and navigation are unaffected: there is still exactly one `ListItem`
per session, so `session_selected` indexing is unchanged. The `REVERSED`
highlight naturally covers both lines of the selected item.

### 2. Detail panel — "Recent sessions (last 5)" (right, ~60% width)

The Detail panel is wider, so the note is appended inline after the duration on
the same line when present. No extra line is added. Sessions without a note are
unchanged. The Detail `Paragraph` has no wrap today, so an over-long note clips
at the panel edge — acceptable and consistent with current behavior.

```
 04.05.2026 09:00:00  →  10:00:00    1:00:00   fixed the parser bug
```

### 3. Detail panel — "Active session" block

When the running session has a note, append it inline after the existing
`started <start> for <dur>` line.

```
Active session
 started 04.05.2026 09:00:00 for 1:00:00   planning the release
```

### Styling

Notes render with a `Color::DarkGray` foreground for a dimmed look. The codebase
already uses explicit `Color`s (e.g. `Color::Green` for accents), so this matches
existing style and is reliable across terminals. Empty or `None` notes render
nothing extra in all three locations.

### Truncation

A small helper truncates a note string to a target column width, appending `…`
when it overflows. Used for the Sessions-panel note line. The Detail panel relies
on the existing clip-at-edge behavior and does not truncate explicitly.

## Testing

TDD. New tests in the `src/tui/view.rs` test module, using sessions with
`note: Some(...)`:

1. Sessions panel: the note text appears in the rendered left-half buffer, and a
   `↳` marker is present.
2. A session with `note: None` adds no `↳` line in the Sessions panel.
3. Detail panel: the note text of a recent (finished) session appears in the
   right-half buffer.
4. Active-session block: the running session's note appears in the right-half
   buffer.

Existing tests keep `note: None` and must stay green. Verify with
`cargo test`, `cargo clippy --all-targets`, and `cargo fmt`.
