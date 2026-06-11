# Show Session Notes in the TUI — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render each session's `note` in the TUI Sessions panel (as a dimmed second line) and Detail panel (inline in the recent-sessions list and the active-session block).

**Architecture:** Rendering-only change confined to `src/tui/view.rs`. Two private helpers (`note_text`, `truncate_ellipsis`) are added and unit-tested, then wired into `draw_sessions_panel` (multi-line `ListItem`) and `draw_detail` (inline dim spans). No changes to `model`, `storage`, `ops`, or `cli`.

**Tech Stack:** Rust, `ratatui` (List/ListItem, Line/Span, Style, Color::DarkGray), `jiff`. Tests use `ratatui::backend::TestBackend`.

**Spec:** `docs/superpowers/specs/2026-06-11-tui-session-notes-display-design.md`

---

## File Structure

- Modify: `src/tui/view.rs`
  - Add two private fns near the top (after `right_aligned_row`): `note_text`, `truncate_ellipsis`.
  - Modify `draw_sessions_panel` (currently lines 182–228): append a dimmed note line to each `ListItem` when the session has a note.
  - Modify `draw_detail` (currently lines 230–297): append the note inline in the active-session block and in the recent-sessions loop.
  - Add tests in the existing `#[cfg(test)] mod tests` block.

All session literals in tests already set `note: <…>` (the field exists), so existing tests are unaffected.

---

### Task 1: Note helpers (`note_text`, `truncate_ellipsis`)

**Files:**
- Modify: `src/tui/view.rs` (add fns after `right_aligned_row`, ~line 68; add tests in the test module)

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `src/tui/view.rs`:

```rust
    #[test]
    fn note_text_filters_empty_and_whitespace() {
        assert_eq!(note_text(&None), None);
        assert_eq!(note_text(&Some(String::new())), None);
        assert_eq!(note_text(&Some("   ".to_string())), None);
        assert_eq!(note_text(&Some("hi".to_string())), Some("hi"));
    }

    #[test]
    fn truncate_ellipsis_leaves_short_text_unchanged() {
        assert_eq!(truncate_ellipsis("abc", 5), "abc");
        assert_eq!(truncate_ellipsis("abc", 3), "abc");
    }

    #[test]
    fn truncate_ellipsis_truncates_with_marker() {
        let out = truncate_ellipsis("abcdef", 4);
        assert_eq!(out.chars().count(), 4);
        assert_eq!(out, "abc…");
    }

    #[test]
    fn truncate_ellipsis_zero_width_is_empty() {
        assert_eq!(truncate_ellipsis("abc", 0), "");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib tui::view::tests::note_text_filters_empty_and_whitespace tui::view::tests::truncate_ellipsis`
Expected: FAIL — `cannot find function note_text` / `truncate_ellipsis in this scope`.

- [ ] **Step 3: Add the helper functions**

Insert after the closing `}` of `right_aligned_row` (around line 68) in `src/tui/view.rs`:

```rust
/// Returns the note text only when present and not blank.
fn note_text(note: &Option<String>) -> Option<&str> {
    note.as_deref().filter(|s| !s.trim().is_empty())
}

/// Truncate `s` to at most `max` columns, replacing the tail with `…` on overflow.
fn truncate_ellipsis(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let head: String = s.chars().take(max - 1).collect();
    format!("{head}…")
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib tui::view::tests::note_text_filters_empty_and_whitespace tui::view::tests::truncate_ellipsis`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/tui/view.rs
git commit -m "feat(tui): add note_text and truncate_ellipsis helpers

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Sessions panel — dimmed note line per session

**Files:**
- Modify: `src/tui/view.rs` `draw_sessions_panel` (currently lines 182–228)
- Test: same file, `mod tests`

- [ ] **Step 1: Write the failing tests**

Add a right-half buffer helper near `buffer_left_half` in the test module (it is reused in Task 3):

```rust
    fn buffer_right_half(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let mid = buf.area.width / 2;
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in mid..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }
```

Add the tests:

