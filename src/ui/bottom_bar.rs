//! Bottom region: row 1 is the passive status line (exchange · market status ·
//! `[tf]` · last refresh); row 2 is the per-view keyhint strip in Normal mode
//! and becomes the `:` / `/` input line otherwise. Fixed columns match
//! `docs/ASCII_REFERENCE.md`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::app::{AppState, InputMode, View};
use crate::data::types::MarketStatus;

use super::theme::Theme;

pub fn render(buf: &mut Buffer, area: Rect, state: &AppState) {
    let theme = Theme::TOKYONIGHT;
    let muted = Style::default().fg(theme.muted);
    let (x, y) = (area.x, area.y);

    // ---- status row: exchange (muted) + status WORD (green/red) ----
    let exchange = format!("{}: ", exchange_label(state));
    buf.set_string(x + 1, y, &exchange, muted);
    let word = status_text(state.market_status);
    buf.set_string(
        x + 1 + exchange.chars().count() as u16,
        y,
        word,
        Style::default().fg(status_color(state.market_status, theme)),
    );
    // The `(Opens …)` next-open hint is deferred (needs a market calendar); the
    // snapshot masks that span. `[tf]` (warn/yellow) shows only on the chart views.
    if matches!(state.view, View::Overview | View::Detail) {
        buf.set_string(
            x + 51,
            y,
            format!("[{}]", state.timeframe.label()),
            Style::default().fg(theme.warn),
        );
    }
    // The clock is right-anchored to the bar (one-column margin). At width 96 this
    // lands at col 73, matching the reference; on a wider terminal it hugs the edge.
    let clock = format!("Last Refresh: {}", state.last_refresh.format("%H:%M:%S"));
    let clock_x = x + area.width.saturating_sub(1 + clock.chars().count() as u16);
    buf.set_string(clock_x, y, clock, muted);

    // ---- hint / input row ----
    let y2 = y + 1;
    match state.input_mode {
        // Keys in accent, their labels in muted.
        InputMode::Normal => {
            let accent = Style::default().fg(theme.accent);
            let mut cx = x + 1;
            for (key, label) in hints(state.view) {
                buf.set_string(cx, y2, key, accent);
                cx += key.chars().count() as u16 + 1;
                buf.set_string(cx, y2, label, muted);
                cx += label.chars().count() as u16 + 2;
            }
        }
        InputMode::Command => {
            buf.set_string(
                x,
                y2,
                format!(":{}▮", state.command_input),
                Style::default().fg(theme.fg),
            );
        }
        InputMode::Search => {
            buf.set_string(
                x,
                y2,
                format!("/{}▮", state.command_input),
                Style::default().fg(theme.fg),
            );
        }
    }
}

/// Overview/Settings show a market-wide "NYSE"; Detail shows the focused
/// symbol's exchange (falling back to NYSE).
fn exchange_label(state: &AppState) -> String {
    match state.view {
        View::Detail => state
            .detail
            .as_ref()
            .and_then(|d| d.quote.as_ref())
            .and_then(|q| q.exchange.clone())
            .unwrap_or_else(|| "NYSE".into()),
        _ => "NYSE".into(),
    }
}

fn status_text(status: MarketStatus) -> &'static str {
    match status {
        MarketStatus::Open => "Open",
        MarketStatus::Closed => "Closed",
        MarketStatus::PreMarket => "Pre-Market",
        MarketStatus::AfterHours => "After-Hours",
    }
}

fn status_color(status: MarketStatus, theme: Theme) -> Color {
    match status {
        MarketStatus::Open => theme.up,
        MarketStatus::Closed => theme.down,
        // Pre/after-hours: open-ish but not regular session.
        MarketStatus::PreMarket | MarketStatus::AfterHours => theme.warn,
    }
}

/// Per-view keyhints as (key, label) pairs — the key is drawn in accent, the label
/// in muted. Concatenated they reproduce the reference hint strings exactly.
fn hints(view: View) -> &'static [(&'static str, &'static str)] {
    match view {
        View::Overview => &[
            ("q", "quit"),
            ("d", "detail"),
            ("/", "search"),
            ("f", "filter"),
            ("s", "sort"),
            ("c", "clear"),
            ("tab", "pane"),
            ("r", "refresh"),
            (":", "cmd"),
            ("?", "help"),
        ],
        View::Detail => &[
            ("esc", "back"),
            ("/", "search"),
            ("1-7", "timeframe"),
            ("e", "export"),
            ("r", "refresh"),
            (":", "cmd"),
            ("?", "help"),
            ("q", "quit"),
        ],
        View::Settings => &[
            ("j/k", "move"),
            ("enter", "edit"),
            ("< >", "change"),
            ("space", "toggle"),
            ("w", "write"),
            ("esc", "back"),
            ("?", "help"),
            ("q", "quit"),
        ],
    }
}
