//! Candlestick chart widget, shared by the grid panes and the Detail chart.
//!
//! `aggregate` is a PURE function (no ratatui) that slices the visible window by
//! timestamp and buckets native candles into `width` columns (unit-tested below);
//! the renderer turns `Column`s into the glyph grid.
//!
//! Glyph contract (do not change): candle bodies `█`, wicks `│`, MA overlays `·`
//! drawn ONLY where a cell is empty — a candle always wins.

use chrono::{Datelike, TimeZone, Utc};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::data::types::{Candle, DisplaySpan, Indicators};

use super::theme::{Theme, glyph};

/// One rendered column: aggregated OHLCV plus indicator values SAMPLED at the
/// bucket's last native index (NOT recomputed on the aggregated series — a
/// "200-day MA" stays a 200-day MA regardless of how many days a column spans).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Column {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub ma50: Option<f64>,
    pub ma200: Option<f64>,
    pub rsi: Option<f64>,
}

/// The timestamp at the left edge of the visible window, given the display span
/// and the series. Right edge is always the last candle (so the chart shows the
/// most recent data). Bucketing/slicing is BY TIMESTAMP — never by bar count,
/// because holidays make counts wrong.
fn window_start_ts(candles: &[Candle], span: DisplaySpan) -> i64 {
    let last = candles.last().map(|c| c.ts).unwrap_or(0);
    match span {
        DisplaySpan::Max => candles.first().map(|c| c.ts).unwrap_or(0),
        DisplaySpan::Last(dur) => last - dur.num_seconds(),
        DisplaySpan::Ytd => Utc
            .timestamp_opt(last, 0)
            .single()
            .and_then(|dt| Utc.with_ymd_and_hms(dt.year(), 1, 1, 0, 0, 0).single())
            .map(|dt| dt.timestamp())
            .unwrap_or(0),
    }
}

/// Slice `candles` to the display window (by timestamp) and bucket into `width`
/// columns. Per bucket: open = first, high = max, low = min, close = last,
/// volume = sum. Empty buckets (more columns than visible candles in a slot) are
/// `None`. Indicators are sampled at each bucket's last native index.
pub fn aggregate(
    candles: &[Candle],
    indicators: &Indicators,
    span: DisplaySpan,
    width: usize,
) -> Vec<Option<Column>> {
    let mut out = vec![None; width];
    if width == 0 {
        return out;
    }
    let start = window_start_ts(candles, span);
    // Indices into `candles` that fall inside the visible window, in order.
    let visible: Vec<usize> = (0..candles.len())
        .filter(|&i| candles[i].ts >= start)
        .collect();
    let n = visible.len();
    if n == 0 {
        return out;
    }

    for (b, slot) in out.iter_mut().enumerate() {
        // Even distribution of the n visible candles across width buckets.
        let lo = b * n / width;
        let hi = (b + 1) * n / width;
        if lo >= hi {
            continue; // empty bucket (n < width)
        }
        let group = &visible[lo..hi];
        let first = candles[group[0]];
        let last_idx = group[group.len() - 1];
        let last = candles[last_idx];

        let mut high = f64::MIN;
        let mut low = f64::MAX;
        let mut volume = 0.0;
        for &i in group {
            high = high.max(candles[i].high);
            low = low.min(candles[i].low);
            volume += candles[i].volume;
        }

        *slot = Some(Column {
            open: first.open,
            high,
            low,
            close: last.close,
            volume,
            ma50: sample(&indicators.ma50, last_idx),
            ma200: sample(&indicators.ma200, last_idx),
            rsi: sample(&indicators.rsi, last_idx),
        });
    }
    out
}

fn sample(series: &[Option<f64>], i: usize) -> Option<f64> {
    series.get(i).copied().flatten()
}

// ---- rendering -------------------------------------------------------------
// A grid pane is a bordered box titled with the symbol + quote, an interior of
// candle columns inset by one padding column on each side, and a bottom border
// carrying the timeframe label. Both dimensions come from `area` — panes stretch
// with the terminal, so the interior width/height are derived, never constants.

