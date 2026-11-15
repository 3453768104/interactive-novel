use crate::lore;
use crate::state::AppState;
use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::params;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

/// Watch worlds/*/lore/*.md and */state.md (external editor changes) and config/* (policy files).
/// The watcher is stored in AppState so it stays alive for the app lifetime.
pub fn start_watcher(handle: AppHandle, data_dir: PathBuf) -> Result<()> {
    let worlds_dir = data_dir.join("worlds");
    let config_dir = data_dir.join("config");
    let h = handle.clone();

    let mut watcher: RecommendedWatcher = notify::recommended_watcher(
        move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                for path in event.paths {
                    on_event(&h, &path);
                }
            }
        },
    )?;

    watcher.watch(&worlds_dir, RecursiveMode::Recursive)?;
    watcher.watch(&config_dir, RecursiveMode::Recursive)?;

    let state = handle.state::<AppState>();
    *state
        .watcher
        .lock()
        .map_err(|_| anyhow::anyhow!("watcher lock poisoned"))? = Some(watcher);
    Ok(())
}

fn on_event(handle: &AppHandle, path: &Path) {
    let state = handle.state::<AppState>();

    if path.starts_with(&state.config_dir) {
        let _ = handle.emit("policy://changed", ());
        return;
    }

    if path.extension().and_then(|e| e.to_str()) != Some("md") {
        return;
    }

    // state.md is a user-editable JSON mirror -> push changes back into the DB.
    if path.file_name().and_then(|s| s.to_str()) == Some("state.md") {
        sync_state_md(handle, path);
        return;
    }

    let Some(lore_parent) = path.parent() else { return };
    if lore_parent.file_name().and_then(|s| s.to_str()) != Some("lore") {
        return;
    }
    let Some(world_dir) = lore_parent.parent() else { return };
    let Some(world_id) = world_dir.file_name().and_then(|s| s.to_str()) else { return };
    let Some(fname) = path.file_name().and_then(|s| s.to_str()) else { return };
    let rel = format!("lore/{fname}");

    match lore::sync_external(&state, world_id, &rel) {
        Ok(true) => {
            let _ = handle.emit(
                "lore://changed",
                json!({ "world_id": world_id, "rel_path": rel }),
            );
        }
        Ok(false) => {}
        Err(e) => eprintln!("[lore sync] {e}"),
    }
}

fn sync_state_md(handle: &AppHandle, path: &Path) {
    let state = handle.state::<AppState>();
    let Some(story_id) = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
    else { return };
    let Ok(text) = fs::read_to_string(path) else { return };
    let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) else { return };
    if end <= start { return; }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text[start..=end]) else { return };
    let new_json = value.to_string();

    let Ok(conn) = state.db.lock() else { return };
    let current: Option<String> = conn
        .query_row(
            "SELECT data_json FROM state_card WHERE story_id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .ok();
    // Idempotent: our own writes already match the DB, so they are a no-op.
    if current.as_deref() == Some(new_json.as_str()) {
        return;
    }
    let rev: i64 = conn
        .query_row(
            "SELECT revision FROM state_card WHERE story_id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .unwrap_or(0)
        + 1;
    let now = AppState::now();
    let _ = conn.execute(
        "UPDATE state_card SET data_json = ?1, revision = ?2, updated_at = ?3, updated_by = 'manual' WHERE story_id = ?4",
        params![new_json, rev, now, story_id],
    );
    let _ = conn.execute(
        "INSERT INTO state_card_history (story_id, revision, data_json, source, created_at) VALUES (?1,?2,?3,'manual',?4)",
        params![story_id, rev, new_json, now],
    );
    drop(conn);
    let _ = handle.emit("state://changed", json!({ "story_id": story_id }));
}
