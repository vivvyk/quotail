//! MCP server (`quotail --mcp`). Read tools (Step 1) + session control (Step 3a).
//!
//! This is the whole differentiator: Quotail has no AI inside it, so analysis
//! happens by attaching Claude to THIS server. The tools are deliberately thin —
//! deserialize args, call `DataStore` (or the socket), serialize the result.
//!
//! Two capability tiers, with graceful degradation:
//!   - READ tools serve straight from this process's OWN `DataStore` + cache (NOT
//!     the running TUI's — they're separate; refetching is cheap, one batch HTTP
//!     call). They work whether or not a TUI is running, from `config.toml`.
//!   - SESSION-CONTROL tools drive a running TUI by writing a `RemoteAction` line
//!     to its Unix socket (`crate::ipc`). No new command logic — the event loop
//!     can't tell it from a keystroke. No TUI running ⇒ a clear, actionable error;
//!     the read tools are unaffected. `get_session` (the read-back) is Step 3b.
//!
//! Layering: `mcp/` depends on `data/`, `config`, and `ipc` (the socket client).
//!
//! stdio contract: stdout is the JSON-RPC channel. Nothing in this path may print
//! to stdout — tool payloads travel as protocol messages, and errors go to stderr.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;

use crate::action::RemoteAction;
use crate::data::store::DataStore;
use crate::data::types::{AssetKind, Timeframe};
use crate::ui::layout::MAX_SLOTS;

/// The market-data + session-control server. Read tools hit `store` directly (its
/// own cache); session tools drive a running TUI over `socket_path`. The watchlist
/// is read fresh from `config.toml` per call so it tracks `:add`/`:rm` the TUI
/// persisted; `socket_path` is stable for the process, so it's resolved once.
#[derive(Clone)]
pub struct QuotailServer {
    store: Arc<DataStore>,
    socket_path: PathBuf,
    tool_router: ToolRouter<Self>,
}

