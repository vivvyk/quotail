//! MCP server (`quotail --mcp`). Read tools — Step 1.
//!
//! This is the whole differentiator: Quotail has no AI inside it, so analysis
//! happens by attaching Claude to THIS server. The tools are deliberately thin —
//! deserialize args, call `DataStore`, serialize the result as JSON text.
//!
//! IMPORTANT: this process has its OWN `DataStore` and its OWN in-memory cache.
//! It does NOT share the running TUI's cache — the read tools work whether or not
//! a TUI is running, straight from `config.toml` + a fresh Yahoo fetch. Refetching
//! is cheap (the batch quote is one HTTP call), so nobody should later assume a
//! shared cache.
//!
//! Layering: `mcp/` depends on `data/` and `config` only. Session tools that DRIVE
//! a live TUI over the Unix socket come in Steps 2–3; nothing here needs a socket.
//!
//! stdio contract: stdout is the JSON-RPC channel. Nothing in this path may print
//! to stdout — tool payloads travel as protocol messages, and errors go to stderr.

use std::sync::Arc;

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;

use crate::data::store::DataStore;
use crate::data::types::{AssetKind, Timeframe};

/// The read-only market-data server. Holds only the store; the watchlist is read
/// fresh from `config.toml` per call so it tracks `:add`/`:rm` the TUI persisted.
#[derive(Clone)]
pub struct QuotailServer {
    store: Arc<DataStore>,
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

/// Watchlist membership row: symbol plus the asset kind it's classified as.
#[derive(serde::Serialize)]
struct WatchlistEntry {
    symbol: String,
    kind: AssetKind,
}

#[tool_router]
impl QuotailServer {
    pub fn new(store: Arc<DataStore>) -> Self {
        Self {
            store,
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
        let status = self.store.market_status().await.map_err(|e| e.to_string())?;
        json_result(&status)
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
                ^GSPC for indices."
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
    Ok(crate::config::load().map_err(|e| e.to_string())?.watchlist())
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

/// Entry point for `quotail --mcp`: serve the read tools over stdio until the
/// client disconnects. The server owns `store` (its own cache, separate from any
/// running TUI).
pub async fn run(store: Arc<DataStore>) -> anyhow::Result<()> {
    let service = QuotailServer::new(store).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
