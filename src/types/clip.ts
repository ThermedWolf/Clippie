/**
 * Shared clip types for the Tauri IPC bridge.
 * Mirrors `src-tauri/src/clips.rs` (serde camelCase) — keep both in sync.
 */

export const CLIP_KINDS = ["text", "image", "files"] as const;

export type ClipKind = (typeof CLIP_KINDS)[number];

export interface ClipSummary {
  id: string;
  kind: ClipKind;
  /** Short human-readable representation shown in the bar. */
  preview: string;
  /** Unix timestamp in milliseconds. */
  createdAt: number;
}
