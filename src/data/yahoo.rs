//! `YahooProvider`: the concrete `Provider` over Yahoo Finance's public JSON.
//!
//! DUMB by contract (see `provider.rs`): one fetch per call, no caching, no rate
//! limiting, no retries beyond the one crumb-refresh below. `DataStore` adds all
//! of that on top.
//!
//! Two endpoints, verified against the live API before this was written:
//! - v8 `/finance/chart/{sym}` — candles + a quote-like `meta`; needs no auth.
//! - v7 `/finance/quote` — batch quotes + fundamentals in ONE call, but gated on
//!   an `A3` cookie + a `crumb` token.

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::Mutex;

use super::provider::{Provider, ProviderError};
use super::types::{AssetKind, Candle, Fundamentals, MarketStatus, Quote, Timeframe};

const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) quotail";
const V7_QUOTE: &str = "https://query1.finance.yahoo.com/v7/finance/quote";
const V8_CHART: &str = "https://query1.finance.yahoo.com/v8/finance/chart";
const CRUMB_URL: &str = "https://query1.finance.yahoo.com/v1/test/getcrumb";
const COOKIE_URL: &str = "https://fc.yahoo.com/";

pub struct YahooProvider {
    client: reqwest::Client,
    /// The `crumb` token, fetched lazily and reused. A `tokio::sync::Mutex`
    /// (not `std::sync::Mutex`) because we hold the guard across `.await` while
    /// fetching it, and a std guard is `!Send` — it would make the future unable
    /// to move between tokio worker threads and fail to compile.
    crumb: Mutex<Option<String>>,
}

impl YahooProvider {
    pub fn new() -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .user_agent(UA)
            // The cookie store is what carries Yahoo's `A3` token from the
            // priming request to the crumb + quote requests automatically.
            .cookie_store(true)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| ProviderError::Unavailable(e.to_string()))?;
        Ok(Self {
            client,
            crumb: Mutex::new(None),
        })
    }

    /// Return a cached crumb, fetching one (and priming the cookie jar) if absent.
    async fn ensure_crumb(&self) -> Result<String, ProviderError> {
        let mut guard = self.crumb.lock().await;
        if let Some(c) = guard.as_ref() {
            return Ok(c.clone());
        }
        // `fc.yahoo.com` 404s but still Set-Cookies the A3 token; the cookie
        // store keeps it. We deliberately ignore the response status here.
        let _ = self.client.get(COOKIE_URL).send().await;
        let crumb = self
            .client
            .get(CRUMB_URL)
            .send()
            .await
            .map_err(net)?
            .text()
            .await
            .map_err(net)?;
        if crumb.is_empty() || crumb.contains('<') {
            // Empty, or an HTML error page instead of the bare token.
            return Err(ProviderError::Unavailable(
                "could not obtain Yahoo crumb".into(),
            ));
        }
        *guard = Some(crumb.clone());
        Ok(crumb)
    }

    /// Batch v7 quote fetch. Used by both `quotes` and `fundamentals` (and
    /// `market_status`) — one endpoint, one shape. Retries once if the crumb
    /// went stale (401), refreshing it in between.
    async fn fetch_raw_quotes(&self, symbols: &[String]) -> Result<Vec<RawQuote>, ProviderError> {
        let joined = symbols.join(",");
        for attempt in 0..2 {
            let crumb = self.ensure_crumb().await?;
            let resp = self
                .client
                .get(V7_QUOTE)
                // reqwest URL-encodes the crumb (it can contain `/` `+` `=`).
                .query(&[("symbols", joined.as_str()), ("crumb", crumb.as_str())])
                .send()
                .await
                .map_err(net)?;

            if resp.status().as_u16() == 401 && attempt == 0 {
                *self.crumb.lock().await = None; // stale crumb: drop it and retry
                continue;
            }
            if !resp.status().is_success() {
                return Err(ProviderError::Unavailable(format!(
                    "yahoo quote http {}",
                    resp.status()
                )));
            }
            let body: QuoteEnvelope = resp.json().await.map_err(parse)?;
            return Ok(body.quote_response.result);
        }
        Err(ProviderError::Unavailable(
            "yahoo quote: crumb refresh did not help".into(),
        ))
    }
}

#[async_trait]
impl Provider for YahooProvider {
    fn name(&self) -> &str {
        "yahoo"
    }

    async fn quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, ProviderError> {
        if symbols.is_empty() {
            return Ok(Vec::new());
        }
        let raw = self.fetch_raw_quotes(symbols).await?;
        Ok(raw.into_iter().map(map_quote).collect())
    }

    async fn candles(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        min_bars: usize,
    ) -> Result<Vec<Candle>, ProviderError> {
        let url = format!("{V8_CHART}/{symbol}");
        let resp = self
            .client
            .get(&url)
            .query(&[
                ("range", timeframe.fetch_range_param(min_bars)),
                ("interval", timeframe.interval_param()),
                ("includePrePost", "false"),
            ])
            .send()
            .await
            .map_err(net)?;
        if !resp.status().is_success() {
            return Err(ProviderError::Unavailable(format!(
                "yahoo chart http {}",
                resp.status()
            )));
        }
        let body: ChartEnvelope = resp.json().await.map_err(parse)?;
        let result = body
            .chart
            .result
            .and_then(|mut r| r.pop())
            .ok_or_else(|| ProviderError::NotFound(symbol.to_string()))?;
        Ok(build_candles(result))
    }

