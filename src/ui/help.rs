//! Help overlay: a fixed 76-col box (`HELP_WIDTH`) centered horizontally at
//! x=10, drawn over the current view. Content is static — it mirrors the keymap
//! in `docs/ASCII_REFERENCE.md`, which the snapshot test asserts against.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::layout::{HELP_HEIGHT, HELP_WIDTH};
use super::theme::Theme;

/// The 24 box rows (each `HELP_WIDTH` = 76 chars). Drawn as-is; the snapshot test
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

/// x offset: centered — (96 - 76) / 2 = 10, matching the reference.
const X: u16 = 10;

pub fn render(frame: &mut Frame) {
    let theme = Theme::TOKYONIGHT;
    let area = frame.area();
    // Center vertically in whatever height we're given (31 → row 3).
    let y = area.y + (area.height.saturating_sub(HELP_HEIGHT)) / 2;
    let style = Style::default().fg(theme.border_focus);
    let buf = frame.buffer_mut();
    for (i, line) in BOX.iter().enumerate() {
        let row = y + i as u16;
        // Guard against a short terminal.
        if Rect::new(X, row, HELP_WIDTH, 1).intersection(area).height == 1 {
            buf.set_string(X, row, line, style);
        }
    }
}
