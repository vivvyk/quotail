//! The event loop: the app's single mutator.
//!
//! Architecture (the ONE rule): many producers → one `mpsc<Action>` → one
//! consumer → one `&mut AppState`. `update()` is that consumer's body. It NEVER
//! awaits a network call: anything that needs data spawns a fetch task (a fourth
//! kind of producer) and returns immediately; when the data lands, that task
//! sends a `*Loaded` / `*Updated` / `FetchFailed` action back through the same
//! channel. That is what keeps the UI responsive while requests are in flight.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::action::Action;
use crate::app::{
    AppState, ChartSlot, DetailState, FocusRegion, InputMode, Session, SortKey, View,
};
use crate::config::Config;
use crate::data::store::DataStore;
use crate::data::types::{Indicators, MarketStatus, Quote, Timeframe};
use crate::ui::layout::{MARQUEE_TICK_MS, MAX_SLOTS};
use crate::ui::table;

/// Handles every producer and `update()` need. `tx` is cloned per producer; the
/// store is `Arc`-shared. Spawned fetch tasks capture clones of these, which is
/// why `update()` can start a fetch and return without awaiting it.
pub struct EventCtx {
    pub tx: UnboundedSender<Action>,
    pub store: Arc<DataStore>,
    pub config: Arc<Config>,
}

// ---- State construction ----------------------------------------------------

/// Build the initial state from config defaults, then overlay a restored session
/// if one exists. A missing session just means "use config defaults".
pub fn initial_state(config: &Config, session: Option<Session>) -> AppState {
    let mut state = AppState {
        view: View::Overview,
        input_mode: InputMode::Normal,
        show_help: false,
        focus: FocusRegion::Table,
        watchlist: config.watchlist(),
        quotes: HashMap::new(),
        filter: config.default_filter(),
        sort: config.default_sort(),
        sort_desc: true,
        selected_row: 0,
        scroll_offset: 0,
        slots: [None, None, None, None],
        focused_slot: 0,
        timeframe: config.default_timeframe(),
        detail: None,
        command_input: String::new(),
        market_status: MarketStatus::Closed,
        last_refresh: Utc::now(),
        status_msg: None,
        marquee_offset: 0,
        should_quit: false,
    };

    if let Some(s) = session {
        state.timeframe = s.timeframe;
        state.view = s.view;
        state.filter = s.filter;
        state.selected_row = s.selected_row;
        state.scroll_offset = s.scroll_offset;
        state.focused_slot = s.focused_slot.min(MAX_SLOTS - 1);
        for (i, sym) in s.slots.iter().enumerate() {
            if let Some(sym) = sym {
                // Restored as loading placeholders; their candles are fetched
                // at startup by the runner (via `refetch_charts`).
                state.slots[i] = Some(ChartSlot {
                    symbol: sym.clone(),
                    candles: Vec::new(),
                    loading: true,
                    error: None,
                });
            }
        }
    }
    state
}

/// The disposable slice of state to persist on quit.
pub fn session_snapshot(state: &AppState) -> Session {
    Session {
        slots: std::array::from_fn(|i| state.slots[i].as_ref().map(|s| s.symbol.clone())),
        focused_slot: state.focused_slot,
        timeframe: state.timeframe,
        view: state.view,
        selected_row: state.selected_row,
        scroll_offset: state.scroll_offset,
        filter: state.filter,
    }
}

// ---- Fetch tasks (async results coming home through the channel) -----------

fn spawn_quotes(ctx: &EventCtx, symbols: Vec<String>, force: bool) {
    let (store, tx) = (ctx.store.clone(), ctx.tx.clone());
    tokio::spawn(async move {
        match store.quotes(&symbols, force).await {
            Ok(quotes) => {
                let _ = tx.send(Action::QuotesUpdated(quotes));
            }
            Err(e) => {
                let _ = tx.send(Action::SetStatus(format!("quotes failed: {e}")));
            }
        }
    });
}

fn spawn_candles(ctx: &EventCtx, symbol: String, timeframe: Timeframe, force: bool) {
    let (store, tx) = (ctx.store.clone(), ctx.tx.clone());
    tokio::spawn(async move {
        match store.candles(&symbol, timeframe, force).await {
            Ok(data) => {
                let _ = tx.send(Action::CandlesLoaded {
                    symbol,
                    timeframe,
                    candles: data.candles,
                    indicators: data.indicators,
                });
            }
            Err(e) => {
                let _ = tx.send(Action::FetchFailed {
                    symbol,
                    error: e.to_string(),
                });
            }
        }
    });
}

