//! Domain types. Provider-agnostic — this is the vocabulary the whole app speaks,
//! and the exact shape that `--json` and the MCP tools serialize.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetKind {
    Stock,
    Crypto,
    Index,
}

impl AssetKind {
    /// Yahoo convention: `^GSPC` is an index, `BTC-USD` is crypto, else a stock.
    pub fn infer(symbol: &str) -> AssetKind {
        if symbol.starts_with('^') {
            AssetKind::Index
        } else if symbol.ends_with("-USD") {
            AssetKind::Crypto
        } else {
            AssetKind::Stock
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    /// Long name, e.g. "Apple Inc." Absent for some indices.
    pub name: Option<String>,
    pub price: f64,
    pub change: f64,
    pub change_pct: f64,
    pub prev_close: f64,
    pub open: Option<f64>,
    pub day_range: (f64, f64),
    pub week52_range: (f64, f64),
    pub volume: Option<f64>,
    pub avg_volume: Option<f64>,
    pub asset: AssetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    /// Unix seconds.
    pub ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Every field is optional on purpose: crypto has no P/E, no EPS, no dividend.
/// The absence is real domain information, not a missing value to paper over.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Fundamentals {
    pub market_cap: Option<f64>,
    pub pe_trailing: Option<f64>,
    pub pe_forward: Option<f64>,
    pub eps_trailing: Option<f64>,
    pub div_yield: Option<f64>,
    pub beta: Option<f64>,
}

/// Computed indicators. Aligned index-for-index with the candle series;
/// leading entries are `None` until the window fills.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Indicators {
    pub rsi: Vec<Option<f64>>,
    pub ma50: Vec<Option<f64>>,
    pub ma200: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Timeframe {
    D1,
    D5,
    M1,
    M6,
    Ytd,
    Y1,
    Max,
}

impl Timeframe {
    /// Order shown in the UI; also the `1`..`7` hotkey order.
    pub const ALL: [Timeframe; 7] = [
        Timeframe::D1,
        Timeframe::D5,
        Timeframe::M1,
        Timeframe::M6,
        Timeframe::Ytd,
        Timeframe::Y1,
        Timeframe::Max,
    ];

    /// Label shown in the status row, e.g. `[1M]`.
    pub fn label(&self) -> &'static str {
        match self {
            Timeframe::D1 => "1D",
            Timeframe::D5 => "5D",
            Timeframe::M1 => "1M",
            Timeframe::M6 => "6M",
            Timeframe::Ytd => "YTD",
            Timeframe::Y1 => "1Y",
            Timeframe::Max => "MAX",
        }
    }

    /// Yahoo `range` parameter.
    pub fn range_param(&self) -> &'static str {
        match self {
            Timeframe::D1 => "1d",
            Timeframe::D5 => "5d",
            Timeframe::M1 => "1mo",
            Timeframe::M6 => "6mo",
            Timeframe::Ytd => "ytd",
            Timeframe::Y1 => "1y",
            Timeframe::Max => "max",
        }
    }

    /// Yahoo `interval` parameter. Chosen so a request never returns
    /// thousands of candles for a pane that can show ~52.
    pub fn interval_param(&self) -> &'static str {
        match self {
            Timeframe::D1 => "5m",
            Timeframe::D5 => "30m",
            Timeframe::M1 | Timeframe::M6 | Timeframe::Ytd | Timeframe::Y1 => "1d",
            Timeframe::Max => "1wk",
        }
    }

    /// Cache TTL. Historical daily candles barely change; intraday does.
    pub fn ttl_secs(&self) -> u64 {
        match self {
            Timeframe::D1 | Timeframe::D5 => 60,
            _ => 3600,
        }
    }

    pub fn parse(s: &str) -> Option<Timeframe> {
        match s.to_ascii_uppercase().as_str() {
            "1D" => Some(Timeframe::D1),
            "5D" => Some(Timeframe::D5),
            "1M" => Some(Timeframe::M1),
            "6M" => Some(Timeframe::M6),
            "YTD" => Some(Timeframe::Ytd),
            "1Y" => Some(Timeframe::Y1),
            "MAX" => Some(Timeframe::Max),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketStatus {
    Open,
    Closed,
    PreMarket,
    AfterHours,
}
