//! SQLite history store.
//!
//! The schema is sync-ready from day one (binding project rule): UUID ids, a
//! per-device `device_id`, millisecond timestamps, and soft-delete tombstones
//! for the future Wi-Fi sync. Image payloads live on disk as content-addressed
//! blobs; SQLite holds metadata only.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::clips::{ClipKind, ClipSummary};
use crate::clipboard::CapturedClip;

/// History cap so the store's disk footprint stays bounded; oldest rows are
/// pruned beyond this.
pub const MAX_HISTORY_ROWS: i64 = 500;

/// Bump when the schema changes; future migrations read and compare this.
const SCHEMA_VERSION: &str = "1";

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Sql(rusqlite::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(error) => write!(f, "io error: {error}"),
            StoreError::Json(error) => write!(f, "json error: {error}"),
            StoreError::Sql(error) => write!(f, "sqlite error: {error}"),
        }
    }
}

impl std::error::Error for StoreError {}

pub struct Store {
    connection: Mutex<Connection>,
    device_id: String,
    blobs_dir: PathBuf,
}

impl Store {
    /// Opens (or creates) the store under `data_dir`, expecting to own it:
    /// `clippie.db` next to a `blobs/` directory for image payloads.
    pub fn open(data_dir: &Path) -> Result<Self, StoreError> {
        std::fs::create_dir_all(data_dir).map_err(StoreError::Io)?;
        let blobs_dir = data_dir.join("blobs");
        std::fs::create_dir_all(&blobs_dir).map_err(StoreError::Io)?;

        let connection = Connection::open(data_dir.join("clippie.db")).map_err(StoreError::Sql)?;
        // WAL keeps concurrent reads (UI) and writes (watcher) non-blocking.
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(StoreError::Sql)?;

        let mut store = Self {
            connection: Mutex::new(connection),
            device_id: String::new(),
            blobs_dir,
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<(), StoreError> {
        let connection = self.lock();
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS clips (
                    id TEXT PRIMARY KEY,
                    device_id TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    content TEXT,
                    blob_path TEXT,
                    file_paths TEXT,
                    hash TEXT NOT NULL,
                    preview TEXT,
                    created_at INTEGER NOT NULL,
                    deleted_at INTEGER
                );
                CREATE INDEX IF NOT EXISTS idx_clips_created_at ON clips(created_at);
                CREATE TABLE IF NOT EXISTS meta (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );",
            )
            .map_err(StoreError::Sql)?;

        Self::ensure_meta(&connection, "schema_version", SCHEMA_VERSION)?;

        // Stable random device id, generated on first run only.
        let stored: Option<String> = connection
            .query_row("SELECT value FROM meta WHERE key = 'device_id'", [], |row| row.get(0))
            .optional()
            .map_err(StoreError::Sql)?;
        let device_id = match stored {
            Some(id) => id,
            None => {
                let id = Uuid::new_v4().to_string();
                Self::ensure_meta(&connection, "device_id", &id)?;
                id
            }
        };
        // Release the guard (it borrows all of `self`) before the field write.
        drop(connection);
        self.device_id = device_id;
        Ok(())
    }

    fn ensure_meta(connection: &Connection, key: &str, value: &str) -> Result<(), StoreError> {
        connection
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO NOTHING",
                params![key, value],
            )
            .map_err(StoreError::Sql)?;
        Ok(())
    }

