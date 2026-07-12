# Quotail MCP Server

Quotail ships an [MCP](https://modelcontextprotocol.io) server that exposes your
market data — and, when the TUI is running, live control of it — to an MCP client
such as Claude Code or Claude Desktop.

**Quotail never calls an LLM.** The server only serves data and drives the UI; the
intelligence runs in your own Claude client, on your own account. That is why
Quotail needs no API key.

There are two tiers of tools:

- **READ** (7 tools) — quotes, candles, indicators, fundamentals, market status.
  These read from Yahoo and your `config.toml`, so they work **whether or not the
  TUI is running**.
- **CONTROL / SESSION** (7 tools) — add/remove symbols, open charts, change the
  timeframe, and read what's currently on screen. These drive a **running TUI**
  over a local Unix socket and return a clear error if no TUI is up.

---

## Setup

### 1. Install a real binary

MCP clients launch a command, so you need `quotail` on disk:

```sh
cargo install --path .
# or, once published:
cargo install quotail
```

This puts `quotail` on your `PATH` (typically `~/.cargo/bin/quotail`).

### 2. Register the server with your client

**Claude Code** (one-liner):

```sh
claude mcp add quotail -- quotail --mcp
```

**Claude Desktop / manual config** — add to your MCP config file:

```json
{
  "mcpServers": {
    "quotail": {
      "command": "quotail",
      "args": ["--mcp"]
    }
  }
}
```

`quotail --mcp` speaks MCP over stdio. It reads your watchlist from
`~/.config/quotail/config.toml` on every call, so it always reflects your current
symbols — including edits made live in the TUI.

---

## The socket (how control works)

The READ tools need nothing but the config file. The CONTROL/SESSION tools reach a
running TUI over a Unix domain socket:

- **Path** — `[mcp].socket_path` in `config.toml`; default
  `~/.local/state/quotail/quotail.sock`.
- **Local-only** — a filesystem socket. It **never touches the network**.
- **Permissions** — created `0600` (owner read/write only).
- **Lifecycle** — the first TUI instance owns the socket and unlinks it on quit.
  A **stale** socket left by a crash is reclaimed on the next launch. If you start
  a **second** TUI while one is already running, it runs normally but without the
  listener (only one instance owns the socket at a time), so control tools target
  the first instance.

### Error behavior when no TUI is running

- READ tools: work normally.
- CONTROL/SESSION tools: return an error explaining that no Quotail TUI could be
  reached (no instance running, or only a stale socket file). Nothing is silently
  dropped.

---

## READ tools

These require no running TUI.

### `get_watchlist`
- **Arguments:** none.
- **Returns:** the watchlist membership, grouped `stocks` / `crypto` / `indices`.
- **When to use:** to see which symbols the user tracks (just the list, no prices).

### `get_watchlist_quotes`
- **Arguments:** none.
- **Returns:** full live quotes for **every** symbol in the watchlist, in one
  batched request — price, absolute & percent change, previous close, open, day
  range, 52-week range, volume, average volume, market cap, exchange, asset kind.
- **When to use:** the go-to for any "how is my watchlist / portfolio doing?"
  question. Call it first — it's the only way to know what the user holds.

### `get_quote`
- **Arguments:** `symbols` — array of ticker strings (Yahoo convention).
- **Returns:** the same quote fields as `get_watchlist_quotes`, for arbitrary
  symbols, fetched as one batched request.
- **When to use:** ad-hoc lookups like "what's TSLA at?" for symbols that may not
  be in the watchlist.

### `get_candles`
- **Arguments:** `symbol` (string), `timeframe` (one of `1D 5D 1M 6M YTD 1Y MAX`).
- **Returns:** OHLCV candles at the provider's native interval. Each candle has
  `ts` (unix seconds), `open`, `high`, `low`, `close`, `volume`.
- **When to use:** price-action and charting questions. For RSI / moving averages,
  use `get_indicators` instead.

### `get_indicators`
- **Arguments:** `symbol` (string), `timeframe` (one of `1D 5D 1M 6M YTD 1Y MAX`).
- **Returns:** `rsi`, `ma50`, `ma200` — each an array aligned index-for-index with
  the candle series (leading entries are `null` until the window fills).
- **When to use:** technical-analysis questions — overbought/oversold, trend,
  golden/death cross.

### `get_fundamentals`
- **Arguments:** `symbol` (string).
- **Returns:** `market_cap`, `pe_trailing`, `pe_forward`, `eps_trailing`,
  `div_yield`, `beta`. Fields are `null` when they don't apply (crypto and indices
  have no P/E or EPS).
- **When to use:** valuation questions.

### `get_market_status`
- **Arguments:** none.
- **Returns:** whether the US equity market is currently open or closed.
- **When to use:** to caveat quote freshness — prices are last-trade when closed.

---

## CONTROL / SESSION tools

These **require a running TUI** and error clearly if none is up. Symbols use Yahoo
convention (`AAPL`, `BTC-USD`, `^GSPC`).

### `get_session`
- **Arguments:** none.
- **Returns:** the current session state — active view (`overview` / `detail` /
  `settings`), the four chart panes (symbols, in order), which pane is focused, the
  global timeframe (as a label like `1Y`), the watchlist filter, the selected
  watchlist symbol, and the detail symbol if any.
- **When to use:** to **see what the user is looking at** before acting — to target
  "the focused pane" or "the chart showing NVDA," or to confirm a command landed.

### `add_symbol`
- **Arguments:** `symbol` (string).
- **Returns:** confirmation. Adds the symbol to the watchlist and persists it to
  `config.toml`.
- **When to use:** "add PLTR to my watchlist."

### `remove_symbol`
- **Arguments:** `symbol` (string).
- **Returns:** confirmation. Removes the symbol and updates `config.toml`.
- **When to use:** "drop Disney from my list."

### `chart_symbol`
- **Arguments:** `symbol` (string).
- **Returns:** confirmation. Opens a chart in the 2×2 grid — fills the first empty
  pane, or replaces the focused pane when all four are full.
- **When to use:** "show / chart / pull up NVDA."

### `open_detail`
- **Arguments:** `symbol` (string).
- **Returns:** confirmation. Opens the full-screen detail drilldown (large chart,
  volume, RSI, fundamentals rail).
- **When to use:** "open the detail view for AAPL."

### `clear_slot`
- **Arguments:** `slot` (integer `0`–`3`, optional). Omit to clear all panes.
- **Returns:** confirmation.
- **When to use:** "clear pane 2" or "clear all the charts."

### `set_timeframe`
- **Arguments:** `timeframe` — a human label, one of `1D 5D 1M 6M YTD 1Y MAX`.
  Applies to both the grid and the detail view.
- **Returns:** confirmation.
- **When to use:** "switch everything to the 1Y timeframe."

---

## Example prompts that work

Read-only (TUI optional):

- "How's my watchlist doing today?"
- "What are my three worst performers right now?"
- "Is NVDA overbought? Check its RSI on the 6-month."
- "Pull up Apple's fundamentals — what's the forward P/E?"
- "Is the market open?"

Driving the TUI (TUI must be running):

- "What am I looking at right now?" → reads `get_session`.
- "Chart the three worst performers in my watchlist." → ranks by change, then opens
  three charts in your terminal.
- "Switch all my charts to 1Y."
- "Add PLTR to my watchlist and open its detail view."
- "Clear the grid."
