//! Detail view: main candlestick chart + MA overlays + volume histogram + RSI on
//! the left, a fundamentals rail on the right.
//!
//! Fully RESPONSIVE. The bar is always the last two rows and the rail is a FIXED
//! 32 cols on the right; the left column takes the rest (`W - 32`). Vertically,
//! volume (5) and rsi (6) are FIXED-HEIGHT strips and the MAIN CHART absorbs the
//! slack in both axes — a wider chart just shows more candle columns over the same
//! span. At 96x31: main 64 wide, rows 0-17 (16 body); volume 18-22; rsi 23-28;
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
    let title = detail.map(main_title).unwrap_or_default();
    super::draw_box(buf, area, &title, border);

    // Replace the plain bottom border with the ma50/ma200 legend + timeframe.
    let bottom = main_bottom_border(state.timeframe.label(), area.width);
    let by = area.y + area.height - 1;
    buf.set_string(area.x, by, bottom, border);
    // Legend dots keep their MA colors (ma50 warn, ma200 heading).
    buf.set_string(
        area.x + 9,
        by,
        glyph::MA_DOT.to_string(),
        Style::default().fg(theme.warn),
    );
    buf.set_string(
        area.x + 19,
        by,
        glyph::MA_DOT.to_string(),
        Style::default().fg(theme.heading),
    );

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
    }
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
    let title = detail.map(volume_title).unwrap_or_else(|| "volume".into());
    super::draw_box(buf, area, &title, Style::default().fg(theme.border));

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
    }
}

fn volume_title(d: &DetailState) -> String {
    match d.quote.as_ref() {
        Some(q) => format!(
            "volume  {} · avg {}",
            fmt_vol(q.volume),
            fmt_vol(q.avg_volume)
        ),
        None => "volume".into(),
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
    let title = detail.map(rsi_title).unwrap_or_else(|| "rsi (14)".into());
    super::draw_box(buf, area, &title, Style::default().fg(theme.border));

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
    }
}

fn rsi_title(d: &DetailState) -> String {
    match last(&d.indicators.rsi) {
        Some(v) => format!("rsi (14)  {v:.1}"),
        None => "rsi (14)".into(),
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
    // The rail spans the full panel height; its content stays top-aligned (rows are
    // measured from the top border, and the rail is always anchored at y=0).
    super::draw_box(buf, area, "fundamentals", Style::default().fg(theme.border));

    let label = Style::default().fg(theme.fg);
    let head = Style::default().fg(theme.heading);
    let value = Style::default().fg(theme.fg);
    let lx = area.x + 2; // label column
    let vend = area.x + DETAIL_RAIL_WIDTH - 3; // value right edge

    let mut put_label = |row: u16, text: &str, style: Style| {
        buf.set_string(lx, row, text, style);
    };

    // Static structure — labels and section headers are the contract; the values
    // (right-aligned) come from the quote / fundamentals / indicators when present.
    put_label(2, "market cap", label);
    put_label(3, "p/e (ttm)", label);
    put_label(4, "p/e (fwd)", label);
    put_label(5, "eps (ttm)", label);
    put_label(6, "div yield", label);
    put_label(7, "beta", label);
    put_label(9, "day range", head);
    put_label(12, "52-wk range", head);
    put_label(15, "indicators", head);
    put_label(16, "ma50", label);
    put_label(17, "ma200", label);
    put_label(18, "rsi (14)", label);
    put_label(20, "session", head);
    put_label(21, "open", label);
    put_label(22, "prev close", label);
    put_label(23, "day high", label);
    put_label(24, "day low", label);

    let Some(d) = detail else { return };
    let f = d.fundamentals.as_ref();

    let mut put_value = |row: u16, text: String| {
        let start = vend.saturating_sub(text.chars().count() as u16 - 1);
        buf.set_string(start, row, text, value);
    };

    if let Some(f) = f {
        if let Some(v) = f.market_cap {
            put_value(2, fmt_cap(v));
        }
        if let Some(v) = f.pe_trailing {
            put_value(3, format!("{v:.1}"));
        }
        if let Some(v) = f.pe_forward {
            put_value(4, format!("{v:.1}"));
        }
        if let Some(v) = f.eps_trailing {
            put_value(5, format!("{v:.2}"));
        }
        if let Some(v) = f.div_yield {
            put_value(6, format!("{v:.2}%"));
        }
        if let Some(v) = f.beta {
            put_value(7, format!("{v:.2}"));
        }
    }
    if let Some(v) = last(&d.indicators.ma50) {
        put_value(16, format!("{v:.2}"));
    }
    if let Some(v) = last(&d.indicators.ma200) {
        put_value(17, format!("{v:.2}"));
    }
    if let Some(v) = last(&d.indicators.rsi) {
        put_value(18, format!("{v:.1}"));
    }

    if let Some(q) = d.quote.as_ref() {
        if let Some(o) = q.open {
            put_value(21, format!("{o:.2}"));
        }
        put_value(22, format!("{:.2}", q.prev_close));
        put_value(23, format!("{:.2}", q.day_range.1));
        put_value(24, format!("{:.2}", q.day_range.0));
        // Range bars (row 10 day, row 13 52-wk) — masked in the snapshot.
        render_range_bar(buf, 10, area, q.day_range, q.price, theme);
        render_range_bar(buf, 13, area, q.week52_range, q.price, theme);
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
            Style::default().fg(theme.muted),
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
