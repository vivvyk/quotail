//! Help overlay: a fixed 76-col box (`HELP_WIDTH`) centered horizontally at
//! x=10, drawn over the current view. Content is static — it mirrors the keymap
//! in `docs/ASCII_REFERENCE.md`, which the snapshot test asserts against.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::layout::{HELP_HEIGHT, HELP_WIDTH};
use super::theme::Theme;

/// The 25 box rows (each `HELP_WIDTH` = 76 chars). Drawn as-is; the snapshot test
/// pins them to the reference so any drift fails loudly.
const BOX: [&str; HELP_HEIGHT as usize] = [
    "┌─ help ─ ? or esc to close ───────────────────────────────────────────────┐",
    "│                                                                          │",
    "│  navigation                       chart grid                             │",
    "│  d         open detail            enter     chart selected               │",
    "│  esc       back to overview       tab       cycle pane focus             │",
    "│  /         search ticker          S-1..4    focus pane n                 │",
    "│  :         command mode           c         clear focused pane           │",
    "│  ?         toggle this help       C         clear all panes              │",
    "│  q         quit                   h / l     table <-> grid               │",
    "│                                                                          │",
    "│  watchlist                        timeframes                             │",
    "│  j / k     move selection         1  2  3   1D    5D    1M               │",
    "│  g / G     jump top / bottom      4  5      6M    YTD                    │",
    "│  f         cycle filter           6  7      1Y    MAX                    │",
    "│  enter     chart this ticker                                             │",
    "│  s / S     sort / reverse                                                │",
    "│  x         remove symbol                                                 │",
    "│  r         refresh data                                                  │",
    "│                                                                          │",
    "│  commands                                                                │",
    "│  :add <SYM>      add to watchlist   :tf <RANGE>      set timeframe       │",
    "│  :rm <SYM>       remove symbol      :export [path]   write JSON          │",
    "│  :detail <SYM>   open drilldown     :settings        open settings       │",
    "│                                                                          │",
    "└──────────────────────────────────────────────────────────────────────────┘",
];

/// Section headings inside the box — colored in heading purple wherever they appear.
const HEADINGS: [&str; 5] = [
    "navigation",
    "chart grid",
    "watchlist",
    "timeframes",
    "commands",
];

pub fn render(frame: &mut Frame, theme: Theme) {
    let area = frame.area();
    // Center in whatever size we're given. At 96x31 this is x=10, y=3 (the
    // reference); at 200 cols it tracks the middle instead of pinning left.
    let x = area.x + area.width.saturating_sub(HELP_WIDTH) / 2;
    let y = area.y + area.height.saturating_sub(HELP_HEIGHT) / 2;
    let border = Style::default().fg(theme.border_focus);
    let muted = Style::default().fg(theme.muted);
    let accent = Style::default().fg(theme.accent);
    let last = BOX.len() - 1;
    let buf = frame.buffer_mut();

    let heading = Style::default().fg(theme.heading);
    for (i, line) in BOX.iter().enumerate() {
        let row = y + i as u16;
        if Rect::new(x, row, HELP_WIDTH, 1).intersection(area).height != 1 {
            continue;
        }
        // Frame rows in border color; body text muted (descriptions), with the side
        // borders repainted so the │ stay border-colored.
        if i == 0 || i == last {
            buf.set_string(x, row, line, border);
            continue;
        }
        buf.set_string(x, row, line, muted);
        buf.set_string(x, row, "│", border);
        buf.set_string(x + HELP_WIDTH - 1, row, "│", border);

        // Recolor each `key  description` span. Spans alternate key, description,
        // key, … so even indices are keys → accent. A span whose FULL text is a
        // section heading gets heading purple instead (matched exactly, so a
        // description like "add to watchlist" never trips the "watchlist" heading).
        let chars: Vec<char> = line.chars().collect();
        for (idx, (col, len)) in key_spans(&chars).into_iter().enumerate() {
            let text: String = chars[col..col + len].iter().collect();
            let style = if HEADINGS.contains(&text.as_str()) {
                heading
            } else if idx % 2 == 0 {
                accent
            } else {
                continue; // descriptions keep the muted base
            };
            buf.set_string(x + col as u16, row, text, style);
        }
    }

    // Title: "help" accent, "? or esc to close" muted.
    if Rect::new(x, y, HELP_WIDTH, 1).intersection(area).height == 1 {
        buf.set_string(x + 3, y, "help", accent);
        buf.set_string(x + 10, y, "? or esc to close", muted);
    }
}

/// Non-space runs in a content line, starting after the `│  ` prefix (col 3). A
/// single space stays inside a run (so `j / k`, `open detail` are one span each);
/// two or more spaces separate runs. Spans alternate key, description, key, … so
/// the caller colors the even-indexed ones (the keys).
fn key_spans(chars: &[char]) -> Vec<(usize, usize)> {
    let n = chars.len();
    let mut spans = Vec::new();
    let mut i = 3;
    while i < n {
        while i < n && chars[i] == ' ' {
            i += 1;
        }
        if i >= n || chars[i] == '│' {
            break;
        }
        let start = i;
        while i < n && chars[i] != '│' {
            if chars[i] == ' ' && chars.get(i + 1) == Some(&' ') {
                break; // two spaces end the run
            }
            i += 1;
        }
        spans.push((start, i - start));
    }
    spans
}