```rust
    #[test]
    fn sessions_panel_shows_note_on_second_line() {
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).unwrap();
        let projects = vec![Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![Session {
                id: SessionId::generate(),
                start: z(9),
                stop: Some(z(10)),
                note: Some("fixed the parser bug".to_string()),
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
        let text = buffer_left_half(&term);
        assert!(text.contains('↳'), "note marker missing: {text}");
        assert!(
            text.contains("fixed the parser bug"),
            "note text missing from sessions panel: {text}"
        );
    }

    #[test]
    fn sessions_panel_no_note_has_no_marker() {
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).unwrap();
        let projects = vec![Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![Session {
                id: SessionId::generate(),
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
        let text = buffer_left_half(&term);
        assert!(
            !text.contains('↳'),
            "note marker should be absent when note is None: {text}"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib tui::view::tests::sessions_panel_shows_note_on_second_line`
Expected: FAIL — assertion `note marker missing` (panel renders only the single row today).

- [ ] **Step 3: Modify `draw_sessions_panel` to append the note line**

In `src/tui/view.rs`, replace the `.map(|s| { … })` body in `draw_sessions_panel`. The current body ends by building a single-line `ListItem` via `right_aligned_row`. Change it so the row is captured, then a note line is conditionally pushed:

```rust
    let items: Vec<ListItem> = sessions
        .iter()
        .map(|s| {
            let stop = s
                .stop
                .as_ref()
                .map(|z| z.strftime("%d.%m.%Y %H:%M:%S").to_string())
                .unwrap_or_default();
            let secs = match &s.stop {
                Some(stop) => stop.timestamp().as_second() - s.start.timestamp().as_second(),
                None => 0,
            };
            let left_text = format!(
                "{}  {} → {}",
                s.id,
                s.start.strftime("%d.%m.%Y %H:%M:%S"),
                stop
            );
            let row = right_aligned_row(
                vec![(left_text, Style::default())],
                (fmt_hms(secs), Style::default()),
                target,
            );
            let mut lines = vec![row];
            if let Some(note) = note_text(&s.note) {
                // 10 spaces aligns the marker under the timestamps (8-char id + 2 spaces).
                let indent = "          ↳ ";
                let avail = target.saturating_sub(indent.chars().count());
                let body = truncate_ellipsis(note, avail);
                lines.push(Line::from(Span::styled(
                    format!("{indent}{body}"),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            ListItem::new(lines)
        })
        .collect();
```

(`Color`, `Line`, `Span`, `Style` are already imported at the top of the file.)

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib tui::view::tests::sessions_panel`
Expected: PASS — including the existing `sessions_panel_*` tests (they use `note: None`, so no second line is added).

- [ ] **Step 5: Commit**

```bash
git add src/tui/view.rs
git commit -m "feat(tui): show session note as dim second line in Sessions panel

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Detail panel — inline note in recent-sessions list and active block

