//! The event loop: mpsc wiring, producer tasks (input/poller/tick/fetch), and
//! `update()` — the ONLY mutator of `AppState`. Filled in in step 3.
