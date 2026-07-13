//! Theme: colors and glyphs. The `tokyonight` values below are the approved
//! default and match `docs/ASCII_REFERENCE.md`. Do not substitute glyphs.
//!
//! COLOR DEPTH. Not every terminal speaks 24-bit RGB. macOS Terminal.app, for one,
//! has an empty `$COLORTERM` and mangles `Color::Rgb` into a wash of blue/cyan —
//! candles, borders, and selection collapse into indistinguishable colors. So the
//! theme ships in THREE parallel palettes ([`Theme::TOKYONIGHT`] for truecolor,
//! [`Theme::TOKYONIGHT_256`] for 256-color, [`Theme::TOKYONIGHT_16`] for 16-color),
//! and [`ColorDepth::detect`] picks one at startup. The 256/16 values are
//! HAND-PICKED, not nearest-RGB conversions — auto-conversion is exactly what
//! destroys the distinctions on a limited terminal.
//!
//! INVARIANT: every palette imposes its OWN background (never `Color::Reset`), so
//! Quotail looks the same on a light or dark terminal — we own the frame.

use ratatui::style::Color;

/// How many colors the terminal can render. Chosen once at startup; each tier maps
/// to a hand-tuned palette via [`Theme::for_depth`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    /// 24-bit RGB. `Color::Rgb` is emitted verbatim.
    Truecolor,
    /// 256-color (xterm-256color). `Color::Indexed` from the 6×6×6 cube + grayscale.
    Ansi256,
    /// 16-color (or worse). Only the standard ANSI names are safe.
    Basic,
}

impl ColorDepth {
    /// Detect the terminal's color depth at startup.
    ///
    /// `QUOTAIL_COLOR=truecolor|256|basic` forces a tier (so the fallbacks can be
    /// inspected on any machine); otherwise we sniff the environment in three tiers:
    ///   - truecolor — `$COLORTERM` is `truecolor` or `24bit`
    ///   - 256-color — `$TERM` contains `256color`
    ///   - basic     — everything else
    ///
    /// crossterm exposes [`crossterm::style::available_color_count`], but it reads
    /// `$COLORTERM` *before* `$TERM` and treats a set-but-empty `$COLORTERM` as 8
    /// colors — which is precisely the Terminal.app case (empty `$COLORTERM`,
    /// `$TERM=xterm-256color`) we need to resolve to 256. So we sniff ourselves.
    pub fn detect() -> ColorDepth {
        if let Ok(forced) = std::env::var("QUOTAIL_COLOR") {
            match forced.trim().to_ascii_lowercase().as_str() {
                "truecolor" | "24bit" | "24" => return ColorDepth::Truecolor,
                "256" | "256color" => return ColorDepth::Ansi256,
                "basic" | "16" | "8" | "ansi" => return ColorDepth::Basic,
                // Unknown value: ignore the override and fall through to detection.
                _ => {}
            }
        }
        Self::from_env(
            std::env::var("COLORTERM").ok().as_deref(),
            std::env::var("TERM").ok().as_deref(),
        )
    }

    /// The pure tier logic, split out so it's testable without touching the process
    /// environment. `""` (set-but-empty `$COLORTERM`) must NOT count as truecolor.
    fn from_env(colorterm: Option<&str>, term: Option<&str>) -> ColorDepth {
        let ct = colorterm.unwrap_or("");
        if ct.contains("truecolor") || ct.contains("24bit") {
            return ColorDepth::Truecolor;
        }
        if term.unwrap_or("").contains("256color") {
            return ColorDepth::Ansi256;
        }
        ColorDepth::Basic
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Window background. Applied to the whole frame so the palette is the palette,
    /// not the terminal's default.
    pub bg: Color,
    /// Default foreground.
    pub fg: Color,
    /// Muted text: labels, hints, axis numbers, the passive status row.
    pub muted: Color,
    /// Positive change, up candles, up volume bars.
    pub up: Color,
    /// Negative change, down candles, down volume bars.
    pub down: Color,
    /// Keybind glyphs in the hint strip, active timeframe markers, range dots.
    pub accent: Color,
    /// Prices in tables and headers.
    pub price: Color,
    /// Active timeframe label, MA50 overlay.
    pub warn: Color,
    /// Panel borders (unfocused).
    pub border: Color,
    /// Panel border when focused, and the help overlay frame.
    pub border_focus: Color,
    /// Section headings, table headers, MA200 overlay.
    pub heading: Color,
    /// Selected table row background.
    pub selection: Color,
    /// Dimmed background content behind the help overlay.
    pub dim: Color,
}

impl Theme {
    pub const TOKYONIGHT: Theme = Theme {
        bg: Color::Rgb(0x16, 0x18, 0x1f),
        fg: Color::Rgb(0xc0, 0xca, 0xf5),
        muted: Color::Rgb(0x56, 0x5f, 0x89),
        up: Color::Rgb(0x9e, 0xce, 0x6a),
        down: Color::Rgb(0xf7, 0x76, 0x8e),
        accent: Color::Rgb(0xff, 0x9e, 0x64),
        price: Color::Rgb(0x7d, 0xcf, 0xff),
        warn: Color::Rgb(0xe0, 0xaf, 0x68),
        border: Color::Rgb(0x3b, 0x42, 0x61),
        border_focus: Color::Rgb(0x7a, 0xa2, 0xf7),
        heading: Color::Rgb(0xbb, 0x9a, 0xf7),
        selection: Color::Rgb(0x2a, 0x2f, 0x45),
        dim: Color::Rgb(0x23, 0x28, 0x38),
    };

