//! `DataStore`: the SMART layer over a dumb `Provider`.
//!
//! Adds the three things providers deliberately lack — an in-memory cache
//! (`DashMap`), per-entry TTLs, and rate limiting (`governor`) — and computes
//! indicators over the fully-warmed candle series so callers get MA/RSI already
//! aligned to the candles.
//!
//! Shared as `Arc<DataStore>` across every tokio task (poller + fetch tasks), so
//! it must be `Send + Sync`. It is, with no locks of our own: `DashMap` shards
//! internally and the rate limiter is `Sync`. `Arc` (not clone-per-task) because
//! the cache must be ONE shared cache — cloning would give each task its own
//! empty map and defeat the point.

use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use thiserror::Error;

use super::indicators::{self, MA_LONG};
use super::provider::{Provider, ProviderError};
use super::types::{Candle, Fundamentals, Indicators, MarketStatus, Quote, Timeframe};

/// Quotes go stale fast; the default poll is 60s and this bounds how long a
/// cached quote is served between polls / on a burst of reads.
const QUOTE_TTL: Duration = Duration::from_secs(30);
/// Fundamentals (market cap, P/E, …) change slowly.
const FUNDAMENTALS_TTL: Duration = Duration::from_secs(3600);
/// Outbound request ceiling. Yahoo is unofficial and throttles; keep it modest.
const MAX_REQUESTS_PER_SEC: u32 = 5;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

/// A cached value stamped with when it was fetched, for TTL checks.
struct Cached<T> {
    value: T,
    at: Instant,
}

impl<T> Cached<T> {
    fn fresh(&self, ttl: Duration) -> bool {
        self.at.elapsed() < ttl
    }
}

/// Candles at their NATIVE interval, plus indicators aligned index-for-index.
/// One entry serves every visible width: the chart widget slices/aggregates the
/// tail it needs and samples these indicators at each bucket's last index.
#[derive(Clone)]
pub struct CandleData {
    pub candles: Vec<Candle>,
    pub indicators: Indicators,
}

pub struct DataStore {
    provider: Box<dyn Provider>,
    quotes: DashMap<String, Cached<Quote>>,
    candles: DashMap<(String, Timeframe), Cached<CandleData>>,
    fundamentals: DashMap<String, Cached<Fundamentals>>,
    limiter: DefaultDirectRateLimiter,
}

impl DataStore {
    pub fn new(provider: Box<dyn Provider>) -> Self {
        let quota = Quota::per_second(NonZeroU32::new(MAX_REQUESTS_PER_SEC).unwrap());
        Self {
            provider,
            quotes: DashMap::new(),
            candles: DashMap::new(),
            fundamentals: DashMap::new(),
            limiter: RateLimiter::direct(quota),
        }
    }

    /// Batch quote fetch. Serves entirely from cache when every symbol is fresh
    /// and `force` is off; otherwise ONE batch call to the provider (68 symbols,
    /// one HTTP request — the whole point of the batch design).
    pub async fn quotes(&self, symbols: &[String], force: bool) -> Result<Vec<Quote>, StoreError> {
        if symbols.is_empty() {
            return Ok(Vec::new());
        }
        let all_fresh = symbols
            .iter()
            .all(|s| self.quotes.get(s).is_some_and(|c| c.fresh(QUOTE_TTL)));
        if !force && all_fresh {
            return Ok(symbols
                .iter()
                .filter_map(|s| self.cached_quote(s))
                .collect());
        }

        self.limiter.until_ready().await;
        let fetched = self.provider.quotes(symbols).await?;
        let now = Instant::now();
        for q in &fetched {
            self.quotes.insert(
                q.symbol.clone(),
                Cached {
                    value: q.clone(),
                    at: now,
                },
            );
        }
        Ok(fetched)
    }

    /// Candles at the native interval with indicators computed over the FULL
    /// warmed series. Cached per `(symbol, timeframe)` under the timeframe's TTL.
    pub async fn candles(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        force: bool,
    ) -> Result<CandleData, StoreError> {
        let key = (symbol.to_string(), timeframe);
        let ttl = Duration::from_secs(timeframe.ttl_secs());
        // The `hit` DashMap guard lives only inside this `if` and is dropped
        // before the re-insert below — holding it across the insert would
        // deadlock the shard.
        if !force
            && let Some(hit) = self.candles.get(&key)
            && hit.fresh(ttl)
        {
            return Ok(hit.value.clone());
        }

        self.limiter.until_ready().await;
        // The store owns the warmup requirement and passes it down; the provider
        // turns MA_LONG into a wide-enough fetch range. No magic 200 downstream.
        let candles = self.provider.candles(symbol, timeframe, MA_LONG).await?;
        let indicators = indicators::compute(&candles);
        let data = CandleData {
            candles,
            indicators,
        };
        self.candles.insert(
            key,
            Cached {
                value: data.clone(),
                at: Instant::now(),
            },
        );
        Ok(data)
    }

    /// Fundamentals for one symbol, cached under a long TTL.
    pub async fn fundamentals(
        &self,
        symbol: &str,
        force: bool,
    ) -> Result<Fundamentals, StoreError> {
        if !force
            && let Some(hit) = self.fundamentals.get(symbol)
            && hit.fresh(FUNDAMENTALS_TTL)
        {
            return Ok(hit.value.clone());
        }
        self.limiter.until_ready().await;
        let data = self.provider.fundamentals(symbol).await?;
        self.fundamentals.insert(
            symbol.to_string(),
            Cached {
                value: data.clone(),
                at: Instant::now(),
            },
        );
        Ok(data)
    }

    /// Market open/closed. Not cached — it's one cheap call and callers ask for
    /// it on the same cadence as a poll.
    pub async fn market_status(&self) -> Result<MarketStatus, StoreError> {
        self.limiter.until_ready().await;
        Ok(self.provider.market_status().await?)
    }

    /// Non-async peek at the last cached quote (ignores TTL). For synchronous
    /// read paths that must not block.
    pub fn cached_quote(&self, symbol: &str) -> Option<Quote> {
        self.quotes.get(symbol).map(|c| c.value.clone())
    }

    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }
}
