//! Settings view: a full-width, editable rendering of `config.toml`, grouped into
//! sections (watchlist / display / data / mcp). j/k select a row; `< >` cycle enum
//! values; space toggles bools; `w` writes back. Text fields (socket_path) and the
//! watchlist lists aren't inline-editable yet — their hints say so.
//!
//! Column geometry (from `docs/ASCII_REFERENCE.md`): label at col 3, value at col
//! 23, hint at col 51. The row model (`ROWS`) is shared with the editing logic so
//! navigation and rendering can never disagree.

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
/// Cells available for a value before the hint column, leaving one gap.
const VALUE_W: usize = (HINT_X - VALUE_X - 1) as usize;

// Enum option lists (also the cycle order for `< >`).
const THEMES: &[&str] = &["tokyonight", "gruvbox", "catppuccin", "nord"];
const TFS: &[&str] = &["1D", "5D", "1M", "6M", "YTD", "1Y", "MAX"];
const FILTERS: &[&str] = &["all", "stocks", "crypto", "indices"];
const SORTS: &[&str] = &["symbol", "price", "change_pct", "market_cap"];
const POLLS: &[u64] = &[15, 30, 60, 300];
const CACHES: &[u64] = &[10, 30, 60];

/// How a row is edited.
#[derive(Clone, Copy)]
enum Edit {
    Enum(&'static [&'static str]),
    Num(&'static [u64]),
    Bool,
    /// Editable elsewhere / not yet (hint explains). `< >`/space are no-ops.
    None,
}

/// One editable row: label, the screen row it renders on, and its editor.
struct Row {
    label: &'static str,
    y: u16,
    edit: Edit,
}

const ROWS: &[Row] = &[
    Row {
        label: "stocks",
        y: 3,
        edit: Edit::None,
    },
    Row {
        label: "crypto",
        y: 4,
        edit: Edit::None,
    },
    Row {
        label: "indices",
        y: 5,
        edit: Edit::None,
    },
    Row {
        label: "theme",
        y: 8,
        edit: Edit::Enum(THEMES),
    },
    Row {
        label: "default_timeframe",
        y: 9,
        edit: Edit::Enum(TFS),
    },
    Row {
        label: "default_filter",
        y: 10,
        edit: Edit::Enum(FILTERS),
    },
    Row {
        label: "default_sort",
        y: 11,
        edit: Edit::Enum(SORTS),
    },
    Row {
        label: "ticker_scroll",
        y: 12,
        edit: Edit::Bool,
    },
    Row {
        label: "provider",
        y: 15,
        edit: Edit::None,
    },
    Row {
        label: "poll_interval_sec",
        y: 16,
        edit: Edit::Num(POLLS),
    },
    Row {
        label: "cache_ttl_sec",
        y: 17,
        edit: Edit::Num(CACHES),
    },
    Row {
        label: "enabled",
        y: 20,
        edit: Edit::Bool,
    },
    Row {
        label: "socket_path",
        y: 21,
        edit: Edit::None,
    },
];

pub const ROW_COUNT: usize = ROWS.len();

