//! `config.toml`: user intent. Hand-edited, dotfile-able, never clobbered by the
//! app. On first run the bundled template (`examples/config.toml`) is copied to
//! `~/.config/quotail/config.toml`; after that it's the user's.
//!
//! Every field carries a serde default so a hand-edited, partially-specified
//! config still loads instead of erroring.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::app::{AssetFilter, SortKey};
use crate::data::types::Timeframe;

/// Compiled-in copy of the template, written verbatim on first run.
const DEFAULT_CONFIG_TOML: &str = include_str!("../examples/config.toml");

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub watchlist: Watchlist,
    pub display: Display,
    pub data: DataCfg,
    pub mcp: Mcp,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Watchlist {
    pub stocks: Vec<String>,
    pub crypto: Vec<String>,
    pub indices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Display {
    pub theme: String,
    pub default_timeframe: String,
    pub default_filter: String,
    pub default_sort: String,
    pub ticker_scroll: bool,
    pub ticker_speed_ms: u64,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            theme: "tokyonight".into(),
            default_timeframe: "1M".into(),
            default_filter: "all".into(),
            default_sort: "change_pct".into(),
            ticker_scroll: true,
            ticker_speed_ms: 140,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DataCfg {
    pub provider: String,
    pub poll_interval_sec: u64,
    pub cache_ttl_sec: u64,
}

impl Default for DataCfg {
    fn default() -> Self {
        Self {
            provider: "yahoo".into(),
            poll_interval_sec: 60,
            cache_ttl_sec: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Mcp {
    pub enabled: bool,
    pub socket_path: String,
}

impl Default for Mcp {
    fn default() -> Self {
        Self {
            enabled: true,
            socket_path: "~/.local/state/quotail/quotail.sock".into(),
        }
    }
}

impl Config {
    /// The full watchlist: stocks, then crypto, then indices — the poll order.
    pub fn watchlist(&self) -> Vec<String> {
        self.watchlist
            .stocks
            .iter()
            .chain(&self.watchlist.crypto)
            .chain(&self.watchlist.indices)
            .cloned()
            .collect()
    }

    pub fn default_timeframe(&self) -> Timeframe {
        Timeframe::parse(&self.display.default_timeframe).unwrap_or(Timeframe::M1)
    }

    pub fn default_filter(&self) -> AssetFilter {
        match self.display.default_filter.as_str() {
            "stocks" => AssetFilter::Stocks,
            "crypto" => AssetFilter::Crypto,
            "indices" => AssetFilter::Indices,
            _ => AssetFilter::All,
        }
    }

    pub fn default_sort(&self) -> SortKey {
        match self.display.default_sort.as_str() {
            "symbol" => SortKey::Symbol,
            "price" => SortKey::Price,
            "market_cap" => SortKey::MarketCap,
            _ => SortKey::ChangePct,
        }
    }

    pub fn poll_interval(&self) -> Duration {
        // Never 0: a 0-second interval would busy-loop the poller.
        Duration::from_secs(self.data.poll_interval_sec.max(1))
    }
}

/// `~/.config/quotail/config.toml`.
pub fn path() -> Result<PathBuf> {
    let dirs =
        ProjectDirs::from("", "", "quotail").context("could not resolve a home directory")?;
    Ok(dirs.config_dir().join("config.toml"))
}

/// Load the config, generating it from the bundled template on first run.
pub fn load() -> Result<Config> {
    let path = path()?;
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating config dir {}", parent.display()))?;
        }
        fs::write(&path, DEFAULT_CONFIG_TOML)
            .with_context(|| format!("writing default config to {}", path.display()))?;
    }
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}
