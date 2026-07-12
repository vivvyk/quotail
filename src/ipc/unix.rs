//! Unix-only implementation of the MCP session socket listener.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

use crate::action::{Action, RemoteAction};

/// A socket message that expects a REPLY, unlike a fire-and-forget `RemoteAction`.
/// Tagged by the same `action` field, so the two are disjoint by tag: a control
/// line never parses as a query and vice versa.
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum SocketQuery {
    GetSession,
}

/// The exact `get_session` request line the client sends (matches `SocketQuery`).
const GET_SESSION_LINE: &[u8] = b"{\"action\":\"get_session\"}\n";

/// Shown when the session socket can't be reached — no TUI, or only a stale file.
/// Actionable (says how to fix it) and reassures that the read tools still work.
pub const NO_TUI: &str = "no running Quotail TUI to control — launch `quotail` in another terminal first. \
    (The read-only market-data tools work without a running TUI.)";

/// Connect to a running TUI's socket and deliver one `RemoteAction` as a JSON line
/// — the client half of the listener above, used by the MCP session tools. A
/// missing path or a stale file (nobody listening) surfaces as [`NO_TUI`], never a
/// hang: this is a one-shot write with no reply to wait on.
pub async fn send_remote(path: &Path, action: &RemoteAction) -> Result<(), String> {
    let mut stream = UnixStream::connect(path)
        .await
        .map_err(|e| match e.kind() {
            ErrorKind::NotFound | ErrorKind::ConnectionRefused => NO_TUI.to_string(),
            _ => format!("could not reach the running Quotail TUI: {e}"),
        })?;
    let mut line = serde_json::to_string(action).map_err(|e| e.to_string())?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .await
        .map_err(|e| format!("failed writing to the Quotail TUI socket: {e}"))?;
    stream
        .flush()
        .await
        .map_err(|e| format!("failed flushing to the Quotail TUI socket: {e}"))?;
    Ok(())
}

/// Outcome of the bind attempt. The caller turns this into an `McpListener` status
/// and decides whether it owns the socket file (and must unlink it on quit).
pub enum BindOutcome {
    /// We bound and are serving. Carries the path we own — unlink it on shutdown.
    Bound(PathBuf),
    /// A live instance already owns the socket; we left it alone.
    OtherInstance,
    /// Bind (or a prerequisite) failed. MCP session tools are unavailable.
    Failed,
}

/// Try to own `path` and, on success, spawn the accept loop as a producer task.
///
/// The stale/live decision is made by CONNECTING, never by file existence:
/// - connect succeeds → a live instance is listening → [`BindOutcome::OtherInstance`]
/// - connect fails (refused / not found) → nothing there → unlink any stale file
///   and bind normally.
pub fn spawn_listener(path: &Path, tx: UnboundedSender<Action>) -> BindOutcome {
    // Is anyone actually listening? A successful connect means yes — back off.
    if StdUnixStream::connect(path).is_ok() {
        return BindOutcome::OtherInstance;
    }
    // Nothing is listening. Any file here is a crashed instance's orphan — remove
    // it so bind() doesn't fail with EADDRINUSE. (Missing file → ignore the error.)
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    if let Some(parent) = path.parent()
        && let Err(_e) = fs::create_dir_all(parent)
    {
        return BindOutcome::Failed;
    }

    let listener = match UnixListener::bind(path) {
        Ok(l) => l,
        Err(_e) => return BindOutcome::Failed,
    };
    // Local-only: owner read/write, nobody else. Best-effort — a perms failure
    // isn't worth refusing service over, but we try before accepting a byte.
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));

    tokio::spawn(accept_loop(listener, tx));
    BindOutcome::Bound(path.to_path_buf())
}

/// Accept connections forever, serving each on its own task so a slow client can't
/// block the others. Ends when the listener errors (e.g. the socket is removed).
async fn accept_loop(listener: UnixListener, tx: UnboundedSender<Action>) {
    while let Ok((stream, _addr)) = listener.accept().await {
        tokio::spawn(serve_connection(stream, tx.clone()));
    }
}

