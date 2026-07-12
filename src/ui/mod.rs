//! UI layer. Reads `&AppState`, never mutates it. `render()` is the root that
//! splits the frame and dispatches to the per-view modules.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::AppState;
use crate::config::Config;

pub mod layout;
pub mod theme;

pub mod bottom_bar;
pub mod chart;
pub mod detail;
pub mod help;
pub mod overview;
pub mod settings;
pub mod table;

use crate::app::View;

/// Draw the whole frame: the current view, then the help overlay on top if open.
/// `config` is read-only reference data (the Settings screen displays it); the
/// mutable app state still flows only through `AppState`.
pub fn render(frame: &mut Frame, state: &AppState, config: &Config) {
    let area = frame.area();
    // Below the minimum, the fixed panels (watchlist 40, rail 32, the strips) no
    // longer fit — bail to a legible notice instead of a broken/panicking layout.
    if area.width < layout::MIN_WIDTH || area.height < layout::MIN_HEIGHT {
        render_too_small(frame);
        return;
    }
    match state.view {
        View::Overview => overview::render(frame, state),
        View::Detail => detail::render(frame, state),
        View::Settings => settings::render(frame, state, config),
    }
    if state.show_help {
        help::render(frame);
    }
}

/// Centered notice shown when the terminal is smaller than `MIN_WIDTH x MIN_HEIGHT`.
fn render_too_small(frame: &mut Frame) {
    let area = frame.area();
    let msg = format!(
        "terminal too small (needs {}x{})",
        layout::MIN_WIDTH,
        layout::MIN_HEIGHT
    );
    let style = Style::default().fg(theme::Theme::TOKYONIGHT.warn);
    let buf = frame.buffer_mut();
    let x = area.x + area.width.saturating_sub(msg.chars().count() as u16) / 2;
    let y = area.y + area.height / 2;
    // Clip: on a truly tiny terminal, write only what fits on the row.
    let room = area.width.saturating_sub(x - area.x) as usize;
    let shown: String = msg.chars().take(room).collect();
    buf.set_string(x, y, shown, style);
}

/// Draw a bordered box. Non-empty `title` renders `┌─ title ─…─┐` in the top
/// border; empty renders a plain `┌─…─┐`. The interior is left for the caller.
pub(crate) fn draw_box(buf: &mut Buffer, area: Rect, title: &str, style: Style) {
    let w = area.width as usize;
    let h = area.height;
    if w < 2 || h < 2 {
        return;
    }
    let top = if title.is_empty() {
        format!("┌{}┐", "─".repeat(w - 2))
    } else {
        let prefix = format!("┌─ {title} ");
        let fill = w.saturating_sub(prefix.chars().count() + 1);
        format!("{prefix}{}┐", "─".repeat(fill))
    };
    buf.set_string(area.x, area.y, top, style);
    for row in 1..h - 1 {
        buf.set_string(area.x, area.y + row, "│", style);
        buf.set_string(area.x + area.width - 1, area.y + row, "│", style);
    }
    buf.set_string(
        area.x,
        area.y + h - 1,
        format!("└{}┘", "─".repeat(w - 2)),
        style,
    );
}
