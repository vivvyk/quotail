//! Detail view: main candlestick chart + MA overlays + volume histogram + RSI on
//! the left, a fundamentals rail on the right.
//!
//! Fully RESPONSIVE. The bar is always the last two rows and the rail is a FIXED
//! 40 cols on the right; the left column takes the rest (`W - 40`). Vertically,
//! volume (4 body) and rsi (5 body) are FIXED-HEIGHT strips and the MAIN CHART
//! absorbs the slack in both axes — a wider chart just shows more candle columns
//! over the same span, and the right gutter carries the price/volume/rsi axis
//! labels. At 96x31: main 56 wide, rows 0-15 (14 body); volume 16-21; rsi 22-28;
//! rail 0-28; bar 29-30.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::{AppState, DetailState};

use super::layout::{DETAIL_AXIS_WIDTH, DETAIL_RAIL_WIDTH, DETAIL_RSI_ROWS, DETAIL_VOLUME_ROWS};
use super::theme::{Theme, glyph};
use super::{bottom_bar, chart};

/// A volume / rsi strip is its body rows plus a top and bottom border.
const VOLUME_H: u16 = DETAIL_VOLUME_ROWS as u16 + 2;
const RSI_H: u16 = DETAIL_RSI_ROWS as u16 + 2;

/// Candle columns for a left-panel of the given width: interior minus the two
/// borders + two pads, minus the price-axis gutter. A wider terminal → more
/// columns over the same span.
fn candle_cols(width: u16) -> usize {
    width.saturating_sub(4 + DETAIL_AXIS_WIDTH as u16) as usize
}

pub fn render(frame: &mut Frame, state: &AppState) {
    let theme = Theme::TOKYONIGHT;
    let area = frame.area();
    let (w, h) = (area.width, area.height);
    let buf = frame.buffer_mut();
    let detail = state.detail.as_ref();

    // The rail is fixed; the main column (chart + strips) absorbs the extra width.
    let main_w = w.saturating_sub(DETAIL_RAIL_WIDTH);
    // Panels occupy everything above the floor bar. The main chart takes whatever
    // the fixed-height volume and rsi strips don't.
    let panels_h = h.saturating_sub(2);
    let main_h = panels_h.saturating_sub(VOLUME_H + RSI_H);

    let main = Rect {
        x: 0,
        y: 0,
        width: main_w,
        height: main_h,
    };
    let volume = Rect {
        x: 0,
        y: main_h,
        width: main_w,
        height: VOLUME_H,
    };
    let rsi = Rect {
        x: 0,
        y: main_h + VOLUME_H,
        width: main_w,
        height: RSI_H,
    };
    let rail = Rect {
        x: main_w,
        y: 0,
        width: DETAIL_RAIL_WIDTH,
        height: panels_h,
    };

    render_main_chart(buf, state, detail, theme, main);
    render_volume(buf, state, detail, theme, volume);
    render_rsi(buf, state, detail, theme, rsi);
    render_rail(buf, detail, theme, rail);

    let bar = Rect {
        x: 0,
        y: h - 2,
        width: w,
        height: 2,
    };
    bottom_bar::render(buf, bar, state);
}

// ---- main chart ------------------------------------------------------------