/// A filled grid pane. `border_style` colors the FRAME (focus dims the border, not
/// the title). The title is painted per-segment: symbol fg, price cyan, change
/// green/red; the timeframe on the bottom border is warn. Candle interior is masked
/// in the overview snapshot; its glyphs are pinned by the unit tests below.
#[allow(clippy::too_many_arguments)] // a pane genuinely has this many inputs
pub fn render_grid_pane(
    buf: &mut Buffer,
    area: Rect,
    symbol: &str,
    price: Option<&str>,
    change_pct: Option<f64>,
    columns: &[Option<Column>],
    tf_label: &str,
    border_style: Style,
    theme: Theme,
) {
    super::draw_box(buf, area, "", border_style, border_style);

    // Title spans over the top border: `┌─ SYM price +chg% ` then the pre-drawn dashes.
    let y = area.y;
    let mut cx = area.x;
    let mut put = |cx: &mut u16, text: &str, st: Style| {
        buf.set_string(*cx, y, text, st);
        *cx += text.chars().count() as u16;
    };
    put(&mut cx, "┌─ ", border_style);
    put(&mut cx, symbol, Style::default().fg(theme.fg));
    if let (Some(p), Some(chg)) = (price, change_pct) {
        put(&mut cx, " ", border_style);
        put(&mut cx, p, Style::default().fg(theme.price));
        put(&mut cx, " ", border_style);
        put(
            &mut cx,
            &format!("{chg:+.2}%"),
            Style::default().fg(theme.change(chg)),
        );
    }
    put(&mut cx, " ", border_style);

    let by = area.y + area.height - 1;
    buf.set_string(
        area.x,
        by,
        tf_bottom_border(area.width as usize, tf_label),
        border_style,
    );
    let label_col = area.x + tf_label_col(area.width as usize, tf_label);
    buf.set_string(label_col, by, tf_label, Style::default().fg(theme.warn));

    // Interior derives from the actual pane size (panes stretch to fill the
    // terminal), inset one padding column each side and one border row each end.
    let interior = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };
    render_candles(buf, interior, columns, theme);
}

/// An empty pane: plain box with a centered `enter to chart` prompt.
pub fn render_empty_pane(buf: &mut Buffer, area: Rect, border_style: Style, theme: Theme) {
    super::draw_box(buf, area, "", border_style, border_style);
    let text = "enter to chart";
    let x = area.x + (area.width - text.len() as u16) / 2;
    let y = area.y + area.height / 2;
    buf.set_string(x, y, text, Style::default().fg(theme.muted));
}

/// Bottom border with the timeframe centered. Biased one cell left so a 28-wide
/// pane splits 10 / 12 dashes around ` 1M `, matching ASCII_REFERENCE.md.
fn tf_bottom_border(width: usize, label: &str) -> String {
    let seg = format!(" {label} ");
    let inner = width.saturating_sub(2);
    let seg_len = seg.chars().count();
    let left = inner.saturating_sub(seg_len).saturating_sub(1) / 2;
    let right = inner.saturating_sub(seg_len).saturating_sub(left);
    format!("└{}{seg}{}┘", "─".repeat(left), "─".repeat(right))
}

/// Column (relative to the pane's left edge) where the timeframe LABEL text begins
/// on the bottom border — matches `tf_bottom_border`'s layout (`└` + left dashes +
/// ` label `). Used to repaint just the label in a highlight color.
fn tf_label_col(width: usize, label: &str) -> u16 {
    let seg_len = label.chars().count() + 2; // ` label `
    let inner = width.saturating_sub(2);
    let left = inner.saturating_sub(seg_len).saturating_sub(1) / 2;
    (1 + left + 1) as u16 // └ + left dashes + leading space
}

/// The price range the chart scales onto rows: min low / max high across visible
/// columns, widened to include any MA point so an MA line can't fall off-pane.
/// `None` for an empty or flat series. Shared by the candle renderer and the axis.
pub(crate) fn price_range(columns: &[Option<Column>]) -> Option<(f64, f64)> {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for c in columns.iter().flatten() {
        hi = hi.max(c.high);
        lo = lo.min(c.low);
        for v in [c.ma50, c.ma200].into_iter().flatten() {
            hi = hi.max(v);
            lo = lo.min(v);
        }
    }
    (hi > lo).then_some((lo, hi))
}

