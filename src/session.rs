//! `session.json`: the app's ephemeral state (open charts, focus, timeframe,
//! view, selection). Written on quit, loaded on start. If it's missing or
//! corrupt we fall back to config defaults SILENTLY — deleting it must lose
//! nothing and must never surface an error. So every function here swallows
//! failure by design (returns `None` / does nothing), never bubbles it up.

use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;

use crate::app::Session;

/// `~/.local/state/quotail/session.json` (falls back to the data dir on
/// platforms without a state dir).
fn path() -> Option<PathBuf> {
    let dirs = ProjectDirs::from("", "", "quotail")?;
    let dir = dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir());
    Some(dir.join("session.json"))
}

/// Load the last session, or `None` if absent or unreadable/corrupt. Never errors.
pub fn load() -> Option<Session> {
    let text = fs::read_to_string(path()?).ok()?;
    serde_json::from_str(&text).ok()
}

/// Persist the session best-effort. Any failure is silently ignored — this file
/// is disposable, and a failed write must not crash the app on quit.
pub fn save(session: &Session) {
    let Some(path) = path() else { return };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(session) {
        let _ = fs::write(path, json);
    }
}