fn render_main_chart(
    buf: &mut Buffer,
    state: &AppState,
    detail: Option<&DetailState>,
    theme: Theme,
    area: Rect,
) {
    let border = Style::default().fg(theme.border);
    let warn = Style::default().fg(theme.warn);
    let heading = Style::default().fg(theme.heading);
    let title = detail.map(main_title).unwrap_or_default();
    super::draw_box(buf, area, &title, border, heading);

    // Replace the plain bottom border with the ma50/ma200 legend + timeframe, then
    // repaint the legend so its colors MATCH the overlay dots they describe (ma50
    // warn, ma200 heading) and the timeframe label in accent.
    let tf = state.timeframe.label();
    let bottom = main_bottom_border(tf, area.width);
    let by = area.y + area.height - 1;
    buf.set_string(area.x, by, bottom, border);
    buf.set_string(area.x + 4, by, "ma50", warn);
    buf.set_string(area.x + 9, by, glyph::MA_DOT.to_string(), warn);
    buf.set_string(area.x + 13, by, "ma200", heading);
    buf.set_string(area.x + 19, by, glyph::MA_DOT.to_string(), heading);
    let tf_col = area.x + area.width.saturating_sub(9 + tf.chars().count() as u16);
    buf.set_string(tf_col, by, tf, Style::default().fg(theme.accent));

    if let Some(d) = detail {
        // Candles inset one padding column; the right gutter carries price labels.
        let n = candle_cols(area.width);
        let interior = Rect {
            x: area.x + 2,
            y: area.y + 1,
            width: n as u16,
            height: area.height.saturating_sub(2),
        };
        let cols = chart::aggregate(&d.candles, &d.indicators, state.timeframe.display_span(), n);
        chart::render_candles(buf, interior, &cols);
        // Price axis in the right gutter (high at top, mid in the middle, low at the
        // bottom) — this is what fills the space right of the candles.
        if let Some((lo, hi)) = chart::price_range(&cols) {
            let muted = Style::default().fg(theme.muted);
            let end = area.x + area.width - 3;
            let top = area.y + 1;
            let bot = area.y + area.height - 2;
            axis_right(buf, end, top, &format!("{hi:.2}"), muted);
            axis_right(
                buf,
                end,
                (top + bot) / 2,
                &format!("{:.2}", (hi + lo) / 2.0),
                muted,
            );
            axis_right(buf, end, bot, &format!("{lo:.2}"), muted);
        }
    }
}

/// Right-align `text` so it ends at column `end_x`.
fn axis_right(buf: &mut Buffer, end_x: u16, y: u16, text: &str, style: Style) {
    let start = end_x.saturating_sub(text.chars().count() as u16 - 1);
    buf.set_string(start, y, text, style);
}

/// `AAPL · Apple Inc. · $326.77  +1.23 (+0.38%)`.
fn main_title(d: &DetailState) -> String {
    match d.quote.as_ref() {
        Some(q) => {
            let name = q.name.clone().unwrap_or_default();
            format!(
                "{} · {} · ${:.2}  {:+.2} ({:+.2}%)",
                d.symbol, name, q.price, q.change, q.change_pct
            )
        }
        None => d.symbol.clone(),
    }
}

/// The main chart's bottom border, carrying the MA legend and the timeframe:
/// `└── ma50 ·── ma200 ·──…── 1M ───────┘`. The legend is fixed; the timeframe
/// keeps 7 trailing dashes so it stays right-anchored regardless of label length.
fn main_bottom_border(tf: &str, width: u16) -> String {
    const PREFIX: &str = "└── ma50 ·── ma200 ·"; // 20 cols
    const RIGHT: usize = 7; // trailing dashes before ┘
    let seg = format!(" {tf} ");
    let left =
        (width as usize).saturating_sub(PREFIX.chars().count() + RIGHT + 1 + seg.chars().count());
    format!("{PREFIX}{}{seg}{}┘", "─".repeat(left), "─".repeat(RIGHT))
}

// ---- volume ----------------------------------------------------------------

