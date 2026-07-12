//! UI layer. Reads `&AppState`, never mutates it. `render()` is the root that
//! splits the frame and dispatches to the per-view modules.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::AppState;

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
/// The Settings screen reads/edits `state.config`; everything flows through
/// `AppState`.
pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    // Paint the theme background across the whole frame first. Later text is drawn
    // with fg-only styles, which patch the fg and PRESERVE this bg.
    frame
        .buffer_mut()
        .set_style(area, Style::default().bg(theme::Theme::TOKYONIGHT.bg));

    // Below the minimum, the fixed panels (watchlist 40, rail 32, the strips) no
    // longer fit — bail to a legible notice instead of a broken/panicking layout.
    if area.width < layout::MIN_WIDTH || area.height < layout::MIN_HEIGHT {
        render_too_small(frame);
        return;
    }
    match state.view {
        View::Overview => overview::render(frame, state),
        View::Detail => detail::render(frame, state),
        View::Settings => settings::render(frame, state),
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

/// Draw a bordered box. Non-empty `title` renders `┌─ title ─…─┐` with the TITLE in
/// `title_style` and the border in `border`; empty renders a plain `┌─…─┐`. Titles
/// get their own style so they don't inherit the (often dimmed) border color.
pub(crate) fn draw_box(
    buf: &mut Buffer,
    area: Rect,
    title: &str,
    border: Style,
    title_style: Style,
) {
    let w = area.width as usize;
    let h = area.height;
    if w < 2 || h < 2 {
        return;
    }
    if title.is_empty() {
        buf.set_string(area.x, area.y, format!("┌{}┐", "─".repeat(w - 2)), border);
    } else {
        // `┌─ ` + title + ` ` + fill dashes + `┐`, with the title painted on its own.
        let title_len = title.chars().count();
        let fill = w.saturating_sub(title_len + 5);
        buf.set_string(area.x, area.y, "┌─ ", border);
        buf.set_string(area.x + 3, area.y, title, title_style);
        buf.set_string(
            area.x + 3 + title_len as u16,
            area.y,
            format!(" {}┐", "─".repeat(fill)),
            border,
        );
    }
    for row in 1..h - 1 {
        buf.set_string(area.x, area.y + row, "│", border);
        buf.set_string(area.x + area.width - 1, area.y + row, "│", border);
    }
    buf.set_string(
        area.x,
        area.y + h - 1,
        format!("└{}┘", "─".repeat(w - 2)),
        border,
    );
}

/// Draw a box whose title is a sequence of independently-colored spans:
/// `┌─ span span … ─…─┐`. For titles that mix colors (e.g. `volume` in heading +
/// its numbers in cyan). The `┌─ ` prefix and trailing gap are border-colored; the
/// dashes and `┐` come from the plain frame underneath.
pub(crate) fn draw_box_titled(
    buf: &mut Buffer,
    area: Rect,
    border: Style,
    spans: &[(&str, Style)],
) {
    draw_box(buf, area, "", border, border);
    if area.width < 2 {
        return;
    }
    let y = area.y;
    let mut cx = area.x;
    let mut put = |cx: &mut u16, text: &str, st: Style| {
        buf.set_string(*cx, y, text, st);
        *cx += text.chars().count() as u16;
    };
    put(&mut cx, "┌─ ", border);
    for (text, st) in spans {
        put(&mut cx, text, *st);
    }
    put(&mut cx, " ", border);
}
