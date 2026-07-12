//! IPC: the TUI's MCP session socket — Step 2.
//!
//! A Unix domain socket listener that is just a FIFTH `tx.clone()` into the one
//! `mpsc<Action>` channel. It reads a line, deserializes a [`RemoteAction`],
//! converts it via the existing `From<RemoteAction> for Action`, and sends it.
//! The event loop cannot tell an MCP command from a keystroke — there is no
//! parallel command path, by design.
//!
//! Local-only and filesystem-permissioned (0600). It never touches the network.
//! `#[cfg(unix)]`: Windows gets the TUI without live attach rather than a broken
//! build — the read tools (`quotail --mcp`) still work there.
//!
//! Stale vs live: a crashed TUI leaves an orphaned socket FILE, which is garbage,
//! not a second instance. We never test for mere file existence — we try to
//! CONNECT. Connection refused ⇒ nothing is listening ⇒ unlink and bind.
//! Connection succeeds ⇒ a live instance owns it ⇒ skip our listener.

#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub use unix::{BindOutcome, send_remote, spawn_listener};

/// The result of trying to bind the session socket. On non-unix builds only the
/// `Unsupported` shape exists; the caller maps every arm to a status either way.
#[cfg(not(unix))]
pub enum BindOutcome {
    Unsupported,
}