/// Draw candle columns into `area` (already the interior — no borders). Each
/// column is one cell wide. Body `█`, wick `│`; MA dots `·` are drawn ONLY into
/// still-empty cells — a candle always wins. Up/down color by close vs open;
/// ma50 in `warn`, ma200 in `heading` (per the theme). Shared by the grid panes
/// and the larger Detail chart.
pub(crate) fn render_candles(
    buf: &mut Buffer,
    area: Rect,
    columns: &[Option<Column>],
    theme: Theme,
) {
    let Some((lo, hi)) = price_range(columns) else {
        return;
    };
    if area.height == 0 {
        return;
    }
    let h = area.height;
    let row_of = |p: f64| -> u16 {
        let t = (hi - p) / (hi - lo);
        (t * (h - 1) as f64).round().clamp(0.0, (h - 1) as f64) as u16
    };

    let n = columns.len().min(area.width as usize);
    for (i, col) in columns.iter().enumerate().take(n) {
        let Some(c) = col else { continue };
        let cx = area.x + i as u16;
        let color = if c.close >= c.open {
            theme.up
        } else {
            theme.down
        };
        let candle = Style::default().fg(color);

        let hi_row = row_of(c.high);
        let lo_row = row_of(c.low);
        let body_top = row_of(c.open.max(c.close));
        let body_bot = row_of(c.open.min(c.close));

        // Wick first (full high→low), then the body overwrites its span.
        for r in hi_row..=lo_row {
            buf.set_string(cx, area.y + r, glyph::CANDLE_WICK.to_string(), candle);
        }
        for r in body_top..=body_bot {
            buf.set_string(cx, area.y + r, glyph::CANDLE_BODY.to_string(), candle);
        }

        // MA dots — only into cells the candle left empty.
        for (val, mcolor) in [(c.ma50, theme.warn), (c.ma200, theme.heading)] {
            if let Some(v) = val {
                let r = row_of(v);
                let empty = buf
                    .cell((cx, area.y + r))
                    .map(|cell| cell.symbol() == " ")
                    .unwrap_or(false);
                if empty {
                    buf.set_string(
                        cx,
                        area.y + r,
                        glyph::MA_DOT.to_string(),
                        Style::default().fg(mcolor),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn candle(ts: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Candle {
        Candle {
            ts,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: v,
        }
    }

    /// 6 candles → 3 columns: buckets [0,1] [2,3] [4,5]. Every OHLCV field is
    /// hand-computed here so the aggregation rule is pinned exactly.
    #[test]
    fn buckets_ohlc_exactly() {
        let candles = vec![
            candle(1, 10.0, 12.0, 9.0, 11.0, 100.0),
            candle(2, 11.0, 15.0, 10.0, 14.0, 200.0),
            candle(3, 14.0, 14.0, 8.0, 9.0, 150.0),
            candle(4, 9.0, 11.0, 7.0, 10.0, 250.0),
            candle(5, 10.0, 13.0, 10.0, 12.0, 300.0),
            candle(6, 12.0, 16.0, 11.0, 15.0, 350.0),
        ];
        let cols = aggregate(&candles, &Indicators::default(), DisplaySpan::Max, 3);
        assert_eq!(cols.len(), 3);

        // bucket 0 = candles 0,1: open=10 (first), high=15, low=9, close=14 (last), vol=300
        let b0 = cols[0].unwrap();
        assert_eq!(
            (b0.open, b0.high, b0.low, b0.close, b0.volume),
            (10.0, 15.0, 9.0, 14.0, 300.0)
        );
        // bucket 1 = candles 2,3: open=14, high=14, low=7, close=10, vol=400
        let b1 = cols[1].unwrap();
        assert_eq!(
            (b1.open, b1.high, b1.low, b1.close, b1.volume),
            (14.0, 14.0, 7.0, 10.0, 400.0)
        );
        // bucket 2 = candles 4,5: open=10, high=16, low=10, close=15, vol=650
        let b2 = cols[2].unwrap();
        assert_eq!(
            (b2.open, b2.high, b2.low, b2.close, b2.volume),
            (10.0, 16.0, 10.0, 15.0, 650.0)
        );
    }

    /// The visible window is sliced BY TIMESTAMP, not by bar count.
    #[test]
    fn slices_visible_window_by_timestamp() {
        let day = 86_400;
        let base = 1_000_000;
        // 5 daily candles; only the last 3 (ts >= last - 2 days) are visible.
        let candles: Vec<Candle> = (0..5)
            .map(|i| candle(base + i * day, i as f64, i as f64, i as f64, i as f64, 1.0))
            .collect();
        let cols = aggregate(
            &candles,
            &Indicators::default(),
            DisplaySpan::Last(Duration::days(2)),
            3,
        );
        // Window start = last(base+4d) - 2d = base+2d → candles 2,3,4 visible → one per column.
        assert_eq!(cols.iter().filter(|c| c.is_some()).count(), 3);
        assert_eq!(cols[0].unwrap().open, 2.0); // candle index 2 is the oldest visible
        assert_eq!(cols[2].unwrap().close, 4.0); // candle index 4 is the newest
    }

    /// Indicators are SAMPLED at each bucket's last native index, not recomputed.
    #[test]
    fn samples_indicators_at_bucket_last_index() {
        let candles: Vec<Candle> = (0..6).map(|i| candle(i, 1.0, 1.0, 1.0, 1.0, 1.0)).collect();
        let indicators = Indicators {
            ma50: vec![None, None, Some(1.0), Some(2.0), Some(3.0), Some(4.0)],
            ma200: vec![None; 6],
            rsi: vec![None; 6],
        };
        let cols = aggregate(&candles, &indicators, DisplaySpan::Max, 3);
        // buckets: [0,1] last idx 1 → None; [2,3] last idx 3 → 2.0; [4,5] last idx 5 → 4.0
        assert_eq!(cols[0].unwrap().ma50, None);
        assert_eq!(cols[1].unwrap().ma50, Some(2.0));
        assert_eq!(cols[2].unwrap().ma50, Some(4.0));
    }

    fn col(
        o: f64,
        h: f64,
        l: f64,
        c: f64,
        ma50: Option<f64>,
        ma200: Option<f64>,
    ) -> Option<Column> {
        Some(Column {
            open: o,
            high: h,
            low: l,
            close: c,
            volume: 0.0,
            ma50,
            ma200,
            rsi: None,
        })
    }

    fn glyphs(buf: &ratatui::buffer::Buffer, x: u16, rows: u16) -> Vec<String> {
        (0..rows)
            .map(|y| buf.cell((x, y)).unwrap().symbol().to_string())
            .collect()
    }

    /// Body `█` spans open→close; wick `│` fills the rest of high→low. Scale here
    /// is 0..8 over 5 rows, so row_of(8)=0, row_of(6)=1, row_of(2)=3, row_of(0)=4.
    #[test]
    fn renders_candle_body_and_wick_exactly() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        // open=2, close=6 (up), high=8, low=0 → wick rows 0-4, body rows 1-3.
        let columns = vec![col(2.0, 8.0, 0.0, 6.0, None, None)];
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 5));
        render_candles(&mut buf, Rect::new(0, 0, 1, 5), &columns, Theme::TOKYONIGHT);
        assert_eq!(glyphs(&buf, 0, 5), vec!["│", "█", "█", "█", "│"]);
    }

    /// MA dots land only in empty cells; where a candle already occupies the row,
    /// the candle wins.
    #[test]
    fn ma_dots_only_fill_empty_cells() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        // Candle at rows 0-1 (high=8 close=8, low=6 open=6). The scale stretches to
        // include the MA points, so lo=2, hi=8 over 5 rows: ma50=2 → bottom row 4
        // (empty → dot); ma200=8 → row 0 (candle already there → no dot).
        let columns = vec![col(6.0, 8.0, 6.0, 8.0, Some(2.0), Some(8.0))];
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 5));
        render_candles(&mut buf, Rect::new(0, 0, 1, 5), &columns, Theme::TOKYONIGHT);
        assert_eq!(glyphs(&buf, 0, 5), vec!["█", "█", " ", " ", "·"]);
    }

    #[test]
    fn tf_bottom_border_matches_reference_split() {
        // 28-wide pane → `└` + 10 dashes + ` 1M ` + 12 dashes + `┘`.
        assert_eq!(tf_bottom_border(28, "1M"), "└────────── 1M ────────────┘");
    }

    #[test]
    fn edge_cases() {
        // Zero width → empty output.
        assert!(aggregate(&[], &Indicators::default(), DisplaySpan::Max, 0).is_empty());
        // No candles → all None columns.
        let cols = aggregate(&[], &Indicators::default(), DisplaySpan::Max, 4);
        assert_eq!(cols, vec![None, None, None, None]);
        // Fewer visible candles than columns → some empty buckets, none duplicated.
        let candles = vec![
            candle(1, 1.0, 1.0, 1.0, 1.0, 1.0),
            candle(2, 2.0, 2.0, 2.0, 2.0, 2.0),
        ];
        let cols = aggregate(&candles, &Indicators::default(), DisplaySpan::Max, 5);
        assert_eq!(cols.iter().filter(|c| c.is_some()).count(), 2);
    }
}