    async fn fundamentals(&self, symbol: &str) -> Result<Fundamentals, ProviderError> {
        let raw = self.fetch_raw_quotes(&[symbol.to_string()]).await?;
        let q = raw
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::NotFound(symbol.to_string()))?;
        Ok(Fundamentals {
            market_cap: q.market_cap,
            pe_trailing: q.trailing_pe,
            pe_forward: q.forward_pe,
            eps_trailing: q.eps_trailing_twelve_months,
            // v7 gives `dividendYield` already as a percent; fall back to the
            // fractional `trailingAnnualDividendYield` (×100) when it's absent.
            div_yield: q
                .dividend_yield
                .or_else(|| q.trailing_annual_dividend_yield.map(|v| v * 100.0)),
            beta: q.beta,
        })
    }

    async fn market_status(&self) -> Result<MarketStatus, ProviderError> {
        // Read `marketState` off a reference index (the S&P 500).
        let raw = self.fetch_raw_quotes(&["^GSPC".to_string()]).await?;
        let state = raw
            .first()
            .and_then(|q| q.market_state.as_deref())
            .unwrap_or("");
        Ok(match state {
            "REGULAR" => MarketStatus::Open,
            "PRE" | "PREPRE" => MarketStatus::PreMarket,
            "POST" | "POSTPOST" => MarketStatus::AfterHours,
            _ => MarketStatus::Closed,
        })
    }
}

// ---- mapping helpers -------------------------------------------------------

fn map_quote(q: RawQuote) -> Quote {
    let price = q.regular_market_price.unwrap_or(0.0);
    Quote {
        name: q.long_name.or(q.short_name),
        price,
        change: q.regular_market_change.unwrap_or(0.0),
        change_pct: q.regular_market_change_percent.unwrap_or(0.0),
        prev_close: q.regular_market_previous_close.unwrap_or(price),
        open: q.regular_market_open,
        // Missing high/low fall back to price so the range is never inverted.
        day_range: (
            q.regular_market_day_low.unwrap_or(price),
            q.regular_market_day_high.unwrap_or(price),
        ),
        week52_range: (
            q.fifty_two_week_low.unwrap_or(price),
            q.fifty_two_week_high.unwrap_or(price),
        ),
        volume: q.regular_market_volume,
        avg_volume: q.average_daily_volume3_month,
        asset: AssetKind::infer(&q.symbol),
        symbol: q.symbol,
    }
}

/// Zip the parallel `timestamp[]` and `indicators.quote[0].{o,h,l,c,v}[]` arrays
/// into candles, skipping any index Yahoo left null (gaps/halts).
fn build_candles(result: ChartResult) -> Vec<Candle> {
    let ts = result.timestamp.unwrap_or_default();
    let Some(q) = result.indicators.quote.into_iter().next() else {
        return Vec::new();
    };
    let mut candles = Vec::with_capacity(ts.len());
    for (i, &ts_i) in ts.iter().enumerate() {
        let (Some(open), Some(high), Some(low), Some(close)) = (
            q.open.get(i).copied().flatten(),
            q.high.get(i).copied().flatten(),
            q.low.get(i).copied().flatten(),
            q.close.get(i).copied().flatten(),
        ) else {
            continue;
        };
        candles.push(Candle {
            ts: ts_i,
            open,
            high,
            low,
            close,
            volume: q.volume.get(i).copied().flatten().unwrap_or(0.0),
        });
    }
    candles
}

fn net(e: reqwest::Error) -> ProviderError {
    ProviderError::Network(e.to_string())
}

fn parse(e: reqwest::Error) -> ProviderError {
    ProviderError::Parse(e.to_string())
}

// ---- wire types: exactly the Yahoo JSON we consume, all fields optional ----

#[derive(Deserialize)]
struct QuoteEnvelope {
    #[serde(rename = "quoteResponse")]
    quote_response: QuoteResult,
}

#[derive(Deserialize)]
struct QuoteResult {
    result: Vec<RawQuote>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawQuote {
    symbol: String,
    long_name: Option<String>,
    short_name: Option<String>,
    regular_market_price: Option<f64>,
    regular_market_change: Option<f64>,
    regular_market_change_percent: Option<f64>,
    regular_market_previous_close: Option<f64>,
    regular_market_open: Option<f64>,
    regular_market_day_high: Option<f64>,
    regular_market_day_low: Option<f64>,
    fifty_two_week_high: Option<f64>,
    fifty_two_week_low: Option<f64>,
    regular_market_volume: Option<f64>,
    average_daily_volume3_month: Option<f64>,
    market_cap: Option<f64>,
    trailing_pe: Option<f64>,
    forward_pe: Option<f64>,
    eps_trailing_twelve_months: Option<f64>,
    dividend_yield: Option<f64>,
    trailing_annual_dividend_yield: Option<f64>,
    beta: Option<f64>,
    market_state: Option<String>,
}

#[derive(Deserialize)]
struct ChartEnvelope {
    chart: ChartBody,
}

#[derive(Deserialize)]
struct ChartBody {
    result: Option<Vec<ChartResult>>,
}

#[derive(Deserialize)]
struct ChartResult {
    timestamp: Option<Vec<i64>>,
    indicators: ChartIndicators,
}

#[derive(Deserialize)]
struct ChartIndicators {
    quote: Vec<ChartQuote>,
}

#[derive(Deserialize)]
struct ChartQuote {
    #[serde(default)]
    open: Vec<Option<f64>>,
    #[serde(default)]
    high: Vec<Option<f64>>,
    #[serde(default)]
    low: Vec<Option<f64>>,
    #[serde(default)]
    close: Vec<Option<f64>>,
    #[serde(default)]
    volume: Vec<Option<f64>>,
}
