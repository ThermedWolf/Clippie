-- Clippie — Hyprland keybinds for the Phase 0 spike
--
-- Wayland applications cannot register global hotkeys, so the compositor
-- owns the binds: each one execs the tiny `clippie-ctl` trigger, which sends
-- a single command to the running Clippie daemon over a Unix socket. When
-- the daemon is not running, `clippie-ctl` exits non-zero immediately — a
-- keypress never hangs.
--
-- Binds (product rules):
--   CTRL ALT + V  reveal the bar (ALWAYS reveals, even after a manual hide)
--   CTRL ALT + H  toggle the bar (hide is a manual opt-out; capture keeps
--                 running while hidden)
--   CTRL ALT + 1  paste the most recent clip (2…9 follow the same pattern)
--
-- How to install (Omarchy keeps Hyprland config under ~/.config/hypr/):
--   1. Build the trigger and put it on your PATH:
--          cargo build --manifest-path <clippie-repo>/src-tauri/Cargo.toml --bin clippie-ctl
--          cp <clippie-repo>/src-tauri/target/debug/clippie-ctl ~/.local/bin/
--   2. Copy this file into ~/.config/hypr/clippie-binds.lua
--   3. Load it from your config, e.g. add to ~/.config/hypr/hyprland.lua:
--          require("hypr.clippie-binds")
--   4. Reload the config (`hyprctl reload` or Omarchy's reload bind).
--
-- Alternatively inject at runtime without editing any config file (not
-- persistent across Hyprland restarts; verified working on 0.56):
--   hyprctl eval '<paste one hl.bind(...) line from below>'
--
-- Written for Hyprland >= 0.56, where binds are declared through the Lua
-- config API (hl.bind); the classic `bind = ...` line syntax no longer
-- parses there.

hl.bind("CTRL + ALT + V", hl.dsp.exec_cmd("clippie-ctl reveal"), {
  description = "Clippie: reveal the clipboard bar",
})
hl.bind("CTRL + ALT + H", hl.dsp.exec_cmd("clippie-ctl toggle"), {
  description = "Clippie: toggle the clipboard bar",
})
hl.bind("CTRL + ALT + 1", hl.dsp.exec_cmd("clippie-ctl paste-1"), {
  description = "Clippie: paste clip 1",
})
