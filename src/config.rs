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

    /// The MCP Unix-socket path with a leading `~/` expanded to the home dir.
    /// Shared by the TUI's listener and the MCP server's client, so both resolve
    /// to the exact same file.
    pub fn socket_path(&self) -> PathBuf {
        let raw = &self.mcp.socket_path;
        if let Some(rest) = raw.strip_prefix("~/")
            && let Some(base) = directories::BaseDirs::new()
        {
            return base.home_dir().join(rest);
        }
        PathBuf::from(raw)
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

/// Persist the live watchlist back to `config.toml`, categorizing symbols by kind.
/// Surgical (via `toml_edit`) so the user's comments and formatting survive. Called
/// when `:add` / `:rm` change the list — those are user intent, not session state.
pub fn persist_watchlist(symbols: &[String]) -> Result<()> {
    let path = path()?;
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("parsing {}", path.display()))?;
    apply_watchlist(&mut doc, symbols);
    fs::write(&path, doc.to_string()).with_context(|| format!("writing {}", path.display()))
}

/// Write the whole editable config back to `config.toml` (settings `w` + the live
/// watchlist), preserving comments/formatting via `toml_edit`.
pub fn save(config: &Config, watchlist: &[String]) -> Result<()> {
    let path = path()?;
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("parsing {}", path.display()))?;
    use toml_edit::{table, value};

    apply_watchlist(&mut doc, watchlist);

    let d = doc["display"].or_insert(table());
    d["theme"] = value(config.display.theme.as_str());
    d["default_timeframe"] = value(config.display.default_timeframe.as_str());
    d["default_filter"] = value(config.display.default_filter.as_str());
    d["default_sort"] = value(config.display.default_sort.as_str());
    d["ticker_scroll"] = value(config.display.ticker_scroll);
    d["ticker_speed_ms"] = value(config.display.ticker_speed_ms as i64);

    let dt = doc["data"].or_insert(table());
    dt["provider"] = value(config.data.provider.as_str());
    dt["poll_interval_sec"] = value(config.data.poll_interval_sec as i64);
    dt["cache_ttl_sec"] = value(config.data.cache_ttl_sec as i64);

    let m = doc["mcp"].or_insert(table());
    m["enabled"] = value(config.mcp.enabled);
    m["socket_path"] = value(config.mcp.socket_path.as_str());

    fs::write(&path, doc.to_string()).with_context(|| format!("writing {}", path.display()))
}

/// Pure core of `persist_watchlist`: rewrite the three watchlist arrays in-place,
/// categorizing each symbol by kind. Editing the existing doc (not reserializing)
/// keeps every comment and blank line the user has.
fn apply_watchlist(doc: &mut toml_edit::DocumentMut, symbols: &[String]) {
    use crate::data::types::AssetKind;

    let mut stocks = toml_edit::Array::new();
    let mut crypto = toml_edit::Array::new();
    let mut indices = toml_edit::Array::new();
    for s in symbols {
        match AssetKind::infer(s) {
            AssetKind::Stock => stocks.push(s.as_str()),
            AssetKind::Crypto => crypto.push(s.as_str()),
            AssetKind::Index => indices.push(s.as_str()),
        }
    }
    let wl = doc["watchlist"].or_insert(toml_edit::table());
    wl["stocks"] = toml_edit::value(stocks);
    wl["crypto"] = toml_edit::value(crypto);
    wl["indices"] = toml_edit::value(indices);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persist_updates_arrays_and_keeps_comments() {
        let mut doc = DEFAULT_CONFIG_TOML
            .parse::<toml_edit::DocumentMut>()
            .unwrap();
        apply_watchlist(
            &mut doc,
            &[
                "AAPL".into(),
                "TSLA".into(),
                "BTC-USD".into(),
                "^GSPC".into(),
            ],
        );
        let out = doc.to_string();
        let cfg: Config = toml::from_str(&out).unwrap();
        assert_eq!(cfg.watchlist.stocks, vec!["AAPL", "TSLA"]);
        assert_eq!(cfg.watchlist.crypto, vec!["BTC-USD"]);
        assert_eq!(cfg.watchlist.indices, vec!["^GSPC"]);
        // Comments and other sections survive the surgical edit.
        assert!(out.contains("# Quotail — default configuration."));
        assert!(out.contains("[mcp]"));
        assert!(out.contains("poll_interval_sec"));
    }
}
