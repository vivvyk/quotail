//! Watchlist table. Rendering lands in step 4; for now this owns the shared
//! ORDERING — filter + sort — because two callers need the exact same order: the
//! renderer (to draw rows) and key handling (to map the selected row back to a
//! symbol). One source of truth, so the cursor never points at a different row
//! than the one on screen.

use std::cmp::Ordering;

use crate::app::{AppState, AssetFilter, SortKey};
use crate::data::types::AssetKind;

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
            // Market cap lives in Fundamentals, not Quote, and isn't cached for
            // the whole watchlist yet — fall back to symbol order until step 4
            // wires per-row fundamentals. Documented, not silently wrong.
            SortKey::MarketCap => a.cmp(b),
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
