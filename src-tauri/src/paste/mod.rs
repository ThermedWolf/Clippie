//! Paste engine: types a stored clip's text into the currently focused window.
//!
//! Animated typing is the default (product rule); terminals get an instant
//! paste because synthetic per-char keystrokes fight the terminal's own
//! input handling. The platform drivers (`linux`/`windows`) detect the paste
//! target, pick the mode, and type; this module holds the shared policy.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as driver;
#[cfg(windows)]
use windows as driver;

/// Per-character delay for animated typing, in milliseconds. Product rule:
/// the typing animation runs at ~8–12 ms per char.
pub const ANIMATED_DELAY_MS: u64 = 10;

/// Window classes that get an instant paste instead of the typing animation.
/// Matched case-insensitively as substrings, so variants like `footclient`
/// and `Alacritty` are covered without listing every spelling.
pub const TERMINAL_CLASSES: [&str; 4] = ["foot", "alacritty", "kitty", "ghostty"];

/// How fast a paste is delivered to the target window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteMode {
    /// Type with a per-character delay (`ANIMATED_DELAY_MS`).
    Animated,
    /// Type everything in one go.
    Instant,
}

impl PasteMode {
    /// Human-readable label for log lines.
    pub fn label(self) -> &'static str {
        match self {
            PasteMode::Animated => "animated",
            PasteMode::Instant => "instant",
        }
    }
}

/// True when the paste target is a terminal, which always gets instant paste.
pub fn is_terminal_class(window_class: &str) -> bool {
    let class = window_class.to_ascii_lowercase();
    TERMINAL_CLASSES.iter().any(|terminal| class.contains(terminal))
}

/// Decides the paste mode from the active window's class. An undetectable
/// target defaults to animated typing (the product default).
pub fn paste_mode_for_class(window_class: Option<&str>) -> PasteMode {
    match window_class {
        Some(class) if is_terminal_class(class) => PasteMode::Instant,
        _ => PasteMode::Animated,
    }
}

/// Result of one platform type operation, for logging.
pub(crate) struct TypeReport {
    pub mode: PasteMode,
    /// Class of the window the paste was delivered to, when detectable.
    pub window_class: Option<String>,
    pub chars: usize,
    pub elapsed: std::time::Duration,
}

/// Types `text` into the focused window via the platform driver and logs the
/// outcome. Runs on its own thread (spawned by the control server), so any
/// failure here is log-only: the hotkey press itself must never hang.
pub fn type_clip_text(text: &str) {
    match driver::type_text(text) {
        Ok(report) => eprintln!(
            "[clippie:paste] typed {} chars in {} ms (mode {}, target class {:?})",
            report.chars,
            report.elapsed.as_millis(),
            report.mode.label(),
            report.window_class,
        ),
        Err(error) => eprintln!("[clippie:paste] paste failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animated_delay_is_in_product_range() {
        // Product rule: ~8–12 ms per character.
        assert!((8..=12).contains(&ANIMATED_DELAY_MS));
    }

    #[test]
    fn known_terminal_classes_get_instant_mode() {
        for class in ["foot", "Alacritty", "kitty", "ghostty", "footclient"] {
            assert_eq!(paste_mode_for_class(Some(class)), PasteMode::Instant, "{class}");
        }
    }

    #[test]
    fn non_terminal_and_unknown_classes_get_animated_mode() {
        assert_eq!(paste_mode_for_class(Some("zenity")), PasteMode::Animated);
        assert_eq!(paste_mode_for_class(Some("firefox")), PasteMode::Animated);
        assert_eq!(paste_mode_for_class(Some("")), PasteMode::Animated);
        // Undetectable target falls back to the animated default.
        assert_eq!(paste_mode_for_class(None), PasteMode::Animated);
    }

    #[test]
    fn terminal_match_is_substring_and_case_insensitive() {
        assert!(is_terminal_class("org.wezfurlong.wezterm-foot"));
        assert!(is_terminal_class("KITTY"));
        assert!(!is_terminal_class("zenity"));
    }
}
