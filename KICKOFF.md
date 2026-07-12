# Claude Code kickoff prompt

Paste the block below into Claude Code from inside the `quotail/` directory.

> **Before you paste:** run `cargo init --name quotail` if you haven't, so there's
> a `Cargo.toml` and a git repo to work against. Commit the pre-written files
> first — you want a clean diff of what Claude Code adds.

---

```
Read these files before writing any code. They are authoritative and already
written — do not rewrite them, do not "improve" them:

  SPEC.md                   the build contract
  docs/ASCII_REFERENCE.md   character-exact renders of all four screens
  src/ui/layout.rs          every dimension
  src/ui/theme.rs           every color and glyph
  src/action.rs             the Action enum (the app's command surface)
  src/app.rs                AppState and friends
  src/data/types.rs         domain types
  src/data/provider.rs      the Provider trait
  examples/config.toml      default config + watchlist

Build Phase 1 as defined in SPEC.md section 3. Do not build Phase 2/3/4.

THE DESIGN IS FINISHED AND APPROVED. Your job is to implement it, not to
redesign it.
  - Do not change any value in layout.rs.
  - Do not substitute glyphs. Candle bodies are █. MA overlays are ·.
  - Do not add panels, borders, or padding not in ASCII_REFERENCE.md.
  - Do not add an AI or chat panel to the TUI. There is none, by design.
  - If something looks wrong to you, SAY SO AND STOP. Do not silently fix it.

THE ONE ARCHITECTURAL RULE:
Many producers -> one mpsc<Action> channel -> one consumer -> one &mut AppState.
Nothing else mutates state. The event loop must NEVER .await a network call:
spawn a fetch task, set loading = true, return immediately, and let the task send
CandlesLoaded back through the channel. If you find yourself awaiting a fetch
inside update(), you have made a mistake.

WORK IN THIS ORDER, and stop for review after each step:

  1. Cargo.toml with the crates from SPEC.md section 5. Module skeleton per
     section 6 (empty modules are fine). Confirm `cargo check` passes.

  2. data/: yahoo.rs (implement Provider), store.rs (DataStore: DashMap cache,
     TTL from Timeframe::ttl_secs, governor rate limiting), indicators.rs
     (RSI/MA as pure functions). Write unit tests for the indicators against
     hand-computed values. NOTE: quotes() is BATCH — one HTTP call for all 68
     symbols, not 68 calls.

  3. event_loop.rs: the mpsc wiring, input/poller/tick tasks, and update() as an
     exhaustive match. config.rs and session.rs. At this point the app should run
     headless and print quotes — no UI yet.

  4. ui/: chart.rs and table.rs first (they're shared), then overview.rs,
     bottom_bar.rs, detail.rs, settings.rs, help.rs. Match ASCII_REFERENCE.md.

  5. tests/: snapshot tests using ratatui::backend::TestBackend at 96x31 with
     FIXED SYNTHETIC DATA (no network). Assert the rendered buffer's chrome
     (borders, labels, column positions) matches docs/ASCII_REFERENCE.md cell for
     cell. Chart interiors get separate tests: feed known candles, assert the
     exact glyph grid. See SPEC.md section 3.

Acceptance is the checklist in SPEC.md section 3. In particular: it must launch
with NO API KEY and NO CONFIG, generate a default config, and show live prices.

I'm using this project to learn Rust — specifically async, ownership, and
lifetimes. When you make a non-obvious choice (Arc vs clone, why a type needs
Send + Sync, why async_trait is required for object safety, where a lifetime
annotation is forced), leave a brief comment explaining WHY, not just what.
Prefer clarity over cleverness.

Start with step 1 and stop.
```

---

## Notes for you (not Claude Code)

**Why the step-by-step gate.** A single "build the whole thing" prompt on a project
this size produces a sprawling half-working mess that's hard to review. Gating after
each step means you review ~1 module at a time and catch drift early — which matters
most in step 4, where the visual contract is easiest to violate.

**The snapshot tests are the real enforcement.** Steps 1–4 can be argued with; step 5
either passes or it doesn't. If Claude Code drifts a border by a column (which I did,
twice, by hand today), the test fails. Consider asking for step 5 *before* step 4 —
write the tests against `ASCII_REFERENCE.md` first, then make them pass. That's the
strongest version of this.

**Watch for these specific temptations:**
- Making `quotes()` per-symbol instead of batch. Kills you on rate limits.
- Awaiting a fetch inside `update()`. Freezes the UI.
- Using `Vec<ChartSlot>` instead of `[Option<ChartSlot>; 4]`. Loses the type-level cap.
- Adding braille charts because they look "better." They were considered and rejected.
- Reintroducing an in-TUI AI panel. It was deliberately removed.

**On the Rust learning goal.** Ask Claude Code to explain any borrow-checker error it
resolves rather than just fixing it. The errors it hits in `event_loop.rs` — `move`
into spawned tasks, `Arc` cloning, `Send + Sync` bounds — are exactly the concepts
worth internalizing, and they're much stickier when you see them fail first.
