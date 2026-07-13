//! `AppState`: the single source of truth.
//!
//! Exactly ONE owner (the event loop), mutated only via `&mut` inside `update()`,
//! read-only (`&AppState`) by the renderer. No locks, no interior mutability —
//! the mpsc channel already serializes every mutation.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::Config;
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
    /// A long watchlist scrolls: j/k past an edge moves this.
    pub scroll_offset: usize,
    /// Data rows the watchlist body can show at the CURRENT terminal height.
    /// Derived from the terminal size (the box stretches vertically), refreshed
    /// on resize — not persisted. Used to keep the selection in view; the renderer
    /// derives the same count straight from its draw area.
    pub viewport_rows: usize,

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

    // ---- Settings ----
    /// The live, editable config. The Settings screen reads and mutates THIS (not
    /// the `Arc<Config>` the producers hold); `w` writes it back to `config.toml`.
    pub config: Config,
    /// Which editable Settings row is selected (j/k). Ephemeral.
    pub settings_row: usize,

    // ---- MCP ----
    /// State of the Unix-socket listener, surfaced (only when degraded) in the
    /// status row. Set once at startup after the bind attempt.
    pub mcp_listener: McpListener,

    // ---- Presentation ----
    /// The active color palette, chosen ONCE at startup from the detected terminal
    /// color depth (truecolor / 256 / 16). Every renderer reads its colors from
    /// here rather than a hardcoded truecolor constant, so a limited terminal is
    /// handed `Color::Indexed`/ANSI names it can actually render. See `ui::theme`.
    pub theme: crate::ui::theme::Theme,

    // ---- Lifecycle ----
    pub should_quit: bool,
}

/// Whether the MCP session socket bound, and if not, why — so the status row can
/// say so instead of degrading silently. Only the degraded variants render text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum McpListener {
    /// Bound and serving — or a platform without the socket (nothing to report).
    #[default]
    Active,
    /// `[mcp].enabled = false`: the user opted out. No notice.
    Disabled,
    /// A live instance already owns the socket; we skipped binding rather than
    /// refuse to launch. The other instance keeps serving Claude.
    OtherInstance,
    /// Bind failed (permissions, unwritable path, …). MCP session tools are off.
    Failed,
}

impl McpListener {
    /// The muted status-row notice, or `None` when there's nothing to surface.
    pub fn notice(self) -> Option<&'static str> {
        match self {
            McpListener::Active | McpListener::Disabled => None,
            McpListener::OtherInstance => Some("MCP: inactive (another instance)"),
            McpListener::Failed => Some("MCP: inactive (socket error)"),
        }
    }
}

/// A read-only view of the live session for the MCP `get_session` tool. Built from
/// `&AppState` by the event loop — the sole owner — and handed back over a
/// `oneshot`, then serialized to JSON on the socket. Distinct from [`Session`] (the
/// on-disk persistence type): this is Claude-facing, so the timeframe is the human
/// label and the selected/detail symbols are resolved rather than raw indices.
#[derive(Debug, Clone, Serialize)]
pub struct SessionSnapshot {
    pub view: View,
    /// Human timeframe label (e.g. `"1Y"`) — the same string `set_timeframe` takes.
    pub timeframe: String,
    /// The four chart panes in order; `None` is an empty pane.
    pub slots: [Option<String>; MAX_SLOTS],
    pub focused_slot: usize,
    pub filter: AssetFilter,
    /// Symbol highlighted in the watchlist table (`None` if the table is empty).
    pub watchlist_selected: Option<String>,
    /// The drilldown symbol when `view` is `Detail`; otherwise `None`.
    pub detail_symbol: Option<String>,
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
