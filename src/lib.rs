//! Quotail library crate: the whole module tree lives here so every binary
//! target — the `quotail` TUI (`main.rs`), the temporary `smoke` bin, and later
//! the MCP/CLI faces — shares one copy of `data/`, `app`, etc. `src/bin/*.rs`
//! can only import a library crate, not another binary's modules, which is why
//! this exists.

// Scaffolding allowance: many public items are defined before their first use
// as Phase 1 is built step by step. Removed once the app is fully wired —
// acceptance requires `cargo clippy -- -D warnings` to be clean.
#![allow(dead_code)]

pub mod action;
pub mod app;
pub mod cli;
pub mod config;
pub mod event_loop;
pub mod session;

pub mod data;
pub mod mcp;
pub mod ui;