    /// 256-color tokyonight. HAND-PICKED `Color::Indexed` values (6×6×6 cube +
    /// 232–255 grayscale ramp), tuned as a set against the `234` background — NOT a
    /// nearest-RGB conversion of the truecolor palette. Every required distinction
    /// (up/down, selection/bg, border/focus, muted/fg, and the accent family) is
    /// preserved. See the palette table in `docs/ASCII_REFERENCE.md`-adjacent notes.
    pub const TOKYONIGHT_256: Theme = Theme {
        bg: Color::Indexed(234),           // #1c1c1c — imposed dark, near-black
        fg: Color::Indexed(189),           // #d7d7ff — light lavender
        muted: Color::Indexed(60),         // #5f5f87 — dim blue-gray labels
        up: Color::Indexed(113),           // #87d75f — green
        down: Color::Indexed(204),         // #ff5f87 — red/pink (max contrast vs up)
        accent: Color::Indexed(216),       // #ffaf87 — orange
        price: Color::Indexed(117),        // #87d7ff — cyan
        warn: Color::Indexed(179),         // #d7af5f — gold
        border: Color::Indexed(238),       // #444444 — dim frame, recedes on bg
        border_focus: Color::Indexed(111), // #87afff — bright blue, obvious focus
        heading: Color::Indexed(141),      // #af87ff — purple
        selection: Color::Indexed(237),    // #3a3a3a — clearly lifted off bg(234)
        dim: Color::Indexed(235),          // #262626 — a hair above bg
    };

    /// 16-color tokyonight: standard ANSI names only — the sole safe choice on a
    /// terminal that can't do 256 colors. Background is ANSI black and every
    /// foreground is legible on it. Lower-priority hues collapse where 16 colors
    /// can't keep them apart (accent≈warn are both warm), but up/down, selection/bg,
    /// border/focus, and muted/fg never do.
    pub const TOKYONIGHT_16: Theme = Theme {
        bg: Color::Black,
        fg: Color::White,               // bright white on black
        muted: Color::DarkGray,         // dim vs fg (never lands on a selection row)
        up: Color::Green,
        down: Color::Red,               // the one distinction that carries the app
        accent: Color::LightYellow,     // no ANSI orange; warm, distinct from Yellow
        price: Color::Cyan,
        warn: Color::Yellow,
        border: Color::Blue,            // dim vs the bright focus below
        border_focus: Color::LightBlue,
        heading: Color::Magenta,
        selection: Color::DarkGray,     // gray highlight row on black
        dim: Color::Black,
    };

    /// The palette for a detected [`ColorDepth`]. This is how a limited terminal
    /// gets `Color::Indexed`/ANSI names instead of the `Color::Rgb` it can't render.
    pub fn for_depth(depth: ColorDepth) -> Theme {
        match depth {
            ColorDepth::Truecolor => Theme::TOKYONIGHT,
            ColorDepth::Ansi256 => Theme::TOKYONIGHT_256,
            ColorDepth::Basic => Theme::TOKYONIGHT_16,
        }
    }

    /// The active theme for THIS terminal — detect the depth, then pick the palette.
    /// Called once at startup; the result is stored on `AppState` and read by every
    /// renderer.
    pub fn detect() -> Theme {
        Theme::for_depth(ColorDepth::detect())
    }

    pub fn by_name(_name: &str) -> Theme {
        // Only tokyonight is implemented; every name falls back to it. Color DEPTH
        // (truecolor/256/16) is orthogonal — see `for_depth`/`detect`.
        Theme::TOKYONIGHT
    }

