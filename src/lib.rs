//! Quotail library crate: the whole module tree lives here so every binary
//! target shares one copy of `data/`, `app`, etc. `src/bin/*.rs` can only import
//! a library crate, not another binary's modules, which is why this exists.

#![allow(dead_code)]

pub mod action;
pub mod app;
pub mod cli;
pub mod config;
pub mod event_loop;
pub mod session;

pub mod data;
pub mod ipc;
pub mod mcp;
pub mod ui;
