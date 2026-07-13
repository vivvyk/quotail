//! Overview view: hot-movers marquee + watchlist table + 2x2 chart grid.
//!
//! Fully RESPONSIVE. The banner (3) is pinned to the top and the bar (2) to the
//! floor; the watchlist (fixed 40 cols) and the chart grid share the region
//! between. The grid absorbs ALL slack in both axes: its height splits into two
//! equal pane rows and its width (`W - 40`) into two panes. At 96x31 this reduces
//! to the historical 28x13 panes. This module only composes; panes come from
//! `chart`, the table from `table`.

use std::cmp::Ordering;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::{AppState, FocusRegion};
use crate::data::types::{Indicators, Quote};

use super::layout::{BANNER_HEIGHT, BOTTOM_HEIGHT, HOT_MOVERS_COUNT, MARQUEE_GAP, WATCHLIST_WIDTH};
use super::theme::{Theme, glyph};
use super::{bottom_bar, chart, table};

pub fn render(frame: &mut Frame, state: &AppState) {
    let theme = state.theme;
    let area = frame.area();
    let (w, h) = (area.width, area.height);
    let buf = frame.buffer_mut();

    render_banner(buf, state, theme, w);

    // The middle region: everything between the banner and the floor bar.
    let mid_y = BANNER_HEIGHT;
    let mid_h = h.saturating_sub(BANNER_HEIGHT + BOTTOM_HEIGHT);

    let watchlist = Rect {
        x: 0,
        y: mid_y,
        width: WATCHLIST_WIDTH,
        height: mid_h,
    };
    table::render(buf, watchlist, state);

    render_grid(buf, state, theme, mid_y, mid_h, w);

    let bar = Rect {
        x: 0,
        y: h - BOTTOM_HEIGHT,
        width: w,
        height: BOTTOM_HEIGHT,
    };
    bottom_bar::render(buf, bar, state);
}

// ---- hot-movers banner -----------------------------------------------------

fn render_banner(buf: &mut Buffer, state: &AppState, theme: Theme, w: u16) {
    let area = Rect {
        x: 0,
        y: 0,
        width: w,
        height: BANNER_HEIGHT,
    };
    // "hot movers" in accent orange — the most visible title on the screen.
    super::draw_box(
        buf,
        area,
        "hot movers",
        Style::default().fg(theme.border),
        Style::default().fg(theme.accent),
    );

    // The tape is one styled cell per column so up/down arrows keep their color.
    // Content is masked in the snapshot (data-dependent); this is the live view.
    let cells = marquee_cells(state, theme);
    if cells.is_empty() {
        return;
    }
    // One leading blank after the border (col 1), tape from col 2 to the last
    // interior column (w - 2).
    let width = w.saturating_sub(3) as usize;
    let offset = state.marquee_offset % cells.len();
    for i in 0..width {
        let (ch, style) = cells[(offset + i) % cells.len()];
        buf.set_string(2 + i as u16, 1, ch.to_string(), style);
    }
}

/// Top movers by absolute change, laid out as `▲ SYM +x.xx%` segments separated
/// by `MARQUEE_GAP` blanks, expanded to (char, style) cells.
fn marquee_cells(state: &AppState, theme: Theme) -> Vec<(char, Style)> {
    let mut movers: Vec<&Quote> = state.quotes.values().collect();
    movers.sort_by(|a, b| {
        b.change_pct
            .abs()
            .partial_cmp(&a.change_pct.abs())
            .unwrap_or(Ordering::Equal)
    });
    movers.truncate(HOT_MOVERS_COUNT);

    let mut cells = Vec::new();
    for q in movers {
        let up = q.change_pct >= 0.0;
        let arrow = if up {
            glyph::UP_ARROW
        } else {
            glyph::DOWN_ARROW
        };
        let style = Style::default().fg(if up { theme.up } else { theme.down });
        let seg = format!("{arrow} {} {:+.2}%", q.symbol, q.change_pct);
        for ch in seg.chars() {
            cells.push((ch, style));
        }
        for _ in 0..MARQUEE_GAP {
            cells.push((' ', Style::default()));
        }
    }
    cells
}

// ---- 2x2 chart grid --------------------------------------------------------

/// The 2x2 grid fills the region right of the watchlist (cols `WATCHLIST_WIDTH`..
/// `w`) and rows `grid_y`..`grid_y+grid_h`. Both dimensions stretch: the height
/// splits into two equal pane rows, the width into two panes (`(w-40)/2` each,
/// remainder to the right so the grid reaches the edge). More width → more candle
/// columns over the SAME span (finer resolution), not a wider time window.
fn render_grid(buf: &mut Buffer, state: &AppState, theme: Theme, grid_y: u16, grid_h: u16, w: u16) {
    let span = state.timeframe.display_span();
    let tf_label = state.timeframe.label();
    let top_h = grid_h / 2;
    let bot_h = grid_h - top_h;
    let grid_w = w.saturating_sub(WATCHLIST_WIDTH);
    let left_w = grid_w / 2;
    let right_w = grid_w - left_w;
    let lx = WATCHLIST_WIDTH;
    let rx = WATCHLIST_WIDTH + left_w;
    // (x, y, width, height) per pane: top-left, top-right, bottom-left, bottom-right.
    let panes = [
        (lx, grid_y, left_w, top_h),
        (rx, grid_y, right_w, top_h),
        (lx, grid_y + top_h, left_w, bot_h),
        (rx, grid_y + top_h, right_w, bot_h),
    ];

    for (i, (px, py, pw, ph)) in panes.into_iter().enumerate() {
        let area = Rect {
            x: px,
            y: py,
            width: pw,
            height: ph,
        };
        let focused = state.focus == FocusRegion::Grid && state.focused_slot == i;
        let border = Style::default().fg(if focused {
            theme.border_focus
        } else {
            theme.border
        });

        match &state.slots[i] {
            Some(slot) => {
                let quote = state.quotes.get(&slot.symbol);
                let price = quote.map(|q| fmt_price(q.price));
                let change = quote.map(|q| q.change_pct);
                // Candle columns = interior width (pane minus 2 borders + 2 pads).
                // Overview panes carry candles only — indicators live on the Detail
                // view, so there are no MA dots to sample here.
                let cols = pw.saturating_sub(4) as usize;
                let columns = chart::aggregate(&slot.candles, &Indicators::default(), span, cols);
                chart::render_grid_pane(
                    buf,
                    area,
                    &slot.symbol,
                    price.as_deref(),
                    change,
                    &columns,
                    tf_label,
                    border,
                    theme,
                );
            }
            None => chart::render_empty_pane(buf, area, border, theme),
        }
    }
}

/// Prices >= 1000 (indices, BTC) drop decimals to fit the narrow pane title.
fn fmt_price(price: f64) -> String {
    if price >= 1000.0 {
        format!("{price:.0}")
    } else {
        format!("{price:.2}")
    }
}
