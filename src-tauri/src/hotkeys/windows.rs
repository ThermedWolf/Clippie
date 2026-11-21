//! Windows global hotkeys: `RegisterHotKey` + a Win32 message pump.
//!
//! Registering with a null window posts `WM_HOTKEY` messages to this thread's
//! own queue, so no helper window is needed. Compile-unverified on the Linux
//! dev machine; verify with `cargo check --target x86_64-pc-windows-gnu`.

use std::sync::Arc;

use windows_sys::Win32::Foundation::{HWND, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG, WM_HOTKEY,
};

use crate::control;
use crate::store::Store;
use tauri::AppHandle;

/// Registered hotkey ids (our own namespace for `WM_HOTKEY`'s wParam).
const HOTKEY_REVEAL: usize = 1;
const HOTKEY_TOGGLE: usize = 2;
/// Paste hotkeys occupy ids 10..19 (Ctrl+Alt+1…9).
const HOTKEY_PASTE_BASE: usize = 10;

/// Virtual-key codes for the hotkey set (ASCII digits/letters == their VK code).
const VK_V: u32 = b'V' as u32;
const VK_H: u32 = b'H' as u32;

pub fn spawn_hotkey_thread(app: AppHandle, store: Arc<Store>) {
    let spawn = std::thread::Builder::new()
        .name("hotkey-watch".into())
        .spawn(move || run_hotkey_loop(app, store));
    if let Err(error) = spawn {
        eprintln!("could not spawn hotkey thread: {error}");
    }
}

fn run_hotkey_loop(app: AppHandle, store: Arc<Store>) {
    let mods = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;

    // A null HWND routes WM_HOTKEY to this thread's message queue.
    let hwnd: HWND = std::ptr::null_mut();
    let registrations: [(usize, u32); 11] = [
        (HOTKEY_REVEAL, VK_V),
        (HOTKEY_TOGGLE, VK_H),
        (HOTKEY_PASTE_BASE, b'1' as u32),
        (HOTKEY_PASTE_BASE + 1, b'2' as u32),
        (HOTKEY_PASTE_BASE + 2, b'3' as u32),
        (HOTKEY_PASTE_BASE + 3, b'4' as u32),
        (HOTKEY_PASTE_BASE + 4, b'5' as u32),
        (HOTKEY_PASTE_BASE + 5, b'6' as u32),
        (HOTKEY_PASTE_BASE + 6, b'7' as u32),
        (HOTKEY_PASTE_BASE + 7, b'8' as u32),
        (HOTKEY_PASTE_BASE + 8, b'9' as u32),
    ];
    for (id, vk) in registrations {
        let registered = unsafe { RegisterHotKey(hwnd, id, mods, vk) };
        if registered == 0 {
            // Commonly a conflict with another app's hotkey; log and keep the
            // rest working rather than aborting the daemon.
            eprintln!("[clippie:hotkey] RegisterHotKey failed for id {id} (vk {vk:#x})");
        }
    }

    let mut message: MSG = unsafe { std::mem::zeroed() };
    loop {
        // GetMessageW blocks until a message arrives: event-driven, no polling.
        let result = unsafe { GetMessageW(&mut message, hwnd, 0, 0) };
        if result <= 0 {
            // 0 = WM_QUIT, -1 = error; either way the hotkey loop ends.
            break;
        }
        if message.message == WM_HOTKEY {
            handle_hotkey(&app, &store, message.wParam);
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn handle_hotkey(app: &AppHandle, store: &Arc<Store>, id: WPARAM) {
    match id as usize {
        HOTKEY_REVEAL => control::reveal_bar(app),
        HOTKEY_TOGGLE => control::toggle_bar(app),
        paste @ HOTKEY_PASTE_BASE..=HOTKEY_PASTE_BASE + 8 => {
            control::paste_clip(store, paste - HOTKEY_PASTE_BASE + 1);
        }
        _ => {}
    }
}