fn spawn_fundamentals(ctx: &EventCtx, symbol: String, force: bool) {
    let (store, tx) = (ctx.store.clone(), ctx.tx.clone());
    tokio::spawn(async move {
        match store.fundamentals(&symbol, force).await {
            Ok(data) => {
                let _ = tx.send(Action::FundamentalsLoaded { symbol, data });
            }
            Err(e) => {
                let _ = tx.send(Action::FetchFailed {
                    symbol,
                    error: e.to_string(),
                });
            }
        }
    });
}

fn spawn_market_status(ctx: &EventCtx) {
    let (store, tx) = (ctx.store.clone(), ctx.tx.clone());
    tokio::spawn(async move {
        if let Ok(status) = store.market_status().await {
            let _ = tx.send(Action::MarketStatusUpdated(status));
        }
    });
}

// ---- Producer tasks --------------------------------------------------------

/// Polls quotes + market status on the configured interval. `tokio::time::interval`
/// fires its first tick immediately, so this doubles as the initial fetch.
fn spawn_poller(ctx: &EventCtx) {
    let store = ctx.store.clone();
    let tx = ctx.tx.clone();
    let symbols = ctx.config.watchlist();
    let period = ctx.config.poll_interval();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        loop {
            ticker.tick().await;
            match store.quotes(&symbols, false).await {
                Ok(quotes) => {
                    if tx.send(Action::QuotesUpdated(quotes)).is_err() {
                        break; // receiver gone: app is shutting down
                    }
                }
                Err(e) => {
                    if tx
                        .send(Action::SetStatus(format!("poll failed: {e}")))
                        .is_err()
                    {
                        break;
                    }
                }
            }
            if let Ok(status) = store.market_status().await {
                let _ = tx.send(Action::MarketStatusUpdated(status));
            }
        }
    });
}

/// Drives the marquee + clock. The app is never fully idle while this runs — a
/// known, config-gated cost noted in the SPEC.
fn spawn_tick(tx: UnboundedSender<Action>, period: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        loop {
            ticker.tick().await;
            if tx.send(Action::Tick).is_err() {
                break;
            }
        }
    });
}

/// Ctrl-C → clean `Quit`. Used only in headless mode; under the TUI's raw terminal
/// Ctrl-C arrives as a keystroke instead (handled in `key_to_action`).
fn spawn_ctrl_c(tx: UnboundedSender<Action>) {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = tx.send(Action::Quit);
        }
    });
}

/// The TUI input producer: a dedicated OS thread doing the BLOCKING terminal read.
/// It's a producer like any other — it forwards raw events as `Action`s and never
/// touches state. A plain thread (not an async task) keeps the blocking read off
/// the tokio worker pool and avoids pulling in a futures stream adapter.
fn spawn_input(tx: UnboundedSender<Action>) {
    use crossterm::event::{self, Event, KeyEventKind};
    std::thread::spawn(move || {
        loop {
            match event::read() {
                // Ignore key-release events (kitty / Windows emit them) so one
                // keystroke isn't processed twice.
                Ok(Event::Key(k)) if k.kind != KeyEventKind::Release => {
                    if tx.send(Action::Key(k)).is_err() {
                        break; // event loop gone — app is shutting down
                    }
                }
                Ok(Event::Resize(w, h)) => {
                    if tx.send(Action::Resize(w, h)).is_err() {
                        break;
                    }
                }
                Ok(_) => {} // mouse / focus / paste — unused
                Err(_) => break,
            }
        }
    });
}

// ---- The consumer: the ONLY thing that mutates AppState --------------------

