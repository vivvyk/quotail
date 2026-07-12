//! RSI(14), MA50, MA200 as PURE functions over a close-price series.
//!
//! Every output vector is aligned index-for-index with its input, with a leading
//! run of `None` while the window warms up (matches `Indicators` in `types.rs`).
//!
//! This module is the SINGLE SOURCE OF TRUTH for the indicator periods and RSI
//! levels — they define what MA50 / MA200 / RSI(14) and the 70/30 bands *are*.
//! `ui/` imports them from here: the `data/` → `ui/` ban is one-directional, so
//! `ui/` depending on `data/` is fine, and no duplication is needed.

use super::types::{Candle, Indicators};

pub const RSI_PERIOD: usize = 14;
/// RSI band drawn at the top of the RSI pane (overbought).
pub const RSI_OVERBOUGHT: f64 = 70.0;
/// RSI band drawn at the bottom of the RSI pane (oversold).
pub const RSI_OVERSOLD: f64 = 30.0;
pub const MA_SHORT: usize = 50;
pub const MA_LONG: usize = 200;

/// Simple moving average of `period`. First value lands at index `period - 1`
/// (that's when the window first fills), so the None-prefix is exactly
/// `period - 1` long. Returns all-`None` if `period` is 0 or exceeds the length.
pub fn sma(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; values.len()];
    if period == 0 || period > values.len() {
        return out;
    }
    // Rolling sum: add the entering value, drop the leaving one. O(n).
    let mut sum = 0.0;
    for i in 0..values.len() {
        sum += values[i];
        if i >= period {
            sum -= values[i - period];
        }
        if i >= period - 1 {
            out[i] = Some(sum / period as f64);
        }
    }
    out
}

/// Wilder's RSI of `period` (the standard — NOT a plain SMA of gains/losses).
///
/// The first average gain/loss is a simple mean of the first `period` deltas;
/// every value after uses Wilder smoothing `avg = (prev*(n-1) + current) / n`.
/// RSI needs `period` deltas = `period + 1` prices, so its first value lands at
/// index `period` and the None-prefix is exactly `period` long (for RSI(14),
/// 14 Nones — one more than an SMA(14), because it consumes differences).
///
/// A series with no losses (e.g. constant price → all deltas 0) yields RSI = 100
/// by convention; this is also the divide-by-zero guard.
pub fn rsi(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let n = values.len();
    let mut out = vec![None; n];
    if period == 0 || n < period + 1 {
        return out;
    }

    // Seed: simple average of the first `period` deltas (values[1..=period]).
    let mut avg_gain = 0.0;
    let mut avg_loss = 0.0;
    for i in 1..=period {
        let delta = values[i] - values[i - 1];
        if delta >= 0.0 {
            avg_gain += delta;
        } else {
            avg_loss -= delta; // -delta = magnitude of the loss
        }
    }
    avg_gain /= period as f64;
    avg_loss /= period as f64;
    out[period] = Some(rsi_from(avg_gain, avg_loss));

    // Wilder smoothing for the rest of the series.
    let n_f = period as f64;
    for i in (period + 1)..n {
        let delta = values[i] - values[i - 1];
        let (gain, loss) = if delta >= 0.0 {
            (delta, 0.0)
        } else {
            (0.0, -delta)
        };
        avg_gain = (avg_gain * (n_f - 1.0) + gain) / n_f;
        avg_loss = (avg_loss * (n_f - 1.0) + loss) / n_f;
        out[i] = Some(rsi_from(avg_gain, avg_loss));
    }
    out
}

/// RSI from an average gain/loss pair. `avg_loss == 0` → 100 (no downside).
fn rsi_from(avg_gain: f64, avg_loss: f64) -> f64 {
    if avg_loss == 0.0 {
        100.0
    } else {
        let rs = avg_gain / avg_loss;
        100.0 - 100.0 / (1.0 + rs)
    }
}

