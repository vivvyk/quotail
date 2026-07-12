//! Domain types. Provider-agnostic — this is the vocabulary the whole app speaks,
//! and the exact shape that `--json` and the MCP tools serialize.

use chrono::Duration;
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
    /// From Yahoo's batch quote response. `None` for indices (no market cap).
    /// Drives the MarketCap table sort; `Fundamentals` keeps its own copy for
    /// the Detail rail.
    pub market_cap: Option<f64>,
    /// Short exchange label ("NYSE", "NASDAQ", …) for the Detail status line.
    /// `None` for many indices/crypto.
    pub exchange: Option<String>,
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

    /// Yahoo `range` to FETCH. `min_bars` is the warmup bars required BEFORE the
    /// visible window; callers pass `MA_LONG`, so the 200 is not hardcoded here.
    /// Returns the smallest range covering `min_bars + visible_bars(self)` native
    /// bars at this timeframe's interval, so MA200 is defined across the whole
    /// visible window (later sliced by timestamp; see `display_span`). If nothing
    /// covers it, returns the widest range — the `fetch_range_*` tests then FAIL,
    /// surfacing that the warmup outgrew what the source can supply.
    pub fn fetch_range_param(&self, min_bars: usize) -> &'static str {
        let required = min_bars + self.visible_bars();
        let ladder = self.range_ladder();
        ladder
            .iter()
            .find(|(_, est)| *est >= required)
            .or_else(|| ladder.last())
            .map(|(range, _)| *range)
            .expect("range_ladder is never empty")
    }

    /// Estimated NATIVE bars in the visible window — for FETCH SIZING ONLY (the
    /// real visible slice is by timestamp; holidays make counts inexact). An
    /// over-estimate is safe: it only widens the fetch.
    fn visible_bars(&self) -> usize {
        match self {
            Timeframe::D1 => 78,   // ~1 session of 5m bars
            Timeframe::D5 => 65,   // ~5 sessions of 30m bars
            Timeframe::M1 => 21,   // ~1 month of trading days
            Timeframe::M6 => 126,  // ~6 months of trading days
            Timeframe::Ytd => 252, // worst case: a full year of trading days
            Timeframe::Y1 => 252,  // ~1 year of trading days
            Timeframe::Max => 0,   // "max" already spans everything
        }
    }

    /// Yahoo ranges valid at this timeframe's native interval, ASCENDING, each
    /// paired with an estimate of the native bars it yields. `fetch_range_param`
    /// takes the first that covers the requirement. Kept within Yahoo's
    /// per-interval history caps (5m / 30m history ≤ ~60 days).
    fn range_ladder(&self) -> &'static [(&'static str, usize)] {
        match self.interval_param() {
            "5m" => &[("1d", 78), ("5d", 390)],
            "30m" => &[("5d", 65), ("1mo", 286)],
            "1d" => &[
                ("1mo", 21),
                ("3mo", 63),
                ("6mo", 126),
                ("1y", 252),
                ("2y", 504),
            ],
            _ => &[("max", usize::MAX)], // 1wk (the MAX timeframe)
        }
    }

    /// The VISIBLE window this timeframe shows. The chart slices candles by
    /// timestamp against this (never by bar count — holidays make counts wrong),
    /// then buckets the slice into the pane's column width. YTD and MAX are not
    /// fixed-length durations, so they are their own variants.
    pub fn display_span(&self) -> DisplaySpan {
        match self {
            Timeframe::D1 => DisplaySpan::Last(Duration::days(1)),
            Timeframe::D5 => DisplaySpan::Last(Duration::days(5)),
            Timeframe::M1 => DisplaySpan::Last(Duration::days(30)),
            Timeframe::M6 => DisplaySpan::Last(Duration::days(182)),
            Timeframe::Ytd => DisplaySpan::Ytd,
            Timeframe::Y1 => DisplaySpan::Last(Duration::days(365)),
            Timeframe::Max => DisplaySpan::Max,
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

/// The visible time window of a chart, resolved against "now" by the renderer.
/// Kept separate from `Timeframe` because YTD/MAX depend on the current date and
/// on the available data, so they can't be a fixed `Duration`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplaySpan {
    /// Visible window is `[now - duration, now]`.
    Last(Duration),
    /// Year-to-date: `[Jan 1 of the current year, now]`.
    Ytd,
    /// Everything available.
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketStatus {
    Open,
    Closed,
    PreMarket,
    AfterHours,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::indicators::MA_LONG;

    /// The test's OWN estimate of native bars a (interval, range) yields —
    /// independent of `Timeframe::range_ladder`. If that ladder is edited to
    /// pick a too-narrow range, or `MA_LONG` grows past what a range can supply,
    /// the coverage assertion below trips here rather than silently shipping an
    /// all-`None` moving average. Panics on any (interval, range) it doesn't
    /// model, so a new range choice can't slip through unasserted.
    fn est_bars(interval: &str, range: &str) -> usize {
        match (interval, range) {
            ("5m", "1d") => 78,
            ("5m", "5d") => 390,
            ("30m", "5d") => 65,
            ("30m", "1mo") => 286,
            ("1d", "1mo") => 21,
            ("1d", "3mo") => 63,
            ("1d", "6mo") => 126,
            ("1d", "1y") => 252,
            ("1d", "2y") => 504,
            ("1wk", "max") => usize::MAX,
            other => panic!("unmodeled (interval, range) = {other:?}"),
        }
    }

    /// The test's OWN estimate of native bars in each timeframe's visible span.
    fn visible_bars(tf: Timeframe) -> usize {
        match tf {
            Timeframe::D1 => 78,
            Timeframe::D5 => 65,
            Timeframe::M1 => 21,
            Timeframe::M6 => 126,
            Timeframe::Ytd => 252,
            Timeframe::Y1 => 252,
            Timeframe::Max => 0,
        }
    }

    #[test]
    fn fetch_range_covers_warmup_plus_visible_for_every_timeframe() {
        for &tf in &Timeframe::ALL {
            let range = tf.fetch_range_param(MA_LONG);
            let fetched = est_bars(tf.interval_param(), range);
            let required = MA_LONG + visible_bars(tf);
            assert!(
                fetched >= required,
                "{tf:?}: range {range:?} yields ~{fetched} bars but needs >= {required} \
                 (warmup {MA_LONG} + visible {})",
                visible_bars(tf),
            );
        }
    }

    #[test]
    fn d5_is_the_tight_one_margin_is_real() {
        // 30m interval: "1mo" yields ~286 bars; warmup 200 + visible ~65 = 265.
        // A margin of ~21 bars — assert it, don't assume it.
        let range = Timeframe::D5.fetch_range_param(MA_LONG);
        assert_eq!(range, "1mo", "D5 range choice changed");
        let fetched = est_bars("30m", range);
        let required = MA_LONG + visible_bars(Timeframe::D5);
        assert!(
            fetched >= required,
            "D5 no longer covered: {fetched} < {required}"
        );
        assert!(
            fetched - required <= 40,
            "D5 margin unexpectedly large ({} bars) — is the range wider than needed?",
            fetched - required,
        );
    }
}
