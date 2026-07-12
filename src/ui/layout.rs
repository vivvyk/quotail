//! Layout constants. These encode the visual contract in `docs/ASCII_REFERENCE.md`.
//!
//! Only the truly FIXED dimensions live here. Everything that stretches with the
//! terminal — pane widths/heights and the candle-column counts that follow from
//! them — is DERIVED at render time from `frame.area()`, not stored as a constant.
//! At 96x31 the derived values reduce to the historical ones (pane 28x13, 24
//! candles; detail main 64 wide, 52 candles), which is what the snapshots pin.

/// Reference terminal size used by the snapshot tests.
pub const REF_WIDTH: u16 = 96;
pub const REF_HEIGHT: u16 = 31;

/// Below this the layout can't hold together, so we show a "too small" notice
/// instead of attempting to render (prevents a panic / unreadable mess).
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;

// ---- Overview ----------------------------------------------------------
/// Hot-movers banner, including its top and bottom borders.
pub const BANNER_HEIGHT: u16 = 3;
/// Watchlist table panel width — FIXED (its content is fixed-column and gains
/// nothing from extra width). The chart grid absorbs all horizontal slack.
pub const WATCHLIST_WIDTH: u16 = 40;
/// Hard cap on chart panes. Enforced by the type: `[Option<ChartSlot>; MAX_SLOTS]`.
pub const MAX_SLOTS: usize = 4;

// Watchlist table column widths (sum + borders == WATCHLIST_WIDTH).
pub const COL_TICKER: usize = 9;
pub const COL_PRICE: usize = 12;
pub const COL_CHANGE_PCT: usize = 9;

// ---- Detail ------------------------------------------------------------
/// Right rail (fundamentals) width — FIXED, like the watchlist. The main chart
/// takes the rest (`W - DETAIL_RAIL_WIDTH`).
pub const DETAIL_RAIL_WIDTH: u16 = 32;
/// Price-axis gutter on the right of the detail chart. The candle-column count is
/// `main_width - 4 - DETAIL_AXIS_WIDTH`.
pub const DETAIL_AXIS_WIDTH: usize = 8;
/// Fixed-height strips (body rows, excluding their two borders).
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
