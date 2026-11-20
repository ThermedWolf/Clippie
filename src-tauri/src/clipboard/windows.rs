//! Windows clipboard capture via a message-only clipboard listener window.
//!
//! `clipboard-win`'s [`Monitor`] blocks in a message pump until the clipboard
//! changes — event-driven, no polling. The monitor window is bound to the
//! thread that created it, so reading the clipboard happens right here.

use std::sync::Arc;
use std::time::Duration;

use clipboard_win::formats;
use clipboard_win::{get_clipboard, Monitor};
use tauri::AppHandle;

use super::{handle_capture, CapturedClip, ClipPayload};
use crate::store::Store;

/// Back-off before recreating the monitor after a failure, so a broken state
/// does not spin the CPU.
const RETRY_DELAY: Duration = Duration::from_secs(2);

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

fn run_watcher(app: AppHandle, store: Arc<Store>) {
    loop {
        // The monitor owns a message-only window bound to this thread; it
        // cannot be moved between threads, so everything happens here.
        let mut monitor = match Monitor::new() {
            Ok(monitor) => monitor,
            Err(error) => {
                eprintln!("clipboard monitor unavailable: {error}");
                std::thread::sleep(RETRY_DELAY);
                continue;
            }
        };
        loop {
            match monitor.recv() {
                // Ok(true): a clipboard update arrived (not a shutdown ping).
                Ok(true) => capture_clipboard(&app, &store),
                Ok(false) => return,
                Err(error) => {
                    eprintln!("clipboard monitor failed: {error}");
                    break;
                }
            }
        }
        std::thread::sleep(RETRY_DELAY);
    }
}

/// Captures one clipboard update: exactly one clip, by format priority
/// (files beat images beat text; an Explorer file copy also offers text).
fn capture_clipboard(app: &AppHandle, store: &Arc<Store>) {
    if formats::FileList.is_available()
        && let Ok(paths) = get_clipboard(formats::FileList)
        && !paths.is_empty()
    {
        handle_capture(app, store, CapturedClip::new(ClipPayload::Files { paths }));
        return;
    }

    // Bitmaps arrive as CF_DIB; arboard decodes them and we re-encode the RGBA
    // pixels to PNG so the store format matches the Wayland capture path.
    if formats::Bitmap.is_available()
        && let Ok(clipboard) = arboard::Clipboard::new()
        && let Ok(image) = clipboard.get_image()
        && let Some(png) = encode_png(&image)
    {
        handle_capture(app, store, CapturedClip::new(ClipPayload::Image { png }));
        return;
    }

    if formats::Unicode.is_available()
        && let Ok(text) = get_clipboard(formats::Unicode)
        && !text.is_empty()
    {
        handle_capture(app, store, CapturedClip::new(ClipPayload::Text { content: text }));
    }
}

/// Encodes arboard's RGBA pixel data as a PNG in memory.
fn encode_png(image: &arboard::ImageData<'_>) -> Option<Vec<u8>> {
    let mut png = Vec::new();
    let encoder = png::Encoder::new(&mut png, image.width as u32, image.height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = match encoder.write_header() {
        Ok(writer) => writer,
        Err(error) => {
            eprintln!("could not encode clipboard image: {error}");
            return None;
        }
    };
    if let Err(error) = writer.write_image_data(&image.bytes) {
        eprintln!("could not encode clipboard image: {error}");
        return None;
    }
    // The writer finalizes the stream (IEND) on drop, completing `png`.
    drop(writer);
    Some(png)
}