/// Apply one action to the state. Exhaustive over `Action` on purpose: adding a
/// variant makes the compiler point here. NEVER awaits — network work is spawned.
pub fn update(state: &mut AppState, action: Action, ctx: &EventCtx) {
    match action {
        // ---- Raw input: interpret against the current mode, then dispatch ----
        Action::Key(key) => {
            if let Some(next) = key_to_action(state, &key) {
                update(state, next, ctx);
            }
        }
        Action::Mouse(_) => {} // mouse is a step-4 bonus, never the only path
        Action::Resize(_, _) => {} // the TUI reads terminal size directly

        // ---- Navigation ----
        Action::SetView(view) => state.view = view,
        Action::SetFocus(region) => state.focus = region,
        Action::OpenDetail(symbol) => {
            let symbol = symbol.to_uppercase();
            state.detail = Some(DetailState {
                symbol: symbol.clone(),
                quote: state.quotes.get(&symbol).cloned(),
                candles: Vec::new(),
                fundamentals: None,
                indicators: Indicators::default(),
                loading: true,
                error: None,
            });
            state.view = View::Detail;
            spawn_candles(ctx, symbol.clone(), state.timeframe, false);
            spawn_fundamentals(ctx, symbol, false);
        }
        Action::Back => {
            // Esc closes the help overlay first, else returns to Overview.
            if state.show_help {
                state.show_help = false;
            } else {
                state.view = View::Overview;
            }
        }

        // ---- Watchlist ----
        Action::AddSymbol(symbol) => {
            let symbol = symbol.to_uppercase();
            if !state.watchlist.iter().any(|s| s == &symbol) {
                state.watchlist.push(symbol.clone());
                spawn_quotes(ctx, vec![symbol.clone()], true); // immediate feedback
                state.status_msg = Some(format!("added {symbol}"));
            }
        }
        Action::RemoveSymbol(symbol) => {
            let symbol = symbol.to_uppercase();
            state.watchlist.retain(|s| s != &symbol);
            state.quotes.remove(&symbol);
            for slot in state.slots.iter_mut() {
                if slot.as_ref().is_some_and(|s| s.symbol == symbol) {
                    *slot = None;
                }
            }
            clamp_selection(state);
            state.status_msg = Some(format!("removed {symbol}"));
        }
        Action::SetFilter(filter) => {
            state.filter = filter;
            state.selected_row = 0;
            state.scroll_offset = 0;
        }
        Action::SetSort(sort) => {
            state.sort = sort;
            state.selected_row = 0;
            state.scroll_offset = 0;
        }
        Action::ToggleSortDirection => state.sort_desc = !state.sort_desc,
        Action::SelectRow(i) => {
            let n = table::visible_symbols(state).len();
            if n > 0 {
                state.selected_row = i.min(n - 1);
                adjust_scroll(state);
            }
        }
        Action::MoveSelection(delta) => {
            let n = table::visible_symbols(state).len();
            if n > 0 {
                let cur = state.selected_row as i64;
                state.selected_row = (cur + delta as i64).clamp(0, n as i64 - 1) as usize;
                adjust_scroll(state);
            }
        }
        Action::JumpTop => {
            state.selected_row = 0;
            state.scroll_offset = 0;
        }
        Action::JumpBottom => {
            let n = table::visible_symbols(state).len();
            if n > 0 {
                state.selected_row = n - 1;
                adjust_scroll(state);
            }
        }

        // ---- Chart grid ----
        Action::AddToSlot(symbol) => {
            let symbol = symbol.to_uppercase();
            // Fill the first empty slot; once all four are full, replace the
            // FOCUSED slot and advance focus. Predictable and aimable, not FIFO.
            let idx = match state.slots.iter().position(|s| s.is_none()) {
                Some(i) => i,
                None => {
                    let focused = state.focused_slot;
                    state.focused_slot = (focused + 1) % MAX_SLOTS;
                    focused
                }
            };
            state.slots[idx] = Some(ChartSlot {
                symbol: symbol.clone(),
                candles: Vec::new(),
                loading: true,
                error: None,
            });
            spawn_candles(ctx, symbol, state.timeframe, false);
        }
        Action::ClearSlot(i) => {
            if i < MAX_SLOTS {
                state.slots[i] = None;
            }
        }
        Action::ClearAllSlots => state.slots = [None, None, None, None],
        Action::FocusSlot(i) => {
            if i < MAX_SLOTS {
                state.focused_slot = i;
                state.focus = FocusRegion::Grid; // focusing a pane focuses the grid
            }
        }
        Action::CycleSlotFocus => {
            state.focused_slot = (state.focused_slot + 1) % MAX_SLOTS;
            state.focus = FocusRegion::Grid; // Tab focuses the grid, like FocusSlot
        }
        Action::SetTimeframe(tf) => {
            if tf != state.timeframe {
                state.timeframe = tf;
                refetch_charts(state, ctx, false); // all panes + detail follow
            }
        }

        // ---- Data results coming home ----
        Action::QuotesUpdated(quotes) => {
            for q in quotes {
                state.quotes.insert(q.symbol.clone(), q);
            }
            state.last_refresh = Utc::now();
        }
        Action::CandlesLoaded {
            symbol,
            timeframe,
            candles,
            indicators,
        } => {
            // Ignore results for a timeframe we've since navigated away from.
            if timeframe != state.timeframe {
                return;
            }
            for slot in state.slots.iter_mut().flatten() {
                if slot.symbol == symbol {
                    slot.candles = candles.clone();
                    slot.loading = false;
                    slot.error = None;
                }
            }
            if let Some(d) = state.detail.as_mut()
                && d.symbol == symbol
            {
                d.candles = candles;
                d.indicators = indicators;
                d.loading = false;
                d.error = None;
            }
        }
        Action::FundamentalsLoaded { symbol, data } => {
            if let Some(d) = state.detail.as_mut()
                && d.symbol == symbol
            {
                d.fundamentals = Some(data);
            }
        }
        Action::MarketStatusUpdated(status) => state.market_status = status,
        Action::FetchFailed { symbol, error } => {
            for slot in state.slots.iter_mut().flatten() {
                if slot.symbol == symbol {
                    slot.loading = false;
                    slot.error = Some(error.clone());
                }
            }
            if let Some(d) = state.detail.as_mut()
                && d.symbol == symbol
            {
                d.loading = false;
                d.error = Some(error.clone());
            }
            state.status_msg = Some(format!("{symbol}: {error}"));
        }
        Action::Refresh => {
            spawn_quotes(ctx, state.watchlist.clone(), true);
            spawn_market_status(ctx);
            refetch_charts(state, ctx, true);
            state.status_msg = Some("refreshing…".into());
        }
        Action::RefreshCompleted => state.status_msg = None,

        // ---- Input modes / bottom bar ----
        Action::EnterCommandMode => {
            state.input_mode = InputMode::Command;
            state.command_input.clear();
        }
        Action::EnterSearchMode => {
            state.input_mode = InputMode::Search;
            state.command_input.clear();
        }
        Action::ExitInputMode => {
            state.input_mode = InputMode::Normal;
            state.command_input.clear();
        }
        Action::InputChar(c) => state.command_input.push(c),
        Action::InputBackspace => {
            state.command_input.pop();
        }
        Action::SubmitInput => {
            let buffer = std::mem::take(&mut state.command_input);
            let mode = state.input_mode;
            state.input_mode = InputMode::Normal;
            match mode {
                InputMode::Command => {
                    for a in parse_command(&buffer) {
                        update(state, a, ctx);
                    }
                }
                InputMode::Search => {
                    let sym = buffer.trim().to_uppercase();
                    if !sym.is_empty() {
                        update(state, Action::OpenDetail(sym), ctx);
                    }
                }
                InputMode::Normal => {}
            }
        }

        // ---- Overlays / misc ----
        Action::ToggleHelp => state.show_help = !state.show_help,
        Action::Export { path } => {
            let quotes: Vec<Quote> = table::visible_symbols(state)
                .iter()
                .filter_map(|s| state.quotes.get(s).cloned())
                .collect();
            match serde_json::to_string_pretty(&quotes) {
                Ok(json) => match path {
                    Some(p) => match std::fs::write(&p, json) {
                        Ok(()) => state.status_msg = Some(format!("exported to {p}")),
                        Err(e) => state.status_msg = Some(format!("export failed: {e}")),
                    },
                    None => {
                        println!("{json}");
                        state.status_msg = Some("exported to stdout".into());
                    }
                },
                Err(e) => state.status_msg = Some(format!("export failed: {e}")),
            }
        }
        Action::SetStatus(msg) => state.status_msg = Some(msg),
        Action::Tick => state.marquee_offset = state.marquee_offset.wrapping_add(1),
        Action::Quit => state.should_quit = true,
    }
}

