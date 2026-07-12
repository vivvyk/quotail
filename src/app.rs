//! `AppState`: the single source of truth.
//!
//! Exactly ONE owner (the event loop), mutated only via `&mut` inside `update()`,
//! read-only (`&AppState`) by the renderer. No locks, no interior mutability —
//! the mpsc channel already serializes every mutation.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::data::types::{Candle, Fundamentals, Indicators, MarketStatus, Quote, Timeframe};
use crate::ui::layout::MAX_SLOTS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum View {
    Overview,
    Detail,
    Settings,
}

/// Modal, vim-style. The SAME keypress means different things per mode:
/// in Normal, `d` opens Detail; in Command, `d` is just a character typed
/// into the buffer. This is what makes the `:` command line work while
/// single-key shortcuts stay available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Command,
    Search,
}

/// Which Overview panel holds keyboard focus. `h`/`l` move between them and the
/// renderer highlights the focused region's border. Ephemeral: deliberately NOT
/// part of `Session`, so every launch starts on the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusRegion {
    Table,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetFilter {
    All,
    Stocks,
    Crypto,
    Indices,
}

impl AssetFilter {
    /// `f` cycles through these in order.
    pub fn next(self) -> AssetFilter {
        match self {
            AssetFilter::All => AssetFilter::Stocks,
            AssetFilter::Stocks => AssetFilter::Crypto,
            AssetFilter::Crypto => AssetFilter::Indices,
            AssetFilter::Indices => AssetFilter::All,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    Symbol,
    Price,
    ChangePct,
    MarketCap,
}

/// One pane of the 2x2 grid.
#[derive(Debug, Clone)]
pub struct ChartSlot {
    pub symbol: String,
    pub candles: Vec<Candle>,
    /// Fetches are async and NOT instant. Without this the pane flickers or
    /// looks frozen while data is in flight.
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DetailState {
    pub symbol: String,
    pub quote: Option<Quote>,
    pub candles: Vec<Candle>,
    pub fundamentals: Option<Fundamentals>,
    pub indicators: Indicators,
    pub loading: bool,
    pub error: Option<String>,
}

pub struct AppState {
    // ---- Navigation ----
    pub view: View,
    pub input_mode: InputMode,
    pub show_help: bool,
    /// Which Overview panel has focus (watchlist table vs the 2x2 chart grid).
    /// Ephemeral — not persisted; always starts on `Table`.
    pub focus: FocusRegion,

    // ---- Watchlist / table ----
    pub watchlist: Vec<String>,
    pub quotes: HashMap<String, Quote>,
    pub filter: AssetFilter,
    pub sort: SortKey,
    pub sort_desc: bool,
    pub selected_row: usize,
    /// The default watchlist is ~68 symbols but only 14 rows are visible,
    /// so the table scrolls. j/k past an edge moves this.
    pub scroll_offset: usize,

    // ---- Chart grid ----
    /// A FIXED ARRAY, not a Vec: the 4-pane cap is enforced by the type.
    /// You cannot have five. `Option` because a slot may be empty.
    pub slots: [Option<ChartSlot>; MAX_SLOTS],
    /// A new AddToSlot replaces THIS pane once all four are full.
    pub focused_slot: usize,
    /// Global across the grid and Detail.
    pub timeframe: Timeframe,

    // ---- Detail ----
    /// `None` when opened from nav with no symbol yet (empty + search prompt).
    pub detail: Option<DetailState>,

    // ---- Bottom region ----
    /// The `:` / `/` buffer. Rendered into row 2 in place of the hint strip.
    pub command_input: String,
    pub market_status: MarketStatus,
    pub last_refresh: DateTime<Utc>,
    pub status_msg: Option<String>,
    /// Cell offset of the hot-movers marquee. Advanced by Action::Tick.
    pub marquee_offset: usize,

    // ---- Lifecycle ----
    pub should_quit: bool,
}

/// The disposable slice of state written to `~/.local/state/quotail/session.json`
/// on quit. NOT the config file — config is user intent (hand-edited, dotfile-able);
/// this is ephemeral. If it is missing or corrupt, fall back to defaults SILENTLY.
/// Deleting it must lose nothing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// `None` = empty pane.
    pub slots: [Option<String>; MAX_SLOTS],
    pub focused_slot: usize,
    pub timeframe: Timeframe,
    pub view: View,
    pub selected_row: usize,
    pub scroll_offset: usize,
    pub filter: AssetFilter,
}
