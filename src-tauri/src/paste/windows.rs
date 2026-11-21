//! Windows paste driver: `SendInput` with `KEYEVENTF_UNICODE` per character.
//!
//! Compile-unverified on the Linux dev machine (no mingw-w64); verify with
//! `cargo check --target x86_64-pc-windows-gnu` once a cross toolchain (or a
//! Windows box) is available.

use std::thread::sleep;
use std::time::{Duration, Instant};

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow};

use super::{paste_mode_for_class, ANIMATED_DELAY_MS, PasteMode, TypeReport};

/// Sleep between input injections so the OS gets a chance to deliver events;
/// purely defensive against busy-waiting the loop for long clips.
const SENDINPUT_PACE: Duration = Duration::from_millis(1);

pub(crate) fn type_text(text: &str) -> Result<TypeReport, String> {
    let window_class = foreground_window_class();
    let mode = if is_elevated_target() {
        // Elevated targets refuse synthetic input from a non-elevated process,
        // so the animation must not even be attempted. Stub until Phase 1
        // (token/integrity-level checks); treated as instant for now.
        PasteMode::Instant
    } else {
        paste_mode_for_class(window_class.as_deref())
    };

    eprintln!(
        "[clippie:paste] target class {:?} → {} mode{}",
        window_class,
        mode.label(),
        match mode {
            PasteMode::Animated => format!(" ({ANIMATED_DELAY_MS} ms/char)"),
            PasteMode::Instant => String::new(),
        },
    );

    let started = Instant::now();
    send_text(text, mode)?;
    Ok(TypeReport {
        mode,
        window_class,
        chars: text.chars().count(),
        elapsed: started.elapsed(),
    })
}

/// Injects every character as a Unicode key-down/key-up pair.
fn send_text(text: &str, mode: PasteMode) -> Result<(), String> {
    for character in text.chars() {
        let down = unicode_input(character, false);
        let up = unicode_input(character, true);
        let sent = unsafe {
            // One call per pair: interleaving with the per-char sleep keeps the
            // animation honest even for large clips.
            SendInput(2, [down, up].as_mut_ptr(), size_of::<INPUT>() as i32)
        };
        if sent != 2 {
            return Err(format!("SendInput delivered {sent}/2 events for '{character}'"));
        }
        if mode == PasteMode::Animated {
            sleep(Duration::from_millis(ANIMATED_DELAY_MS));
        } else {
            sleep(SENDINPUT_PACE);
        }
    }
    Ok(())
}

/// Builds a single `KEYEVENTF_UNICODE` input event for one character.
fn unicode_input(character: char, key_up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: character as u16,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Class of the foreground window (the paste target). `Ctrl+Alt` hotkeys and
/// the focusless overlay bar never move foreground focus, so this is the
/// window the user was working in.
fn foreground_window_class() -> Option<String> {
    unsafe {
        let handle = GetForegroundWindow();
        if handle.is_null() {
            return None;
        }
        let mut buffer = [0u16; 256];
        let length = GetClassNameW(handle, buffer.as_mut_ptr(), buffer.len() as i32);
        if length <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buffer[..length as usize]))
    }
}

/// True when the foreground window runs elevated (admin) and would drop our
/// synthetic input. TODO(phase-1): compare the target process token integrity
/// level with ours (OpenProcess + GetTokenInformation); until then, paste into
/// elevated targets is not detected.
fn is_elevated_target() -> bool {
    false
}
