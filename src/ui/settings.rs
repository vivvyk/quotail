//! Settings view: a full-width, read-mostly rendering of `config.toml`. Each row
//! is `label · value · hint`, grouped into sections (watchlist / display / data /
//! mcp). Editing is Phase-2; for now this reflects the loaded config.
//!
//! Column geometry (from `docs/ASCII_REFERENCE.md`): label at col 3, value at col
//! 23, hint at col 51.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::AppState;
use crate::config::Config;

use super::theme::Theme;
use super::{bottom_bar, draw_box};

const LABEL_X: u16 = 3;
const VALUE_X: u16 = 23;
const HINT_X: u16 = 51;
/// Cells available for a value before the hint column, leaving one gap. Longer
/// values (e.g. a real `socket_path`) are elided so they never overrun the hint.
const VALUE_W: usize = (HINT_X - VALUE_X - 1) as usize;

pub fn render(frame: &mut Frame, state: &AppState, config: &Config) {
    let theme = Theme::TOKYONIGHT;
    let full = frame.area();
    let (w, height) = (full.width, full.height);
    let buf = frame.buffer_mut();

    // Fixed-height content, top-aligned; the box spans the full width and the bar is
    // anchored to the floor, with blank slack between them (rows 26-28 at 96x31).
    const PANEL_HEIGHT: u16 = 26;
    let area = Rect {
        x: 0,
        y: 0,
        width: w,
        height: PANEL_HEIGHT,
    };
    // The title carries the canonical config path (masked in the snapshot — it is
    // resolved from the user's home at runtime).
    draw_box(
        buf,
        area,
        "settings ─ ~/.config/quotail/config.toml",
        Style::default().fg(theme.border),
    );

    let d = &config.display;
    let dt = &config.data;
    let wl = &config.watchlist;

    section(buf, 2, "watchlist", theme);
    row(
        buf,
        3,
        "stocks",
        &symbols(wl.stocks.len()),
        "enter to edit  ·  top 50 by index weight",
        theme,
    );
    row(
        buf,
        4,
        "crypto",
        &symbols(wl.crypto.len()),
        "enter to edit",
        theme,
    );
    row(
        buf,
        5,
        "indices",
        &symbols(wl.indices.len()),
        "enter to edit",
        theme,
    );

    section(buf, 7, "display", theme);
    row(
        buf,
        8,
        "theme",
        &d.theme,
        "< >  tokyonight, gruvbox, catppuccin, nord",
        theme,
    );
    row(
        buf,
        9,
        "default_timeframe",
        &d.default_timeframe,
        "< >  1D 5D 1M 6M YTD 1Y MAX",
        theme,
    );
    row(
        buf,
        10,
        "default_filter",
        &d.default_filter,
        "< >  all, stocks, crypto, indices",
        theme,
    );
    row(
        buf,
        11,
        "default_sort",
        &d.default_sort,
        "< >  symbol, price, change_pct, mkt_cap",
        theme,
    );
    row(
        buf,
        12,
        "ticker_scroll",
        &d.ticker_scroll.to_string(),
        &format!("space to toggle  ·  speed {}ms", d.ticker_speed_ms),
        theme,
    );

    section(buf, 14, "data", theme);
    row(buf, 15, "provider", &dt.provider, "read-only in MVP", theme);
    row(
        buf,
        16,
        "poll_interval_sec",
        &dt.poll_interval_sec.to_string(),
        "< >  15, 30, 60, 300",
        theme,
    );
    row(
        buf,
        17,
        "cache_ttl_sec",
        &dt.cache_ttl_sec.to_string(),
        "< >  10, 30, 60",
        theme,
    );

    section(buf, 19, "mcp", theme);
    row(
        buf,
        20,
        "enabled",
        &config.mcp.enabled.to_string(),
        "space to toggle",
        theme,
    );
    row(
        buf,
        21,
        "socket_path",
        &config.mcp.socket_path,
        "enter to edit",
        theme,
    );

    buf.set_string(
        LABEL_X,
        23,
        "w writes changes to config.toml",
        Style::default().fg(theme.muted),
    );

    let bar = Rect {
        x: 0,
        y: height - 2,
        width: w,
        height: 2,
    };
    bottom_bar::render(buf, bar, state);
}

fn section(buf: &mut Buffer, y: u16, name: &str, theme: Theme) {
    buf.set_string(LABEL_X, y, name, Style::default().fg(theme.heading));
}

fn row(buf: &mut Buffer, y: u16, label: &str, value: &str, hint: &str, theme: Theme) {
    buf.set_string(LABEL_X, y, label, Style::default().fg(theme.fg));
    buf.set_string(
        VALUE_X,
        y,
        elide(value, VALUE_W),
        Style::default().fg(theme.price),
    );
    buf.set_string(HINT_X, y, hint, Style::default().fg(theme.muted));
}

/// Fit `s` into `max` cells, keeping the informative TAIL (paths end in the file
/// name) behind a leading `…` when it doesn't fit.
fn elide(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let tail: String = s.chars().skip(n - (max - 1)).collect();
    format!("…{tail}")
}

fn symbols(n: usize) -> String {
    format!("{n} symbols")
}