pub fn render(frame: &mut Frame, state: &AppState) {
    let theme = Theme::TOKYONIGHT;
    let full = frame.area();
    let (w, height) = (full.width, full.height);
    let buf = frame.buffer_mut();

    // Fixed-height content, top-aligned; the box spans the full width and the bar is
    // anchored to the floor, with blank slack between (rows 26-28 at 96x31).
    const PANEL_HEIGHT: u16 = 26;
    let area = Rect {
        x: 0,
        y: 0,
        width: w,
        height: PANEL_HEIGHT,
    };
    // Title: "settings" heading; the config path (masked in the snapshot) muted.
    draw_box(
        buf,
        area,
        "",
        Style::default().fg(theme.border),
        Style::default().fg(theme.heading),
    );
    buf.set_string(0, 0, "┌─ ", Style::default().fg(theme.border));
    buf.set_string(3, 0, "settings", Style::default().fg(theme.heading));
    buf.set_string(11, 0, " ─ ", Style::default().fg(theme.border));
    buf.set_string(
        14,
        0,
        "~/.config/quotail/config.toml ",
        Style::default().fg(theme.muted),
    );

    for (y, name) in [(2, "watchlist"), (7, "display"), (14, "data"), (19, "mcp")] {
        buf.set_string(LABEL_X, y, name, Style::default().fg(theme.heading));
    }

    let cfg = &state.config;
    for (i, r) in ROWS.iter().enumerate() {
        draw_row(buf, w, r, i, i == state.settings_row, cfg, theme);
    }

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

fn draw_row(
    buf: &mut Buffer,
    w: u16,
    r: &Row,
    idx: usize,
    selected: bool,
    cfg: &Config,
    theme: Theme,
) {
    let y = r.y;
    // Selected row: paint the interior with the selection background, brighten the
    // label to fg. Others: label muted.
    let (bg, label_fg) = if selected {
        buf.set_string(
            1,
            y,
            " ".repeat(w.saturating_sub(2) as usize),
            Style::default().bg(theme.selection),
        );
        (Some(theme.selection), theme.fg)
    } else {
        (None, theme.muted)
    };
    let style = |fg| {
        let s = Style::default().fg(fg);
        if let Some(b) = bg { s.bg(b) } else { s }
    };

    buf.set_string(LABEL_X, y, r.label, style(label_fg));
    buf.set_string(
        VALUE_X,
        y,
        elide(&value_str(idx, cfg), VALUE_W),
        style(theme.price),
    );
    // Clip the hint so it never overruns the right border.
    let hint_w = (w.saturating_sub(1 + HINT_X)) as usize;
    let h = hint(idx, cfg);
    let shown: String = h.chars().take(hint_w).collect();
    buf.set_string(HINT_X, y, shown, style(theme.muted));
}

/// The value string shown for a row.
fn value_str(idx: usize, c: &Config) -> String {
    match idx {
        0 => format!("{} symbols", c.watchlist.stocks.len()),
        1 => format!("{} symbols", c.watchlist.crypto.len()),
        2 => format!("{} symbols", c.watchlist.indices.len()),
        3 => c.display.theme.clone(),
        4 => c.display.default_timeframe.clone(),
        5 => c.display.default_filter.clone(),
        6 => c.display.default_sort.clone(),
        7 => c.display.ticker_scroll.to_string(),
        8 => c.data.provider.clone(),
        9 => c.data.poll_interval_sec.to_string(),
        10 => c.data.cache_ttl_sec.to_string(),
        11 => c.mcp.enabled.to_string(),
        12 => c.mcp.socket_path.clone(),
        _ => String::new(),
    }
}

/// The right-hand hint for a row. Non-editable-here rows say how / that it's TBD.
fn hint(idx: usize, c: &Config) -> String {
    match idx {
        0..=2 => "edit via :add / :rm".into(),
        3 => "< >  tokyonight, gruvbox, catppuccin, nord".into(),
        4 => "< >  1D 5D 1M 6M YTD 1Y MAX".into(),
        5 => "< >  all, stocks, crypto, indices".into(),
        6 => "< >  symbol, price, change_pct, mkt_cap".into(),
        7 => format!("space to toggle  ·  speed {}ms", c.display.ticker_speed_ms),
        8 => "read-only".into(),
        9 => "< >  15, 30, 60, 300  ·  applies on restart".into(),
        10 => "< >  10, 30, 60".into(),
        11 => "space to toggle".into(),
        12 => "inline edit (TBD) — edit in config.toml".into(),
        _ => String::new(),
    }
}

// ---- editing (called from the event loop) ----------------------------------

/// Cycle the selected row's enum/number value by `delta` (wrapping). No-op on
/// non-cyclable rows. Returns whether anything changed (for status feedback).
pub fn cycle(config: &mut Config, row: usize, delta: i32) -> bool {
    match ROWS.get(row).map(|r| r.edit) {
        Some(Edit::Enum(opts)) => {
            let cur = value_str(row, config);
            let next = cycle_str(&cur, opts, delta);
            set_enum(config, row, next);
            true
        }
        Some(Edit::Num(opts)) => {
            let cur = value_str(row, config).parse::<u64>().unwrap_or(opts[0]);
            let next = cycle_num(cur, opts, delta);
            set_num(config, row, next);
            true
        }
        _ => false,
    }
}

/// Toggle the selected row if it's a bool. Returns whether it did.
pub fn toggle(config: &mut Config, row: usize) -> bool {
    match ROWS.get(row).map(|r| r.edit) {
        Some(Edit::Bool) => {
            match row {
                7 => config.display.ticker_scroll = !config.display.ticker_scroll,
                11 => config.mcp.enabled = !config.mcp.enabled,
                _ => {}
            }
            true
        }
        _ => false,
    }
}

fn set_enum(c: &mut Config, row: usize, v: String) {
    match row {
        3 => c.display.theme = v,
        4 => c.display.default_timeframe = v,
        5 => c.display.default_filter = v,
        6 => c.display.default_sort = v,
        _ => {}
    }
}

fn set_num(c: &mut Config, row: usize, v: u64) {
    match row {
        9 => c.data.poll_interval_sec = v,
        10 => c.data.cache_ttl_sec = v,
        _ => {}
    }
}

fn cycle_str(cur: &str, opts: &[&str], delta: i32) -> String {
    let i = opts.iter().position(|o| *o == cur).unwrap_or(0) as i32;
    let n = opts.len() as i32;
    let ni = (i + delta).rem_euclid(n) as usize;
    opts[ni].to_string()
}

fn cycle_num(cur: u64, opts: &[u64], delta: i32) -> u64 {
    let i = opts.iter().position(|o| *o == cur).unwrap_or(0) as i32;
    let n = opts.len() as i32;
    opts[(i + delta).rem_euclid(n) as usize]
}

/// Fit `s` into `max` cells, keeping the informative TAIL behind a leading `…`.
fn elide(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let tail: String = s.chars().skip(n - (max - 1)).collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_wraps_enum_and_number_rows() {
        let mut c = Config::default();
        assert!(cycle(&mut c, 3, 1));
        assert_eq!(c.display.theme, "gruvbox");
        assert!(cycle(&mut c, 3, -1));
        assert_eq!(c.display.theme, "tokyonight");
        assert!(cycle(&mut c, 9, 1)); // 60 -> 300
        assert_eq!(c.data.poll_interval_sec, 300);
        assert!(cycle(&mut c, 9, 1)); // 300 -> 15 (wrap)
        assert_eq!(c.data.poll_interval_sec, 15);
        assert!(!cycle(&mut c, 12, 1)); // socket_path is not cyclable
    }

    #[test]
    fn toggle_flips_only_bool_rows() {
        let mut c = Config::default();
        assert!(toggle(&mut c, 7));
        assert!(!c.display.ticker_scroll);
        assert!(toggle(&mut c, 11));
        assert!(!c.mcp.enabled);
        assert!(!toggle(&mut c, 3)); // theme is not a toggle
    }
}
