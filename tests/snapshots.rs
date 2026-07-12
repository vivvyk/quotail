//! Snapshot tests: render each view at 96x31 with FIXED synthetic data (no
//! network) and assert the rendered buffer's CHROME matches the corresponding
//! block in `docs/ASCII_REFERENCE.md`.
//!
//! Per SPEC section 3, the reference pins the *frame* (borders, labels, column
//! positions, panel widths) — NOT candle/volume interiors, which depend on data.
//! So each test carries a `masks` list of interior rectangles that are excluded
//! from the comparison. Those interiors get their own exact-glyph tests once the
//! chart renderer exists.
//!
//! STATUS: the renderers are stubs, so these currently FAIL against a blank
//! buffer — that's the tests-first checkpoint. Interior masks are filled in as
//! each screen is built.

use std::collections::HashMap;

use chrono::{TimeZone, Utc};
use ratatui::{Terminal, backend::TestBackend};

use quotail::app::{
    AppState, AssetFilter, ChartSlot, DetailState, FocusRegion, InputMode, SortKey, View,
};
use quotail::data::indicators::compute;
use quotail::data::types::{AssetKind, Candle, Fundamentals, MarketStatus, Quote, Timeframe};

// ---- reference extraction --------------------------------------------------

/// The four fenced code blocks in ASCII_REFERENCE.md, in file order:
/// [0] Overview, [1] Detail, [2] Help, [3] Settings.
fn ref_blocks() -> Vec<Vec<String>> {
    let doc = include_str!("../docs/ASCII_REFERENCE.md");
    let mut blocks = Vec::new();
    let mut cur: Option<Vec<String>> = None;
    for line in doc.lines() {
        if line.trim_start().starts_with("```") {
            match cur.take() {
                Some(block) => blocks.push(block),
                None => cur = Some(Vec::new()),
            }
        } else if let Some(block) = cur.as_mut() {
            block.push(line.to_string());
        }
    }
    blocks
}

// ---- render + compare ------------------------------------------------------

fn render_lines(state: &AppState) -> Vec<String> {
    render_at(state, 96, 31)
}

/// Render a view at an arbitrary terminal size (for the responsive-layout tests).
fn render_at(state: &AppState, w: u16, h: u16) -> Vec<String> {
    // The Settings screen reads config; other views ignore it. A default config is
    // enough here — Settings values (and the config path) are masked in that test.
    let config = quotail::config::Config::default();
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| quotail::ui::render(f, state, &config))
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect()
}

/// Compare rendered lines to the reference, ignoring masked interior cells.
/// Returns a readable report on mismatch, or `None` if the chrome matches.
fn chrome_diff(
    name: &str,
    expected: &[String],
    actual: &[String],
    masks: &[(usize, usize, usize)],
) -> Option<String> {
    let masked = |r: usize, c: usize| {
        masks
            .iter()
            .any(|&(mr, c0, c1)| mr == r && c >= c0 && c < c1)
    };
    let mut mismatches = 0;
    let mut first: Option<usize> = None;
    for (r, exp) in expected.iter().enumerate() {
        let ec: Vec<char> = exp.chars().collect();
        let ac: Vec<char> = actual
            .get(r)
            .map(|s| s.chars().collect())
            .unwrap_or_default();
        for c in 0..96 {
            if masked(r, c) {
                continue;
            }
            let e = ec.get(c).copied().unwrap_or(' ');
            let a = ac.get(c).copied().unwrap_or(' ');
            if e != a {
                mismatches += 1;
                first.get_or_insert(r);
            }
        }
    }
    // Rows beyond the reference block must be blank.
    for (r, line) in actual.iter().enumerate().skip(expected.len()) {
        if line.chars().any(|ch| ch != ' ') {
            mismatches += 1;
            first.get_or_insert(r);
        }
    }
    first.map(|r| {
        format!(
            "{name}: {mismatches} chrome mismatch(es). First differing row {r}:\n  expected |{}|\n  actual   |{}|",
            expected.get(r).map(String::as_str).unwrap_or("<beyond block>"),
            actual.get(r).map(String::as_str).unwrap_or(""),
        )
    })
}

// ---- fixtures --------------------------------------------------------------

