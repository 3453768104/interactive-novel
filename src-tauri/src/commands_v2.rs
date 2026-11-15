//! v2 commands: POV, structured policy read model, characters, app settings.
//! Kept in a separate module so commands.rs (the turn pipeline) stays focused.

use crate::context;
use crate::domain::*;
use crate::error::{cmd_err, CmdResult};
use crate::state::AppState;
use rusqlite::params;
use tauri::State;

// ---------------------------------------------------------------------------
// Point of view / reading behaviour
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn get_story_settings(state: State<'_, AppState>, story_id: String) -> CmdResult<StorySettings> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let row = conn
        .query_row(
            "SELECT pov, show_scene_markers FROM story_settings WHERE story_id = ?1",
            params![story_id],
            |r| {
                Ok(StorySettings {
                    pov: r.get::<_, String>(0)?,
                    show_scene_markers: r.get::<_, i64>(1)? != 0,
                })
            },
        )
        .unwrap_or_default();
    Ok(row)
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_story_settings(
    state: State<'_, AppState>,
    story_id: String,
    pov: String,
    show_scene_markers: bool,
) -> CmdResult<StorySettings> {
    if !["protagonist", "omniscient"].contains(&pov.as_str()) {
        return Err(format!("未知视角: {pov}"));
    }
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO story_settings (story_id, pov, show_scene_markers, updated_at)
         VALUES (?1,?2,?3,?4)
         ON CONFLICT(story_id) DO UPDATE SET pov = excluded.pov,
             show_scene_markers = excluded.show_scene_markers,
             updated_at = excluded.updated_at",
        params![story_id, pov, if show_scene_markers { 1 } else { 0 }, AppState::now()],
    )
    .map_err(cmd_err)?;
    Ok(StorySettings { pov, show_scene_markers })
}

// ---------------------------------------------------------------------------
// Structured config read models (the YAML files remain the authority)
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn get_app_settings(state: State<'_, AppState>) -> CmdResult<AppSettings> {
    Ok(context::app_settings(&*state))
}

/// Typed read model of engines.yaml for the GUI form. The YAML file stays the
/// single source of truth; the form patches that text line by line.
#[tauri::command(rename_all = "snake_case")]
pub fn get_engines(state: State<'_, AppState>) -> CmdResult<EnginesConfig> {
    let text = context::read_config_text(&*state, "engines.yaml").map_err(cmd_err)?;
    serde_yaml::from_str(&text).map_err(|e| format!("engines.yaml 解析失败: {e}"))
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_policy(state: State<'_, AppState>, mode: String) -> CmdResult<Policy> {
    // Surface a broken file instead of quietly handing back the fallback.
    if let Some(err) = context::policy_parse_error(&*state, &mode) {
        return Err(err);
    }
    context::load_policy(&*state, &mode).map_err(cmd_err)
}

// ---------------------------------------------------------------------------
// Characters
// ---------------------------------------------------------------------------

fn character_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Character> {
    Ok(Character {
        id: r.get(0)?,
        world_id: r.get(1)?,
        story_id: r.get(2)?,
        name: r.get(3)?,
        kind: r.get(4)?,
        importance: r.get(5)?,
        preset_card_id: r.get(6)?,
        current_state: r.get(7)?,
        first_turn: r.get(8)?,
        inject: r.get::<_, i64>(9)? != 0,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
    })
}

const CHARACTER_COLS: &str = "id, world_id, story_id, name, kind, importance,
    preset_card_id, current_state, first_turn, inject, created_at, updated_at";

fn fetch_character(conn: &rusqlite::Connection, id: &str) -> CmdResult<Character> {
    let sql = format!("SELECT {CHARACTER_COLS} FROM characters WHERE id = ?1");
    conn.query_row(&sql, params![id], character_from_row).map_err(cmd_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_characters(
    state: State<'_, AppState>,
    world_id: String,
    story_id: String,
) -> CmdResult<Vec<Character>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let sql = format!(
        "SELECT {CHARACTER_COLS} FROM characters
         WHERE world_id = ?1 AND (story_id IS NULL OR story_id = ?2)
         ORDER BY CASE importance WHEN 'favorite' THEN 0 WHEN 'notable' THEN 1 ELSE 2 END,
                  kind, name"
    );
    let mut stmt = conn.prepare(&sql).map_err(cmd_err)?;
    let rows = stmt
        .query_map(params![world_id, story_id], character_from_row)
        .map_err(cmd_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(cmd_err)?;
    Ok(rows)
}

/// Manual creation / edit from the reading UI (the "＋" corner badge).
#[tauri::command(rename_all = "snake_case")]
pub fn upsert_character(
    state: State<'_, AppState>,
    world_id: String,
    story_id: String,
    name: String,
    importance: String,
    current_state: String,
) -> CmdResult<Character> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("角色名不能为空".into());
    }
    let imp = match importance.as_str() {
        "favorite" | "notable" => importance.as_str(),
        _ => "passerby",
    };
    let st = &*state;
    let now = AppState::now();
    let conn = st.db.lock().map_err(cmd_err)?;
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM characters WHERE world_id = ?1 AND story_id = ?2
             AND name = ?3 AND kind = 'emergent'",
            params![world_id, story_id, name],
            |r| r.get(0),
        )
        .ok();
    if let Some(id) = existing {
        conn.execute(
            "UPDATE characters SET importance = ?1, inject = ?2, current_state = ?3, updated_at = ?4 WHERE id = ?5",
            params![imp, if imp == "passerby" { 0 } else { 1 }, current_state, now, id],
        )
        .map_err(cmd_err)?;
        return fetch_character(&conn, &id);
    }
    let id = AppState::new_id();
    conn.execute(
        "INSERT INTO characters
            (id, world_id, story_id, name, kind, importance, current_state,
             first_turn, inject, created_at, updated_at)
         VALUES (?1,?2,?3,?4,'emergent',?5,?6,NULL,?7,?8,?8)",
        params![
            id, world_id, story_id, name, imp, current_state,
            if imp == "passerby" { 0 } else { 1 }, now
        ],
    )
    .map_err(cmd_err)?;
    fetch_character(&conn, &id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_character_importance(
    state: State<'_, AppState>,
    id: String,
    importance: String,
) -> CmdResult<Character> {
    let imp = match importance.as_str() {
        "favorite" | "notable" | "passerby" => importance.as_str(),
        _ => return Err(format!("未知重要度: {importance}")),
    };
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "UPDATE characters SET importance = ?1, inject = ?2, updated_at = ?3 WHERE id = ?4",
        params![
            imp,
            if imp == "passerby" { 0 } else { 1 },
            AppState::now(),
            id
        ],
    )
    .map_err(cmd_err)?;
    fetch_character(&conn, &id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_character(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute("DELETE FROM characters WHERE id = ?1", params![id])
        .map_err(cmd_err)?;
    Ok(())
}
