//! Layout constants. These encode the visual contract in `docs/ASCII_REFERENCE.md`.
//! Do not change these values — the snapshot tests assert against them.

/// Reference terminal width used by snapshot tests.
pub const REF_WIDTH: u16 = 96;
/// Reference terminal height used by snapshot tests.
pub const REF_HEIGHT: u16 = 31;

// ---- Overview ----------------------------------------------------------
/// Hot-movers banner, including its top and bottom borders.
pub const BANNER_HEIGHT: u16 = 3;
/// Watchlist table panel width.
pub const WATCHLIST_WIDTH: u16 = 40;
/// Chart grid width (2 panes across).
pub const GRID_WIDTH: u16 = 56;
/// A single chart pane in the 2x2 grid.
pub const PANE_WIDTH: u16 = 28;
pub const PANE_HEIGHT: u16 = 11;
/// Candle columns inside a grid pane (PANE_WIDTH - 2 border - 2 padding).
pub const PANE_CANDLES: usize = 24;
/// Chart body rows inside a grid pane.
pub const PANE_CHART_ROWS: usize = 9;
/// Hard cap on chart panes. Enforced by the type: `[Option<ChartSlot>; MAX_SLOTS]`.
pub const MAX_SLOTS: usize = 4;

// Watchlist table column widths (sum + borders == WATCHLIST_WIDTH).
pub const COL_TICKER: usize = 9;
pub const COL_PRICE: usize = 12;
pub const COL_CHANGE_PCT: usize = 9;

// ---- Detail ------------------------------------------------------------
/// Left column (chart + volume + rsi).
pub const DETAIL_MAIN_WIDTH: u16 = 64;
/// Right rail (fundamentals).
pub const DETAIL_RAIL_WIDTH: u16 = 32;
/// Candle columns in the detail chart.
pub const DETAIL_CANDLES: usize = 52;
/// Price-axis gutter on the right of the detail chart.
pub const DETAIL_AXIS_WIDTH: usize = 8;
/// Body rows: main chart, volume histogram, rsi pane.
pub const DETAIL_CHART_ROWS: usize = 14;
pub const DETAIL_VOLUME_ROWS: usize = 3;
pub const DETAIL_RSI_ROWS: usize = 4;
/// Fundamentals rail: label column then right-aligned value column.
pub const RAIL_LABEL_WIDTH: usize = 15;
pub const RAIL_VALUE_WIDTH: usize = 13;

// ---- Help overlay ------------------------------------------------------
pub const HELP_WIDTH: u16 = 76;
pub const HELP_HEIGHT: u16 = 24;

// ---- Bottom region -----------------------------------------------------
/// Row 1 = passive state, row 2 = keyhints (becomes the `:` command line).
pub const BOTTOM_HEIGHT: u16 = 2;

// ---- Indicators --------------------------------------------------------
// Indicator periods & RSI levels live in their domain owner, data/indicators.rs
// (single source of truth). `ui/` imports them from there — `ui/` may depend on
// `data/`; the ban is one-directional.

// ---- Ticker marquee ----------------------------------------------------
/// Symbols shown in the scrolling hot-movers tape.
pub const HOT_MOVERS_COUNT: usize = 10;
/// Gap in spaces between marquee entries.
pub const MARQUEE_GAP: usize = 6;
/// Advance one cell per this interval. Also the redraw tick when scrolling.
pub const MARQUEE_TICK_MS: u64 = 140;
