import { invoke } from "@tauri-apps/api/core";

import type { ClipSummary } from "$types/clip";

/** Fetches the most recent clips from the Rust core (empty until the store lands). */
export async function getRecentClips(): Promise<ClipSummary[]> {
  return invoke<ClipSummary[]>("get_recent_clips");
}
