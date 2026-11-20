mod clipboard;
mod clips;
mod store;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(target_os = "linux")]
use gtk::prelude::WidgetExt;
use tauri::{AppHandle, LogicalPosition, Manager, Monitor, WebviewWindow, WindowEvent};

/// Clipboard change event pushed to the frontend when a new clip is stored.
/// Must match `CLIPBOARD_UPDATED_EVENT` in `src/types/events.ts`.
pub const CLIPBOARD_UPDATED_EVENT: &str = "clippie://clipboard-updated";

/// Label of the overlay bar window declared in `tauri.conf.json`.
const OVERLAY_WINDOW_LABEL: &str = "overlay";
/// Fallback overlay geometry; mirrors `tauri.conf.json` and is only used when
/// the window config cannot be read at runtime.
const OVERLAY_FALLBACK_SIZE: (f64, f64) = (420.0, 56.0);

/// Logical (scale-independent) size of the overlay bar, read from the window
/// config so the frontend CSS and this positioning logic stay in sync.
fn overlay_logical_size(app: &AppHandle) -> (f64, f64) {
    let window_config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == OVERLAY_WINDOW_LABEL);

    match window_config {
        Some(config) => (config.width, config.height),
        None => OVERLAY_FALLBACK_SIZE,
    }
}

/// Picks the monitor the bar should dock to. X11 and Windows expose a primary
/// monitor; GDK/Wayland does not, so `primary_monitor()` returns None there
/// and the monitor the window currently sits on is used instead.
fn overlay_target_monitor(overlay: &WebviewWindow) -> Option<Monitor> {
    overlay
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| overlay.current_monitor().ok().flatten())
        .or_else(|| overlay.available_monitors().ok().and_then(|all| all.into_iter().next()))
}

/// Docks the overlay bar to the top-center of the target monitor, flush with
/// the top edge (y = 0). Coordinates are computed from the actual monitor size
/// instead of hardcoding pixels.
fn position_overlay_top_center(app: &AppHandle) -> Result<(), tauri::Error> {
    let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW_LABEL) else {
        eprintln!("overlay window '{OVERLAY_WINDOW_LABEL}' not found; skipping positioning");
        return Ok(());
    };

    let Some(monitor) = overlay_target_monitor(&overlay) else {
        eprintln!("no monitor detected; bar stays at its default position");
        return Ok(());
    };

    let monitor_size = monitor.size().to_logical::<f64>(monitor.scale_factor());
    let (bar_width, _) = overlay_logical_size(app);
    let x = ((monitor_size.width - bar_width) / 2.0).max(0.0);

    overlay.set_position(LogicalPosition::new(x, 0.0))
}

/// Works around a GTK3 sizing quirk that would map the bar 200px tall
/// instead of the configured 56px: a webview child reports a natural size of
/// 0x0, and GtkWindow then assumes `NO_CONTENT_CHILD_NAT` (200px) as its
/// natural size — which a non-resizable window enforces as WM min/max
/// geometry hints, overriding the configured height. An explicit size
/// request bypasses both the 200px fallback and the hint pinning, so the
/// bar maps at its configured size.
#[cfg(target_os = "linux")]
fn pin_overlay_size(app: &AppHandle) -> Result<(), tauri::Error> {
    let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW_LABEL) else {
        eprintln!("overlay window '{OVERLAY_WINDOW_LABEL}' not found; skipping size pin");
        return Ok(());
    };

    let (width, height) = overlay_logical_size(app);
    // Must run before the window is first mapped (setup() precedes the event
    // loop), so the initial configure request already uses the right size.
    overlay
        .gtk_window()?
        .set_size_request(width as i32, height as i32);
    Ok(())
}

/// Re-docks the bar once the compositor has actually placed the window.
/// setup() runs before the window is mapped, and compositors are free to
/// override pre-map positions with their own placement (Hyprland's floating
/// placement races setup()'s set_position), so the position is re-asserted on
/// the first placement-related window event. On Wayland a client position is
/// ignored by protocol anyway — there the Hyprland window rules
/// (assets/hyprland/clippie-windowrules.lua) enforce the dock position.
fn dock_overlay_once_placed(app: &AppHandle) {
    let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW_LABEL) else {
        return;
    };

    // Re-assert only once: afterwards the dock position is final for the
    // session (the bar position is a setting, not a draggable surface).
    let re_docked = Arc::new(AtomicBool::new(false));
    let app = app.clone();
    overlay.on_window_event(move |event| {
        // Resized/Moved fire when the compositor (re)configures the window,
        // e.g. when it maps; Focused covers placements that emit neither.
        let is_placement_event = matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::Moved(_) | WindowEvent::Focused(_)
        );
        if !is_placement_event || re_docked.swap(true, Ordering::Relaxed) {
            return;
        }

        // Same policy as setup(): a failed re-dock only leaves the bar where
        // the compositor put it; log it and keep running.
        if let Err(error) = position_overlay_top_center(&app) {
            eprintln!("could not re-dock overlay bar top-center: {error}");
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // History store under the user data dir (XDG data dir on Linux,
            // Roaming AppData on Windows). A store failure at startup is
            // fatal: without it there is no history feature at all.
            let data_dir = dirs::data_dir()
                .map(|dir| dir.join("clippie"))
                .ok_or("could not determine user data directory")?;
            let store = Arc::new(store::Store::open(&data_dir).map_err(|error| error.to_string())?);

            // Event-driven capture: the watcher thread feeds clips into the
            // store and pushes `CLIPBOARD_UPDATED_EVENT` to the frontend.
            clipboard::spawn_watcher(app.handle().clone(), Arc::clone(&store));
            app.manage(store);

            // A failed dock-in only leaves the bar at its default position;
            // log it and keep running rather than aborting the daemon.
            if let Err(error) = position_overlay_top_center(app.handle()) {
                eprintln!("could not dock overlay bar top-center: {error}");
            }
            // Same policy for the size pin: a GTK3-only workaround; on other
            // platforms the window config is honored as-is.
            #[cfg(target_os = "linux")]
            if let Err(error) = pin_overlay_size(app.handle()) {
                eprintln!("could not pin overlay bar size: {error}");
            }
            dock_overlay_once_placed(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![clips::get_recent_clips])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
