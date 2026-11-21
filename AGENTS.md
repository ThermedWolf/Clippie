# Clippie

Cross-platform (Windows + Linux) clipboard manager: floating bar docked to a screen edge, global hotkeys, animated paste, Wi-Fi sync planned. Runs as an always-on background daemon — efficiency is a hard requirement: event-driven everywhere, no polling, single instance, minimal RAM/CPU when idle.

## Status

- Phase 0 spike, parts 1–3 implemented and verified end-to-end on Hyprland:
  - Part 1–2 (bar + capture + store): transparent always-on-top bar (focus:false, never takes keyboard focus), event-driven Wayland watcher (wl-clipboard-rs data-control), SQLite store at `~/.local/share/clippie/clippie.db` (sync-ready schema: UUIDv7 ids, `device_id`, timestamps, soft-delete tombstones), content-addressed image blobs under `~/.local/share/clippie/blobs/`, files as path references only, 500-row cap, consecutive-duplicate dedupe by content hash.
  - Part 3 (hotkey delivery + paste): Unix-socket control server at `$XDG_RUNTIME_DIR/clippie/control.sock` + `clippie-ctl` trigger binary (`<reveal|toggle|paste-1..9>`; exits 2 with a clear message when the daemon isn't running). Hyprland binds exec `clippie-ctl`; protocol is one line per command (`PASTE <n>`), reply `ok`/`err <msg>`. Paste engine drives `wtype`: animated typing at 10 ms/char by default, instant for terminals (foot/alacritty/kitty/ghostty, substring+case-insensitive match), undetectable target defaults to animated. Paste runs on a worker thread (hotkey press never hangs); the target window keeps focus throughout (verified).
- Hotkey bind injection (verified): `hyprctl eval 'hl.bind("CTRL + ALT + V", hl.dsp.exec_cmd("<abs path>/clippie-ctl reveal"), { description = "…" })'` — no config file edit needed; `hl.unbind("CTRL + ALT + V")` reverts. Binds exec the ctl binary, which talks to the daemon over the socket. Real key events were verified with a uinput injector (`tools/ukey.py`, same mechanism as ydotool; needs `/dev/uinput` writable by the seat user): toggle-hide (Ctrl+Alt+H), reveal-while-hidden (Ctrl+Alt+V), and paste (Ctrl+Alt+1) all fired through the full chain key → bind → ctl → socket → daemon.
- wtype gotchas (installed 0.4, verified empirically): text MUST be an argv argument — stdin is never read (prints usage, types nothing; the piped-stdin design was silently a no-op and is fixed). `-d <ms>` per-keystroke delay works with argv text. Multiple text args get a space inserted between them, so chunking is not possible; clips > 96 KiB are rejected with a clear error (kernel caps one argv at 128 KiB). Clip text is briefly visible in `ps` — accepted spike trade-off (module doc documents it).
- Windows code paths (watcher, paste driver, `RegisterHotKey` hotkey loop) are written but compile-unverified — no mingw-w64 cross-compiler on the dev machine (`libsqlite3-sys` build script fails first). Verify on a Windows box or after installing mingw-w64: `rustup target add x86_64-pc-windows-gnu && cargo check --target x86_64-pc-windows-gnu`.
- Verified commands (run from repo root): `npm install` (deps), `npm run tauri dev` (dev app; also rebuilds+restarts the daemon on Rust changes), `npm run check` (svelte-check), `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` (Rust lint), `cargo test --manifest-path src-tauri/Cargo.toml` (30 unit tests). `src-tauri/Cargo.toml` sets `default-run = "clippie"` (two bins — without it `cargo run` fails on ambiguity).
- Next spike work: bar layout polish (scroll/grid), settings window, then Windows verification. The decisions below were agreed with the user and are binding — don't re-litigate the stack or product rules.

## Locked stack

- Tauri 2: Rust core in `src-tauri/` (daemon: clipboard watcher, hotkey engine, paste engine, SQLite store; sync module later), Svelte 5 + TypeScript + Tailwind in `src/` (overlay bar + settings window).
- SQLite for history/settings; schema must stay sync-ready from day one: UUIDs, `device_id`, timestamps, soft-delete tombstones. Retrofitting this is expensive.
- Image blobs live on disk, content-addressed; SQLite holds metadata only.

## Platform constraints (reread before touching platform code)

- Primary Linux target is Wayland/Hyprland (user runs Omarchy). Global hotkeys, drag-copy (hold chord + select + release = copy), and animated paste (synthetic keystrokes) are impossible app-side on Wayland: they require Hyprland cooperation — generated config binds / the global-shortcuts protocol — and typing via `wtype`. X11 uses xdotool; Windows uses native hooks + SendInput. Clippie should auto-detect Hyprland and offer to install its config snippet.
- Windows: Ctrl+Alt is delivered as AltGr on international layouts. Every hotkey MUST be remappable in settings; Ctrl+Alt stays the default chord.
- Release scope: full support on Windows + Hyprland + X11. Other Wayland desktops get reduced functionality by design — don't add workarounds for them.
- The bar is a layer-shell surface on Wayland (dockable to any edge) and a borderless always-on-top window on Windows. The overlay window is sized exactly to the bar's content (no click-through hacks).

## Product rules (user-agreed, don't drift)

- Clip types: text, images, files. Files are stored as path references + metadata only — never copied into the store.
- Paste: typing animation by default (~8–12 ms per char), with automatic instant-paste fallback for terminals and elevated windows.
- Ctrl+Alt+V (history) ALWAYS reveals the bar, even if it was hidden with Ctrl+Alt+H. Hiding is a manual opt-out; clipboard capture and hotkeys keep running while hidden.
- Hotkeys: Ctrl+Alt+1…9 paste the nth clip; ←/→ scroll history, ↓ expands to a grid, ↑ collapses (↑ again at 1-row shrinks the bar away); Ctrl+Alt+H toggles the bar.
- Bar position setting: top/bottom/left/right. Layout must stay orientation-aware: left/right render a vertical column with transposed arrow navigation.
- i18n: Dutch + English catalogs, system locale as default.
- Animations (copy fly-in, paste typing, bar expand/shrink) are individually toggleable, plus a global reduced mode.
- Autostart on boot: systemd user service on Linux, Task Scheduler/Run key on Windows.
- Phasing: Phase 0 spike first (transparent always-on-top bar + clipboard capture + one hotkey + one paste on both OSes) to de-risk the platform layer before any polish.
