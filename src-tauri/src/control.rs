//! Control plane: a local Unix-socket command server for hotkey actions.
//!
//! Wayland applications cannot register global hotkeys, so on Hyprland the
//! compositor binds exec the `clippie-ctl` trigger, which sends one command
//! over this socket (`assets/hyprland/clippie-binds.lua`). Windows hotkeys
//! ([`crate::hotkeys`]) call the same handlers directly — this module is the
//! single funnel for reveal/toggle/paste, whatever delivered the keystroke.
//!
//! Protocol: one command line per connection (`REVEAL`, `TOGGLE`,
//! `PASTE <1..9>`), answered with a single `ok` or `err <reason>` line.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::store::Store;

/// Name of the socket directory/file under the XDG runtime dir.
const SOCKET_DIR: &str = "clippie";
const SOCKET_FILE: &str = "control.sock";

/// Highest clip position a hotkey can request (Ctrl+Alt+1…9, product rule).
const MAX_PASTE_POSITION: usize = 9;

/// One paste runs at a time; a second trigger while typing is rejected instead
/// of interleaving two `wtype` streams into the target window.
static PASTE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Path of the control socket. Both the daemon and `clippie-ctl` resolve it
/// the same way, keyed to the user's session runtime dir.
pub fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    runtime_dir.join(SOCKET_DIR).join(SOCKET_FILE)
}

/// Binds the control socket and serves commands until the process exits.
/// A bind failure is log-and-continue: the daemon keeps running (capture and
/// UI still work), only compositor-triggered actions are unavailable.
pub fn spawn_control_server(app: AppHandle, store: Arc<Store>) {
    let spawn = std::thread::Builder::new()
        .name("control-server".into())
        .spawn(move || run_server(app, store));
    if let Err(error) = spawn {
        eprintln!("could not spawn control server: {error}");
    }
}

fn run_server(app: AppHandle, store: Arc<Store>) {
    let path = socket_path();
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        eprintln!("could not create control socket dir {}: {error}", parent.display());
        return;
    }
    // A leftover socket file from a crashed daemon would fail the bind.
    match std::fs::remove_file(&path) {
        Ok(()) => eprintln!("[clippie:control] removed stale socket {}", path.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => eprintln!("could not clear stale socket: {error}"),
    }

    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("could not bind control socket {}: {error}", path.display());
            return;
        }
    };
    eprintln!("[clippie:control] listening on {}", path.display());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => serve_connection(stream, &app, &store),
            Err(error) => eprintln!("[clippie:control] accept failed: {error}"),
        }
    }
}

/// Handles one trigger connection: one command in, one reply out.
fn serve_connection(mut stream: UnixStream, app: &AppHandle, store: &Arc<Store>) {
    // A wedged client must never hang the server; one command is tiny.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

    let mut line = String::new();
    {
        let mut reader = BufReader::new(&stream);
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
    }

    let reply = dispatch(line.trim(), app, store);
    let _ = stream.write_all(reply.as_bytes());
    let _ = stream.write_all(b"\n");
}

/// Runs one command and returns the reply line.
fn dispatch(command: &str, app: &AppHandle, store: &Arc<Store>) -> String {
    eprintln!("[clippie:control] command: {command}");
    match parse_command(command) {
        Ok(ControlCommand::Reveal) => {
            reveal_bar(app);
            "ok".into()
        }
        Ok(ControlCommand::Toggle) => {
            toggle_bar(app);
            "ok".into()
        }
        Ok(ControlCommand::Paste(position)) => paste_clip(store, position),
        Err(message) => format!("err {message}"),
    }
}

/// The commands the control plane accepts.
#[derive(Debug, PartialEq, Eq)]
enum ControlCommand {
    Reveal,
    Toggle,
    Paste(usize),
}

/// Parses one wire command line; the error message becomes the `err` reply.
fn parse_command(line: &str) -> Result<ControlCommand, String> {
    let (verb, argument) = line.split_once(' ').unwrap_or((line, ""));
    match (verb, argument) {
        ("REVEAL", "") => Ok(ControlCommand::Reveal),
        ("TOGGLE", "") => Ok(ControlCommand::Toggle),
        ("PASTE", position) => match position.parse::<usize>() {
            Ok(position @ 1..=MAX_PASTE_POSITION) => Ok(ControlCommand::Paste(position)),
            _ => Err(format!("paste position must be 1..{MAX_PASTE_POSITION}")),
        },
        _ => Err(format!("unknown command '{line}'")),
    }
}

