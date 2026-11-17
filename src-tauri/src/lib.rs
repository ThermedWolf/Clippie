mod clips;

use tauri::{AppHandle, LogicalPosition, Manager};

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

/// Docks the overlay bar to the top-center of the primary monitor, flush with
/// the top edge (y = 0). Coordinates are computed from the actual monitor size
/// instead of hardcoding pixels.
fn position_overlay_top_center(app: &AppHandle) -> Result<(), tauri::Error> {
    let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW_LABEL) else {
        eprintln!("overlay window '{OVERLAY_WINDOW_LABEL}' not found; skipping positioning");
        return Ok(());
    };

    let Some(monitor) = overlay.primary_monitor()? else {
        eprintln!("no primary monitor detected; bar stays at its default position");
        return Ok(());
    };

    let monitor_size = monitor.size().to_logical::<f64>(monitor.scale_factor());
    let (bar_width, _) = overlay_logical_size(app);
    let x = ((monitor_size.width - bar_width) / 2.0).max(0.0);

    overlay.set_position(LogicalPosition::new(x, 0.0))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // A failed dock-in only leaves the bar at its default position;
            // log it and keep running rather than aborting the daemon.
            if let Err(error) = position_overlay_top_center(app.handle()) {
                eprintln!("could not dock overlay bar top-center: {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![clips::get_recent_clips])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