fn render_volume(
    buf: &mut Buffer,
    state: &AppState,
    detail: Option<&DetailState>,
    theme: Theme,
    area: Rect,
) {
    // Title: "volume" in heading, the M/B numbers in cyan.
    let border = Style::default().fg(theme.border);
    let heading = Style::default().fg(theme.heading);
    let price = Style::default().fg(theme.price);
    match detail.and_then(|d| d.quote.as_ref()) {
        Some(q) => {
            let (vol, avg) = (fmt_vol(q.volume), fmt_vol(q.avg_volume));
            super::draw_box_titled(
                buf,
                area,
                border,
                &[
                    ("volume  ", heading),
                    (&vol, price),
                    (" · avg ", heading),
                    (&avg, price),
                ],
            );
        }
        None => super::draw_box_titled(buf, area, border, &[("volume", heading)]),
    }

    if let Some(d) = detail {
        let interior = Rect {
            x: area.x + 2,
            y: area.y + 1,
            width: candle_cols(area.width) as u16,
            height: area.height.saturating_sub(2),
        };
        let cols = chart::aggregate(
            &d.candles,
            &d.indicators,
            state.timeframe.display_span(),
            candle_cols(area.width),
        );
        render_volume_bars(buf, interior, &cols, theme);
        // Axis gutter: peak volume at the top, 0 at the bottom.
        let peak = cols
            .iter()
            .flatten()
            .map(|c| c.volume)
            .fold(0.0_f64, f64::max);
        if peak > 0.0 {
            let muted = Style::default().fg(theme.muted);
            let end = area.x + area.width - 3;
            axis_right(buf, end, area.y + 1, &fmt_vol(Some(peak)), muted);
            axis_right(buf, end, area.y + area.height - 2, "0", muted);
        }
    }
}

fn render_volume_bars(
    buf: &mut Buffer,
    area: Rect,
    columns: &[Option<chart::Column>],
    theme: Theme,
) {
    let max = columns
        .iter()
        .flatten()
        .map(|c| c.volume)
        .fold(0.0_f64, f64::max);
    if max <= 0.0 {
        return;
    }
    let h = area.height;
    for (i, col) in columns.iter().enumerate().take(area.width as usize) {
        let Some(c) = col else { continue };
        let bars = ((c.volume / max) * h as f64).round() as u16;
        let color = if c.close >= c.open {
            theme.up
        } else {
            theme.down
        };
        for r in 0..bars.min(h) {
            buf.set_string(
                area.x + i as u16,
                area.y + h - 1 - r,
                glyph::VOLUME_BAR.to_string(),
                Style::default().fg(color),
            );
        }
    }
}

// ---- rsi -------------------------------------------------------------------

fn render_rsi(
    buf: &mut Buffer,
    state: &AppState,
    detail: Option<&DetailState>,
    theme: Theme,
    area: Rect,
) {
    // Title: "rsi (14)" in heading, the current value in cyan.
    let border = Style::default().fg(theme.border);
    let heading = Style::default().fg(theme.heading);
    let price = Style::default().fg(theme.price);
    match detail.and_then(|d| last(&d.indicators.rsi)) {
        Some(v) => {
            let val = format!("{v:.1}");
            super::draw_box_titled(buf, area, border, &[("rsi (14)  ", heading), (&val, price)]);
        }
        None => super::draw_box_titled(buf, area, border, &[("rsi (14)", heading)]),
    }

    if let Some(d) = detail {
        let interior = Rect {
            x: area.x + 2,
            y: area.y + 1,
            width: candle_cols(area.width) as u16,
            height: area.height.saturating_sub(2),
        };
        let cols = chart::aggregate(
            &d.candles,
            &d.indicators,
            state.timeframe.display_span(),
            candle_cols(area.width),
        );
        render_rsi_line(buf, interior, &cols, theme);
        // Axis gutter: the 70 / 30 band levels align with the top / bottom rows.
        let muted = Style::default().fg(theme.muted);
        let end = area.x + area.width - 3;
        axis_right(buf, end, area.y + 1, "70", muted);
        axis_right(buf, end, area.y + area.height - 2, "30", muted);
    }
}