/// Shows the bar unconditionally (product rule: Ctrl+Alt+V always reveals,
/// even after a manual hide with Ctrl+Alt+H).
pub fn reveal_bar(app: &AppHandle) {
    let Some(overlay) = app.get_webview_window(crate::OVERLAY_WINDOW_LABEL) else {
        eprintln!("[clippie:control] overlay window not found; cannot reveal");
        return;
    };
    if let Err(error) = overlay.show() {
        eprintln!("[clippie:control] could not show bar: {error}");
        return;
    }
    // Re-dock for compositors that honor client positions (X11); under
    // Wayland the window rules enforce placement and ignore this.
    if let Err(error) = crate::position_overlay_top_center(app) {
        eprintln!("[clippie:control] could not re-dock bar: {error}");
    }
    eprintln!("[clippie:control] bar revealed");
}

/// Hides the bar when shown, reveals it when hidden (product rule: Ctrl+Alt+H
/// toggles). Capture and hotkeys keep running while hidden.
pub fn toggle_bar(app: &AppHandle) {
    let Some(overlay) = app.get_webview_window(crate::OVERLAY_WINDOW_LABEL) else {
        eprintln!("[clippie:control] overlay window not found; cannot toggle");
        return;
    };
    match overlay.is_visible() {
        Ok(true) => {
            if let Err(error) = overlay.hide() {
                eprintln!("[clippie:control] could not hide bar: {error}");
                return;
            }
            eprintln!("[clippie:control] bar hidden (was visible)");
        }
        Ok(false) => {
            eprintln!("[clippie:control] bar hidden (was hidden) → revealing");
            reveal_bar(app);
        }
        Err(error) => eprintln!("[clippie:control] could not read bar visibility: {error}"),
    }
}

/// Looks up the clip and, when it is pasteable text, types it in the
/// background. The reply goes out before typing finishes: hotkey exec must
/// not wait on a long animated paste.
pub fn paste_clip(store: &Arc<Store>, position: usize) -> String {
    match store.nth_recent_clip(position) {
        Err(error) => format!("err could not read clip {position}: {error}"),
        Ok(None) => format!("err no clip at position {position}"),
        Ok(Some(clip)) if clip.kind != crate::clips::ClipKind::Text || clip.content.is_none() => {
            format!(
                "err clip {position} is {} — only text clips paste in this spike",
                clip.kind.label()
            )
        }
        // `unwrap` is safe: the guard above rejected `content: None`.
        Ok(Some(clip)) => {
            if PASTE_IN_PROGRESS.swap(true, Ordering::SeqCst) {
                return "err a paste is already in progress".into();
            }
            let content = clip.content.unwrap_or_default();
            let spawned = std::thread::Builder::new()
                .name("paste-worker".into())
                .spawn(move || {
                    crate::paste::type_clip_text(&content);
                    PASTE_IN_PROGRESS.store(false, Ordering::SeqCst);
                });
            match spawned {
                Ok(_worker) => "ok".into(),
                Err(error) => {
                    PASTE_IN_PROGRESS.store(false, Ordering::SeqCst);
                    format!("err could not start paste: {error}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_supported_command() {
        assert_eq!(parse_command("REVEAL"), Ok(ControlCommand::Reveal));
        assert_eq!(parse_command("TOGGLE"), Ok(ControlCommand::Toggle));
        assert_eq!(parse_command("PASTE 1"), Ok(ControlCommand::Paste(1)));
        assert_eq!(parse_command("PASTE 9"), Ok(ControlCommand::Paste(9)));
    }

    #[test]
    fn rejects_out_of_range_and_malformed_paste_positions() {
        assert!(parse_command("PASTE 0").is_err());
        assert!(parse_command("PASTE 10").is_err());
        assert!(parse_command("PASTE abc").is_err());
        assert!(parse_command("PASTE").is_err());
    }

    #[test]
    fn rejects_unknown_and_typed_verbs() {
        assert!(parse_command("BLAH").is_err());
        assert!(parse_command("REVEAL extra").is_err());
        assert!(parse_command("").is_err());
    }
}
