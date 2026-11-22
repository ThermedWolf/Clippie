-- Clippie — Hyprland window rules for the overlay bar
--
-- Pins the Clippie overlay bar (app_id/class "clippie") as a floating,
-- pinned, undecorated 420x56 bar docked flush to the top-center of the
-- monitor. Under Wayland an app cannot position its own toplevel window,
-- so the compositor must do it — these rules are the Hyprland half of the
-- Phase 0 placement story (the Rust side only covers compositors that do
-- honor client positions, e.g. X11).
--
-- Written for Hyprland >= 0.56, where window rules are declared through the
-- Lua config API (hl.window_rule); the classic `windowrulev2 = ...` line
-- syntax no longer parses there.
--
-- How to install (Omarchy keeps Hyprland config under ~/.config/hypr/):
--   1. Copy this file into ~/.config/hypr/clippie-windowrules.lua
--   2. Load it from your config, e.g. add to ~/.config/hypr/hyprland.lua:
--          require("hypr.clippie-windowrules")
--   3. Reload the config (`hyprctl reload` or Omarchy's reload bind).
--
-- NOTE: a layer-shell surface (wlr-layer-shell) is the long-term proper
-- solution for docked bars — tracked as a Phase 2 candidate. These rules are
-- the stopgap, and they leave the placement user-overridable in their own
-- config (last matching rule wins).

hl.window_rule({
  match = { class = "^(clippie)$" },
  float = true,
  pin = true,
  border_size = 0, -- replaces the legacy `noborder` rule
  rounding = 0, -- replaces the legacy `norounding` rule
  size = { 420, 56 },
  -- Monitor-local expression: top edge, horizontally centered on the monitor.
  move = { "(monitor_w - window_w) / 2", 0 },
})
