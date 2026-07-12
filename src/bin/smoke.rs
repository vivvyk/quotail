//! TEMPORARY smoke test — delete before merge (it's not part of Phase 1).
//!
//! Fetches AAPL through the REAL `DataStore` → `YahooProvider` (live network)
//! and prints the numbers to eyeball against TradingView/StockCharts:
//! last close, RSI(14), MA50, MA200.
//!
//!   cargo run --bin smoke

use quotail::data::store::DataStore;
use quotail::data::types::Timeframe;
use quotail::data::yahoo::YahooProvider;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = DataStore::new(Box::new(YahooProvider::new()?));

    // force = true so we bypass the (empty) cache and actually hit Yahoo.
    let quotes = store.quotes(&["AAPL".to_string()], true).await?;
    let q = quotes.first().expect("expected an AAPL quote");
    println!("AAPL  {}", q.name.as_deref().unwrap_or("?"));
    println!(
        "  quote price : {:.2}   change {:+.2} ({:+.2}%)",
        q.price, q.change, q.change_pct
    );

    // 1Y timeframe → native 1d bars, fetched over ~2y so MA200 is warm.
    let data = store.candles("AAPL", Timeframe::Y1, true).await?;
    let bars = data.candles.len();
    let last_close = data.candles.last().map(|c| c.close);
    let last_date = data
        .candles
        .last()
        .and_then(|c| chrono::DateTime::from_timestamp(c.ts, 0))
        .map(|dt| dt.format("%Y-%m-%d").to_string());
    println!("  last bar date: {last_date:?}  (compare against THIS date)");

    // Indicators are aligned index-for-index with candles; take the last value.
    let last = |v: &Vec<Option<f64>>| v.last().copied().flatten();
    let rsi = last(&data.indicators.rsi);
    let ma50 = last(&data.indicators.ma50);
    let ma200 = last(&data.indicators.ma200);

    let defined = |v: &Vec<Option<f64>>| v.iter().filter(|x| x.is_some()).count();

    println!("\n1Y candles: {bars} native daily bars (fetched ~2y for warmup)");
    println!("  last close : {last_close:?}");
    println!(
        "  RSI(14)    : {rsi:?}   ({} of {bars} defined)",
        defined(&data.indicators.rsi)
    );
    println!(
        "  MA50       : {ma50:?}   ({} of {bars} defined)",
        defined(&data.indicators.ma50)
    );
    println!(
        "  MA200      : {ma200:?}   ({} of {bars} defined)",
        defined(&data.indicators.ma200)
    );

    Ok(())
}
