//! State-machine regressions: view transitions must leave the Overview-scoped
//! modes (`focus`, `input_mode`) coherent. Driven through the real `update()` with
//! synthetic key events — no network (spawned fetch tasks just fail harmlessly).

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;

use quotail::action::Action;
use quotail::app::{AppState, FocusRegion, InputMode, View};
use quotail::config::Config;
use quotail::data::store::DataStore;
use quotail::data::yahoo::YahooProvider;
use quotail::event_loop::{EventCtx, initial_state, update};

fn ctx() -> EventCtx {
    let (tx, _rx) = mpsc::unbounded_channel();
    let store = Arc::new(DataStore::new(Box::new(YahooProvider::new().unwrap())));
    EventCtx {
        tx,
        store,
        config: Arc::new(Config::default()),
    }
}

fn key(state: &mut AppState, ctx: &EventCtx, code: KeyCode) {
    update(state, Action::Key(KeyEvent::from(code)), ctx);
}

fn base() -> AppState {
    let mut cfg = Config::default();
    cfg.watchlist.stocks = vec!["AAPL".into(), "TSLA".into(), "NVDA".into()];
    initial_state(&cfg, None)
}

/// The reported bug: focus the chart grid (`l`), open Detail (`d`), Esc back — and
/// j/k are dead on the watchlist because `focus` was stranded on `Grid`. After the
/// fix, returning to Overview restores `focus == Table` and `input_mode == Normal`,
/// and a `j` keypress moves the selection again.
#[tokio::test]
async fn back_from_detail_restores_table_focus_and_normal_mode() {
    let ctx = ctx();
    let mut s = base();

    key(&mut s, &ctx, KeyCode::Char('l')); // focus the chart grid
    assert_eq!(s.focus, FocusRegion::Grid);

    key(&mut s, &ctx, KeyCode::Char('d')); // open Detail on the selected row
    assert_eq!(s.view, View::Detail);

    key(&mut s, &ctx, KeyCode::Esc); // Back to Overview
    assert_eq!(s.view, View::Overview);
    assert_eq!(s.input_mode, InputMode::Normal, "input_mode stranded");
    assert_eq!(s.focus, FocusRegion::Table, "focus stranded on Grid");

    // The payoff: `j` produces MoveSelection again (observable as a cursor move).
    let before = s.selected_row;
    key(&mut s, &ctx, KeyCode::Char('j'));
    assert_eq!(s.selected_row, before + 1, "j did not move the selection");
}

/// Back from Settings must land on a coherent Overview too (same choke point).
#[tokio::test]
async fn back_from_settings_restores_table_focus() {
    let ctx = ctx();
    let mut s = base();

    key(&mut s, &ctx, KeyCode::Char('l')); // grid focus
    update(&mut s, Action::SetView(View::Settings), &ctx);
    assert_eq!(s.view, View::Settings);

    key(&mut s, &ctx, KeyCode::Esc); // Back
    assert_eq!(s.view, View::Overview);
    assert_eq!(s.focus, FocusRegion::Table);
    assert_eq!(s.input_mode, InputMode::Normal);
}

/// Guard rail: closing the help overlay with Esc must NOT change view or focus —
/// `Back` only resets when it actually returns to Overview.
#[tokio::test]
async fn esc_closing_help_does_not_touch_focus() {
    let ctx = ctx();
    let mut s = base();

    key(&mut s, &ctx, KeyCode::Char('l')); // grid focus
    key(&mut s, &ctx, KeyCode::Char('?')); // open help
    assert!(s.show_help);

    key(&mut s, &ctx, KeyCode::Esc); // closes help only
    assert!(!s.show_help);
    assert_eq!(
        s.focus,
        FocusRegion::Grid,
        "help-close should leave focus alone"
    );
}
