# Clippie

> A lightweight clipboard manager for Windows & Linux — a floating bar that keeps every copy one keystroke away.

**Status: early development.** The features below describe the agreed product design. There is nothing to install yet; build instructions will land with the first working build.

## What is Clippie?

Clippie runs quietly in the background as a small floating bar at the edge of your screen. It records what you copy, and lets you recall, preview and paste any previous clip with a single chord — with smooth animations for every interaction. It is built to be efficient: event-driven, no polling, minimal RAM and near-zero CPU while idle.

## Features (planned)

- **Clipboard history** — text, images and files, deduplicated and capped.
- **Instant recall** — paste the 1st–9th most recent clip directly, or browse history in the bar.
- **Drag-copy** — hold the chord, drag over any text, and it copies on release.
- **Animated paste** — a quick letter-by-letter typing effect, with an automatic instant-paste fallback for apps where synthetic input misbehaves (e.g. terminals).
- **Floating bar** — collapsed to a subtle pill, expands on hover; dock it to the top, bottom, left or right edge of your screen.
- **Hide on demand** — toggle the bar completely out of the way; capture and hotkeys keep working.
- **Settings** — theming, animation levels (including a reduced mode), fully remappable hotkeys, Dutch & English (system language by default).
- **Device sync (later)** — share clips between your own devices over Wi-Fi, peer-to-peer and end-to-end encrypted. No accounts, no cloud.

## Platform support

| Platform | Support |
|---|---|
| Windows 10 / 11 | Full |
| Linux — X11 | Full |
| Linux — Hyprland (Wayland) | Full, via a small compositor config Clippie generates for you |
| Linux — other Wayland desktops | Reduced: history and the bar work, but global hotkeys, drag-copy and animated paste require desktop-environment support |

## Keyboard shortcuts (defaults — every binding is remappable)

| Shortcut | Action |
|---|---|
| `Ctrl+Alt` (hold) + drag | Copy the selected text on release |
| `Ctrl+Alt+V` | Show history strip (always reveals the bar, even when hidden) |
| `Ctrl+Alt+1` … `Ctrl+Alt+9` | Paste the 1st–9th most recent clip |
| `←` / `→` | Scroll through older clips |
| `↓` / `↑` | Expand to a larger list / collapse back |
| `Ctrl+Alt+H` | Show / hide the bar completely |

> Windows note: on international keyboard layouts, `Ctrl+Alt` is also how `AltGr` is delivered. Clippie's defaults can be changed in settings if this interferes with typing special characters.

## Roadmap

1. **Phase 0 — Spike** — minimal bar on both OSes: transparency, always-on-top, clipboard capture, one hotkey, one paste.
2. **Phase 1 — Core** — full history, hotkeys, arrow navigation, drag-copy, animated paste with fallback, copy animation.
3. **Phase 2 — Polish** — settings, theming, Dutch/English, autostart, tray, hide/show.
4. **Phase 3 — Sync** — Wi-Fi peer discovery, pairing, end-to-end encrypted transfer.

## Tech

Built with [Tauri 2](https://tauri.app): a Rust core (clipboard, hotkeys, storage, sync) with a Svelte 5 interface — chosen for a small footprint and full animation control.

## License

[MIT](LICENSE)
