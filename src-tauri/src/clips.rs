use serde::{Deserialize, Serialize};

use crate::store::{Store, MAX_HISTORY_ROWS};

/// What kind of content a clipboard entry holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
    Files,
}

impl ClipKind {
    /// Storage label; matches serde's lowercase serialization, so the DB kind
    /// column round-trips through serde unchanged.
    pub fn label(self) -> &'static str {
        match self {
            ClipKind::Text => "text",
            ClipKind::Image => "image",
            ClipKind::Files => "files",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "text" => Some(ClipKind::Text),
            "image" => Some(ClipKind::Image),
            "files" => Some(ClipKind::Files),
            _ => None,
        }
    }
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

/// Returns the most recent live (non-tombstoned) clips, newest first.
///
/// The history is capped at `MAX_HISTORY_ROWS`, so one page covers the whole
/// store and the frontend badge count is the real total.
#[tauri::command]
pub fn get_recent_clips(store: tauri::State<'_, std::sync::Arc<Store>>) -> Result<Vec<ClipSummary>, String> {
    store
        .recent_clips(MAX_HISTORY_ROWS)
        .map_err(|error| format!("could not load clips: {error}"))
}