fn quote(sym: &str, price: f64, change_pct: f64) -> Quote {
    Quote {
        symbol: sym.to_string(),
        name: Some(sym.to_string()),
        price,
        change: price * change_pct / 100.0,
        change_pct,
        prev_close: price,
        open: Some(price),
        day_range: (price * 0.99, price * 1.01),
        week52_range: (price * 0.6, price * 1.05),
        volume: Some(1.0e6),
        avg_volume: Some(1.2e6),
        market_cap: Some(1.0e12),
        exchange: None,
        asset: AssetKind::infer(sym),
    }
}

/// Deterministic synthetic OHLCV so the dump shows real charts (no RNG — a trend
/// plus two sines). 12-hour spacing packs enough bars into the 1M window to fill
/// the 52-col detail chart; 300 bars give MA200 room to warm up.
fn synth_candles(base_price: f64) -> Vec<Candle> {
    let step = 43_200_i64; // 12h
    let start = 1_700_000_000_i64;
    (0..300)
        .map(|i| {
            let t = i as f64;
            let mid = base_price * (1.0 + 0.15 * (t / 40.0).sin() + 0.0008 * t);
            let amp = base_price * 0.02;
            let open = mid + amp * (t * 0.7).sin();
            let close = mid + amp * (t * 0.7 + 1.0).sin();
            let high = open.max(close) + amp * 0.5 * (t * 1.3).cos().abs();
            let low = open.min(close) - amp * 0.5 * (t * 1.1).sin().abs();
            let volume = 1.0e6 * (1.0 + 0.5 * (t * 0.5).sin().abs());
            Candle {
                ts: start + i * step,
                open,
                high,
                low,
                close,
                volume,
            }
        })
        .collect()
}

fn base_state() -> AppState {
    AppState {
        view: View::Overview,
        input_mode: InputMode::Normal,
        show_help: false,
        focus: FocusRegion::Table,
        watchlist: Vec::new(),
        quotes: HashMap::new(),
        filter: AssetFilter::All,
        sort: SortKey::ChangePct,
        sort_desc: true,
        selected_row: 0,
        scroll_offset: 0,
        slots: [None, None, None, None],
        focused_slot: 0,
        timeframe: Timeframe::M1,
        detail: None,
        command_input: String::new(),
        market_status: MarketStatus::Closed,
        // Fixed so the "Last Refresh: 22:20:01" clock is deterministic.
        last_refresh: Utc.with_ymd_and_hms(2024, 1, 6, 22, 20, 1).unwrap(),
        status_msg: None,
        marquee_offset: 0,
        should_quit: false,
    }
}

fn overview_state() -> AppState {
    let mut s = base_state();
    let rows = [
        ("AAPL", 315.32, -0.28),
        ("MSFT", 456.66, -0.97),
        ("NVDA", 210.96, 4.03),
        ("AMZN", 238.18, 0.11),
        ("META", 620.80, 0.43),
        ("GOOGL", 357.18, -0.48),
        ("AVGO", 182.44, 1.12),
        ("TSLA", 407.76, 0.30),
        ("BRK-B", 498.10, 0.08),
        ("LLY", 812.35, -1.44),
        ("JPM", 264.90, 0.62),
        ("V", 341.20, 0.19),
        ("XOM", 118.77, -0.31),
        ("UNH", 529.44, 2.05),
    ];
    for (sym, price, chg) in rows {
        s.watchlist.push(sym.to_string());
        s.quotes.insert(sym.to_string(), quote(sym, price, chg));
    }
    s.slots[0] = Some(ChartSlot {
        symbol: "NVDA".into(),
        candles: synth_candles(210.96),
        loading: false,
        error: None,
    });
    s.slots[1] = Some(ChartSlot {
        symbol: "BTC-USD".into(),
        candles: synth_candles(63972.0),
        loading: false,
        error: None,
    });
    s.slots[2] = Some(ChartSlot {
        symbol: "AAPL".into(),
        candles: synth_candles(315.32),
        loading: false,
        error: None,
    });
    s
}

