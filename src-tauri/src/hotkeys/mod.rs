//! Global hotkey delivery per platform.
//!
//! Wayland gives applications no global-hotkey access: on Hyprland the binds
//! live in the compositor config (`assets/hyprland/clippie-binds.lua`) and
//! exec the `clippie-ctl` trigger, which talks to the control socket
//! ([`crate::control`]). X11 (XGrabKey) is a later-phase item.

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::spawn_hotkey_thread;
