# Quotail — Specification

A terminal market-analysis app for stocks, crypto, and indices. Written in Rust
with `ratatui` + `tokio`. Keyboard-first, vim-modal, no API keys, no database.

**One core, three faces.** A single `DataStore` sits behind three shells:

| Face | Entry | Purpose |
|---|---|---|
| TUI | `quotail` | The dashboard. |
| JSON CLI | `quotail quote AAPL --json` | Pipeable to `jq`. Unix composability. |
| MCP server | `quotail --mcp` | Claude reads and drives Quotail. |

The MCP server is the differentiator. There is **no AI inside the TUI** — no chat
pane, no API key, no BYOK. Analysis happens by attaching Claude (Code or Desktop)
to Quotail's MCP server. This keeps the app entirely key-free: `cargo install
quotail` and it works.

---

## 0. Sources of truth — do not restate, do not redesign

These files already exist in the repo and are **authoritative**. Read them before
writing anything. Where this spec and a file disagree, the file wins.

| File | Owns |
|---|---|
| `docs/ASCII_REFERENCE.md` | **The visual contract.** Character-exact renders of all four screens. |
| `src/ui/layout.rs` | Every width, height, and column position. |
| `src/ui/theme.rs` | Every color (hex) and glyph. |
| `src/action.rs` | The `Action` enum + `RemoteAction`. The app's command surface. |
| `src/app.rs` | `AppState`, `View`, `InputMode`, `ChartSlot`, `DetailState`, `Session`. |
| `src/data/types.rs` | `Quote`, `Candle`, `Fundamentals`, `Indicators`, `Timeframe`, `AssetKind`. |
| `src/data/provider.rs` | The `Provider` trait and `ProviderError`. |
| `examples/config.toml` | The default config, including the default watchlist. |

### Anti-creativity clause

The UI design is **finished and approved**. Do not improve it.

- Do not change any dimension in `layout.rs`.
- Do not substitute glyphs. Candle bodies are `█`, not braille. MA overlays are `·`.
- Do not add borders, padding, or panels that are not in `ASCII_REFERENCE.md`.
- Do not reorder or rename tabs, columns, or keybinds.
- Do not add an AI/chat panel to the TUI.

If something in the design seems wrong, **say so and stop** — do not silently fix it.

---

## 1. Architecture

### The one rule

**Many producers → one `mpsc<Action>` channel → one consumer → one `&mut AppState` → redraw.**

Nothing else mutates state. There are no locks in the UI layer because there is
nothing to lock: the channel serializes every mutation by construction.

```
input task ──┐
poller task ─┤
tick task ───┼──> mpsc::UnboundedSender<Action> ──> event loop ──> &mut AppState ──> render
fetch tasks ─┤                                        (sole rx)
MCP socket ──┘
```

Consequences that matter:

- **The event loop never `.await`s a network call.** `AddToSlot("NVDA")` sets
  `loading: true`, spawns a fetch task, and returns immediately. When the data
  lands, that task sends `CandlesLoaded { .. }` back through the channel. The UI
  stays responsive throughout. This is the single most important structural
  property of the app.
- **MCP control is nearly free.** The socket listener is a fourth `tx.clone()`.
  It deserializes `RemoteAction` → `Action` and sends it. The event loop cannot
  tell it apart from a keystroke.
- `update()` is one exhaustive `match action { .. }`. Adding an `Action` makes the
  compiler frog-march you to every place that needs updating.

### Layers

```
ui/          reads &AppState, never mutates
event_loop   the ONLY mutator; owns rx
data/        knows nothing about ui/ or app
mcp/, cli    depend on data/ ONLY — which is why they work with no TUI running
```

### Data layer

Providers are **dumb**: fetch from one source, no caching, no retries.
`DataStore` is **smart**: cache + TTL + rate limit, shared as `Arc<DataStore>`.

- `quotes()` is **batch by design**. The default watchlist is 68 symbols. Yahoo
  accepts a comma-joined list, so one poll = one HTTP call. Per-symbol fetching
  would mean 68 requests/minute and immediate throttling.
- TTLs live on `Timeframe::ttl_secs()`. Intraday candles 60s; daily 1h.
- **Cache is in-memory only (`DashMap`).** No SQLite in MVP. No database, no
  daemon, nothing running. (SQLite persistence for historical daily candles is a
  Phase 4 optimization — those candles never change.)

### Files on disk

| Path | Owner | Disposable? |
|---|---|---|
| `~/.config/quotail/config.toml` | The **user**. Hand-edited, dotfile-able. | No |
| `~/.local/state/quotail/session.json` | The **app**. Written on quit. | **Yes** |

Session = open chart slots, focused slot, timeframe, view, selection. If it is
missing or corrupt, **fall back to defaults silently** — never crash, never warn.
Deleting it must lose nothing. Config is generated from `examples/config.toml` on
first run.

---

## 2. Behavior

### Views

Three: **Overview**, **Detail**, **Settings**. No sidebar — three views don't earn
one. Navigation is the bottom bar plus hotkeys.

**Overview** — hot-movers marquee, watchlist table (left, 40 cols), 2×2 chart grid
(right, 56 cols).

- Filter (`All / Stocks / Crypto / Indices`) is a control **inside** the table, not
  a top-level tab.
- Table **scrolls**: ~68 symbols, 14 visible rows. `j`/`k` past an edge moves
  `scroll_offset`. Thumb on the right edge.
- **Slot fill policy:** fill the first empty slot in order. Once all 4 are full, a
  new add **replaces the focused slot**, then advances focus. Predictable and
  aimable — not silent FIFO.
- Hot movers: sort the whole universe by `|change_pct|`, take the top 10, scroll
  right-to-left (content drifts left; new symbols enter from the right).

**Detail** — reached by `d`/`Enter` on a row, or `:detail <SYM>`. Opened from nav
with no symbol → empty with a search prompt.

- Chart (14 rows) with MA50/MA200 overlaid, volume histogram (3 rows), RSI (4 rows,
  70/30 bands), fundamentals rail (32 cols).
- **MA dots only draw where a cell is empty. A candle always wins.** The price data
  is the signal; the average is annotation.

**Settings** — `:settings`. `w` writes to `config.toml`.

### Modal input

`InputMode::Normal | Command | Search`. The **same keypress means different things
per mode** — in Normal, `d` opens Detail; in Command, `d` is a character in the
buffer.

**Bottom region is two rows:**

```
NYSE: Closed (Opens Mon 09:30)          [1M]        Last Refresh: 22:20:01
 q quit  d detail  / search  f filter  s sort  ...          ← row 2, Normal mode
