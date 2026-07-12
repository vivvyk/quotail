//! Quotail entry point. Parses args and dispatches to the TUI (`--mcp` and the
//! JSON CLI are later phases). The module tree lives in the library crate
//! (`src/lib.rs`) so every binary target shares it.

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;

use quotail::config;
use quotail::data::store::DataStore;
use quotail::data::yahoo::YahooProvider;
use quotail::event_loop;

#[derive(Parser)]
#[command(
    name = "quotail",
    about = "Terminal market analysis for stocks, crypto, and indices."
)]
struct Cli {
    /// Run the MCP server (Phase 2 — not yet implemented).
    #[arg(long)]
    mcp: bool,
    /// Run without the TUI, printing quotes to stdout. For debugging / CI, or a
    /// terminal that can't do raw mode.
    #[arg(long)]
    headless: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.mcp {
        anyhow::bail!("--mcp is a Phase 2 feature and isn't built yet");
    }

    // First run generates ~/.config/quotail/config.toml from the bundled template.
    let config = config::load()?;
    let store = Arc::new(DataStore::new(Box::new(YahooProvider::new()?)));

    // The ratatui TUI is the default; both frontends share one event loop and store.
    if cli.headless {
        event_loop::run_headless(config, store).await
    } else {
        event_loop::run_tui(config, store).await
    }
}