// ---- Helpers ---------------------------------------------------------------

/// Refetch every open chart (grid panes + detail) at the current timeframe.
fn refetch_charts(state: &mut AppState, ctx: &EventCtx, force: bool) {
    let tf = state.timeframe;
    for slot in state.slots.iter_mut().flatten() {
        slot.loading = true;
        slot.error = None;
        spawn_candles(ctx, slot.symbol.clone(), tf, force);
    }
    if let Some(d) = state.detail.as_mut() {
        d.loading = true;
        d.error = None;
        let symbol = d.symbol.clone();
        spawn_candles(ctx, symbol.clone(), tf, force);
        spawn_fundamentals(ctx, symbol, force);
    }
}

/// Keep the scroll window so the selected row stays visible (14-row body).
fn adjust_scroll(state: &mut AppState) {
    let rows = table::TABLE_VISIBLE_ROWS;
    if state.selected_row < state.scroll_offset {
        state.scroll_offset = state.selected_row;
    } else if state.selected_row >= state.scroll_offset + rows {
        state.scroll_offset = state.selected_row + 1 - rows;
    }
}

/// Clamp the selection after the visible list shrinks (e.g. a removal).
fn clamp_selection(state: &mut AppState) {
    let n = table::visible_symbols(state).len();
    if n == 0 {
        state.selected_row = 0;
    } else if state.selected_row >= n {
        state.selected_row = n - 1;
    }
    adjust_scroll(state);
}

