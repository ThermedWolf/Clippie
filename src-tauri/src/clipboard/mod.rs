//! Clipboard capture: platform watcher threads that feed captured clips into
//! the store and broadcast them to the frontend.
//!
//! Every watcher is event-driven (Wayland data-control watch / Win32 clipboard
//! listener): the thread blocks on the OS event queue and never polls.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::spawn_watcher;
#[cfg(windows)]
pub use windows::spawn_watcher;

use sha2::{Digest, Sha256};
// Rust 2024 has no trait prelude: `emit` needs this trait import.
use tauri::Emitter;

use crate::clips::ClipKind;
use crate::store::Store;

/// Upper bound for text previews shown in the bar; the CSS truncates further.
const PREVIEW_MAX_CHARS: usize = 80;

/// The actual content of a captured clip. Mirrors the clip kinds the product
/// supports: text, images, files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipPayload {
    Text { content: String },
    Image { png: Vec<u8> },
    Files { paths: Vec<String> },
}

impl ClipPayload {
    /// Canonical bytes this payload is identified by. Two captures with equal
    /// canonical bytes are the same clip.
    fn canonical_bytes(&self) -> Vec<u8> {
        match self {
            // Text is identified by its exact UTF-8 bytes.
            ClipPayload::Text { content } => content.as_bytes().to_vec(),
            // Images by the exact encoded PNG bytes.
            ClipPayload::Image { png } => png.clone(),
            // Files by their ordered path list; JSON encoding is the canonical
            // form (paths may contain any bytes, including newlines).
            ClipPayload::Files { paths } => serde_json::to_vec(paths).unwrap_or_default(),
        }
    }
}

/// A fully captured clipboard entry: payload plus the identity/summary fields
/// derived from it, ready for the store.
#[derive(Debug, Clone)]
pub struct CapturedClip {
    pub kind: ClipKind,
    pub payload: ClipPayload,
    /// SHA-256 hex digest of the payload's canonical bytes.
    pub hash: String,
    pub preview: String,
}

impl CapturedClip {
    pub fn new(payload: ClipPayload) -> Self {
        let kind = match payload {
            ClipPayload::Text { .. } => ClipKind::Text,
            ClipPayload::Image { .. } => ClipKind::Image,
            ClipPayload::Files { .. } => ClipKind::Files,
        };
        let canonical = payload.canonical_bytes();
        let hash = hex_encode(&Sha256::digest(&canonical));
        let preview = match &payload {
            ClipPayload::Text { content } => text_preview(content),
            ClipPayload::Image { png } => image_preview(png),
            ClipPayload::Files { paths } => files_preview(paths),
        };
        Self { kind, payload, hash, preview }
    }
}

/// Streams a captured clip into the history and broadcasts it to the frontend.
///
/// Shared by every platform watcher. Deduping happens in the store: when the
/// capture is a repeat of the newest clip, nothing is stored and no event is
/// emitted, so re-copying never spams the history or the badge count.
pub fn handle_capture(app: &tauri::AppHandle, store: &Store, clip: CapturedClip) {
    match store.insert_capture(&clip) {
        // A stored clip is broadcast so the bar updates live; the payload is
        // the same `ClipSummary` the store returns.
        Ok(Some(summary)) => {
            if let Err(error) = app.emit(crate::CLIPBOARD_UPDATED_EVENT, &summary) {
                // The clip is safely stored; a missed UI notification must
                // not fail the capture, so log and continue.
                eprintln!("could not emit clipboard event: {error}");
            }
        }
        // Repeat of the newest clip: intentionally not stored or re-emitted.
        Ok(None) => {}
        // Watcher policy: a failed store write must never take the daemon
        // down; the capture is lost, so say so.
        Err(error) => eprintln!("could not store clip: {error}"),
    }
}

/// Collapses whitespace runs (including newlines) into single spaces so a
/// multi-line copy previews as one line.
fn text_preview(content: &str) -> String {
    let collapsed = content.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&collapsed, PREVIEW_MAX_CHARS)
}

/// Images have no filename (blobs are content-addressed); the PNG IHDR
/// dimensions are the most useful identifier. Reading them needs no decoder:
/// width/height sit at fixed big-endian offsets after the 8-byte signature.
fn image_preview(png: &[u8]) -> String {
    const IHDR_DIMS_OFFSET: usize = 16;
    const IHDR_DIMS_END: usize = 24;

    let dims = if png.len() >= IHDR_DIMS_END {
        let width = u32::from_be_bytes(png[IHDR_DIMS_OFFSET..IHDR_DIMS_OFFSET + 4].try_into().expect("4-byte slice"));
        let height = u32::from_be_bytes(png[IHDR_DIMS_END - 4..IHDR_DIMS_END].try_into().expect("4-byte slice"));
        format!(" {width}×{height} px")
    } else {
        String::new()
    };
    format!("Image{dims}")
}

fn files_preview(paths: &[String]) -> String {
    let count = paths.len();
    let parent = paths
        .first()
        .and_then(|path| std::path::Path::new(path).parent())
        .map(std::path::Path::to_string_lossy)
        .unwrap_or_default();
    let unit = if count == 1 { "file" } else { "files" };
    truncate_chars(&format!("{count} {unit} — {parent}"), PREVIEW_MAX_CHARS)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let cut: String = text.chars().take(max_chars).collect();
    format!("{cut}…")
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_text_yields_same_hash() {
        let a = CapturedClip::new(ClipPayload::Text { content: "hello".into() });
        let b = CapturedClip::new(ClipPayload::Text { content: "hello".into() });
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.preview, "hello");
    }

    #[test]
    fn different_text_yields_different_hash() {
        let a = CapturedClip::new(ClipPayload::Text { content: "hello".into() });
        let b = CapturedClip::new(ClipPayload::Text { content: "world".into() });
        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn file_order_matters_for_identity() {
        let a = CapturedClip::new(ClipPayload::Files { paths: vec!["/a".into(), "/b".into()] });
        let b = CapturedClip::new(ClipPayload::Files { paths: vec!["/b".into(), "/a".into()] });
        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn text_preview_collapses_newlines_and_truncates() {
        let preview = text_preview("line one\nline two\tmore");
        assert_eq!(preview, "line one line two more");

        let long = "x".repeat(PREVIEW_MAX_CHARS + 10);
        let preview = text_preview(&long);
        assert_eq!(preview.chars().count(), PREVIEW_MAX_CHARS + 1); // + the ellipsis
        assert!(preview.ends_with('…'));
    }

    #[test]
    fn image_preview_reads_png_dimensions() {
        // 1x1 transparent PNG.
        let png: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        ];
        let preview = CapturedClip::new(ClipPayload::Image { png: png.to_vec() }).preview;
        assert_eq!(preview, "Image 1×1 px");
    }

    #[test]
    fn files_preview_names_count_and_parent() {
        let preview = files_preview(&["/tmp/opencode/a.png".into(), "/tmp/opencode/b.png".into()]);
        assert_eq!(preview, "2 files — /tmp/opencode");
    }
}