// ---- tool argument schemas (local to mcp/, so deriving JsonSchema here doesn't
//      leak schemars into the data layer) --------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SymbolsArgs {
    /// Ticker symbols in Yahoo convention (e.g. `AAPL`, `BTC-USD`, `^GSPC`).
    symbols: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SymbolArg {
    /// A single ticker in Yahoo convention (e.g. `AAPL`, `BTC-USD`, `^GSPC`).
    symbol: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ChartArgs {
    /// A single ticker in Yahoo convention (e.g. `AAPL`, `BTC-USD`).
    symbol: String,
    /// One of: `1D`, `5D`, `1M`, `6M`, `YTD`, `1Y`, `MAX`.
    timeframe: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct TimeframeArg {
    /// Human timeframe label — one of `1D`, `5D`, `1M`, `6M`, `YTD`, `1Y`, `MAX`.
    timeframe: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ClearSlotArg {
    /// Pane index `0`–`3` to clear one chart; omit to clear ALL four panes.
    slot: Option<usize>,
}

/// Watchlist membership row: symbol plus the asset kind it's classified as.
#[derive(serde::Serialize)]
struct WatchlistEntry {
    symbol: String,
    kind: AssetKind,
}

#[tool_router]
impl QuotailServer {
    pub fn new(store: Arc<DataStore>, socket_path: PathBuf) -> Self {
        Self {
            store,
            socket_path,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "List the symbols in the user's own watchlist, each with its \
        asset kind (stock/crypto/index). Use this when you only need the membership \
        list; use get_watchlist_quotes when you need live prices. Reads the user's \
        config.toml, so it reflects the current watchlist."
    )]
    async fn get_watchlist(&self) -> Result<CallToolResult, String> {
        let entries: Vec<WatchlistEntry> = watchlist_symbols()?
            .into_iter()
            .map(|s| WatchlistEntry {
                kind: AssetKind::infer(&s),
                symbol: s,
            })
            .collect();
        json_result(&entries)
    }

    #[tool(
        description = "THE tool for 'how is my watchlist/portfolio doing?'. Returns \
        full live quotes — price, absolute and percent change, previous close, day \
        range, 52-week range, volume, and market cap — for EVERY symbol in the \
        user's own watchlist, in one batched request. Call this first for any \
        portfolio-level question: you do not otherwise know what the user holds. \
        Reads the watchlist from config.toml (stays current with TUI edits)."
    )]
    async fn get_watchlist_quotes(&self) -> Result<CallToolResult, String> {
        let symbols = watchlist_symbols()?;
        let quotes = self
            .store
            .quotes(&symbols, false)
            .await
            .map_err(|e| e.to_string())?;
        json_result(&quotes)
    }

    #[tool(
        description = "Live quotes for arbitrary symbols (not necessarily in the \
        watchlist), fetched as ONE batched request. Returns the same fields as \
        get_watchlist_quotes. Use for ad-hoc lookups like 'what's TSLA at?'."
    )]
    async fn get_quote(
        &self,
        Parameters(args): Parameters<SymbolsArgs>,
    ) -> Result<CallToolResult, String> {
        let symbols = normalize_symbols(args.symbols);
        let quotes = self
            .store
            .quotes(&symbols, false)
            .await
            .map_err(|e| e.to_string())?;
        json_result(&quotes)
    }

    #[tool(
        description = "OHLCV candle history for one symbol over a timeframe, at the \
        provider's native interval. Timeframe must be one of 1D, 5D, 1M, 6M, YTD, \
        1Y, MAX. Each candle has ts (unix seconds), open, high, low, close, volume. \
        Use for price-action / charting questions. For RSI and moving averages call \
        get_indicators instead."
    )]
    async fn get_candles(
        &self,
        Parameters(args): Parameters<ChartArgs>,
    ) -> Result<CallToolResult, String> {
        let tf = parse_timeframe(&args.timeframe)?;
        let data = self
            .store
            .candles(&args.symbol.to_uppercase(), tf, false)
            .await
            .map_err(|e| e.to_string())?;
        json_result(&data.candles)
    }

    #[tool(
        description = "Technical indicators for one symbol over a timeframe: RSI, \
        MA50, and MA200, each an array aligned index-for-index with the candle \
        series (leading entries are null until the window fills). Timeframe must be \
        one of 1D, 5D, 1M, 6M, YTD, 1Y, MAX. Use for technical-analysis questions \
        (overbought/oversold, trend, golden/death cross)."
    )]
    async fn get_indicators(
        &self,
        Parameters(args): Parameters<ChartArgs>,
    ) -> Result<CallToolResult, String> {
        let tf = parse_timeframe(&args.timeframe)?;
        let data = self
            .store
            .candles(&args.symbol.to_uppercase(), tf, false)
            .await
            .map_err(|e| e.to_string())?;
        json_result(&data.indicators)
    }

    #[tool(
        description = "Fundamentals for one symbol: market cap, trailing and forward \
        P/E, trailing EPS, dividend yield, and beta. Fields are null when they don't \
        apply (crypto and indices have no P/E or EPS). Use for valuation questions."
    )]
    async fn get_fundamentals(
        &self,
        Parameters(args): Parameters<SymbolArg>,
    ) -> Result<CallToolResult, String> {
        let data = self
            .store
            .fundamentals(&args.symbol.to_uppercase(), false)
            .await
            .map_err(|e| e.to_string())?;
        json_result(&data)
    }

    #[tool(
        description = "Whether the US equity market is currently open or closed. Use \
        to caveat quote freshness (prices are last-trade when closed)."
    )]
    async fn get_market_status(&self) -> Result<CallToolResult, String> {
        let status = self
            .store
            .market_status()
            .await
            .map_err(|e| e.to_string())?;
        json_result(&status)
    }

    // ---- session control (drive a running TUI over the socket) --------------
    // Each is a one-way RemoteAction; there is NO new command logic — the TUI's
    // event loop treats it exactly like the equivalent keystroke. All require a
    // running TUI and return a clear error if none is (the read tools above do
    // not — they work regardless).

    #[tool(
        description = "Add a symbol to the RUNNING TUI's watchlist (also persisted to \
        config.toml). Requires a running Quotail TUI; errors clearly if none is up. \
        Symbol in Yahoo convention (e.g. AAPL, BTC-USD, ^GSPC)."
    )]
    async fn add_symbol(
        &self,
        Parameters(args): Parameters<SymbolArg>,
    ) -> Result<CallToolResult, String> {
        let symbol = args.symbol.trim().to_uppercase();
        self.drive(
            RemoteAction::AddSymbol {
                symbol: symbol.clone(),
            },
            format!("added {symbol} to the watchlist"),
        )
        .await
    }

    #[tool(
        description = "Remove a symbol from the RUNNING TUI's watchlist (also updates \
        config.toml). Requires a running Quotail TUI. Symbol in Yahoo convention."
    )]
    async fn remove_symbol(
        &self,
        Parameters(args): Parameters<SymbolArg>,
    ) -> Result<CallToolResult, String> {
        let symbol = args.symbol.trim().to_uppercase();
        self.drive(
            RemoteAction::RemoveSymbol {
                symbol: symbol.clone(),
            },
            format!("removed {symbol} from the watchlist"),
        )
        .await
    }

    #[tool(
        description = "Open a chart for a symbol in the RUNNING TUI's 2x2 grid: fills \
        the first empty pane, or replaces the focused pane when all four are full. \
        Use this to 'show/chart/pull up' a symbol. Requires a running Quotail TUI. \
        Symbol in Yahoo convention."
    )]
    async fn chart_symbol(
        &self,
        Parameters(args): Parameters<SymbolArg>,
    ) -> Result<CallToolResult, String> {
        let symbol = args.symbol.trim().to_uppercase();
        self.drive(
            RemoteAction::ChartSymbol {
                symbol: symbol.clone(),
            },
            format!("charting {symbol} in the grid"),
        )
        .await
    }

    #[tool(
        description = "Clear chart panes in the RUNNING TUI. Pass slot 0-3 to clear \
        one pane; omit slot to clear all four. Requires a running Quotail TUI."
    )]
    async fn clear_slot(
        &self,
        Parameters(args): Parameters<ClearSlotArg>,
    ) -> Result<CallToolResult, String> {
        if let Some(n) = args.slot
            && n >= MAX_SLOTS
        {
            return Err(format!("slot must be 0-{}, got {n}", MAX_SLOTS - 1));
        }
        let msg = match args.slot {
            Some(n) => format!("cleared pane {n}"),
            None => "cleared all panes".to_string(),
        };
        self.drive(RemoteAction::ClearSlot { slot: args.slot }, msg)
            .await
    }

    #[tool(
        description = "Open the full-screen detail drilldown for a symbol in the \
        RUNNING TUI (large chart, volume, RSI, fundamentals rail). Requires a running \
        Quotail TUI. Symbol in Yahoo convention."
    )]
    async fn open_detail(
        &self,
        Parameters(args): Parameters<SymbolArg>,
    ) -> Result<CallToolResult, String> {
        let symbol = args.symbol.trim().to_uppercase();
        self.drive(
            RemoteAction::OpenDetail {
                symbol: symbol.clone(),
            },
            format!("opened detail for {symbol}"),
        )
        .await
    }

    #[tool(
        description = "Set the global chart timeframe in the RUNNING TUI (applies to \
        the grid and detail). Pass a human label: 1D, 5D, 1M, 6M, YTD, 1Y, or MAX. \
        Requires a running Quotail TUI."
    )]
    async fn set_timeframe(
        &self,
        Parameters(args): Parameters<TimeframeArg>,
    ) -> Result<CallToolResult, String> {
        // The wire form of Timeframe is its serde name (Y1, YTD, …), NOT the human
        // label (1Y). Parse the label here so Claude passes what a user would say.
        let tf = parse_timeframe(&args.timeframe)?;
        self.drive(
            RemoteAction::SetTimeframe { timeframe: tf },
            format!("timeframe set to {}", tf.label()),
        )
        .await
    }

    #[tool(
        description = "Read the RUNNING TUI's current session state: the active view \
        (overview/detail/settings), the four chart panes (symbols, in order), which \
        pane is focused, the global timeframe (as a label like 1Y), the watchlist \
        filter, the selected watchlist symbol, and the detail symbol if any. Call \
        this to SEE what the user is currently looking at before acting — e.g. to \
        target 'the focused pane' or 'the chart showing NVDA', or to confirm a \
        command took effect. Requires a running Quotail TUI."
    )]
    async fn get_session(&self) -> Result<CallToolResult, String> {
        self.session().await
    }
}