/// RSI band at 70/30 (top / bottom rows), points scaled over that 30–70 window.
fn render_rsi_line(buf: &mut Buffer, area: Rect, columns: &[Option<chart::Column>], theme: Theme) {
    let h = area.height;
    if h == 0 {
        return;
    }
    // Map an RSI value onto rows with 70 at the top row and 30 at the bottom.
    let row_of = |v: f64| -> u16 {
        let t = (70.0 - v) / 40.0;
        (t * (h - 1) as f64).round().clamp(0.0, (h - 1) as f64) as u16
    };
    for level in [70.0_f64, 30.0] {
        let r = row_of(level);
        for x in 0..area.width {
            let cx = area.x + x;
            buf.set_string(
                cx,
                area.y + r,
                glyph::RSI_BAND.to_string(),
                Style::default().fg(theme.muted),
            );
        }
    }
    for (i, col) in columns.iter().enumerate().take(area.width as usize) {
        if let Some(c) = col
            && let Some(v) = c.rsi
        {
            buf.set_string(
                area.x + i as u16,
                area.y + row_of(v),
                glyph::RSI_POINT.to_string(),
                Style::default().fg(theme.accent),
            );
        }
    }
}

// ---- fundamentals rail -----------------------------------------------------

fn render_rail(buf: &mut Buffer, detail: Option<&DetailState>, theme: Theme, area: Rect) {
    let border = Style::default().fg(theme.border);
    let heading = Style::default().fg(theme.heading);
    let muted = Style::default().fg(theme.muted); // labels
    let fg = Style::default().fg(theme.fg); // ordinary values
    let price = Style::default().fg(theme.price); // market cap
    // The rail spans the full panel height; its content stays top-aligned (rows are
    // measured from the top border, and the rail is always anchored at y=0).
    super::draw_box(buf, area, "fundamentals", border, heading);

    let lx = area.x + 2; // label column
    let vend = area.x + DETAIL_RAIL_WIDTH - 3; // value right edge

    // Row labels in muted, section headers in heading purple.
    for (row, text) in [
        (2, "market cap"),
        (3, "p/e (ttm)"),
        (4, "p/e (fwd)"),
        (5, "eps (ttm)"),
        (6, "div yield"),
        (7, "beta"),
        (16, "ma50"),
        (17, "ma200"),
        (18, "rsi (14)"),
        (21, "open"),
        (22, "prev close"),
        (23, "day high"),
        (24, "day low"),
    ] {
        buf.set_string(lx, row, text, muted);
    }
    for (row, text) in [
        (9, "day range"),
        (12, "52-wk range"),
        (15, "indicators"),
        (20, "session"),
    ] {
        buf.set_string(lx, row, text, heading);
    }

    let Some(d) = detail else { return };

    if let Some(f) = d.fundamentals.as_ref() {
        if let Some(v) = f.market_cap {
            rail_value(buf, vend, 2, &fmt_cap(v), price); // market cap = cyan
        }
        if let Some(v) = f.pe_trailing {
            rail_value(buf, vend, 3, &format!("{v:.1}"), fg);
        }
        if let Some(v) = f.pe_forward {
            rail_value(buf, vend, 4, &format!("{v:.1}"), fg);
        }
        if let Some(v) = f.eps_trailing {
            rail_value(buf, vend, 5, &format!("{v:.2}"), fg);
        }
        if let Some(v) = f.div_yield {
            rail_value(buf, vend, 6, &format!("{v:.2}%"), fg);
        }
        if let Some(v) = f.beta {
            rail_value(buf, vend, 7, &format!("{v:.2}"), fg);
        }
    }
    // ma50 in warn, ma200 in heading (matching the overlay dots), each with a
    // trend arrow (▲ price above the MA, ▼ below).
    let price_now = d.quote.as_ref().map(|q| q.price);
    if let Some(v) = last(&d.indicators.ma50) {
        rail_ma(
            buf,
            vend,
            16,
            v,
            price_now,
            Style::default().fg(theme.warn),
            theme,
        );
    }
    if let Some(v) = last(&d.indicators.ma200) {
        rail_ma(buf, vend, 17, v, price_now, heading, theme);
    }
    if let Some(v) = last(&d.indicators.rsi) {
        rail_value(buf, vend, 18, &format!("{v:.1}"), fg);
    }

    if let Some(q) = d.quote.as_ref() {
        if let Some(o) = q.open {
            rail_value(buf, vend, 21, &format!("{o:.2}"), fg);
        }
        rail_value(buf, vend, 22, &format!("{:.2}", q.prev_close), fg);
        rail_value(buf, vend, 23, &format!("{:.2}", q.day_range.1), fg);
        rail_value(buf, vend, 24, &format!("{:.2}", q.day_range.0), fg);
        // Range bars (row 10 day, row 13 52-wk) — masked in the snapshot.
        render_range_bar(buf, 10, area, q.day_range, q.price, theme);
        render_range_bar(buf, 13, area, q.week52_range, q.price, theme);
    }
}