:add NVDA▮                                                  ← row 2, Command mode
```

Row 2 is the keyhint strip in Normal, and **the same row** becomes the `:` command
line in Command mode. Vim's bottom line. It must be the last row.

Commands: `:add <SYM>` `:rm <SYM>` `:detail <SYM>` `:chart <SYM>` `:clear [1-4|all]`
`:tf <1D|5D|1M|6M|YTD|1Y|MAX>` `:export [path]` `:settings` `:refresh` `:q`

Full keymap is in `ASCII_REFERENCE.md` (the help overlay screen). `1`–`7` are
timeframes globally, so slot focus is `Shift+1`–`4`.

### "Realtime"

There is none. Yahoo has no websocket; we **poll** (default 60s) and `r` forces a
refresh. Because prices can be a minute stale, **`Last Refresh` in the status row
is load-bearing** — hiding it would be lying about staleness in a finance tool. Do
not remove it.

The marquee redraws on `Action::Tick` (~140ms), which means the app is **never
idle** while `ticker_scroll` is on. That is a known cost, config-gated.

---

## 3. Phase 1 — the MVP to build

**Scope.** Everything above except MCP and the JSON CLI.

1. Cargo project; `ratatui` + `crossterm` + `tokio` scaffold.
2. The `Action`-channel event loop with input, poller, and tick tasks.
3. Config load (generate from `examples/config.toml` on first run) + session save/load.
4. `YahooProvider` implementing `Provider`; `DataStore` with `DashMap` cache + TTL
   + `governor` rate limiting.
5. `indicators.rs` — RSI(14), MA50, MA200 as pure functions.
6. Overview: marquee, scrolling filterable sortable table, 2×2 grid, bottom bar.
7. Detail: chart + MA overlays + volume + RSI + fundamentals rail.
8. Settings; help overlay; `:` command mode.

**Out of scope for Phase 1:** MCP, JSON CLI, options, heatmaps, alerts, SQLite,
websockets, per-pane timeframes, themes other than `tokyonight`.

### Acceptance criteria

- [ ] `cargo run` launches with **no API key and no config**, generating a default
      config, and shows live prices for the 68-symbol default watchlist.
- [ ] `cargo clippy -- -D warnings` and `cargo fmt --check` are clean.
- [ ] Fully operable **keyboard-only**. Mouse is a bonus, never the only path.
- [ ] Four charts can be added, focused, cleared; the 5th replaces the focused pane.
- [ ] Quit → relaunch restores the exact chart slots, timeframe, and view.
- [ ] Deleting `session.json` loses nothing and produces no error.
- [ ] The UI **never blocks** on a fetch. Panes show a loading state.
- [ ] **Snapshot tests pass** (below).

### Snapshot tests — this is how "looks exactly like the mockup" is enforced

`ratatui::backend::TestBackend` renders into an in-memory `Buffer` you can assert
against. Use it.

- Render each view at **96×31** with a **fixed synthetic fixture** (hardcoded
  quotes/candles — no network) and assert the buffer equals the corresponding block
  in `docs/ASCII_REFERENCE.md`, cell for cell.
- **Important scoping:** `ASCII_REFERENCE.md` pins the **frame** — borders, labels,
  column positions, panel widths. Every row in it is exactly 96 chars. It does
  **not** pin candle/volume interiors, which depend on data. So:
  - **Chrome** → assert against `ASCII_REFERENCE.md` directly.
  - **Chart interiors** → separate tests: feed known synthetic candles, assert the
    exact glyph grid (a green `█` body here, a `│` wick there, an MA `·` only where
    no candle sits).
- A layout regression must be a **failing test**, not a matter of taste.

---

## 4. Later phases

**Phase 2 — MCP.** `quotail --mcp`. Read tools (`get_watchlist_quotes`, `get_quote`,
`get_candles`, `get_fundamentals`, `get_indicators`, `get_market_status`) work by
reading `config.toml`, so they function **whether or not the TUI is running**. If an
instance is up, also connect to it over the Unix socket for session tools
(`get_session`, `add_symbol`, `chart_symbol`, `open_detail`, `set_timeframe`) — these
are `RemoteAction` → `Action`, already defined in `src/action.rs`. Graceful
degradation: no TUI → still useful; TUI → Claude can drive the UI. Local-only,
filesystem-permissioned, never touches the network. Add `ipc/` (socket listener in
the TUI, a fifth `tx.clone()`).

**Phase 3 — JSON CLI.** `quotail quote AAPL --json`, `quotail candles NVDA --tf 1M
--json` → stdout. Nearly free: the domain types already derive `Serialize`. Same
serialization as `:export` and the MCP tools.

**Phase 4 — depth.** SQLite persistence for historical daily candles (they never
change → instant 1Y charts on relaunch). Single-flight request dedup (four panes
requesting AAPL at once should fire **one** request, not four — the 2×2 grid creates
exactly this stampede). Additional themes. Per-pane timeframes. Alerts. Finnhub
provider. Options payoff lab. Sector heatmap.

---

## 5. Crates

`ratatui`, `crossterm`, `tokio`, `reqwest`, `serde`, `serde_json`, `toml`,
`async-trait`, `dashmap`, `governor`, `anyhow`, `thiserror`, `chrono`, `clap`,
`directories`. Phase 2 adds `rmcp`.

## 6. Module layout

```
src/
├── main.rs           entry; arg parse → TUI | --mcp | CLI
├── app.rs            ✅ written
├── action.rs         ✅ written
├── event_loop.rs     channel wiring, spawn producers, update() dispatch
├── config.rs         config.toml load/save/defaults
├── session.rs        session.json load/save (disposable)
├── data/
│   ├── types.rs      ✅ written
│   ├── provider.rs   ✅ written
│   ├── yahoo.rs      YahooProvider impl
│   ├── store.rs      DataStore: cache, TTL, rate limit
│   └── indicators.rs RSI, MA50, MA200 (pure fns)
├── ui/
│   ├── mod.rs        render() root; layout split
│   ├── layout.rs     ✅ written
│   ├── theme.rs      ✅ written
│   ├── overview.rs   marquee + table + chart grid
│   ├── detail.rs     drilldown
│   ├── settings.rs
│   ├── help.rs       overlay
│   ├── bottom_bar.rs status row + hint/command row
│   ├── chart.rs      candlestick widget (shared: grid + detail)
│   └── table.rs      watchlist table
├── mcp/              Phase 2
├── ipc/              Phase 2
└── cli.rs            Phase 3
```

Dependency direction is **one way**: `data/` never imports `ui/` or `app`.