    /// Color for a signed change value.
    pub fn change(&self, v: f64) -> Color {
        if v >= 0.0 { self.up } else { self.down }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- detection: the three tiers, incl. the Terminal.app case ----

    #[test]
    fn detects_truecolor_from_colorterm() {
        assert_eq!(
            ColorDepth::from_env(Some("truecolor"), Some("xterm-256color")),
            ColorDepth::Truecolor
        );
        assert_eq!(
            ColorDepth::from_env(Some("24bit"), None),
            ColorDepth::Truecolor
        );
    }

    #[test]
    fn empty_colorterm_falls_through_to_term() {
        // The exact macOS Terminal.app bug: $COLORTERM is set-but-empty and
        // $TERM=xterm-256color. This MUST resolve to 256, not basic.
        assert_eq!(
            ColorDepth::from_env(Some(""), Some("xterm-256color")),
            ColorDepth::Ansi256
        );
        assert_eq!(
            ColorDepth::from_env(None, Some("screen-256color")),
            ColorDepth::Ansi256
        );
    }

    #[test]
    fn falls_back_to_basic() {
        assert_eq!(ColorDepth::from_env(Some(""), Some("xterm")), ColorDepth::Basic);
        assert_eq!(ColorDepth::from_env(None, None), ColorDepth::Basic);
    }

    // ---- palette distinctions that must survive at every depth ----

    /// Assert the priority-ordered distinctions from the spec all hold for `t`.
    fn assert_distinct(name: &str, t: Theme) {
        let req = [
            ("up vs down", t.up, t.down),
            ("selection vs bg", t.selection, t.bg),
            ("border vs border_focus", t.border, t.border_focus),
            ("muted vs fg", t.muted, t.fg),
            // accent family: distinct from each other and from fg.
            ("accent vs warn", t.accent, t.warn),
            ("accent vs heading", t.accent, t.heading),
            ("accent vs price", t.accent, t.price),
            ("warn vs heading", t.warn, t.heading),
            ("warn vs price", t.warn, t.price),
            ("heading vs price", t.heading, t.price),
            ("accent vs fg", t.accent, t.fg),
            ("warn vs fg", t.warn, t.fg),
            ("heading vs fg", t.heading, t.fg),
            ("price vs fg", t.price, t.fg),
        ];
        for (label, a, b) in req {
            assert_ne!(a, b, "{name}: {label} collapsed ({a:?} == {b:?})");
        }
    }

    #[test]
    fn every_depth_keeps_its_distinctions() {
        assert_distinct("truecolor", Theme::TOKYONIGHT);
        assert_distinct("256", Theme::TOKYONIGHT_256);
        assert_distinct("16", Theme::TOKYONIGHT_16);
    }

    #[test]
    fn no_palette_uses_reset_background() {
        // The frame is always painted, so bg must be a concrete color at every depth.
        for t in [Theme::TOKYONIGHT, Theme::TOKYONIGHT_256, Theme::TOKYONIGHT_16] {
            assert_ne!(t.bg, Color::Reset, "bg must be imposed, never Reset");
        }
    }

    #[test]
    fn fallbacks_avoid_rgb() {
        // A limited terminal must never be handed Color::Rgb — that's the bug.
        for t in [Theme::TOKYONIGHT_256, Theme::TOKYONIGHT_16] {
            for c in [
                t.bg, t.fg, t.muted, t.up, t.down, t.accent, t.price, t.warn, t.border,
                t.border_focus, t.heading, t.selection, t.dim,
            ] {
                assert!(!matches!(c, Color::Rgb(..)), "fallback leaked Rgb: {c:?}");
            }
        }
    }
}

/// Glyphs. These are part of the visual contract — do not swap them.
pub mod glyph {
    /// Candle body. Solid block, deliberately not braille.
    pub const CANDLE_BODY: char = '█';
    /// Candle wick.
    pub const CANDLE_WICK: char = '│';
    /// Volume histogram bar.
    pub const VOLUME_BAR: char = '█';
    /// Moving-average overlay. Drawn ONLY where the cell is empty:
    /// a candle always wins over an MA dot.
    pub const MA_DOT: char = '·';
    /// RSI line point.
    pub const RSI_POINT: char = '●';
    /// RSI 70/30 band.
    pub const RSI_BAND: char = '─';
    /// Position marker on the day / 52-week range bars.
    pub const RANGE_MARKER: char = '●';
    /// Hot-movers tape.
    pub const UP_ARROW: char = '▲';
    pub const DOWN_ARROW: char = '▼';
    /// Sort direction indicator in the table header.
    pub const SORT_DESC: char = '▼';
    pub const SORT_ASC: char = '▲';
    /// Watchlist scrollbar: thumb and track.
    pub const SCROLL_THUMB: char = '█';
    pub const SCROLL_TRACK: char = '│';
}
