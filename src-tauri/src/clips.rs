use serde::{Deserialize, Serialize};

/// What kind of content a clipboard entry holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
    Files,
}

/// Compact view of a clipboard entry, shared with the frontend over the IPC bridge.
/// Field names serialize as camelCase to match `src/types/clip.ts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipSummary {
    pub id: String,
    pub kind: ClipKind,
    pub preview: String,
    pub created_at: u64,
}

/// Returns the most recent clips, newest first.
///
/// Phase 0 spike: the SQLite store lands in part 2, so an empty history is
/// reported until then. The signature is already the final IPC shape.
#[tauri::command]
pub fn get_recent_clips() -> Vec<ClipSummary> {
    Vec::new()
}