fn detail_state() -> AppState {
    let mut s = base_state();
    s.view = View::Detail;
    let mut q = quote("AAPL", 326.77, 0.38);
    // Detail's status line shows the symbol's own exchange (from the quote).
    q.exchange = Some("NASDAQ".into());
    q.name = Some("Apple Inc.".into());
    let candles = synth_candles(326.77);
    let indicators = compute(&candles);
    s.detail = Some(DetailState {
        symbol: "AAPL".into(),
        quote: Some(q),
        candles,
        fundamentals: Some(Fundamentals {
            market_cap: Some(4.71e12),
            pe_trailing: Some(32.4),
            pe_forward: Some(28.1),
            eps_trailing: Some(9.73),
            div_yield: Some(0.42),
            beta: Some(1.24),
        }),
        indicators,
        loading: false,
        error: None,
    });
    s
}

fn settings_state() -> AppState {
    let mut s = base_state();
    s.view = View::Settings;
    s
}

fn help_state() -> AppState {
    let mut s = overview_state();
    s.show_help = true;
    s
}

// ---- the tests -------------------------------------------------------------
// masks: (row, col_start, col_end_exclusive) — interior regions excluded from
// the chrome comparison. Empty for now; populated as each renderer is built.

/// Overview interior masks. Everything data-dependent is excluded so only the
/// frame is asserted: the marquee tape, the watchlist rows + scrollbar, the three
/// filled panes' titles + candle bodies, and the deferred `(Opens …)` hint. The
/// empty pane (slot 3, bottom-right) is asserted in full — its plain border and
/// centered `enter to chart` ARE the contract.
fn overview_masks() -> Vec<(usize, usize, usize)> {
    let mut m = vec![(1, 1, 95)]; // marquee tape (cols 1..95)
    // Watchlist body: rows 5-18, cols 1-38 (data + scrollbar; borders 0/39 kept).
    // The scrollbar track (col 38) runs on down through the blank rows 19-27 in the
    // reference's full ~68-symbol list; the 14-symbol fixture draws none, so mask it.
    for r in 5..=18 {
        m.push((r, 1, 39));
    }
    for r in 19..=27 {
        m.push((r, 38, 39));
    }
    // Filled panes stretch to 13 rows each. Top panes 0/1 span rows 3-15 (title
    // row 3, candles 4-14, tf bottom border row 15). Bottom pane 2 spans rows 16-28
    // (title row 16, candles 17-27, tf bottom border row 28). Interiors 41..67 /
    // 69..95. The empty pane 3 (cols 68-95, rows 16-28) is asserted in full.
    m.push((3, 41, 67));
    m.push((3, 69, 95));
    for r in 4..=14 {
        m.push((r, 41, 67));
        m.push((r, 69, 95));
    }
    m.push((16, 41, 67));
    for r in 17..=27 {
        m.push((r, 41, 67));
    }
    m
}

/// Throwaway: writes all four rendered screens (at 96x31 and 120x50) to
/// `docs/render_dump.txt` for eyeballing.
/// Run with `cargo test --test snapshots dump_screens -- --ignored`.
#[test]
#[ignore]
fn dump_screens() {
    let ruler = |w: usize, f: &dyn Fn(usize) -> u32| {
        (0..w)
            .map(|i| char::from_digit(f(i), 10).unwrap())
            .collect::<String>()
    };
    let mut out = String::new();
    for (w, h) in [(96_u16, 31_u16), (120, 50), (60, 20)] {
        out.push_str(&format!("\n########## {w}x{h} ##########\n"));
        for (name, st) in [
            ("OVERVIEW", overview_state()),
            ("DETAIL", detail_state()),
            ("SETTINGS", settings_state()),
            ("HELP", help_state()),
        ] {
            out.push_str(&format!("\n===== {name} ({w}x{h}) =====\n"));
            out.push_str(&format!(
                "    {}\n",
                ruler(w as usize, &|i| (i / 10) as u32 % 10)
            ));
            out.push_str(&format!("    {}\n", ruler(w as usize, &|i| i as u32 % 10)));
            for (r, line) in render_at(&st, w, h).iter().enumerate() {
                out.push_str(&format!("{r:2}: {line}\n"));
            }
        }
    }
    std::fs::write("docs/render_dump.txt", out).unwrap();
}