/// One connection: newline-delimited JSON. A `get_session` query gets a snapshot
/// written back; any other line is a fire-and-forget `RemoteAction` sent into the
/// shared channel — indistinguishable from a keystroke. Malformed lines are dropped
/// (a client bug can't crash the TUI or inject data; `RemoteAction` already excludes
/// `Key` and every data-result variant).
async fn serve_connection(stream: UnixStream, tx: UnboundedSender<Action>) {
    let (read_half, mut write_half) = stream.into_split();
    let mut lines = BufReader::new(read_half).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if serde_json::from_str::<SocketQuery>(line).is_ok() {
            if !answer_session(&tx, &mut write_half).await {
                break;
            }
        } else if let Ok(remote) = serde_json::from_str::<RemoteAction>(line) {
            // Receiver gone ⇒ app is shutting down; stop reading.
            if tx.send(Action::from(remote)).is_err() {
                break;
            }
        }
    }
}

/// Handle one `get_session`: hand the consumer a `oneshot` sender, await the
/// snapshot it fills in, and write it back as a JSON line. Returns `false` (stop
/// serving this connection) when the app is shutting down, the consumer drops the
/// reply, or the socket write fails. Crucially the consumer fills the oneshot and
/// returns WITHOUT awaiting us, so this await can't deadlock the event loop.
async fn answer_session(tx: &UnboundedSender<Action>, w: &mut OwnedWriteHalf) -> bool {
    let (reply_tx, reply_rx) = oneshot::channel();
    if tx
        .send(Action::SessionRequested { reply: reply_tx })
        .is_err()
    {
        return false; // consumer gone → app shutting down
    }
    let Ok(snapshot) = reply_rx.await else {
        return false; // reply dropped without a value → close (client sees EOF)
    };
    let Ok(mut json) = serde_json::to_string(&snapshot) else {
        return false;
    };
    json.push('\n');
    w.write_all(json.as_bytes()).await.is_ok()
}