    /// Inserts a captured clip, returning its summary for the UI event.
    ///
    /// Returns `Ok(None)` when the capture repeats the newest live clip:
    /// consecutive dedupe by content hash, so re-copying never duplicates a
    /// row. Checking the newest row (rather than an in-memory last hash) makes
    /// the dedupe survive daemon restarts.
    pub fn insert_capture(&self, clip: &CapturedClip) -> Result<Option<ClipSummary>, StoreError> {
        let connection = self.lock();

        let newest_hash: Option<String> = connection
            .query_row(
                "SELECT hash FROM clips WHERE deleted_at IS NULL ORDER BY created_at DESC, id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::Sql)?;
        if newest_hash.as_deref() == Some(clip.hash.as_str()) {
            return Ok(None);
        }

        let (content, blob_path, file_paths) = match &clip.payload {
            crate::clipboard::ClipPayload::Text { content } => (Some(content.clone()), None, None),
            crate::clipboard::ClipPayload::Image { png } => (None, Some(self.store_blob(&clip.hash, png)?), None),
            crate::clipboard::ClipPayload::Files { paths } => {
                // Paths are referenced, never copied into the store (binding rule).
                (None, None, Some(serde_json::to_string(paths).map_err(StoreError::Json)?))
            }
        };

        // UUIDv7 ids sort by creation time, so (created_at, id) orders rows
        // deterministically even for copies in the same millisecond.
        let id = Uuid::now_v7().to_string();
        let created_at = unix_millis();
        connection
            .execute(
                "INSERT INTO clips (id, device_id, kind, content, blob_path, file_paths, hash, preview, created_at, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
                params![
                    id,
                    self.device_id,
                    clip.kind.label(),
                    content,
                    blob_path,
                    file_paths,
                    clip.hash,
                    clip.preview,
                    created_at
                ],
            )
            .map_err(StoreError::Sql)?;

        Self::prune_history(&connection)?;
        Ok(Some(ClipSummary {
            id,
            kind: clip.kind,
            preview: clip.preview.clone(),
            // Timestamps come from `unix_millis` and are never negative.
            created_at: created_at as u64,
        }))
    }

    /// The most recent live (non-tombstoned) clips, newest first.
    pub fn recent_clips(&self, limit: i64) -> Result<Vec<ClipSummary>, StoreError> {
        let connection = self.lock();
        let mut statement = connection
            .prepare(
                "SELECT id, kind, preview, created_at
                 FROM clips
                 WHERE deleted_at IS NULL
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?1",
            )
            .map_err(StoreError::Sql)?;
        let rows = statement
            .query_map(params![limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(StoreError::Sql)?;

        let mut clips = Vec::new();
        for row in rows {
            let (id, kind, preview, created_at) = row.map_err(StoreError::Sql)?;
            match ClipKind::from_label(&kind) {
                Some(parsed_kind) => clips.push(ClipSummary {
                    id,
                    kind: parsed_kind,
                    preview,
                    created_at: created_at as u64,
                }),
                // A corrupt row must not fail the whole history; drop it loudly.
                None => eprintln!("skipping clip '{id}' with unknown kind '{kind}'"),
            }
        }
        Ok(clips)
    }

    /// Keeps the newest `MAX_HISTORY_ROWS` rows, deleting the rest. Oldest
    /// rows are hard-deleted (not tombstoned): the cap is about local growth,
    /// and pruned rows that never synced have no peer copies. The sync design
    /// will revisit deletion semantics.
    fn prune_history(connection: &Connection) -> Result<(), StoreError> {
        connection
            .execute(
                "DELETE FROM clips WHERE id IN (
                    SELECT id FROM clips ORDER BY created_at DESC, id DESC LIMIT -1 OFFSET ?1
                )",
                params![MAX_HISTORY_ROWS],
            )
            .map_err(StoreError::Sql)?;
        Ok(())
    }

    /// Persists image bytes as a content-addressed `<hash>.png`; an existing
    /// blob is reused, so repeated copies of the same image share one file.
    fn store_blob(&self, hash: &str, png: &[u8]) -> Result<String, StoreError> {
        let path = self.blobs_dir.join(format!("{hash}.png"));
        if !path.exists() {
            std::fs::write(&path, png).map_err(StoreError::Io)?;
        }
        Ok(path.to_string_lossy().into_owned())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A poisoned mutex means a panic while holding the connection; the
        // store cannot recover from that, but failing loudly beats hanging.
        self.connection.lock().expect("clip store mutex poisoned")
    }
}

fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since_epoch| since_epoch.as_millis() as i64)
        // Clocks before 1970 do not happen in practice; clamp instead of failing.
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::ClipPayload;

    fn temp_store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = Store::open(dir.path()).expect("store");
        (store, dir)
    }

    fn text_clip(content: &str) -> CapturedClip {
        CapturedClip::new(ClipPayload::Text { content: content.into() })
    }

    #[test]
    fn generates_stable_device_id() {
        let (store, _dir) = temp_store();
        let first = store.device_id.clone();
        assert!(!first.is_empty());

        // Reopening the same data dir keeps the id stable.
        let reopened = Store::open(_dir.path()).expect("reopen");
        assert_eq!(reopened.device_id, first);
    }

    #[test]
    fn stores_and_lists_newest_first() {
        let (store, _dir) = temp_store();
        store.insert_capture(&text_clip("first")).expect("insert").expect("stored");
        store.insert_capture(&text_clip("second")).expect("insert").expect("stored");

        let clips = store.recent_clips(MAX_HISTORY_ROWS).expect("recent");
        assert_eq!(clips.len(), 2);
        assert_eq!(clips[0].preview, "second");
        assert_eq!(clips[1].preview, "first");
        assert!(clips[0].created_at >= clips[1].created_at);
    }

    #[test]
    fn consecutive_duplicate_is_not_stored() {
        let (store, _dir) = temp_store();
        let clip = text_clip("repeat me");
        store.insert_capture(&clip).expect("insert 1").expect("stored");
        let deduped = store.insert_capture(&clip).expect("insert 2");
        assert!(deduped.is_none(), "consecutive repeat must not be stored");

        let clips = store.recent_clips(MAX_HISTORY_ROWS).expect("recent");
        assert_eq!(clips.len(), 1);
    }

    #[test]
    fn alternating_copies_are_both_stored() {
        // Dedupe is consecutive-only: a, b, a must keep all three.
        let (store, _dir) = temp_store();
        let a = text_clip("a");
        let b = text_clip("b");
        store.insert_capture(&a).expect("a1").expect("stored");
        store.insert_capture(&b).expect("b").expect("stored");
        store.insert_capture(&a).expect("a2").expect("stored");
        assert_eq!(store.recent_clips(MAX_HISTORY_ROWS).expect("recent").len(), 3);
    }
}