#[tool_handler]
impl ServerHandler for QuotailServer {
    fn get_info(&self) -> ServerInfo {
        // `Implementation::from_build_env()` bakes in rmcp's OWN crate name/version
        // (its `env!` expands at rmcp's compile time), so build ours explicitly —
        // `env!` here expands in the quotail crate.
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("quotail", env!("CARGO_PKG_VERSION")))
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "Quotail serves live market data for the user's OWN watchlist plus \
                any symbol, read from Yahoo. You have no other way to know what the \
                user holds — for any 'how is my watchlist/portfolio doing?' question, \
                call get_watchlist_quotes first. Other tools: get_watchlist (just the \
                membership list), get_quote (arbitrary symbols, batched), get_candles \
                and get_indicators (OHLCV history and RSI/MA50/MA200 over a timeframe \
                in {1D,5D,1M,6M,YTD,1Y,MAX}), get_fundamentals (valuation), and \
                get_market_status. Symbols use Yahoo conventions: BTC-USD for crypto, \
                ^GSPC for indices. If a Quotail TUI is running you can also DRIVE it: \
                add_symbol, remove_symbol, chart_symbol (open a chart in the grid), \
                open_detail, clear_slot, and set_timeframe (pass a human label like \
                1Y), and READ its live state with get_session (open charts, focused \
                pane, timeframe, selection) — call that to see what the user is \
                looking at before acting. Those require a running TUI and error \
                clearly if none is up; the read tools work either way."
                    .to_string(),
            )
    }
}

