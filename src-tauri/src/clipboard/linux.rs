//! Wayland clipboard capture via the data-control watch mechanism.
//!
//! `wl-clipboard-rs`'s [`Watcher`] blocks on compositor events, so the watcher
//! thread never polls: it sleeps until the selection actually changes.

use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use percent_encoding::percent_decode_str;
use tauri::AppHandle;
use wl_clipboard_rs::paste::Seat;
use wl_clipboard_rs::watch::{ClipboardEvent, ClipboardType, Offer, Watcher};

use super::{handle_capture, CapturedClip, ClipPayload};
use crate::store::Store;

/// Back-off before recreating the watcher after a failure, so a missing
/// compositor or seat does not spin the CPU.
const RETRY_DELAY: Duration = Duration::from_secs(2);

/// MIME types this watcher recognizes, in capture priority order (files beat
/// images beat text: a file manager's file copy also offers plain text).
const URI_LIST_MIME: &str = "text/uri-list";
const PNG_MIME: &str = "image/png";
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const TEXT_MIMES: [&str; 2] = ["text/plain;charset=utf-8", "text/plain"];

pub fn spawn_watcher(app: AppHandle, store: Arc<Store>) {
    let spawn = std::thread::Builder::new()
        .name("clipboard-watch".into())
        .spawn(move || run_watcher(app, store));
    // A missing watcher thread means no capture at all; the app still runs so
    // the UI can report the (empty) history.
    if let Err(error) = spawn {
        eprintln!("could not spawn clipboard watcher: {error}");
    }
}

/// Keeps one watcher alive for the process' lifetime, recreating it whenever
/// it dies (compositor restart, Wayland disconnect). Watcher errors are
/// log-and-continue by policy: the daemon must never panic.
fn run_watcher(app: AppHandle, store: Arc<Store>) {
    loop {
        // Watch only the regular clipboard: capturing the primary selection
        // would flood the history with everything the user selects, not copies.
        match Watcher::new(ClipboardType::Regular, Seat::Unspecified) {
            Ok(mut watcher) => event_loop(&mut watcher, &app, &store),
            Err(error) => eprintln!("clipboard watcher unavailable: {error}"),
        }
        std::thread::sleep(RETRY_DELAY);
    }
}

fn event_loop(watcher: &mut Watcher, app: &AppHandle, store: &Arc<Store>) {
    loop {
        match watcher.next_event() {
            Ok(Some(ClipboardEvent::Changed {
                mime_types,
                mut offer,
                ..
            })) => capture_changed(&mime_types, &mut offer, app, store),
            // Selection cleared: nothing to capture.
            Ok(Some(ClipboardEvent::Cleared { .. })) => {}
            // Watcher cancelled or Wayland failed: recreate after the retry
            // delay (or never, if the process is shutting down).
            Ok(None) => return,
            Err(error) => {
                eprintln!("clipboard watch failed: {error}");
                return;
            }
        }
    }
}

/// Captures one selection change: exactly one clip, picked by MIME priority.
fn capture_changed(
    mime_types: &[String],
    offer: &mut Offer<'_>,
    app: &AppHandle,
    store: &Arc<Store>,
) {
    let offers = |wanted: &str| mime_types.iter().any(|mime| mime == wanted);

    if offers(URI_LIST_MIME)
        && let Some(paths) = read_file_paths(offer)
    {
        handle_capture(app, store, CapturedClip::new(ClipPayload::Files { paths }));
        return;
    }

    if offers(PNG_MIME)
        && let Some(png) = read_png(offer)
    {
        handle_capture(app, store, CapturedClip::new(ClipPayload::Image { png }));
        return;
    }

    // Text is the fallback so a link copy (uri-list of http URLs, plus a plain
    // text offer) still lands in the history as text.
    if let Some(content) = TEXT_MIMES.iter().find_map(|mime| read_text(offer, mime)) {
        handle_capture(app, store, CapturedClip::new(ClipPayload::Text { content }));
    }
}

fn read_text(offer: &mut Offer<'_>, mime: &str) -> Option<String> {
    let mut pipe = match offer.receive(mime) {
        Ok(pipe) => pipe,
        // The source may vanish before we read (app closed); skip the capture.
        Err(error) => {
            eprintln!("could not request {mime} from clipboard: {error}");
            return None;
        }
    };
    let mut contents = String::new();
    if let Err(error) = pipe.read_to_string(&mut contents) {
        eprintln!("could not read {mime} from clipboard: {error}");
        return None;
    }
    // An empty offer is not a clip; also keeps "cleared" selections out.
    if contents.is_empty() {
        return None;
    }
    Some(contents)
}

fn read_png(offer: &mut Offer<'_>) -> Option<Vec<u8>> {
    let mut pipe = match offer.receive(PNG_MIME) {
        Ok(pipe) => pipe,
        Err(error) => {
            eprintln!("could not request {PNG_MIME} from clipboard: {error}");
            return None;
        }
    };
    let mut png = Vec::new();
    if let Err(error) = pipe.read_to_end(&mut png) {
        eprintln!("could not read {PNG_MIME} from clipboard: {error}");
        return None;
    }
    // Only actual PNGs: some sources advertise image/png loosely, and a
    // mis-decoded blob would otherwise be written into the store.
    if !png.starts_with(&PNG_SIGNATURE) {
        eprintln!("ignoring clipboard image: not a PNG payload");
        return None;
    }
    Some(png)
}

fn read_file_paths(offer: &mut Offer<'_>) -> Option<Vec<String>> {
    let contents = read_text(offer, URI_LIST_MIME)?;
    let paths = file_paths_from_uri_list(&contents);
    // No file:// entries means the uri-list holds remote links; returning None
    // falls through to the plain-text capture instead.
    if paths.is_empty() {
        return None;
    }
    Some(paths)
}

/// Parses `text/uri-list` (one URI per line, `#` comments) into plain file
/// paths. Percent-encoded file URLs are decoded back to real paths; non-file
/// URIs (http links, mailto, …) are dropped here, not turned into clips.
fn file_paths_from_uri_list(contents: &str) -> Vec<String> {
    contents
        .lines()
        // `lines()` keeps the trailing `\r` of CRLF-terminated lists.
        .map(|line| line.trim_end_matches('\r'))
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.strip_prefix("file://"))
        .map(|path| percent_decode_str(path).decode_utf8_lossy().into_owned())
        .filter(|path| !path.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_list_yields_decoded_paths() {
        let paths = file_paths_from_uri_list("file:///tmp/opencode/my%20file.png\n");
        assert_eq!(paths, vec!["/tmp/opencode/my file.png"]);
    }

    #[test]
    fn uri_list_handles_crlf_comments_and_remote_links() {
        let list = "# comment\r\nfile:///a.png\r\nhttps://example.com/x\r\n\r\n";
        let paths = file_paths_from_uri_list(list);
        assert_eq!(paths, vec!["/a.png"]);
    }
}
