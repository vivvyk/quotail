//! Watchlist table. Rendering lands in step 4; for now this owns the shared
//! ORDERING — filter + sort — because two callers need the exact same order: the
//! renderer (to draw rows) and key handling (to map the selected row back to a
//! symbol). One source of truth, so the cursor never points at a different row
//! than the one on screen.

use std::cmp::Ordering;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::{AppState, AssetFilter, FocusRegion, SortKey};
use crate::data::types::AssetKind;

use super::theme::{Theme, glyph};

/// Rows visible in the table body at once (the panel shows 14; ~68 symbols
/// scroll past it).
pub const TABLE_VISIBLE_ROWS: usize = 14;

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
    let theme = Theme::TOKYONIGHT;
    let border = if state.focus == FocusRegion::Table {
        theme.border_focus
    } else {
        theme.border
    };
    let title = format!("watchlist ─ {}", filter_title(state.filter));
    super::draw_box(buf, area, &title, Style::default().fg(border));

    let x = area.x;
    let heading = Style::default().fg(theme.heading);
    let hy = area.y + 1;
    buf.set_string(x + 2, hy, "ticker", heading);
    put_right(buf, x + PRICE_END, hy, "price", heading);
    put_right(buf, x + CHG_END, hy, "chg%", heading);
    // Sort arrow next to the active column (fixture sorts by change_pct).
    if state.sort == SortKey::ChangePct {
        let arrow = if state.sort_desc {
            glyph::SORT_DESC
        } else {
            glyph::SORT_ASC
        };
        buf.set_string(
            x + SORT_COL,
            hy,
            arrow.to_string(),
            Style::default().fg(theme.accent),
        );
    }

    // Data rows: the visible slice of the filtered/sorted list.
    let rows = visible_symbols(state);
    for (i, sym) in rows
        .iter()
        .enumerate()
        .skip(state.scroll_offset)
        .take(TABLE_VISIBLE_ROWS)
    {
        let ry = area.y + 2 + (i - state.scroll_offset) as u16;
        let selected = i == state.selected_row;
        let row_style = if selected {
            Style::default().fg(theme.fg).bg(theme.selection)
        } else {
            Style::default().fg(theme.fg)
        };
        buf.set_string(x + 2, ry, sym, row_style);
        if let Some(q) = state.quotes.get(sym) {
            put_right(
                buf,
                x + PRICE_END,
                ry,
                &format!("{:.2}", q.price),
                row_style,
            );
            let chg_style = if selected {
                row_style
            } else {
                Style::default().fg(theme.change(q.change_pct))
            };
            put_right(
                buf,
                x + CHG_END,
                ry,
                &format!("{:+.2}%", q.change_pct),
                chg_style,
            );
        }
    }

    render_scrollbar(buf, area, rows.len(), state.scroll_offset);
}

/// A right-anchored thumb on the panel's right-inner column (col 38).
fn render_scrollbar(buf: &mut Buffer, area: Rect, total: usize, offset: usize) {
    let theme = Theme::TOKYONIGHT;
    if total <= TABLE_VISIBLE_ROWS {
        return;
    }
    let track_h = TABLE_VISIBLE_ROWS as u16;
    let top = area.y + 2;
    let thumb_h = ((track_h as usize * TABLE_VISIBLE_ROWS) / total).max(1) as u16;
    let max_off = total - TABLE_VISIBLE_ROWS;
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

/// The filter segment of the panel title, with the active one bracketed.
fn filter_title(filter: AssetFilter) -> String {
    let seg = |f: AssetFilter, label: &str| {
        if f == filter {
            format!("[{label}]")
        } else {
            label.to_string()
        }
    };
    format!(
        "{} {} {} {}",
        seg(AssetFilter::All, "All"),
        seg(AssetFilter::Stocks, "Stk"),
        seg(AssetFilter::Crypto, "Cry"),
        seg(AssetFilter::Indices, "Idx"),
    )
}