fn next_sort(sort: SortKey) -> SortKey {
    match sort {
        SortKey::Symbol => SortKey::Price,
        SortKey::Price => SortKey::ChangePct,
        SortKey::ChangePct => SortKey::MarketCap,
        SortKey::MarketCap => SortKey::Symbol,
    }
}

/// Translate a raw keypress into a semantic action, per the current mode. This
/// is where "the same key means different things" lives: in text-entry modes a
/// letter edits the buffer; in Normal it's a shortcut.
fn key_to_action(state: &AppState, key: &KeyEvent) -> Option<Action> {
    use KeyCode::*;

    // Ctrl-C always quits. Raw mode stops the terminal from turning it into a
    // SIGINT — it arrives as a keystroke — so handle it explicitly, and BEFORE mode
    // dispatch so it also works while typing a `:` command (where plain `c` is text).
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == Char('c') {
        return Some(Action::Quit);
    }

    if matches!(state.input_mode, InputMode::Command | InputMode::Search) {
        return match key.code {
            Enter => Some(Action::SubmitInput),
            Esc => Some(Action::ExitInputMode),
            Backspace => Some(Action::InputBackspace),
            Char(c) => Some(Action::InputChar(c)),
            _ => None,
        };
    }

    match key.code {
        Char('q') => Some(Action::Quit),
        Char('?') => Some(Action::ToggleHelp),
        Char(':') => Some(Action::EnterCommandMode),
        Char('/') => Some(Action::EnterSearchMode),
        Esc => Some(Action::Back),
        Char('d') => table::selected_symbol(state).map(Action::OpenDetail),
        Enter => table::selected_symbol(state).map(Action::AddToSlot),
        // Table navigation acts only when the table is focused; a no-op on the grid.
        Char('j') | Down => (state.focus == FocusRegion::Table).then_some(Action::MoveSelection(1)),
        Char('k') | Up => (state.focus == FocusRegion::Table).then_some(Action::MoveSelection(-1)),
        Char('g') => Some(Action::JumpTop),
        Char('G') => Some(Action::JumpBottom),
        Char('f') => Some(Action::SetFilter(state.filter.next())),
        Char('s') => Some(Action::SetSort(next_sort(state.sort))),
        Char('S') => Some(Action::ToggleSortDirection),
        Char('c') => Some(Action::ClearSlot(state.focused_slot)),
        Char('C') => Some(Action::ClearAllSlots),
        Char('x') => table::selected_symbol(state).map(Action::RemoveSymbol),
        Char('r') => Some(Action::Refresh),
        Tab => Some(Action::CycleSlotFocus),
        Char('h') => Some(Action::SetFocus(FocusRegion::Table)),
        Char('l') => Some(Action::SetFocus(FocusRegion::Grid)),
        // Shift+1..4 focus a pane. Terminals deliver this two ways: the digit
        // WITH a Shift modifier (kitty protocol), or the shifted symbol with no
        // modifier (legacy). Handle both so it works either way.
        Char(c @ '1'..='4') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            Some(Action::FocusSlot(c as usize - '1' as usize))
        }
        Char('!') => Some(Action::FocusSlot(0)),
        Char('@') => Some(Action::FocusSlot(1)),
        Char('#') => Some(Action::FocusSlot(2)),
        Char('$') => Some(Action::FocusSlot(3)),
        // 1..7 (unmodified) select a timeframe globally.
        Char(c @ '1'..='7') => {
            let idx = c as usize - '1' as usize;
            Some(Action::SetTimeframe(Timeframe::ALL[idx]))
        }
        _ => None,
    }
}

