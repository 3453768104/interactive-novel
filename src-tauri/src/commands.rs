use crate::context;
use crate::domain::*;
use crate::engine;
use crate::error::{cmd_err, CmdResult};
use crate::lore;
use crate::state::{AppState, PendingTurn};
use rusqlite::params;
use serde_json::{json, Value};
use std::fs;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

fn log_generate(app: &AppHandle, data: Value) {
    if let Some(st) = app.try_state::<AppState>() {
        crate::diagnostics::log(&st, "generate", data);
    }
}

/// Drop a turn that will never be applied (error / user cancelled).
///
/// Without this the orphan `parse_status = 'pending'` row keeps occupying a
/// turn_index, so every later turn number silently skips one.
fn discard_turn(app: &AppHandle, turn_id: &str) {
    let st = app.state::<AppState>();
    if let Ok(conn) = st.db.lock() {
        let _ = conn.execute(
            "DELETE FROM passages WHERE id = ?1 AND parse_status = 'pending'",
            params![turn_id],
        );
    }
    if let Ok(mut m) = st.pending.lock() {
        m.remove(turn_id);
    };
    if let Ok(mut m) = st.cancels.lock() {
        m.remove(turn_id);
    };
}

// ---------------------------------------------------------------------------
// App / World
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn get_app_info(state: State<'_, AppState>) -> CmdResult<AppInfo> {
    let st = &*state;
    Ok(AppInfo {
        data_dir: st.data_dir.to_string_lossy().into(),
        config_dir: st.config_dir.to_string_lossy().into(),
        worlds_dir: st.worlds_dir.to_string_lossy().into(),
        db_path: st.db_path.to_string_lossy().into(),
        portable: st.portable,
        migrated_from: st.migrated_from.clone(),
        needs_merge: st
            .needs_merge
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone(),
        defaults_version: crate::config_migrate::defaults_version(st),
        config_warnings: context::config_warnings(st),
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_worlds(state: State<'_, AppState>) -> CmdResult<Vec<World>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let mut stmt = conn
        .prepare("SELECT id, name, description, created_at, updated_at, mirror_path FROM worlds ORDER BY created_at")
        .map_err(cmd_err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(World {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
                mirror_path: r.get(5)?,
            })
        })
        .map_err(cmd_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(cmd_err)?;
    Ok(rows)
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_world(
    state: State<'_, AppState>,
    name: String,
    description: String,
) -> CmdResult<World> {
    let st = &*state;
    let id = AppState::new_id();
    let now = AppState::now();
    let (rel, sha) = lore::init_world_files(st, &id).map_err(cmd_err)?;
    // Portable builds store a RELATIVE mirror path so the whole folder can move.
    let mirror = id.clone();
    let doc_id = AppState::new_id();

    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO worlds (id, name, description, created_at, updated_at, mirror_path) VALUES (?1,?2,?3,?4,?4,?5)",
        params![id, name, description, now, mirror],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO lore_docs (id, world_id, title, rel_path, sha256, chunk_count, origin, created_at) VALUES (?1,?2,?3,?4,?5,0,'empty',?6)",
        params![doc_id, id, "设定", rel, sha, now],
    )
    .map_err(cmd_err)?;
    lore::reslice(&conn, &doc_id, crate::defaults::LORE_TEMPLATE).map_err(cmd_err)?;
    conn.execute(
        "UPDATE lore_docs SET chunk_count = (SELECT COUNT(*) FROM lore_chunks WHERE doc_id = ?1) WHERE id = ?1",
        params![doc_id],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "INSERT OR IGNORE INTO lore_cards
            (doc_id, world_id, category_id, tags, summary, char_count, updated_at)
         VALUES (?1, ?2, NULL, '[]', '', 0, ?3)",
        params![doc_id, id, now],
    )
    .map_err(cmd_err)?;
    drop(conn);

    Ok(World {
        id,
        name,
        description,
        created_at: now.clone(),
        updated_at: now,
        mirror_path: mirror,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_world(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let st = &*state;
    let dir = lore::world_dir(st, &id);
    if dir.exists() {
        let backup = st
            .backups_dir
            .join(format!("world-{}-{}", id, chrono::Local::now().format("%Y%m%d-%H%M%S")));
        let _ = fs::rename(&dir, backup);
    }
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute("DELETE FROM worlds WHERE id = ?1", params![id])
        .map_err(cmd_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

fn story_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Story> {
    Ok(Story {
        id: r.get(0)?,
        world_id: r.get(1)?,
        name: r.get(2)?,
        engine_mode: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
        mirror_path: r.get(6)?,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_stories(state: State<'_, AppState>, world_id: String) -> CmdResult<Vec<Story>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let mut stmt = conn
        .prepare("SELECT id, world_id, name, engine_mode, created_at, updated_at, mirror_path FROM stories WHERE world_id = ?1 ORDER BY created_at")
        .map_err(cmd_err)?;
    let rows = stmt
        .query_map(params![world_id], story_from_row)
        .map_err(cmd_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(cmd_err)?;
    Ok(rows)
}

pub fn empty_state_json() -> String {
    json!({
        "world": { "time": "", "place": "", "situation": "", "threads": [] },
        "player": {
            "body": "", "mind": "", "mood": "", "goal": "",
            "abilities": [], "items": [], "conditions": []
        },
        "delta_log": ""
    })
    .to_string()
}

pub fn write_state_md(st: &AppState, story_id: &str, data_json: &str) {
    let path = st
        .worlds_dir
        .join(story_dir_rel(st, story_id))
        .join("state.md");
    let _ = fs::create_dir_all(path.parent().unwrap());
    let pretty = serde_json::from_str::<Value>(data_json)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| data_json.to_string());
    let _ = fs::write(
        path,
        format!("# 状态卡（可直接编辑本 JSON，保存后应用会自动同步）\n\n{pretty}\n"),
    );
}

fn story_dir_rel(st: &AppState, story_id: &str) -> String {
    // story mirror_path is absolute; derive the relative folder from the DB instead.
    let conn = st.db.lock().ok();
    conn.and_then(|c| {
        c.query_row(
            "SELECT w.id FROM stories s JOIN worlds w ON s.world_id = w.id WHERE s.id = ?1",
            params![story_id],
            |r| r.get::<_, String>(0),
        )
        .ok()
    })
    .map(|world_id| format!("{world_id}/stories/{story_id}"))
    .unwrap_or_default()
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_story(
    state: State<'_, AppState>,
    world_id: String,
    name: String,
    protagonist_name: String,
    persona_md: String,
    opening_scene_md: String,
    engine_mode: String,
) -> CmdResult<Story> {
    let st = &*state;
    let id = AppState::new_id();
    let now = AppState::now();
    let rel = format!("{world_id}/stories/{id}");
    let mirror = st.worlds_dir.join(&rel);
    fs::create_dir_all(&mirror).map_err(cmd_err)?;
    // Store the relative form; the absolute path is only used for mkdir above.
    let mirror_str = rel.clone();
    let state_json = empty_state_json();

    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO stories (id, world_id, name, engine_mode, created_at, updated_at, mirror_path) VALUES (?1,?2,?3,?4,?5,?5,?6)",
        params![id, world_id, name, engine_mode, now, mirror_str],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO protagonists (story_id, name, persona_md, opening_scene_md) VALUES (?1,?2,?3,?4)",
        params![id, protagonist_name, persona_md, opening_scene_md],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO state_card (story_id, revision, data_json, updated_at, updated_by) VALUES (?1,1,?2,?3,'manual')",
        params![id, state_json, now],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO summary_state (story_id, content_md, covered_through_turn, updated_at) VALUES (?1,'',0,?2)",
        params![id, now],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "INSERT OR IGNORE INTO story_settings (story_id, pov, show_scene_markers, updated_at) VALUES (?1,'protagonist',1,?2)",
        params![id, now],
    )
    .map_err(cmd_err)?;
    drop(conn);

    write_state_md(st, &id, &state_json);
    let _ = fs::write(mirror.join("story.md"), "# 正文\n");
    let _ = fs::write(mirror.join("summary.md"), "# 滚动摘要\n");

    Ok(Story {
        id,
        world_id,
        name,
        engine_mode,
        created_at: now.clone(),
        updated_at: now,
        mirror_path: mirror_str,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_story(state: State<'_, AppState>, id: String) -> CmdResult<StoryDetail> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let story = conn
        .query_row(
            "SELECT id, world_id, name, engine_mode, created_at, updated_at, mirror_path FROM stories WHERE id = ?1",
            params![id],
            story_from_row,
        )
        .map_err(cmd_err)?;
    let protagonist = conn
        .query_row(
            "SELECT story_id, name, persona_md, opening_scene_md FROM protagonists WHERE story_id = ?1",
            params![id],
            |r| {
                Ok(Protagonist {
                    story_id: r.get(0)?,
                    name: r.get(1)?,
                    persona_md: r.get(2)?,
                    opening_scene_md: r.get(3)?,
                })
            },
        )
        .map_err(cmd_err)?;
    Ok(StoryDetail { story, protagonist })
}

/// Switch the engine of a story in place; history and memory are shared.
#[tauri::command(rename_all = "snake_case")]
pub fn set_story_mode(
    state: State<'_, AppState>,
    id: String,
    engine_mode: String,
) -> CmdResult<()> {
    if !["api", "local", "paste"].contains(&engine_mode.as_str()) {
        return Err(format!("未知模式: {engine_mode}"));
    }
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "UPDATE stories SET engine_mode = ?1, updated_at = ?2 WHERE id = ?3",
        params![engine_mode, AppState::now(), id],
    )
    .map_err(cmd_err)?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_story(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute("DELETE FROM stories WHERE id = ?1", params![id])
        .map_err(cmd_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Lore documents
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn list_lore_docs(state: State<'_, AppState>, world_id: String) -> CmdResult<Vec<LoreDoc>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    lore::list_docs(&conn, &world_id).map_err(cmd_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn read_lore_doc(state: State<'_, AppState>, id: String) -> CmdResult<LoreDocContent> {
    let st = &*state;
    let (doc, content, sha) = lore::read_doc(st, &id).map_err(cmd_err)?;
    Ok(LoreDocContent {
        id: doc.id,
        title: doc.title,
        rel_path: doc.rel_path,
        content,
        sha256: sha,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_lore_doc(
    state: State<'_, AppState>,
    id: String,
    content: String,
    base_sha256: String,
    force: bool,
) -> CmdResult<LoreDocContent> {
    let st = &*state;
    let sha = lore::save_doc(st, &id, &content, &base_sha256, force).map_err(cmd_err)?;
    let (doc, _, _) = lore::read_doc(st, &id).map_err(cmd_err)?;
    Ok(LoreDocContent {
        id: doc.id,
        title: doc.title,
        rel_path: doc.rel_path,
        content,
        sha256: sha,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_lore_doc(
    state: State<'_, AppState>,
    world_id: String,
    title: String,
) -> CmdResult<LoreDoc> {
    let st = &*state;
    lore::create_doc(st, &world_id, &title).map_err(cmd_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_lore_doc(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let st = &*state;
    lore::delete_doc(st, &id).map_err(cmd_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn open_lore_external(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let st = &*state;
    let (doc, _, _) = lore::read_doc(st, &id).map_err(cmd_err)?;
    let path = lore::doc_abs_path(st, &doc);
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy().to_string()])
            .spawn()
            .map_err(cmd_err)?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(cmd_err)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Passages / memory
// ---------------------------------------------------------------------------

fn choices_for(conn: &rusqlite::Connection, passage_id: &str) -> Vec<Choice> {
    let mut stmt = match conn.prepare(
        "SELECT id, passage_id, ordinal, label, action_md, is_free_input, selected_at FROM choices WHERE passage_id = ?1 ORDER BY ordinal",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stmt.query_map(params![passage_id], |r| {
        Ok(Choice {
            id: r.get(0)?,
            passage_id: r.get(1)?,
            ordinal: r.get(2)?,
            label: r.get(3)?,
            action_md: r.get(4)?,
            is_free_input: r.get::<_, i64>(5)? != 0,
            selected_at: r.get(6)?,
        })
    })
    .ok()
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_passages(state: State<'_, AppState>, story_id: String) -> CmdResult<Vec<Passage>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.story_id, p.turn_index, p.trigger_action_md, p.content_md,
                    p.raw_output, p.parse_status, p.engine_mode, p.bundle_snapshot, p.created_at,
                    COALESCE(m.stop_decision, 1), COALESCE(m.stop_reason, ''),
                    COALESCE(m.stop_trigger, ''), COALESCE(m.pov, 'protagonist'),
                    COALESCE(m.analysis_json, '')
             FROM passages p LEFT JOIN passage_meta m ON m.passage_id = p.id
             WHERE p.story_id = ?1 ORDER BY p.turn_index",
        )
        .map_err(cmd_err)?;
    let mut out = Vec::new();
    let rows = stmt
        .query_map(params![story_id], |r| {
            Ok(Passage {
                id: r.get(0)?,
                story_id: r.get(1)?,
                turn_index: r.get(2)?,
                trigger_action_md: r.get(3)?,
                content_md: r.get(4)?,
                raw_output: r.get(5)?,
                parse_status: r.get(6)?,
                engine_mode: r.get(7)?,
                bundle_snapshot: r.get(8)?,
                created_at: r.get(9)?,
                choices: Vec::new(),
                stop_decision: r.get::<_, i64>(10)? != 0,
                stop_reason: r.get(11)?,
                stop_trigger: r.get(12)?,
                pov: r.get(13)?,
                analysis_json: r.get(14)?,
            })
        })
        .map_err(cmd_err)?;
    for row in rows {
        let mut p = row.map_err(cmd_err)?;
        p.choices = choices_for(&conn, &p.id);
        out.push(p);
    }
    Ok(out)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_state_card(state: State<'_, AppState>, story_id: String) -> CmdResult<StateCard> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.query_row(
        "SELECT story_id, revision, data_json, updated_at, updated_by FROM state_card WHERE story_id = ?1",
        params![story_id],
        |r| {
            Ok(StateCard {
                story_id: r.get(0)?,
                revision: r.get(1)?,
                data_json: r.get(2)?,
                updated_at: r.get(3)?,
                updated_by: r.get(4)?,
            })
        },
    )
    .map_err(cmd_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_state_card(
    state: State<'_, AppState>,
    story_id: String,
    data_json: String,
) -> CmdResult<StateCard> {
    let st = &*state;
    // Validate JSON before writing.
    serde_json::from_str::<Value>(&data_json).map_err(|e| format!("状态卡不是合法 JSON: {e}"))?;
    let now = AppState::now();
    let conn = st.db.lock().map_err(cmd_err)?;
    let rev: i64 = conn
        .query_row(
            "SELECT revision FROM state_card WHERE story_id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .unwrap_or(0)
        + 1;
    conn.execute(
        "UPDATE state_card SET data_json = ?1, revision = ?2, updated_at = ?3, updated_by = 'manual' WHERE story_id = ?4",
        params![data_json, rev, now, story_id],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO state_card_history (story_id, revision, data_json, source, created_at) VALUES (?1,?2,?3,'manual',?4)",
        params![story_id, rev, data_json, now],
    )
    .map_err(cmd_err)?;
    drop(conn);
    write_state_md(st, &story_id, &data_json);
    Ok(StateCard {
        story_id,
        revision: rev,
        data_json,
        updated_at: now,
        updated_by: "manual".into(),
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_summary(state: State<'_, AppState>, story_id: String) -> CmdResult<SummaryState> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.query_row(
        "SELECT story_id, content_md, covered_through_turn, updated_at FROM summary_state WHERE story_id = ?1",
        params![story_id],
        |r| {
            Ok(SummaryState {
                story_id: r.get(0)?,
                content_md: r.get(1)?,
                covered_through_turn: r.get(2)?,
                updated_at: r.get(3)?,
            })
        },
    )
    .map_err(cmd_err)
}

/// Reopen from turn T: drop passages at/after T and restore the memory layers
/// as they were right before T. Lore documents are never touched.
///
/// Prefers the autosave written at the end of turn T-1 (it holds characters
/// too); falls back to the state-card history for stories written before v3.
#[tauri::command(rename_all = "snake_case")]
pub fn reopen_turn(
    state: State<'_, AppState>,
    story_id: String,
    turn_index: i64,
) -> CmdResult<()> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;

    let snapshot = crate::commands_v3::latest_snapshot_before(&conn, &story_id, turn_index - 1);
    let mut restored: Option<String> = None;
    if let Some((snap_turn, state_json, summary_md, chars_json)) = snapshot {
        crate::commands_v3::apply_snapshot(
            &conn,
            &story_id,
            snap_turn,
            &state_json,
            &summary_md,
            &chars_json,
        )
        .map_err(cmd_err)?;
        restored = Some(state_json);
    }
    // Always drop the turns at/after T (apply_snapshot only drops the ones AFTER
    // the snapshot, and the snapshot may be older than T-1).
    conn.execute(
        "DELETE FROM passages WHERE story_id = ?1 AND turn_index >= ?2",
        params![story_id, turn_index],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "DELETE FROM story_saves WHERE story_id = ?1 AND turn_index >= ?2",
        params![story_id, turn_index],
    )
    .map_err(cmd_err)?;

    if restored.is_none() {
        let prev_state: Option<String> = conn
            .query_row(
                "SELECT data_json FROM state_card_history WHERE story_id = ?1 AND CAST(source AS INTEGER) < ?2 ORDER BY revision DESC LIMIT 1",
                params![story_id, turn_index],
                |r| r.get(0),
            )
            .ok();
        if let Some(data) = prev_state {
            conn.execute(
                "UPDATE state_card SET data_json = ?1, revision = revision + 1, updated_by = 'reopen' WHERE story_id = ?2",
                params![data, story_id],
            )
            .map_err(cmd_err)?;
            restored = Some(data);
        }
    }
    drop(conn);
    if let Some(data) = restored {
        write_state_md(st, &story_id, &data);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Context bundle / generation
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn build_bundle(
    state: State<'_, AppState>,
    story_id: String,
    action_md: String,
) -> CmdResult<ContextBundle> {
    let st = &*state;
    context::build_bundle(st, &story_id, &action_md, false).map_err(cmd_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn generate_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    story_id: String,
    action_md: String,
    auto_continue: Option<bool>,
) -> CmdResult<Value> {
    let st = &*state;
    // 「看小说」模式：本回合强制不停、不给选项，让剧情自己往下走。
    let auto = auto_continue.unwrap_or(false);
    let bundle = context::build_bundle(st, &story_id, &action_md, auto).map_err(cmd_err)?;
    let conn = st.db.lock().map_err(cmd_err)?;
    let turn_index: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(turn_index), -1) + 1 FROM passages WHERE story_id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .map_err(cmd_err)?;
    let passage_id = AppState::new_id();
    let now = AppState::now();
    conn.execute(
        "INSERT INTO passages (id, story_id, turn_index, trigger_action_md, parse_status, engine_mode, bundle_snapshot, prompt_token_est, created_at)
         VALUES (?1,?2,?3,?4,'pending',?5,?6,?7,?8)",
        params![passage_id, story_id, turn_index, action_md, bundle.mode, bundle.text, bundle.report.total_est as i64, now],
    )
    .map_err(cmd_err)?;
    // Audit trail: the exact prompt, so the reader can open "这回合注入了什么".
    let meta = json!({
        "mode": bundle.mode,
        "pov": bundle.pov,
        "template": bundle.template,
        "report": bundle.report,
    });
    conn.execute(
        "INSERT OR REPLACE INTO turn_prompts (turn_id, kind, text, est, meta_json, created_at)
         VALUES (?1,'narration',?2,?3,?4,?5)",
        params![
            passage_id,
            bundle.text,
            bundle.report.total_est as i64,
            meta.to_string(),
            now
        ],
    )
    .map_err(cmd_err)?;
    drop(conn);

    let pending = PendingTurn {
        story_id: story_id.clone(),
        turn_index,
        mode: bundle.mode.clone(),
    };
    st.pending
        .lock()
        .map_err(cmd_err)?
        .insert(passage_id.clone(), pending);

    if bundle.mode != "paste" {
        let profile = engine::profile_for(st, &bundle.mode)
            .map_err(cmd_err)?
            .ok_or_else(|| format!("engines.yaml 中缺少 {} 模式配置", bundle.mode))?;
        let cancel = CancellationToken::new();
        st.cancels
            .lock()
            .map_err(cmd_err)?
            .insert(passage_id.clone(), cancel.clone());
        let turn_id = passage_id.clone();
        let prompt = bundle.text.clone();
        let app_for_delta = app.clone();
        let app_for_done = app.clone();
        let app_for_cleanup = app.clone();
        let mode = bundle.mode.clone();
        let second_pass = context::app_settings(st).analysis_second_pass;
        let action_for_analysis = action_md.clone();
        // Built before the task starts: the analysis pass needs the same
        // baseline (state card + injected characters) the reader sees.
        let analysis_memory = if second_pass {
            context::analysis_context(st, &story_id)
        } else {
            String::new()
        };

        let started = std::time::Instant::now();
        let prompt_est = bundle.report.total_est;
        tauri::async_runtime::spawn(async move {
            let on_delta = {
                let app = app_for_delta.clone();
                let tid = turn_id.clone();
                move |delta: String| {
                    let _ = app.emit("gen://delta", json!({ "turn_id": tid, "delta": delta }));
                }
            };
            match engine::stream_chat(&profile, &prompt, cancel.clone(), on_delta).await {
                Ok(raw) => {
                    // A stopped turn is thrown away instead of being written as
                    // a truncated passage — half a paragraph is worse than none.
                    if cancel.is_cancelled() {
                        discard_turn(&app_for_cleanup, &turn_id);
                        let _ = app_for_done.emit(
                            "gen://done",
                            json!({
                                "turn_id": turn_id,
                                "raw_output": raw,
                                "parsed": engine::parse_output(&raw, &mode),
                                "cancelled": true
                            }),
                        );
                        return;
                    }
                    let mut parsed = engine::parse_output(&raw, &mode);
                    // Optional independent analysis stage (default off). Its
                    // prompt is stored too, so the inspector can show both.
                    if second_pass && parsed.analysis_json.trim().is_empty() && !cancel.is_cancelled() {
                        let analysis_prompt = engine::analysis_prompt(
                            &parsed.narration,
                            &action_for_analysis,
                            &analysis_memory,
                        );
                        if let Some(st) = app_for_cleanup.try_state::<AppState>() {
                            if let Ok(conn) = st.db.lock() {
                                let _ = conn.execute(
                                    "INSERT OR REPLACE INTO turn_prompts
                                        (turn_id, kind, text, est, meta_json, created_at)
                                     VALUES (?1,'analysis',?2,?3,?4,?5)",
                                    params![
                                        turn_id,
                                        analysis_prompt,
                                        crate::context::estimate(
                                            &analysis_prompt,
                                            &crate::domain::EstimatorCfg::default()
                                        ) as i64,
                                        json!({"mode": mode, "second_pass": true}).to_string(),
                                        AppState::now()
                                    ],
                                );
                            }
                        }
                        if let Ok(extra) = engine::analyze_with_prompt(&profile, &analysis_prompt).await
                        {
                            if !extra.trim().is_empty() {
                                parsed.analysis_json = extra;
                            }
                        }
                    }
                    log_generate(
                        &app_for_done,
                        json!({
                            "mode": mode,
                            "turn_id": turn_id,
                            "prompt_est": prompt_est,
                            "ms": started.elapsed().as_millis() as u64,
                            "warnings": parsed.warnings,
                            "cancelled": cancel.is_cancelled(),
                        }),
                    );
                    let _ = app_for_done.emit(
                        "gen://done",
                        json!({ "turn_id": turn_id, "raw_output": raw, "parsed": parsed, "cancelled": cancel.is_cancelled() }),
                    );
                }
                Err(e) => {
                    log_generate(
                        &app_for_cleanup,
                        json!({
                            "mode": mode,
                            "turn_id": turn_id,
                            "prompt_est": prompt_est,
                            "ms": started.elapsed().as_millis() as u64,
                            "error": e.to_string(),
                        }),
                    );
                    discard_turn(&app_for_cleanup, &turn_id);
                    let _ = app_for_done
                        .emit("gen://error", json!({ "turn_id": turn_id, "message": e.to_string() }));
                }
            }
        });
    }

    Ok(json!({
        "turn_id": passage_id,
        "turn_index": turn_index,
        "bundle": bundle,
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub fn cancel_generate(state: State<'_, AppState>, turn_id: String) -> CmdResult<()> {
    let st = &*state;
    if let Some(token) = st.cancels.lock().map_err(cmd_err)?.get(&turn_id) {
        token.cancel();
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn submit_pasted_output(
    state: State<'_, AppState>,
    turn_id: String,
    raw_output: String,
) -> CmdResult<ParsedOutput> {
    let st = &*state;
    let mode = st
        .pending
        .lock()
        .map_err(cmd_err)?
        .get(&turn_id)
        .map(|p| p.mode.clone())
        .unwrap_or_else(|| "paste".into());
    Ok(engine::parse_output(&raw_output, &mode))
}

#[tauri::command(rename_all = "snake_case")]
pub fn apply_output(
    state: State<'_, AppState>,
    turn_id: String,
    parsed: ParsedOutput,
) -> CmdResult<Passage> {
    let st = &*state;
    let pending = st.pending.lock().map_err(cmd_err)?.get(&turn_id).cloned();
    let Some(pending) = pending else {
        // Idempotent: a duplicated `gen://done` (double-registered listener,
        // a retried paste, a double click) must not surface as an error — the
        // turn is already in the story, so just hand it back.
        let conn = st.db.lock().map_err(cmd_err)?;
        let existing = load_passage(&conn, &turn_id);
        let status: Option<String> = conn
            .query_row(
                "SELECT parse_status FROM passages WHERE id = ?1",
                params![turn_id],
                |r| r.get(0),
            )
            .ok();
        drop(conn);
        return match (existing, status.as_deref()) {
            (Ok(p), Some(s)) if s != "pending" => Ok(p),
            _ => Err("找不到待处理的回合（可能已应用过）".to_string()),
        };
    };
    let settings = context::app_settings(st);
    let policy = context::load_policy(st, &pending.mode).unwrap_or_default();
    finish_apply(st, turn_id, pending, parsed, settings, policy)
}

fn load_passage(conn: &rusqlite::Connection, turn_id: &str) -> CmdResult<Passage> {
    let mut passage = conn
        .query_row(
            "SELECT p.id, p.story_id, p.turn_index, p.trigger_action_md, p.content_md,
                    p.raw_output, p.parse_status, p.engine_mode, p.bundle_snapshot, p.created_at,
                    COALESCE(m.stop_decision, 1), COALESCE(m.stop_reason, ''),
                    COALESCE(m.stop_trigger, ''), COALESCE(m.pov, 'protagonist'),
                    COALESCE(m.analysis_json, '')
             FROM passages p LEFT JOIN passage_meta m ON m.passage_id = p.id
             WHERE p.id = ?1",
            params![turn_id],
            |r| {
                Ok(Passage {
                    id: r.get(0)?,
                    story_id: r.get(1)?,
                    turn_index: r.get(2)?,
                    trigger_action_md: r.get(3)?,
                    content_md: r.get(4)?,
                    raw_output: r.get(5)?,
                    parse_status: r.get(6)?,
                    engine_mode: r.get(7)?,
                    bundle_snapshot: r.get(8)?,
                    created_at: r.get(9)?,
                    choices: Vec::new(),
                    stop_decision: r.get::<_, i64>(10)? != 0,
                    stop_reason: r.get(11)?,
                    stop_trigger: r.get(12)?,
                    pov: r.get(13)?,
                    analysis_json: r.get(14)?,
                })
            },
        )
        .map_err(cmd_err)?;
    passage.choices = choices_for(conn, turn_id);
    Ok(passage)
}

/// How many consecutive "keep writing" turns came immediately before this one.
fn trailing_continue_turns(conn: &rusqlite::Connection, story_id: &str) -> usize {
    let mut stmt = match conn.prepare(
        "SELECT m.stop_decision FROM passage_meta m
         JOIN passages p ON p.id = m.passage_id
         WHERE p.story_id = ?1 ORDER BY p.turn_index DESC LIMIT 30",
    ) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    stmt.query_map(params![story_id], |r| r.get::<_, i64>(0))
        .ok()
        .map(|rows| {
            rows.filter_map(|r| r.ok())
                .take_while(|n| *n == 0)
                .count()
        })
        .unwrap_or(0)
}

/// Nominate a character seen in play. If a preset (or already known) character
/// has the same name, its row is upgraded instead of duplicated — otherwise the
/// 角色 card "唐三" and the emergent "唐三" would both show up in the archive.
fn upsert_emergent_character(
    conn: &rusqlite::Connection,
    world_id: &str,
    story_id: &str,
    name: &str,
    importance: &str,
    state: &str,
    turn: i64,
    now: &str,
) -> rusqlite::Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(());
    }
    // "favorite" is a legacy tier: everything elevated is simply notable now.
    let elevated = importance == "notable";
    // Prefer the story's own row, then a world-level preset with the same name.
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT id, kind FROM characters WHERE world_id = ?1 AND name = ?2
               AND (story_id = ?3 OR story_id IS NULL)
             ORDER BY CASE WHEN story_id IS NULL THEN 1 ELSE 0 END LIMIT 1",
            params![world_id, name, story_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    if let Some((id, kind)) = existing {
        conn.execute(
            "UPDATE characters SET current_state = ?1, updated_at = ?2,
                first_turn = COALESCE(first_turn, ?3),
                importance = CASE WHEN ?4 THEN 'notable' ELSE importance END,
                inject = CASE WHEN ?4 THEN 1 ELSE inject END
             WHERE id = ?5",
            params![state, now, turn, elevated, id],
        )?;
        // A promoted preset now behaves like a story character: it keeps
        // getting状态 updates, so give it a story scope on first promotion.
        if kind == "preset" && elevated {
            let _ = conn.execute(
                "UPDATE characters SET story_id = ?1 WHERE id = ?2 AND story_id IS NULL",
                params![story_id, id],
            );
        }
    } else {
        conn.execute(
            "INSERT INTO characters
                (id, world_id, story_id, name, kind, importance, current_state,
                 first_turn, inject, created_at, updated_at)
             VALUES (?1,?2,?3,?4,'emergent',?5,?6,?7,?8,?9,?9)",
            params![
                AppState::new_id(),
                world_id,
                story_id,
                name,
                if elevated { "notable" } else { "passerby" },
                state,
                turn,
                if elevated { 1 } else { 0 },
                now
            ],
        )?;
    }
    Ok(())
}

fn finish_apply(
    st: &AppState,
    turn_id: String,
    pending: PendingTurn,
    parsed: ParsedOutput,
    settings: AppSettings,
    policy: Policy,
) -> CmdResult<Passage> {
    let now = AppState::now();
    let status = if parsed.needs_fix() { "needs_fix" } else { "ok" };
    let analysis = engine::analysis_for(&parsed);
    // Resolve the mirror path BEFORE taking the db lock (story_dir_rel locks it too).
    let rel = story_dir_rel(st, &pending.story_id);
    let conn = st.db.lock().map_err(cmd_err)?;

    let consecutive = trailing_continue_turns(&conn, &pending.story_id);
    // Hard guard against a small model that never decides to stop.
    let stop = parsed.stop || (consecutive + 1) >= settings.max_consecutive_turns.max(1);
    let pov: String = conn
        .query_row(
            "SELECT pov FROM story_settings WHERE story_id = ?1",
            params![pending.story_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "protagonist".into());
    let world_id: String = conn
        .query_row(
            "SELECT world_id FROM stories WHERE id = ?1",
            params![pending.story_id],
            |r| r.get(0),
        )
        .unwrap_or_default();

    conn.execute(
        "UPDATE passages SET content_md = ?1, raw_output = ?2, parse_status = ?3,
            state_delta_md = ?4, summary_delta_md = ?5, output_token_est = ?6 WHERE id = ?7",
        params![
            parsed.narration,
            parsed.raw_output,
            status,
            parsed.state_delta,
            parsed.summary_delta,
            crate::context::estimate(&parsed.raw_output, &crate::domain::EstimatorCfg::default()) as i64,
            turn_id
        ],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "INSERT OR REPLACE INTO passage_meta
            (passage_id, stop_decision, stop_reason, stop_trigger, pov, analysis_json, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            turn_id,
            if stop { 1 } else { 0 },
            parsed.stop_reason,
            parsed.stop_trigger,
            pov,
            parsed.analysis_json,
            now
        ],
    )
    .map_err(cmd_err)?;

    // A "keep writing" turn intentionally has no options.
    if stop {
        for (idx, label) in parsed.choices.iter().enumerate() {
            conn.execute(
                "INSERT INTO choices (id, passage_id, ordinal, label, action_md, is_free_input)
                 VALUES (?1,?2,?3,?4,?4,0)",
                params![AppState::new_id(), turn_id, idx as i64, label],
            )
            .map_err(cmd_err)?;
        }
    }

    let every_n = policy.throttle.summary_every_n.max(1) as i64;
    let state_mode = policy.throttle.state_mode.as_str();
    let write_state = state_mode != "manual"
        && (state_mode != "every_n" || (pending.turn_index + 1) % every_n == 0);
    let summary_delta = analysis
        .as_ref()
        .map(|a| a.summary.clone())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| parsed.summary_delta.clone());
    let write_summary =
        !summary_delta.trim().is_empty() && (pending.turn_index + 1) % every_n == 0;

    let mut state_to_write: Option<String> = None;
    if write_state {
        let current: String = conn
            .query_row(
                "SELECT data_json FROM state_card WHERE story_id = ?1",
                params![pending.story_id],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| empty_state_json());
        let mut v: Value = serde_json::from_str(&current).unwrap_or_else(|_| json!({}));
        if !v.is_object() {
            v = json!({});
        }
        let mut changed = false;
        if let Some(a) = analysis.as_ref() {
            for (layer, src) in [("world", &a.world), ("player", &a.player)] {
                if !v[layer].is_object() {
                    v[layer] = json!({});
                }
                let m = v[layer].as_object_mut().unwrap();
                for (k, val) in src.iter() {
                    let skip = val.is_null()
                        || (val.is_string()
                            && val.as_str().unwrap_or("").trim().is_empty())
                        || (val.is_array() && val.as_array().map(|x| x.is_empty()).unwrap_or(false));
                    if skip {
                        continue;
                    }
                    m.insert(k.clone(), val.clone());
                    changed = true;
                }
            }
            for c in &a.characters {
                let _ = upsert_emergent_character(
                    &conn,
                    &world_id,
                    &pending.story_id,
                    &c.name,
                    &c.importance,
                    &c.current_state,
                    pending.turn_index,
                    &now,
                );
                changed = true;
            }
            for name in &a.new_character_candidates {
                let _ = upsert_emergent_character(
                    &conn,
                    &world_id,
                    &pending.story_id,
                    name,
                    "passerby",
                    "",
                    pending.turn_index,
                    &now,
                );
            }
        } else if !parsed.state_delta.trim().is_empty() {
            // Legacy STATE_DELTA contract: keep a bounded log (never overwrite edits).
            let old = v["delta_log"].as_str().unwrap_or("").to_string();
            let mut log = format!(
                "{old}\n[回合 {}] {}",
                pending.turn_index,
                parsed.state_delta.trim()
            );
            if log.chars().count() > 6000 {
                log = log.chars().rev().take(6000).collect::<String>().chars().rev().collect();
            }
            v["delta_log"] = json!(log);
            changed = true;
        }
        if changed {
            let new_json = v.to_string();
            let rev: i64 = conn
                .query_row(
                    "SELECT revision FROM state_card WHERE story_id = ?1",
                    params![pending.story_id],
                    |r| r.get(0),
                )
                .unwrap_or(0)
                + 1;
            conn.execute(
                "UPDATE state_card SET data_json = ?1, revision = ?2, updated_at = ?3, updated_by = 'ai' WHERE story_id = ?4",
                params![new_json, rev, now, pending.story_id],
            )
            .map_err(cmd_err)?;
            conn.execute(
                "INSERT INTO state_card_history (story_id, revision, data_json, source, created_at)
                 VALUES (?1,?2,?3,?4,?5)",
                params![pending.story_id, rev, new_json, pending.turn_index.to_string(), now],
            )
            .map_err(cmd_err)?;
            state_to_write = Some(new_json);
        }
    }

    let mut summary_to_write: Option<String> = None;
    if write_summary {
        let old: String = conn
            .query_row(
                "SELECT content_md FROM summary_state WHERE story_id = ?1",
                params![pending.story_id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        let merged = format!("{old}\n{}", summary_delta.trim());
        conn.execute(
            "UPDATE summary_state SET content_md = ?1, covered_through_turn = ?2, updated_at = ?3 WHERE story_id = ?4",
            params![merged, pending.turn_index, now, pending.story_id],
        )
        .map_err(cmd_err)?;
        conn.execute(
            "INSERT INTO summary_history (story_id, content_md, from_turn, to_turn, created_at)
             VALUES (?1,?2,?3,?3,?4)",
            params![pending.story_id, summary_delta, pending.turn_index, now],
        )
        .map_err(cmd_err)?;
        summary_to_write = Some(merged);
    }

    // Mirror the story body (DB is authoritative; this file is read-only display).
    let mut body = String::from("# 正文\n");
    let mut stmt = conn
        .prepare(
            "SELECT turn_index, trigger_action_md, content_md FROM passages
             WHERE story_id = ?1 AND parse_status != 'pending' ORDER BY turn_index",
        )
        .map_err(cmd_err)?;
    let rows = stmt
        .query_map(params![pending.story_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(cmd_err)?;
    for row in rows {
        let (idx, action, content) = row.map_err(cmd_err)?;
        body.push_str(&format!("\n\n## 回合 {idx}\n\n> 行动：{action}\n\n{content}\n"));
    }
    drop(stmt);

    // Autosave point: what the memory layers look like right now. Restoring it
    // later only has to drop the turns after this one.
    let _ = crate::commands_v3::write_snapshot(
        &conn,
        &pending.story_id,
        pending.turn_index,
        "auto",
        "",
    );
    // A director note is spent once it has been obeyed.
    crate::commands_v3::clear_director_note(&conn, &pending.story_id);
    crate::search::reindex_passage(&conn, &turn_id);

    let passage = load_passage(&conn, &turn_id)?;
    drop(conn);

    // Filesystem writes happen after the DB lock is released.
    if let Some(s) = state_to_write {
        write_state_md(st, &pending.story_id, &s);
    }
    if let Some(s) = summary_to_write {
        let _ = fs::write(
            st.worlds_dir.join(&rel).join("summary.md"),
            format!("# 滚动摘要\n\n{s}\n"),
        );
    }
    let _ = fs::write(st.worlds_dir.join(&rel).join("story.md"), body);

    st.pending.lock().map_err(cmd_err)?.remove(&turn_id);
    st.cancels.lock().map_err(cmd_err)?.remove(&turn_id);
    let _ = status;
    Ok(passage)
}