#[test]
fn overview_chrome_matches_reference() {
    let expected = &ref_blocks()[0];
    let actual = render_lines(&overview_state());
    if let Some(report) = chrome_diff("overview", expected, &actual, &overview_masks()) {
        panic!("{report}");
    }
}

/// Detail interior masks. Asserted chrome: every box border, the main chart's
/// ma50/ma200/tf bottom border, the volume/rsi bottom borders, the fundamentals
/// title + static rail labels + section headers, and the status/hints bar. Masked:
/// the three panel interiors (candles, price axis, volume bars, rsi line), the
/// data-bearing panel titles, the rail values + range bars, and the `(Opens …)`
/// hint.
fn detail_masks() -> Vec<(usize, usize, usize)> {
    let mut m = Vec::new();
    // Main chart absorbs slack: title (row 0) + body (rows 1-16), interior 1-62.
    // (Bottom border row 17 carries the ma legend + tf and is asserted.)
    for r in 0..=16 {
        m.push((r, 1, 63));
    }
    // Volume strip: title (row 18) + bars (rows 19-21). Bottom border row 22 asserted.
    for r in 18..=21 {
        m.push((r, 1, 63));
    }
    // RSI strip: title (row 23) + body (rows 24-27). Bottom border row 28 asserted.
    for r in 23..=27 {
        m.push((r, 1, 63));
    }
    // Rail is top-aligned, so its label/value rows are unchanged. Values (right of
    // the labels): fundamentals rows 2-7, indicators 16-18, session 21-24.
    for r in [2, 3, 4, 5, 6, 7, 16, 17, 18, 21, 22, 23, 24] {
        m.push((r, 81, 95));
    }
    // Range bars span the whole rail interior.
    m.push((10, 65, 95));
    m.push((13, 65, 95));
    m
}

#[test]
fn detail_chrome_matches_reference() {
    let expected = &ref_blocks()[1];
    let actual = render_lines(&detail_state());
    if let Some(report) = chrome_diff("detail", expected, &actual, &detail_masks()) {
        panic!("{report}");
    }
}

/// Settings interior masks. Asserted: the frame, the section headers (watchlist /
/// display / data / mcp), the row labels (col 3), the `w writes…` line, and the
/// status/hints bar. Masked: the title's config path (env-dependent), every
/// value + hint (cols 22-94 on data rows), and the `(Opens …)` hint. The default
/// config used by the test has an empty watchlist and a different socket path, so
/// values can't be asserted here.
fn settings_masks() -> Vec<(usize, usize, usize)> {
    let mut m = vec![(0, 1, 95)]; // title path
    for r in [3, 4, 5, 8, 9, 10, 11, 12, 15, 16, 17, 20, 21] {
        m.push((r, 22, 95));
    }
    m
}

#[test]
fn settings_chrome_matches_reference() {
    let expected = &ref_blocks()[3];
    let actual = render_lines(&settings_state());
    if let Some(report) = chrome_diff("settings", expected, &actual, &settings_masks()) {
        panic!("{report}");
    }
}