/// Parse a `:` command buffer (without the leading `:`) into zero or more actions.
fn parse_command(buffer: &str) -> Vec<Action> {
    let parts: Vec<&str> = buffer.split_whitespace().collect();
    let up = |s: &str| s.to_uppercase();
    match parts.as_slice() {
        ["add", s] => vec![Action::AddSymbol(up(s))],
        ["rm", s] | ["remove", s] => vec![Action::RemoveSymbol(up(s))],
        ["detail", s] => vec![Action::OpenDetail(up(s))],
        ["chart", s] => vec![Action::AddToSlot(up(s))],
        ["clear"] | ["clear", "all"] => vec![Action::ClearAllSlots],
        ["clear", n] => match n.parse::<usize>() {
            Ok(i) if (1..=MAX_SLOTS).contains(&i) => vec![Action::ClearSlot(i - 1)],
            _ => vec![Action::SetStatus(format!("clear: bad slot '{n}'"))],
        },
        ["tf", r] => match Timeframe::parse(r) {
            Some(tf) => vec![Action::SetTimeframe(tf)],
            None => vec![Action::SetStatus(format!("tf: unknown range '{r}'"))],
        },
        ["export"] => vec![Action::Export { path: None }],
        ["export", p] => vec![Action::Export {
            path: Some(p.to_string()),
        }],
        ["settings"] => vec![Action::SetView(View::Settings)],
        ["refresh"] => vec![Action::Refresh],
        ["q"] | ["quit"] => vec![Action::Quit],
        [] => vec![],
        _ => vec![Action::SetStatus(format!("unknown command: {buffer}"))],
    }
}

// ---- Headless runner (step 3: prove the pipeline, no UI) -------------------

/// Run the event loop with no TUI, printing quotes as they arrive. This is the
/// step-3 deliverable; the ratatui frontend replaces the print/loop in step 4
/// while reusing everything above unchanged.
pub async fn run_headless(config: Config, store: Arc<DataStore>) -> anyhow::Result<()> {
    let session = crate::session::load();
    let config = Arc::new(config);
    let mut state = initial_state(&config, session);

    let (tx, mut rx) = mpsc::unbounded_channel();
    let ctx = EventCtx {
        tx: tx.clone(),
        store,
        config: config.clone(),
    };

    // Kick off fetches for any charts restored from the session.
    refetch_charts(&mut state, &ctx, false);

    spawn_poller(&ctx);
    spawn_tick(tx.clone(), Duration::from_millis(MARQUEE_TICK_MS));
    spawn_ctrl_c(tx);

    println!(
        "quotail headless — polling {} symbols every {}s (Ctrl-C to quit)",
        state.watchlist.len(),
        config.data.poll_interval_sec
    );

    while let Some(action) = rx.recv().await {
        // Only print when data actually landed, not on every marquee tick.
        let data_event = matches!(
            action,
            Action::QuotesUpdated(_)
                | Action::MarketStatusUpdated(_)
                | Action::CandlesLoaded { .. }
                | Action::FundamentalsLoaded { .. }
                | Action::FetchFailed { .. }
        );
        update(&mut state, action, &ctx);
        if state.should_quit {
            break;
        }
        if data_event {
            print_headless(&state);
        }
    }

    crate::session::save(&session_snapshot(&state));
    println!("\nsession saved. bye.");
    Ok(())
}

// ---- TUI runner (the default) ----------------------------------------------