/// Right-align `text` ending at `vend` on `row`.
fn rail_value(buf: &mut Buffer, vend: u16, row: u16, text: &str, style: Style) {
    let start = vend.saturating_sub(text.chars().count() as u16 - 1);
    buf.set_string(start, row, text, style);
}

/// A MA row: value ending two cells before `vend`, then a ▲/▼ trend arrow at `vend`
/// colored up/down.
fn rail_ma(
    buf: &mut Buffer,
    vend: u16,
    row: u16,
    ma: f64,
    price: Option<f64>,
    val: Style,
    theme: Theme,
) {
    let text = format!("{ma:.2}");
    let end = vend.saturating_sub(2);
    let start = end.saturating_sub(text.chars().count() as u16 - 1);
    buf.set_string(start, row, &text, val);
    if let Some(p) = price {
        let (arrow, color) = if p >= ma {
            (glyph::UP_ARROW, theme.up)
        } else {
            (glyph::DOWN_ARROW, theme.down)
        };
        buf.set_string(vend, row, arrow.to_string(), Style::default().fg(color));
    }
}

/// `312.17 ─────●──────── 316.91` — a track with a `●` at the current price.
fn render_range_bar(
    buf: &mut Buffer,
    row: u16,
    area: Rect,
    range: (f64, f64),
    price: f64,
    theme: Theme,
) {
    let (lo, hi) = range;
    let lx = area.x + 2;
    let lo_s = format!("{lo:.2}");
    let hi_s = format!("{hi:.2}");
    buf.set_string(lx, row, &lo_s, Style::default().fg(theme.muted));
    let hi_start = (area.x + DETAIL_RAIL_WIDTH - 3).saturating_sub(hi_s.chars().count() as u16 - 1);
    buf.set_string(hi_start, row, &hi_s, Style::default().fg(theme.muted));

    // Track between the two labels.
    let track_start = lx + lo_s.chars().count() as u16 + 1;
    let track_end = hi_start.saturating_sub(1);
    if track_end <= track_start {
        return;
    }
    let width = track_end - track_start;
    for x in 0..width {
        buf.set_string(
            track_start + x,
            row,
            glyph::RSI_BAND.to_string(),
            Style::default().fg(theme.border), // bar line = border
        );
    }
    let t = if hi > lo {
        ((price - lo) / (hi - lo)).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let marker = track_start + (t * (width - 1) as f64).round() as u16;
    buf.set_string(
        marker,
        row,
        glyph::RANGE_MARKER.to_string(),
        Style::default().fg(theme.accent),
    );
}

// ---- formatting ------------------------------------------------------------

fn last(series: &[Option<f64>]) -> Option<f64> {
    series.iter().rev().flatten().copied().next()
}

/// Volume with a magnitude suffix: `48.2M`, `1.3B`.
fn fmt_vol(v: Option<f64>) -> String {
    match v {
        Some(x) if x >= 1e9 => format!("{:.1}B", x / 1e9),
        Some(x) if x >= 1e6 => format!("{:.1}M", x / 1e6),
        Some(x) if x >= 1e3 => format!("{:.1}K", x / 1e3),
        Some(x) => format!("{x:.0}"),
        None => "—".into(),
    }
}

/// Market cap: `$4.71 T`, `$812 B`.
fn fmt_cap(x: f64) -> String {
    if x >= 1e12 {
        format!("${:.2} T", x / 1e12)
    } else if x >= 1e9 {
        format!("${:.0} B", x / 1e9)
    } else {
        format!("${:.0} M", x / 1e6)
    }
}