**Files:**
- Modify: `src/tui/view.rs` `draw_detail` (currently lines 230–297)
- Test: same file, `mod tests` (reuses `buffer_right_half` from Task 2)

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn detail_recent_list_shows_note_inline() {
        // Wide backend so the long detail row (timestamps + duration + note)
        // is not clipped before the note, and lands in the right half.
        let backend = TestBackend::new(200, 30);
        let mut term = Terminal::new(backend).unwrap();
        let projects = vec![Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![Session {
                id: SessionId::generate(),
                start: z(9),
                stop: Some(z(10)),
                note: Some("refactored intersect".to_string()),
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
        let text = buffer_right_half(&term);
        assert!(
            text.contains("refactored intersect"),
            "recent-session note missing from detail panel: {text}"
        );
    }

    #[test]
    fn detail_active_session_shows_note_inline() {
        let backend = TestBackend::new(200, 30);
        let mut term = Terminal::new(backend).unwrap();
        let projects = vec![Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![Session {
                id: SessionId::generate(),
                start: z(11),
                stop: None,
                note: Some("planning the release".to_string()),
            }],
        }];
        let names = vec![ProjectName::parse("p").unwrap()];
        let state = AppState::new(date(2026, 5, 4), names);
        let now = z(12);
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
        // The running session does not appear in the Sessions panel, so a full-buffer
        // match is unambiguous here.
        let buf = term.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                text.push_str(buf[(x, y)].symbol());
            }
            text.push('\n');
        }
        assert!(
            text.contains("planning the release"),
            "active-session note missing from detail panel: {text}"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib tui::view::tests::detail_recent_list_shows_note_inline tui::view::tests::detail_active_session_shows_note_inline`
Expected: FAIL — note strings absent from the detail panel.

- [ ] **Step 3a: Append the note in the active-session block**

In `draw_detail`, the active-session block currently pushes a single formatted `Line`. Replace that block (the `if let Some(s) = p.running_session() { … }` body) with:

```rust
    // Active sessions
    if let Some(s) = p.running_session() {
        let r = (data.now.timestamp().as_second() - s.start.timestamp().as_second()).max(0);
        lines.push(Line::from(Span::styled(
            "Active session",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        let mut spans = vec![Span::raw(format!(
            " started {} for {}",
            s.start.strftime("%d.%m.%Y %H:%M:%S"),
            fmt_hms(r)
        ))];
        if let Some(note) = note_text(&s.note) {
            spans.push(Span::styled(
                format!("   {note}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        lines.push(Line::from(spans));
        lines.push(Line::from(""));
    }
```

- [ ] **Step 3b: Append the note in the recent-sessions loop**

Replace the `for s in p.sessions.iter().rev().take(5) { … }` body (which currently pushes one `Line::from(format!(…))`) with:

```rust
    // Last 5 sessions
    for s in p.sessions.iter().rev().take(5) {
        let stop = match &s.stop {
            Some(z) => z.strftime("%d.%m.%Y %H:%M:%S").to_string(),
            None => "running".to_string(),
        };
        let secs = match &s.stop {
            Some(stop) => stop.timestamp().as_second() - s.start.timestamp().as_second(),
            None => (data.now.timestamp().as_second() - s.start.timestamp().as_second()).max(0),
        };
        let mut spans = vec![Span::raw(format!(
            " {}  →  {}    {}",
            s.start.strftime("%d.%m.%Y %H:%M:%S"),
            stop,
            fmt_hms(secs)
        ))];
        if let Some(note) = note_text(&s.note) {
            spans.push(Span::styled(
                format!("   {note}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        lines.push(Line::from(spans));
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib tui::view::tests::detail_recent_list_shows_note_inline tui::view::tests::detail_active_session_shows_note_inline`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/view.rs
git commit -m "feat(tui): show session note inline in Detail panel

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Full verification (test + clippy + fmt)

**Files:** none (verification only)

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: PASS — all unit and integration tests green.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets`
Expected: no warnings.

- [ ] **Step 3: Run fmt**

Run: `cargo fmt`
Expected: no diff (or apply formatting). If it changed files, review and stage them.

- [ ] **Step 4: Commit any formatting changes**

```bash
git add -A
git commit -m "style: apply cargo fmt" || echo "nothing to format"
```

---

## Self-Review

- **Spec coverage:**
  - Sessions panel dim second line → Task 2. ✓
  - "Some and non-empty" gating → `note_text` (Task 1), used in Tasks 2 & 3. ✓
  - Truncation with `…` in narrow panel → `truncate_ellipsis` (Task 1), used in Task 2. ✓
  - Detail recent-list inline note → Task 3 Step 3b. ✓
  - Detail active-session inline note → Task 3 Step 3a. ✓
  - `Color::DarkGray` styling → Tasks 2 & 3. ✓
  - Empty/None renders nothing extra → `note_text` returns `None`; tested in Task 1 and Task 2 (`sessions_panel_no_note_has_no_marker`). ✓
  - Tests for all three locations → Tasks 2 & 3. ✓
- **Placeholder scan:** none — every code/test step contains full code.
- **Type consistency:** `note_text(&Option<String>) -> Option<&str>` and `truncate_ellipsis(&str, usize) -> String` are defined in Task 1 and called with matching signatures in Tasks 2–3. `Color`, `Line`, `Span`, `Style`, `Modifier`, `fmt_hms` are already imported in `view.rs`.
