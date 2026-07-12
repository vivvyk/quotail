//! Data layer. Depends on nothing in `ui/` or `app` — dependency direction is
//! one way, which is what lets `mcp/` and `cli` reuse it with no TUI running.

pub mod indicators;
pub mod provider;
pub mod store;
pub mod types;
pub mod yahoo;