/// Client half of `get_session`: connect, send the query, and read one response
/// line — with a TIMEOUT so a wedged event loop surfaces as an error, never a hang
/// (the annoying failure mode). A missing/stale socket is [`NO_TUI`].
pub async fn query_session(path: &Path, timeout: Duration) -> Result<String, String> {
    let stream = UnixStream::connect(path)
        .await
        .map_err(|e| match e.kind() {
            ErrorKind::NotFound | ErrorKind::ConnectionRefused => NO_TUI.to_string(),
            _ => format!("could not reach the running Quotail TUI: {e}"),
        })?;
    let (read_half, mut write_half) = stream.into_split();
    write_half
        .write_all(GET_SESSION_LINE)
        .await
        .map_err(|e| format!("failed writing to the Quotail TUI socket: {e}"))?;
    write_half.flush().await.map_err(|e| e.to_string())?;

    let mut lines = BufReader::new(read_half).lines();
    match tokio::time::timeout(timeout, lines.next_line()).await {
        Err(_elapsed) => {
            Err("timed out waiting for the Quotail TUI to reply — it may be busy or wedged".into())
        }
        Ok(Err(e)) => Err(format!(
            "error reading the session from the Quotail TUI: {e}"
        )),
        Ok(Ok(None)) => {
            Err("the Quotail TUI closed the connection without sending a session".into())
        }
        Ok(Ok(Some(line))) => Ok(line),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::types::Timeframe;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio::sync::mpsc::{self, UnboundedReceiver};

    fn temp_sock(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        // pid + tag keeps parallel test runs from colliding without needing a clock.
        p.push(format!("quotail-test-{}-{tag}.sock", std::process::id()));
        let _ = fs::remove_file(&p);
        p
    }

    /// Receive the next action, failing fast instead of hanging the suite forever
    /// if a regression drops it (as a bad wire format silently would).
    async fn next_action(rx: &mut UnboundedReceiver<Action>) -> Action {
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("no action arrived within 5s")
            .expect("channel closed")
    }

    /// Send one `RemoteAction` as a JSON line over a fresh connection. Serializing
    /// the real type (not a hand-written string) exercises the EXACT wire format
    /// the MCP server produces — e.g. a Timeframe travels as its serde name `Y1`,
    /// not the human `1Y` label.
    async fn send(path: &Path, action: &RemoteAction) {
        let mut client = UnixStream::connect(path).await.unwrap();
        let mut line = serde_json::to_string(action).unwrap();
        line.push('\n');
        client.write_all(line.as_bytes()).await.unwrap();
        client.flush().await.unwrap();
    }

    #[tokio::test]
    async fn remote_action_over_socket_becomes_an_action() {
        let path = temp_sock("roundtrip");
        let (tx, mut rx) = mpsc::unbounded_channel();
        assert!(matches!(spawn_listener(&path, tx), BindOutcome::Bound(_)));

        send(
            &path,
            &RemoteAction::SetTimeframe {
                timeframe: Timeframe::Y1,
            },
        )
        .await;

        assert!(matches!(
            next_action(&mut rx).await,
            Action::SetTimeframe(Timeframe::Y1)
        ));
        let _ = fs::remove_file(&path);
    }

    #[tokio::test]
    async fn malformed_line_is_ignored_then_valid_line_still_works() {
        let path = temp_sock("malformed");
        let (tx, mut rx) = mpsc::unbounded_channel();
        assert!(matches!(spawn_listener(&path, tx), BindOutcome::Bound(_)));

        let mut client = UnixStream::connect(&path).await.unwrap();
        client.write_all(b"not json at all\n").await.unwrap();
        client
            .write_all(b"{\"action\":\"add_symbol\",\"symbol\":\"TSLA\"}\n")
            .await
            .unwrap();
        client.flush().await.unwrap();

        assert!(matches!(next_action(&mut rx).await, Action::AddSymbol(s) if s == "TSLA"));
        let _ = fs::remove_file(&path);
    }

    #[tokio::test]
    async fn stale_socket_file_is_reclaimed() {
        let path = temp_sock("stale");
        // Simulate a crashed instance: a leftover FILE with nothing listening.
        fs::write(&path, b"").unwrap();
        assert!(path.exists());

        let (tx, _rx) = mpsc::unbounded_channel();
        // Connect would fail (nothing listening) → we unlink and bind.
        assert!(matches!(spawn_listener(&path, tx), BindOutcome::Bound(_)));
        let _ = fs::remove_file(&path);
    }

    #[tokio::test]
    async fn live_instance_is_detected_and_not_disturbed() {
        let path = temp_sock("live");
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        assert!(matches!(spawn_listener(&path, tx1), BindOutcome::Bound(_)));

        // A second bind on the same path sees a LIVE listener → backs off.
        let (tx2, _rx2) = mpsc::unbounded_channel();
        assert!(matches!(
            spawn_listener(&path, tx2),
            BindOutcome::OtherInstance
        ));

        // The original listener is untouched: it still delivers actions.
        send(&path, &RemoteAction::Refresh).await;
        assert!(matches!(next_action(&mut rx1).await, Action::Refresh));
        let _ = fs::remove_file(&path);
    }

    fn sample_snapshot() -> crate::app::SessionSnapshot {
        use crate::app::{AssetFilter, SessionSnapshot, View};
        SessionSnapshot {
            view: View::Overview,
            timeframe: "1Y".to_string(),
            slots: [Some("NVDA".to_string()), None, None, None],
            focused_slot: 0,
            filter: AssetFilter::All,
            watchlist_selected: Some("AAPL".to_string()),
            detail_symbol: None,
        }
    }

    #[tokio::test]
    async fn get_session_round_trips_over_socket() {
        let path = temp_sock("session");
        let (tx, mut rx) = mpsc::unbounded_channel();
        assert!(matches!(spawn_listener(&path, tx), BindOutcome::Bound(_)));

        // Stand in for the event loop: fill the oneshot from a fixture and return —
        // exactly what update()'s SessionRequested arm does.
        tokio::spawn(async move {
            if let Some(Action::SessionRequested { reply }) = rx.recv().await {
                let _ = reply.send(sample_snapshot());
            }
        });

        let json = query_session(&path, Duration::from_secs(5)).await.unwrap();
        assert!(json.contains("\"timeframe\":\"1Y\""), "got: {json}");
        assert!(
            json.contains("NVDA") && json.contains("AAPL"),
            "got: {json}"
        );
        let _ = fs::remove_file(&path);
    }

    #[tokio::test]
    async fn get_session_times_out_when_reply_is_withheld() {
        let path = temp_sock("session-timeout");
        let (tx, mut rx) = mpsc::unbounded_channel();
        assert!(matches!(spawn_listener(&path, tx), BindOutcome::Bound(_)));

        // Stand-in that RECEIVES the request but HOLDS the sender without replying —
        // a wedged event loop. Holding (not dropping) it means the client never gets
        // an EOF, so ONLY its timeout can save it. This is the annoying hang the
        // client-side timeout exists to prevent.
        tokio::spawn(async move {
            let held = rx.recv().await;
            tokio::time::sleep(Duration::from_secs(30)).await;
            drop(held);
        });

        let start = std::time::Instant::now();
        let err = query_session(&path, Duration::from_millis(300))
            .await
            .unwrap_err();
        assert!(
            err.contains("timed out"),
            "expected a timeout error, got: {err}"
        );
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "must fail fast, not hang"
        );
        let _ = fs::remove_file(&path);
    }
}