#[test]
fn help_chrome_matches_reference() {
    // The help overlay is a static box centered horizontally at x=10 and
    // vertically in the 31-row frame → top row 3. Assert only the box region
    // (cols 10..86); the dimmed background outside it isn't part of the contract.
    let block = &ref_blocks()[2];
    let actual = render_lines(&help_state());
    const Y_OFF: usize = 3;

    let mut diffs = Vec::new();
    for (i, exp) in block.iter().enumerate() {
        let ec: Vec<char> = exp.chars().collect();
        let ac: Vec<char> = actual[Y_OFF + i].chars().collect();
        for (c, &e) in ec.iter().enumerate().take(86).skip(10) {
            let a = ac.get(c).copied().unwrap_or(' ');
            if e != a {
                diffs.push(format!(
                    "row {} col {c}: expected {e:?} got {a:?}",
                    Y_OFF + i
                ));
            }
        }
    }
    assert!(
        diffs.is_empty(),
        "help box mismatch ({} cells):\n{}",
        diffs.len(),
        diffs
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ---- responsive layout (taller/wider terminals) ----------------------------
// The 96x31 snapshots pin the frame; these assert the vertical behavior when the
// terminal is bigger: the bar hugs the floor and the charts absorb the slack while
// the fixed strips (volume/rsi) do not.

fn row_has(lines: &[String], r: usize, needle: &str) -> bool {
    lines.get(r).map(|l| l.contains(needle)).unwrap_or(false)
}

fn char_at(lines: &[String], r: usize, c: usize) -> Option<char> {
    lines.get(r).and_then(|l| l.chars().nth(c))
}

#[test]
fn overview_grid_stretches_and_bar_hugs_floor() {
    let lines = render_at(&overview_state(), 120, 50);
    assert_eq!(lines.len(), 50);
    // Bar on the last two rows.
    assert!(row_has(&lines, 49, "q quit"), "hints must be the last row");
    assert!(
        row_has(&lines, 48, "NYSE"),
        "status must be the second-last row"
    );
    // Vertical growth: the top pane-row's `1M` bottom border sits at row 24 (it was
    // row 15 at 96x31); the watchlist border reaches row 47.
    assert!(row_has(&lines, 24, " 1M "), "top panes should reach row 24");
    assert!(
        row_has(&lines, 47, "└"),
        "watchlist should reach the floor bar"
    );
    // Horizontal growth: the grid fills to the right edge — the top-right pane's
    // bottom-border corner is the last column (119), where 96x31 had col 67 interior.
    assert_eq!(
        char_at(&lines, 24, 119),
        Some('┘'),
        "grid must reach col 119"
    );
    assert_eq!(
        char_at(&lines, 24, 67),
        Some('─'),
        "old pane edge is now interior"
    );
}

#[test]
fn detail_main_chart_stretches_strips_stay_fixed() {
    let lines = render_at(&detail_state(), 120, 50);
    assert_eq!(lines.len(), 50);
    assert!(
        row_has(&lines, 49, "esc back"),
        "hints must be the last row"
    );
    assert!(
        row_has(&lines, 48, "NASDAQ"),
        "status must be the second-last row"
    );
    // Vertical growth: the ma50/ma200 bottom border is at row 36 (was row 17).
    assert!(
        row_has(&lines, 36, "ma50"),
        "main chart should stretch to row 36"
    );
    // Fixed strips keep their heights and sit just below the enlarged chart.
    assert!(
        row_has(&lines, 37, "volume"),
        "volume strip stays 5 rows tall"
    );
    assert!(
        row_has(&lines, 42, "rsi (14)"),
        "rsi strip stays 6 rows tall"
    );
    // Horizontal growth: the main column is now 88 wide (W - 32 rail), so its bottom
    // border corner is col 87, and the fixed rail hugs the right edge (corner 119).
    assert_eq!(
        char_at(&lines, 36, 87),
        Some('┘'),
        "main chart widens to col 87"
    );
    assert_eq!(
        char_at(&lines, 0, 119),
        Some('┐'),
        "rail stays pinned to the right"
    );
}

#[test]
fn tiny_terminal_shows_too_small_notice() {
    let lines = render_at(&overview_state(), 60, 20);
    let joined = lines.join("\n");
    assert!(
        joined.contains("terminal too small (needs 80x24)"),
        "sub-minimum terminals get the notice, got:\n{joined}"
    );
    // The real layout is NOT attempted (no banner / watchlist chrome).
    assert!(
        !joined.contains("hot movers"),
        "must not render the real layout"
    );
    assert!(!joined.contains("watchlist"));
}

#[test]
fn settings_is_top_aligned_with_bar_on_floor() {
    let lines = render_at(&settings_state(), 120, 50);
    assert_eq!(lines.len(), 50);
    assert!(
        row_has(&lines, 49, "j/k move"),
        "hints must be the last row"
    );
    // Fixed-height panel: its bottom border stays at row 25 regardless of height.
    assert!(row_has(&lines, 25, "└"), "settings box stays 26 rows tall");
    // The slack between the panel and the floor bar is blank.
    for (r, line) in lines.iter().enumerate().take(48).skip(26) {
        assert!(
            line.trim().is_empty(),
            "row {r} should be blank slack, got: {line:?}"
        );
    }
}