impl QuotailServer {
    /// Deliver a one-way `RemoteAction` to the running TUI over the socket, returning
    /// a confirmation on success or a clear, actionable error when no TUI is running.
    /// One-shot write, no reply to await — so it can't hang.
    #[cfg(unix)]
    async fn drive(&self, action: RemoteAction, ok_msg: String) -> Result<CallToolResult, String> {
        crate::ipc::send_remote(&self.socket_path, &action).await?;
        Ok(CallToolResult::success(vec![ContentBlock::text(ok_msg)]))
    }

    /// Non-unix builds have no session socket: session tools fail cleanly rather
    /// than pretend to work. (The read tools remain fully functional.)
    #[cfg(not(unix))]
    async fn drive(
        &self,
        _action: RemoteAction,
        _ok_msg: String,
    ) -> Result<CallToolResult, String> {
        Err(
            "controlling a running TUI requires a Unix socket, which isn't available \
             on this platform"
                .to_string(),
        )
    }

    /// Read-back of the running TUI's session state. Round-trips a request over the
    /// socket with a timeout, so a wedged TUI errors instead of hanging the tool.
    #[cfg(unix)]
    async fn session(&self) -> Result<CallToolResult, String> {
        // 3s is orders of magnitude above the event loop's normal reply latency;
        // only a truly wedged loop would hit it, and then we want an error, not a
        // hang. The response is already JSON — relay it verbatim.
        let json = crate::ipc::query_session(&self.socket_path, Duration::from_secs(3)).await?;
        Ok(CallToolResult::success(vec![ContentBlock::text(json)]))
    }

    #[cfg(not(unix))]
    async fn session(&self) -> Result<CallToolResult, String> {
        Err(
            "reading a running TUI's session requires a Unix socket, which isn't \
             available on this platform"
                .to_string(),
        )
    }
}

// ---- helpers ---------------------------------------------------------------

/// Serialize any value as pretty JSON text wrapped in a tool result.
fn json_result<T: serde::Serialize>(value: &T) -> Result<CallToolResult, String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

/// The user's watchlist, read fresh from `config.toml` each call so it tracks
/// symbols the TUI `:add`ed and persisted.
fn watchlist_symbols() -> Result<Vec<String>, String> {
    Ok(crate::config::load()
        .map_err(|e| e.to_string())?
        .watchlist())
}

/// Uppercase + drop blanks. Yahoo symbols are uppercase; this mirrors what the
/// TUI does on `:add`, so an MCP caller and a keystroke resolve the same ticker.
fn normalize_symbols(symbols: Vec<String>) -> Vec<String> {
    symbols
        .into_iter()
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_timeframe(s: &str) -> Result<Timeframe, String> {
    Timeframe::parse(s)
        .ok_or_else(|| format!("invalid timeframe {s:?}; valid: 1D 5D 1M 6M YTD 1Y MAX"))
}

/// Entry point for `quotail --mcp`: serve the tools over stdio until the client
/// disconnects. The server owns `store` (its own cache, separate from any running
/// TUI) and the resolved session socket path (stable for the process; the read
/// tools re-read the watchlist per call, but the socket path never changes).
pub async fn run(store: Arc<DataStore>) -> anyhow::Result<()> {
    let socket_path = crate::config::load()?.socket_path();
    let service = QuotailServer::new(store, socket_path)
        .serve(stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