/// Run the full ratatui frontend. Reuses the exact same event loop, producers,
/// and `DataStore` as headless — the ONLY differences are an input producer and
/// that the consumer draws a frame after each action instead of printing.
///
/// The ONE rule still holds: producers (input thread, poller, tick, fetch tasks)
/// feed one channel; the consumer below is the sole mutator and never awaits a
/// network call. `terminal.draw` is fast local I/O, not a fetch.
pub async fn run_tui(config: Config, store: Arc<DataStore>) -> anyhow::Result<()> {
    let session = crate::session::load();
    let config = Arc::new(config);
    let mut state = initial_state(&config, session);

    let (tx, mut rx) = mpsc::unbounded_channel();
    let ctx = EventCtx {
        tx: tx.clone(),
        store,
        config: config.clone(),
    };

    // Kick off fetches for restored charts, then start every producer.
    refetch_charts(&mut state, &ctx, false);
    spawn_poller(&ctx);
    spawn_tick(tx.clone(), Duration::from_millis(MARQUEE_TICK_MS));
    spawn_input(tx); // consumes the last `tx`; producers keep their own clones

    // `try_init` enables raw mode, enters the alternate screen, and installs a
    // panic hook that restores the terminal — so a panic mid-render won't leave the
    // user's shell in raw mode.
    let mut terminal = ratatui::try_init().map_err(|e| {
        anyhow::anyhow!("could not start the TUI ({e}); not a terminal? try --headless")
    })?;
    let outcome = run_loop(&mut terminal, &mut state, &ctx, &mut rx).await;
    ratatui::restore();

    // Persist the session regardless of how the loop ended.
    crate::session::save(&session_snapshot(&state));
    outcome
}

/// The consumer loop: draw once, then redraw after each batch of actions. Bursts
/// (e.g. a poll landing many quotes) are drained before drawing so we paint once.
async fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    ctx: &EventCtx,
    rx: &mut mpsc::UnboundedReceiver<Action>,
) -> anyhow::Result<()> {
    let config = ctx.config.clone();
    terminal.draw(|f| crate::ui::render(f, state, &config))?;

    while let Some(action) = rx.recv().await {
        update(state, action, ctx);
        // Coalesce anything already queued so a burst redraws only once.
        while let Ok(action) = rx.try_recv() {
            update(state, action, ctx);
        }
        if state.should_quit {
            break;
        }
        terminal.draw(|f| crate::ui::render(f, state, &config))?;
    }
    Ok(())
}

fn print_headless(state: &AppState) {
    println!(
        "\n── quotail ── {} ── last refresh {} ──",
        market_label(state.market_status),
        state.last_refresh.format("%H:%M:%S")
    );
    for sym in table::visible_symbols(state).iter().take(15) {
        match state.quotes.get(sym) {
            Some(q) => println!("  {:<8} {:>10.2}  {:>+7.2}%", sym, q.price, q.change_pct),
            None => println!("  {:<8} {:>10}  {:>8}", sym, "—", "loading"),
        }
    }
    let open_charts: Vec<&str> = state
        .slots
        .iter()
        .flatten()
        .map(|s| s.symbol.as_str())
        .collect();
    if !open_charts.is_empty() {
        println!("  charts: {}", open_charts.join(", "));
    }
    if let Some(msg) = &state.status_msg {
        println!("  status: {msg}");
    }
}

fn market_label(status: MarketStatus) -> &'static str {
    match status {
        MarketStatus::Open => "OPEN",
        MarketStatus::Closed => "CLOSED",
        MarketStatus::PreMarket => "PRE-MARKET",
        MarketStatus::AfterHours => "AFTER-HOURS",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_add_uppercases() {
        assert!(matches!(
            parse_command("add nvda").as_slice(),
            [Action::AddSymbol(s)] if s == "NVDA"
        ));
    }

    #[test]
    fn command_tf_and_clear() {
        assert!(matches!(
            parse_command("tf 1m").as_slice(),
            [Action::SetTimeframe(Timeframe::M1)]
        ));
        assert!(matches!(
            parse_command("clear all").as_slice(),
            [Action::ClearAllSlots]
        ));
        assert!(matches!(
            parse_command("clear 3").as_slice(),
            [Action::ClearSlot(2)]
        ));
    }

    #[test]
    fn command_unknown_is_a_status_not_a_panic() {
        assert!(matches!(
            parse_command("frobnicate xyz").as_slice(),
            [Action::SetStatus(_)]
        ));
        assert!(parse_command("").is_empty());
    }

    #[test]
    fn next_sort_cycles_back_to_start() {
        let mut s = SortKey::Symbol;
        for _ in 0..4 {
            s = next_sort(s);
        }
        assert_eq!(s, SortKey::Symbol);
    }
}
