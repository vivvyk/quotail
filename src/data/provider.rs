//! The `Provider` trait: the seam that makes data sources swappable.
//!
//! Providers are DUMB. They fetch from one source and nothing else — no caching,
//! no rate limiting, no retries. All of that lives in `DataStore`, one layer up.

use async_trait::async_trait;
use thiserror::Error;

use super::types::{Candle, Fundamentals, MarketStatus, Quote, Timeframe};

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("network error: {0}")]
    Network(String),

    #[error("could not parse response: {0}")]
    Parse(String),

    #[error("symbol not found: {0}")]
    NotFound(String),

    #[error("rate limited by {provider}, retry after {retry_after_secs}s")]
    RateLimited {
        provider: String,
        retry_after_secs: u64,
    },

    #[error("provider unavailable: {0}")]
    Unavailable(String),
}

/// `Send + Sync` is required: the provider is shared across tokio tasks
/// inside an `Arc<DataStore>`, and the compiler will only permit that if it
/// can prove the type is safe to move between and share across threads.
///
/// `#[async_trait]` is needed because native `async fn` in traits is not
/// object-safe — and `DataStore` holds a `Box<dyn Provider>` so the source
/// can be chosen at runtime.
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;

    /// BATCH BY DESIGN. The default watchlist is ~68 symbols; fetching them
    /// one at a time would mean 68 requests per poll and immediate throttling.
    /// Yahoo accepts a comma-joined symbol list, so this is ONE http call.
    async fn quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, ProviderError>;

    /// `min_bars` = warmup bars the caller needs BEFORE the visible window (the
    /// store passes `MA_LONG`). The provider widens its fetch to cover the
    /// warmup plus the visible span, so indicators are defined across the whole
    /// display. It stays dumb: it does not know why 200, only that it must
    /// return at least that much history ahead of what will be shown.
    async fn candles(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        min_bars: usize,
    ) -> Result<Vec<Candle>, ProviderError>;

    async fn fundamentals(&self, symbol: &str) -> Result<Fundamentals, ProviderError>;

    async fn market_status(&self) -> Result<MarketStatus, ProviderError>;
}
