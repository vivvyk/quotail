//! Watchlist table. Owns the shared ORDERING — filter + sort — because two callers
//! need the exact same order: the renderer (to draw rows) and key handling (to map
//! the selected row back to a symbol). One source of truth, so the cursor never
//! points at a different row than the one on screen.

use std::cmp::Ordering;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::{AppState, AssetFilter, FocusRegion, SortKey};
use crate::data::types::AssetKind;

use super::theme::{Theme, glyph};

/// Fallback body-row count before the first terminal size is known (headless /
/// initial state). The real count is derived from the panel height via
/// [`visible_rows`] — the watchlist box stretches with the terminal.
pub const TABLE_VISIBLE_ROWS: usize = 14;

/// Data rows the table body can show at a given panel height: the panel minus its
/// title row, header row, and bottom border. The watchlist box stretches to fill
/// the terminal, so a taller terminal shows MORE symbols — this is derived, never
/// a fixed cap. (At the 96x31 reference the box is 26 tall → 23 rows.)
pub fn visible_rows(panel_height: u16) -> usize {
    (panel_height as usize).saturating_sub(3).max(1)
}

/// Does an asset kind pass the current filter?
pub fn asset_matches(filter: AssetFilter, kind: AssetKind) -> bool {
    matches!(
        (filter, kind),
        (AssetFilter::All, _)
            | (AssetFilter::Stocks, AssetKind::Stock)
            | (AssetFilter::Crypto, AssetKind::Crypto)
            | (AssetFilter::Indices, AssetKind::Index)
    )
}

/// The watchlist as currently shown: filtered by asset kind (inferred from the
/// symbol, so filtering works even before any quote has arrived), then sorted.
pub fn visible_symbols(state: &AppState) -> Vec<String> {
    let mut syms: Vec<String> = state
        .watchlist
        .iter()
        .filter(|s| asset_matches(state.filter, AssetKind::infer(s)))
        .cloned()
        .collect();
    sort_symbols(&mut syms, state);
    syms
}

fn sort_symbols(syms: &mut [String], state: &AppState) {
    let field = |s: &str| state.quotes.get(s);
    syms.sort_by(|a, b| {
        let ord = match state.sort {
            SortKey::Symbol => a.cmp(b),
            SortKey::Price => cmp_opt(field(a).map(|q| q.price), field(b).map(|q| q.price)),
            SortKey::ChangePct => cmp_opt(
                field(a).map(|q| q.change_pct),
                field(b).map(|q| q.change_pct),
            ),
            SortKey::MarketCap => cmp_opt(
                field(a).and_then(|q| q.market_cap),
                field(b).and_then(|q| q.market_cap),
            ),
        };
        if state.sort_desc { ord.reverse() } else { ord }
    });
}

