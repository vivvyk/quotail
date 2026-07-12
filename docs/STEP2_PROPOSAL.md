# Step 2 proposal — lookback window + indicators (pre-implementation)

Good catch, and it's a real bug: with a 1d interval a "1M" fetch returns ~22
bars, so MA50/MA200 are `None` for the entire series and never draw. Here's the
fix I want to build, plus the two things in it that need sign-off before code.

## The rendering model I'm assuming (please confirm)

The panes are a fixed width — `PANE_CANDLES = 24` (grid) and
`DETAIL_CANDLES = 52` (detail) — and I'm reading those as **one candle per
column at the timeframe's interval**, with the chart drawing the **last N**
candles. The instruction ("render only the visible tail") matches that literally.

One consequence I want on the record, not silently absorbed: under this model the
*interval* is what distinguishes timeframes, and a long timeframe shows its
**recent tail at that interval**, not its whole span. So "1Y" (interval `1d`, 52
columns) draws the last ~52 trading days ≈ 2.5 months of daily bars, not a
compressed full year. If you actually want a long timeframe to span its full
duration inside 52 columns, that requires **bucket-aggregating** many bars into
each column — a different and bigger change that also changes where MAs are
computed. I'm proposing the tail model; flag me if you meant aggregation.

## The API change

**`Provider::candles` — add an explicit `lookback` (min bars to return):**

```rust
async fn candles(
    &self,
    symbol: &str,
    timeframe: Timeframe,
    lookback: usize,          // NEW: warmup + visible; provider guarantees ≥ this many trailing bars
) -> Result<Vec<Candle>, ProviderError>;
```

Why a parameter and not a constant inside Yahoo: the provider stays *dumb* and
knows nothing about MA200. The caller states how many bars it needs; the
provider's only new job is picking a Yahoo `range` wide enough to yield
`lookback` bars at `timeframe.interval_param()`, then returning the trailing
`lookback` (trimming the leading excess so state sizes are predictable). If the
symbol is too young, it returns what exists.

**`Timeframe` (in `types.rs`) — add a fetch-range selector:**

```rust
/// The Yahoo `range` needed to yield ≥ `min_bars` at this timeframe's interval.
/// Decoupled from the user-facing label: a "1M" view fetches ~1y of daily bars.
pub fn fetch_range_param(&self, min_bars: usize) -> &'static str;
```

This supersedes `range_param()` for fetching (I'd keep `range_param()` around,
unused by the fetch path, unless you'd rather I delete it). It's allowed to
return Yahoo ranges outside our enum (`2y`, `5y`) since the fetch span is
internal.

**`DataStore` — owns the warmup constant and computes indicators over the warm series:**

```rust
pub async fn candles(&self, symbol: &str, tf: Timeframe)
    -> Result<(Vec<Candle>, Indicators), StoreError>;
// requests lookback = DETAIL_CANDLES + MA_LONG = 52 + 200 = 252
// computes RSI/MA50/MA200 over the FULL 252-bar series, caches the pair by (symbol, tf)
```

252 = the widest consumer (52) + the longest indicator (200). One cache entry
serves both grid (slices last 24) and detail (slices last 52); both tails are
fully warm because 200 bars precede the first visible one. The chart widget
slices candles **and** indicators by the same tail range, so the "aligned
index-for-index, equal length" invariant is preserved, and the price axis scales
to the **visible** tail only (warmup bars are math, never drawn).

## What it costs in request size

The batch `quotes()` call is **unchanged** — still one HTTP call for all 68
symbols. Only per-chart `candles()` grows, and it's still **one HTTP call per
chart**, fetched once per `(symbol, tf)` and cached under the existing TTL. The
cost is a wider `range`:

| TF  | interval | old range (~bars) | new range (~bars) | ~× bars |
|-----|----------|-------------------|-------------------|---------|
| 1D  | 5m       | 1d (~78)          | 5d (~390)         | ~5×     |
| 5D  | 30m      | 5d (~65)          | 1mo (~273)        | ~4×     |
| 1M  | 1d       | 1mo (~22)         | 1y (~252)         | ~11×    |
| 6M  | 1d       | 6mo (~126)        | 1y (~252)         | ~2×     |
| YTD | 1d       | ytd (varies)      | 1y (~252)         | ~1–2×   |
| 1Y  | 1d       | 1y (~252)         | 2y (~504)         | ~2×     |
| MAX | 1wk      | max               | max               | ~1×     |

In bytes that's trivial — ~252 OHLCV bars ≈ 12–15 KB uncompressed (~4–6 KB
gzipped) per chart response. All new ranges stay within Yahoo's per-interval
history caps (5m ≤ ~60d, 30m ≤ ~60d). The real "cost" is conceptual, not
bandwidth: **daily-interval candle requests fetch ~2–11× more bars**, once,
cached.

## indicators.rs

**RSI variant: standard Wilder RSI(14).** First `avgGain`/`avgLoss` = the simple
mean of the first 14 deltas; every value after uses Wilder smoothing
`avg = (prev*(n−1) + current)/n`. I'm *not* using a plain SMA throughout — that's
the "plausible but silently wrong" version (it reacts too fast and disagrees with
every charting package). Constant price → all deltas 0 → `avgLoss == 0` → RSI =
100 by convention (this is also the divide-by-zero guard).

**One correction to the spec's invariant — flagging, not fixing silently.** The
spec says the None-prefix should be "exactly period-1 long," under the RSI bullet.
That's the **MA** invariant: `sma(period)` first resolves at index `period−1`, so
MA50 → 49 Nones, MA200 → 199 Nones. But **Wilder RSI(14) needs 14 *deltas* = 15
prices**, so its first value lands at index 14 → **14 leading Nones (= period),
not 13**. StockCharts' worked example confirms this: "the first RSI value appears
on the 15th day." So my structural tests will assert:

- `sma`: None-prefix == `period − 1`
- `rsi`: None-prefix == `period` (14)

If you actually want 13, that forces a non-standard warmup (13 deltas) that
disagrees with StockCharts/TradingView — I'd recommend against it. **Confirm 14
and I'll proceed.**

**Test independence.** Expected values will come from two sources that don't
share my implementation's assumptions:

1. **StockCharts' published Wilder RSI(14) worked example** (its fixed 33-close
   series and published RSI outputs), hardcoded, with the first value's
   arithmetic spelled out in a comment so you can check it by hand.
2. **A separate brute-force `rsi` written straight from the formula** in the test
   module, asserted to agree with the real impl on the fixed series — a shared
   misconception would have to occur *twice, differently* to pass.

Plus the structural invariants: `output.len() == input.len()`, the exact
None-prefix lengths above, and constant-price → RSI 100. `sma` gets a
hand-computed small case (`sma([1,2,3,4,5], 3) == [None,None,2,3,4]`).

## I need three things before writing step 2

1. **Rendering model**: tail-at-interval (my proposal), or bucket-aggregation?
   (Tail is what I'll build unless you say otherwise.)
2. **RSI None-prefix**: confirm **14** (standard Wilder), overriding the spec's
   "period−1".
3. **`fetch_range_param` touches `types.rs`** (an authoritative file) and changes
   `Provider::candles`'s signature (also authoritative). Confirm you're
   authorizing those edits.

Stopping here for your call.
