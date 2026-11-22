//! Linux paste driver: `wtype` (Wayland virtual-keyboard protocol).
//!
//! `wtype` sends real key events through the compositor, so the text lands in
//! whatever window currently has keyboard focus. The overlay bar is a
//! `focus: false` window and the hotkey path never takes focus, so that stays
//! the user's previously-focused window.
//!
//! The text is passed as an argv argument because the installed wtype (0.4)
//! does not read stdin at all — `printf x | wtype` prints its usage line and
//! types nothing (verified at runtime). Trade-off: the clip is briefly
//! visible in `ps` output, and a single exec argument is capped by the
//! kernel at 128 KiB (`MAX_ARG_STRLEN`), which the size guard below turns
//! into a clear error instead of an opaque spawn failure.

use std::process::{Command, Stdio};
use std::time::Instant;

use serde::Deserialize;

use super::{paste_mode_for_class, ANIMATED_DELAY_MS, PasteMode, TypeReport};

/// `wtype` binary; a hard runtime dep of the paste feature on Wayland.
const WTYPE_BIN: &str = "wtype";

pub(crate) fn type_text(text: &str) -> Result<TypeReport, String> {
    // hyprctl is queried per paste (on demand): paste is a rare, user-triggered
    // action, so this is not a polling path.
    let window_class = active_window_class().map(|window| window.class);
    let mode = paste_mode_for_class(window_class.as_deref());

    eprintln!(
        "[clippie:paste] target class {:?} → {} mode{}",
        window_class,
        mode.label(),
        match mode {
            PasteMode::Animated => format!(" ({ANIMATED_DELAY_MS} ms/char)"),
            PasteMode::Instant => {
                if window_class.as_deref().map(super::is_terminal_class).unwrap_or(false) {
                    format!(" (detected terminal class {:?})", window_class)
                } else {
                    String::new()
                }
            }
        },
    );

    let started = Instant::now();
    run_wtype(text, mode)?;
    Ok(TypeReport {
        mode,
        window_class,
        chars: text.chars().count(),
        elapsed: started.elapsed(),
    })
}

/// Largest clip handed to wtype as one argv argument. The kernel caps a
/// single argument at 128 KiB (`MAX_ARG_STRLEN`); this leaves headroom for
/// argv/env overhead so `spawn` never fails with E2BIG on a legal clip.
const MAX_ARGV_TEXT_BYTES: usize = 96 * 1024;

/// Spawns `wtype` with the clip text as an argument and waits for it to
/// finish typing.
fn run_wtype(text: &str, mode: PasteMode) -> Result<(), String> {
    if text.len() > MAX_ARGV_TEXT_BYTES {
        return Err(format!(
            "clip too large for the wtype argv transport ({} bytes, max {} KiB)",
            text.len(),
            MAX_ARGV_TEXT_BYTES / 1024,
        ));
    }
    let output = wtype_command(text, mode)
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("could not start {WTYPE_BIN}: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{WTYPE_BIN} exited with {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    Ok(())
}

/// Builds the wtype invocation: text as a single argv argument (wtype 0.4
/// ignores stdin), with the per-keystroke delay flag only in animated mode.
fn wtype_command(text: &str, mode: PasteMode) -> Command {
    let mut command = Command::new(WTYPE_BIN);
    if mode == PasteMode::Animated {
        command.arg("-d").arg(ANIMATED_DELAY_MS.to_string());
    }
    command.arg(text);
    command
}

/// Minimal view of `hyprctl activewindow -j`; only the class is needed.
#[derive(Debug, Deserialize)]
struct ActiveWindow {
    #[serde(default)]
    class: String,
}

/// Reads the class of the currently focused window via `hyprctl`. Returns
/// `None` outside Hyprland, with no active window, or when `hyprctl` fails —
/// the caller then falls back to animated mode.
fn active_window_class() -> Option<ActiveWindow> {
    let output = Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()
        .ok()?;
    // With no active window hyprctl prints a non-JSON placeholder; treat any
    // parse failure as "unknown target".
    serde_json::from_slice(&output.stdout).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_window_rejects_non_json_output() {
        // `hyprctl activewindow -j` prints `invalid` when nothing is focused;
        // that must map to None, not an error.
        assert!(serde_json::from_str::<ActiveWindow>("invalid").is_err());
    }

    #[test]
    fn wtype_args_animated_uses_delay_flag_and_text_arg() {
        // Mode → argv mapping is pure policy; assert the delay flag and the
        // clip text are both present as arguments in animated mode.
        let argv = format!("{:?}", wtype_command("clip text", PasteMode::Animated));
        assert!(argv.contains("\"-d\"") && argv.contains("\"10\""), "{argv}");
        assert!(argv.contains("clip text"), "{argv}");
    }

    #[test]
    fn wtype_args_instant_omits_delay_flag() {
        let argv = format!("{:?}", wtype_command("clip text", PasteMode::Instant));
        assert!(!argv.contains("-d"), "{argv}");
        assert!(argv.contains("clip text"), "{argv}");
    }

    #[test]
    fn oversized_clip_is_rejected_before_spawn() {
        // wtype only takes text as one argv argument, capped by the kernel at
        // 128 KiB; an oversized clip must fail with a clear error instead of
        // an opaque E2BIG (run_wtype checks before spawning, so this test
        // never launches a process).
        let text = "x".repeat(MAX_ARGV_TEXT_BYTES + 1);
        let error = run_wtype(&text, PasteMode::Instant).unwrap_err();
        assert!(error.contains("too large"), "{error}");
    }
}