/// Compare two optional numbers; `None` sorts last (missing data at the bottom).
fn cmp_opt(a: Option<f64>, b: Option<f64>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// The symbol under the selection cursor, if any.
pub fn selected_symbol(state: &AppState) -> Option<String> {
    visible_symbols(state).into_iter().nth(state.selected_row)
}

// ---- rendering -------------------------------------------------------------
// Column geometry (matches ASCII_REFERENCE.md, panel width 40):
//   ticker  left-aligned at col 2
//   price   right-aligned ending col 22
//   chg%    right-aligned ending col 31
//   sort ▼  at col 33 (next to the active column)
//   scrollbar at col 38, border at col 39

/// Right edge (inclusive col) of the price / change columns.
const PRICE_END: u16 = 22;
const CHG_END: u16 = 31;
const SORT_COL: u16 = 33;
const SCROLL_COL: u16 = 38;

pub fn render(buf: &mut Buffer, area: Rect, state: &AppState) {
    let theme = state.theme;
    // Focus dims the BORDER only; the title/content keep their own colors.
    let border = Style::default().fg(if state.focus == FocusRegion::Table {
        theme.border_focus
    } else {
        theme.border
    });
    let heading = Style::default().fg(theme.heading);
    let muted = Style::default().fg(theme.muted);
    let accent = Style::default().fg(theme.accent);

    // Plain frame, then paint the title with per-segment colors: "watchlist" in
    // heading purple, the active filter tab in accent orange, the rest muted.
    super::draw_box(buf, area, "", border, border);
    let x = area.x;
    let y = area.y;
    let mut cx = x;
    let mut put = |cx: &mut u16, text: &str, st: Style| {
        buf.set_string(*cx, y, text, st);
        *cx += text.chars().count() as u16;
    };
    put(&mut cx, "┌─ ", border);
    put(&mut cx, "watchlist", heading);
    put(&mut cx, " ─ ", border);
    for (f, label) in [
        (AssetFilter::All, "All"),
        (AssetFilter::Stocks, "Stk"),
        (AssetFilter::Crypto, "Cry"),
        (AssetFilter::Indices, "Idx"),
    ] {
        if f == state.filter {
            put(&mut cx, &format!("[{label}]"), accent);
        } else {
            put(&mut cx, label, muted);
        }
        put(&mut cx, " ", border);
    }

    let hy = area.y + 1;
    buf.set_string(x + 2, hy, "ticker", heading);
    put_right(buf, x + PRICE_END, hy, "price", heading);
    put_right(buf, x + CHG_END, hy, "chg%", heading);
    if state.sort == SortKey::ChangePct {
        let arrow = if state.sort_desc {
            glyph::SORT_DESC
        } else {
            glyph::SORT_ASC
        };
        buf.set_string(x + SORT_COL, hy, arrow.to_string(), heading);
    }

    // Data rows: symbol fg, price cyan, chg green/red; selected row gets the
    // selection background layered under all three columns.
    let visible = visible_rows(area.height);
    let rows = visible_symbols(state);
    for (i, sym) in rows
        .iter()
        .enumerate()
        .skip(state.scroll_offset)
        .take(visible)
    {
        let ry = area.y + 2 + (i - state.scroll_offset) as u16;
        let bg = |st: Style| {
            if i == state.selected_row {
                st.bg(theme.selection)
            } else {
                st
            }
        };
        buf.set_string(x + 2, ry, sym, bg(Style::default().fg(theme.fg)));
        if let Some(q) = state.quotes.get(sym) {
            put_right(
                buf,
                x + PRICE_END,
                ry,
                &format!("{:.2}", q.price),
                bg(Style::default().fg(theme.price)),
            );
            put_right(
                buf,
                x + CHG_END,
                ry,
                &format!("{:+.2}%", q.change_pct),
                bg(Style::default().fg(theme.change(q.change_pct))),
            );
        }
    }

    render_scrollbar(buf, area, rows.len(), state.scroll_offset, visible, theme);
}

/// A right-anchored thumb on the panel's right-inner column (col 38). `visible` is
/// the body-row count for the current panel height.
fn render_scrollbar(
    buf: &mut Buffer,
    area: Rect,
    total: usize,
    offset: usize,
    visible: usize,
    theme: Theme,
) {
    if total <= visible {
        return;
    }
    let track_h = visible as u16;
    let top = area.y + 2;
    let thumb_h = ((track_h as usize * visible) / total).max(1) as u16;
    let max_off = total - visible;
    let thumb_y = (offset * (track_h - thumb_h) as usize)
        .checked_div(max_off)
        .unwrap_or(0) as u16;
    for r in 0..track_h {
        let ch = if r >= thumb_y && r < thumb_y + thumb_h {
            glyph::SCROLL_THUMB
        } else {
            glyph::SCROLL_TRACK
        };
        buf.set_string(
            area.x + SCROLL_COL,
            top + r,
            ch.to_string(),
            Style::default().fg(theme.muted),
        );
    }
}

/// Set a right-aligned string ending at inclusive column `end_col`.
fn put_right(buf: &mut Buffer, end_col: u16, y: u16, text: &str, style: Style) {
    let len = text.chars().count() as u16;
    let x = end_col.saturating_sub(len.saturating_sub(1));
    buf.set_string(x, y, text, style);
}
