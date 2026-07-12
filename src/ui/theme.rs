//! Theme: colors and glyphs. The `tokyonight` values below are the approved
//! default and match `docs/ASCII_REFERENCE.md`. Do not substitute glyphs.

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
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

    pub fn by_name(name: &str) -> Theme {
        match name {
            // gruvbox / catppuccin / nord are follow-ups; default for now.
            _ => Theme::TOKYONIGHT,
        }
    }

    /// Color for a signed change value.
    pub fn change(&self, v: f64) -> Color {
        if v >= 0.0 { self.up } else { self.down }
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
