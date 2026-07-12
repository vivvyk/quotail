//! `Action` is the single command surface of the whole app.
//!
//! Every producer sends `Action`s into one `mpsc` channel; one consumer (the
//! event loop) drains it and is the ONLY thing that mutates `AppState`. That is
//! why there are no locks anywhere in the UI layer.
//!
//! Producers:
//!   1. input task        -> Action::Key / Mouse / Resize
//!   2. poller task       -> Action::QuotesUpdated
//!   3. tick task         -> Action::Tick
//!   4. fetch tasks       -> Action::CandlesLoaded / FundamentalsLoaded / FetchFailed
//!   5. MCP socket        -> the semantic variants below (AddSymbol, OpenDetail, ...)
//!
//! The loop cannot tell a keystroke apart from an MCP command — which is exactly
//! why MCP control costs almost nothing to add. Claude never sends `Key`; it sends
//! the same semantic Actions that `:add` produces.

use crossterm::event::{KeyEvent, MouseEvent};
use serde::{Deserialize, Serialize};

use crate::app::{AssetFilter, FocusRegion, SortKey, View};
use crate::data::types::{Candle, Fundamentals, Indicators, MarketStatus, Quote, Timeframe};

#[derive(Debug, Clone)]
pub enum Action {
    // ---- Raw input (input task only) -----------------------------------
    // The input task is dumb: it forwards raw keys. The event loop interprets
    // them against the current InputMode and may dispatch a semantic Action.
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),

    // ---- Navigation ----------------------------------------------------
    SetView(View),
    /// Move keyboard focus between the watchlist table and the chart grid (`h`/`l`).
    SetFocus(FocusRegion),
    /// Open the drilldown for a symbol. From a table row (`d` / Enter) or MCP.
    OpenDetail(String),
    /// Esc: Detail/Settings -> Overview.
    Back,

    // ---- Watchlist -----------------------------------------------------
    AddSymbol(String),
    RemoveSymbol(String),
    SetFilter(AssetFilter),
    SetSort(SortKey),
    ToggleSortDirection,
    SelectRow(usize),
    /// j/k and arrows. Scrolls the table when it hits an edge.
    MoveSelection(i32),
    JumpTop,
    JumpBottom,

    // ---- Chart grid ----------------------------------------------------
    /// Fill the first empty slot; if all 4 are full, replace the FOCUSED slot
    /// and advance focus. Predictable and aimable, unlike silent FIFO.
    AddToSlot(String),
    ClearSlot(usize),
    ClearAllSlots,
    FocusSlot(usize),
    CycleSlotFocus,
    /// Global across all panes and Detail (per-pane is a follow-up).
    SetTimeframe(Timeframe),

    // ---- Data (from fetch tasks — the async results coming home) --------
    QuotesUpdated(Vec<Quote>),
    CandlesLoaded {
        symbol: String,
        timeframe: Timeframe,
        candles: Vec<Candle>,
        indicators: Indicators,
    },
    FundamentalsLoaded {
        symbol: String,
        data: Fundamentals,
    },
    MarketStatusUpdated(MarketStatus),
    FetchFailed {
        symbol: String,
        error: String,
    },
    /// Manual `r`. Bypasses cache TTL.
    Refresh,
    RefreshCompleted,

    // ---- Input modes / bottom bar --------------------------------------
    // Row 2 of the bottom region is the keyhint strip in Normal mode and
    // becomes a `:` command line in Command mode. Same cells, two states.
    EnterCommandMode,
    EnterSearchMode,
    ExitInputMode,
    InputChar(char),
    InputBackspace,
    /// Enter: parse the buffer and dispatch the resulting Action(s).
    SubmitInput,

    // ---- Overlays ------------------------------------------------------
    ToggleHelp,

    // ---- Misc ----------------------------------------------------------
    /// `:export [path]` — writes the current view as JSON. Same serialization
    /// the `--json` CLI and the MCP tools use.
    Export {
        path: Option<String>,
    },
    /// Transient message in the status row (errors, confirmations).
    SetStatus(String),
    /// Periodic redraw. Drives the marquee and the clock. Note this means the
    /// app is never fully idle while ticker_scroll is on.
    Tick,
    Quit,
}

/// The subset of `Action` that arrives over the MCP Unix socket.
/// Deliberately excludes raw input and data-result variants — Claude issues
/// intent, not keystrokes, and cannot inject fabricated market data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RemoteAction {
    AddSymbol { symbol: String },
    RemoveSymbol { symbol: String },
    ChartSymbol { symbol: String },
    ClearSlot { slot: Option<usize> },
    OpenDetail { symbol: String },
    SetTimeframe { timeframe: Timeframe },
    Refresh,
}

impl From<RemoteAction> for Action {
    fn from(r: RemoteAction) -> Action {
        match r {
            RemoteAction::AddSymbol { symbol } => Action::AddSymbol(symbol),
            RemoteAction::RemoveSymbol { symbol } => Action::RemoveSymbol(symbol),
            RemoteAction::ChartSymbol { symbol } => Action::AddToSlot(symbol),
            RemoteAction::ClearSlot { slot } => match slot {
                Some(i) => Action::ClearSlot(i),
                None => Action::ClearAllSlots,
            },
            RemoteAction::OpenDetail { symbol } => Action::OpenDetail(symbol),
            RemoteAction::SetTimeframe { timeframe } => Action::SetTimeframe(timeframe),
            RemoteAction::Refresh => Action::Refresh,
        }
    }
}