/// Compute all three indicators from a candle series (over close prices).
/// Aligned index-for-index with `candles`.
pub fn compute(candles: &[Candle]) -> Indicators {
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    Indicators {
        rsi: rsi(&closes, RSI_PERIOD),
        ma50: sma(&closes, MA_SHORT),
        ma200: sma(&closes, MA_LONG),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    // ---- sma: hand-computed, checkable by eye --------------------------

    #[test]
    fn sma_hand_computed() {
        // sma([1,2,3,4,5], 3):
        //   idx0,1 -> None (window not full)
        //   idx2 = (1+2+3)/3 = 2.0
        //   idx3 = (2+3+4)/3 = 3.0
        //   idx4 = (3+4+5)/3 = 4.0
        let out = sma(&[1.0, 2.0, 3.0, 4.0, 5.0], 3);
        assert_eq!(out, vec![None, None, Some(2.0), Some(3.0), Some(4.0)]);
    }

    #[test]
    fn sma_none_prefix_is_period_minus_one() {
        let values: Vec<f64> = (0..10).map(|x| x as f64).collect();
        let out = sma(&values, 4);
        let none_prefix = out.iter().take_while(|v| v.is_none()).count();
        assert_eq!(none_prefix, 4 - 1);
        assert_eq!(out.len(), values.len());
    }

    #[test]
    fn sma_period_longer_than_series_is_all_none() {
        assert_eq!(sma(&[1.0, 2.0], 5), vec![None, None]);
    }

    // ---- rsi: verified against an INDEPENDENT source -------------------
    //
    // Series and published outputs are from StockCharts' Wilder RSI(14) worked
    // example (the canonical 33-close table). The first two values are derived
    // BY HAND below so they don't share code with the implementation.

    /// StockCharts' example closing prices (33 of them).
    const SC_CLOSES: [f64; 33] = [
        44.3389, 44.0902, 44.1497, 43.6124, 44.3278, 44.8264, 45.0955, 45.4245, 45.8433, 46.0826,
        45.8931, 46.0328, 45.6140, 46.2820, 46.2820, 46.0028, 46.0328, 46.4116, 46.2222, 45.6439,
        46.2122, 46.2521, 45.7137, 46.4515, 45.7835, 45.3548, 44.0288, 44.1783, 44.2181, 44.5672,
        43.4205, 42.6628, 43.1314,
    ];

    #[test]
    fn rsi_first_value_hand_derived() {
        // First 14 deltas (prices[1..=14]). Gains summed, losses summed:
        //   gains  = .0595+.7154+.4986+.2691+.3290+.4188+.2393+.1397+.6680+0
        //          = 3.3374  -> avg_gain = 3.3374/14 = 0.238386
        //   losses = .2487+.5373+.1895+.4188 = 1.3943 -> avg_loss = 0.099593
        //   RS  = 0.238386 / 0.099593 = 2.39359
        //   RSI = 100 - 100/(1+2.39359) = 70.53
        let out = rsi(&SC_CLOSES, 14);
        assert!(out[14].is_some());
        assert!(
            approx(out[14].unwrap(), 70.53, 0.02),
            "first RSI = {:?}, expected ~70.53",
            out[14]
        );
    }

    #[test]
    fn rsi_second_value_hand_derived_exercises_wilder_smoothing() {
        // delta[15] = 46.0028 - 46.2820 = -0.2792 (a loss)
        //   avg_gain = (0.238386*13 + 0)/14      = 0.221358
        //   avg_loss = (0.099593*13 + 0.2792)/14 = 0.112422
        //   RS  = 0.221358 / 0.112422 = 1.96899
        //   RSI = 100 - 100/(1+1.96899) = 66.32
        let out = rsi(&SC_CLOSES, 14);
        assert!(
            approx(out[15].unwrap(), 66.32, 0.02),
            "second RSI = {:?}, expected ~66.32",
            out[15]
        );
    }

    /// A second implementation written straight from the formula, structured
    /// differently from `rsi` (explicit gain/loss vectors, no shared helper). If
    /// both agree, a single shared misconception cannot have produced them.
    fn rsi_reference(values: &[f64], period: usize) -> Vec<Option<f64>> {
        let n = values.len();
        let mut out = vec![None; n];
        if period == 0 || n < period + 1 {
            return out;
        }
        let gains: Vec<f64> = (1..n)
            .map(|i| (values[i] - values[i - 1]).max(0.0))
            .collect();
        let losses: Vec<f64> = (1..n)
            .map(|i| (values[i - 1] - values[i]).max(0.0))
            .collect();
        // gains[k] corresponds to delta between values[k] and values[k+1].
        let mut g = gains[..period].iter().sum::<f64>() / period as f64;
        let mut l = losses[..period].iter().sum::<f64>() / period as f64;
        let rsi_at = |g: f64, l: f64| {
            if l == 0.0 {
                100.0
            } else {
                100.0 - 100.0 / (1.0 + g / l)
            }
        };
        out[period] = Some(rsi_at(g, l));
        for k in period..gains.len() {
            g = (g * (period as f64 - 1.0) + gains[k]) / period as f64;
            l = (l * (period as f64 - 1.0) + losses[k]) / period as f64;
            out[k + 1] = Some(rsi_at(g, l));
        }
        out
    }

    #[test]
    fn rsi_agrees_with_independent_reference() {
        let a = rsi(&SC_CLOSES, 14);
        let b = rsi_reference(&SC_CLOSES, 14);
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            match (a[i], b[i]) {
                (None, None) => {}
                (Some(x), Some(y)) => {
                    assert!(approx(x, y, 1e-9), "index {i}: impl={x} reference={y}")
                }
                _ => panic!("index {i}: None/Some mismatch: {:?} vs {:?}", a[i], b[i]),
            }
        }
    }

    // ---- rsi: structural invariants ------------------------------------

    #[test]
    fn rsi_length_and_none_prefix() {
        let out = rsi(&SC_CLOSES, 14);
        assert_eq!(out.len(), SC_CLOSES.len());
        // Exactly `period` (14) leading Nones — NOT period-1. RSI consumes 14
        // deltas = 15 prices, so the first value is at index 14.
        let none_prefix = out.iter().take_while(|v| v.is_none()).count();
        assert_eq!(none_prefix, 14);
        assert!(out[14].is_some());
    }

    #[test]
    fn rsi_constant_price_is_100_no_divide_by_zero() {
        // All deltas 0 -> avg_loss 0 -> RSI 100 by convention. The classic
        // divide-by-zero edge case.
        let flat = vec![50.0; 30];
        let out = rsi(&flat, 14);
        assert_eq!(out.len(), flat.len());
        for (i, v) in out.iter().enumerate() {
            if i < 14 {
                assert!(v.is_none());
            } else {
                assert_eq!(*v, Some(100.0));
            }
        }
    }

    #[test]
    fn rsi_too_short_is_all_none() {
        // 14 prices = only 13 deltas, one short of a seed.
        let out = rsi(&SC_CLOSES[..14], 14);
        assert!(out.iter().all(|v| v.is_none()));
        assert_eq!(out.len(), 14);
    }
}
