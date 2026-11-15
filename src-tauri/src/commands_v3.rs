//! v3 commands: what-was-sent inspector, director mode, save points, pinned lore.
//!
//! Kept apart from commands.rs so the turn pipeline stays readable. Every
//! function here is a side-car concern: none of them mutates a passage.

use crate::context;
use crate::domain::*;
use crate::error::{cmd_err, CmdResult};
use crate::state::AppState;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tauri::State;

// ---------------------------------------------------------------------------
// Context inspector — the exact prompt each turn was built from
// ---------------------------------------------------------------------------

fn prompt_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TurnPrompt> {
    Ok(TurnPrompt {
        turn_id: r.get(0)?,
        kind: r.get(1)?,
        text: r.get(2)?,
        est: r.get(3)?,
        meta_json: r.get(4)?,
        created_at: r.get(5)?,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_turn_prompts(
    state: State<'_, AppState>,
    turn_id: String,
) -> CmdResult<Vec<TurnPrompt>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let mut stmt = conn
        .prepare(
            "SELECT turn_id, kind, text, est, meta_json, created_at FROM turn_prompts
             WHERE turn_id = ?1 ORDER BY CASE kind WHEN 'narration' THEN 0 ELSE 1 END",
        )
        .map_err(cmd_err)?;
    let rows = stmt
        .query_map(params![turn_id], prompt_from_row)
        .map_err(cmd_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(cmd_err)?;
    Ok(rows)
}

/// Build the prompt for an action WITHOUT calling the model — the reader can
/// see exactly what the next turn would be given before committing.
#[tauri::command(rename_all = "snake_case")]
pub fn preview_bundle(
    state: State<'_, AppState>,
    story_id: String,
    action_md: String,
) -> CmdResult<Value> {
    let st = &*state;
    let bundle = context::build_bundle(st, &story_id, &action_md, false).map_err(cmd_err)?;
    Ok(json!({
        "text": bundle.text,
        "report": bundle.report,
        "mode": bundle.mode,
        "template": bundle.template,
        "pov": bundle.pov,
    }))
}

// ---------------------------------------------------------------------------
// Director mode
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn get_director(state: State<'_, AppState>, story_id: String) -> CmdResult<StoryDirector> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    Ok(conn
        .query_row(
            "SELECT enabled, outline_md, next_note, updated_at FROM story_director WHERE story_id = ?1",
            params![story_id],
            |r| {
                Ok(StoryDirector {
                    story_id: String::new(),
                    enabled: r.get::<_, i64>(0)? != 0,
                    outline_md: r.get(1)?,
                    next_note: r.get(2)?,
                    updated_at: r.get(3)?,
                })
            },
        )
        .map(|mut d| {
            d.story_id = story_id.clone();
            d
        })
        .unwrap_or_else(|_| StoryDirector {
            story_id: story_id.clone(),
            ..Default::default()
        }))
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_director(
    state: State<'_, AppState>,
    story_id: String,
    enabled: bool,
    outline_md: String,
    next_note: String,
) -> CmdResult<StoryDirector> {
    let st = &*state;
    let now = AppState::now();
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO story_director (story_id, enabled, outline_md, next_note, updated_at)
         VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(story_id) DO UPDATE SET enabled = excluded.enabled,
             outline_md = excluded.outline_md, next_note = excluded.next_note,
             updated_at = excluded.updated_at",
        params![story_id, if enabled { 1 } else { 0 }, outline_md, next_note, now],
    )
    .map_err(cmd_err)?;
    Ok(StoryDirector {
        story_id,
        enabled,
        outline_md,
        next_note,
        updated_at: now,
    })
}

/// Drop the one-shot instruction once it has been spent on a turn.
pub(crate) fn clear_director_note(conn: &Connection, story_id: &str) {
    let _ = conn.execute(
        "UPDATE story_director SET next_note = '' WHERE story_id = ?1 AND next_note <> ''",
        params![story_id],
    );
}

// ---------------------------------------------------------------------------
// 状态卡历史 / 撤销
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct StateRevision {
    pub revision: i64,
    pub source: String,
    pub created_at: String,
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_state_history(
    state: State<'_, AppState>,
    story_id: String,
    limit: Option<usize>,
) -> CmdResult<Vec<StateRevision>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let mut stmt = conn
        .prepare(
            "SELECT revision, source, created_at FROM state_card_history
             WHERE story_id = ?1 ORDER BY revision DESC LIMIT ?2",
        )
        .map_err(cmd_err)?;
    let rows = stmt
        .query_map(params![story_id, limit.unwrap_or(30).clamp(1, 200) as i64], |r| {
            Ok(StateRevision {
                revision: r.get(0)?,
                source: r.get(1)?,
                created_at: r.get(2)?,
            })
        })
        .map_err(cmd_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(cmd_err)?;
    Ok(rows)
}

/// 回退到上一次修改前的状态卡（AI 写错、手滑都能救）。
#[tauri::command(rename_all = "snake_case")]
pub fn undo_state(state: State<'_, AppState>, story_id: String) -> CmdResult<StateCard> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let current: i64 = conn
        .query_row(
            "SELECT revision FROM state_card WHERE story_id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    // 找当前 revision 之前最近的一条快照
    let prev: Option<(i64, String)> = conn
        .query_row(
            "SELECT revision, data_json FROM state_card_history
             WHERE story_id = ?1 AND revision < ?2 ORDER BY revision DESC LIMIT 1",
            params![story_id, current],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((rev, data)) = prev else {
        return Err("没有更早的状态可回退".into());
    };
    let new_rev = current + 1;
    let now = AppState::now();
    conn.execute(
        "UPDATE state_card SET data_json = ?1, revision = ?2, updated_at = ?3, updated_by = 'undo'
         WHERE story_id = ?4",
        params![data, new_rev, now, story_id],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO state_card_history (story_id, revision, data_json, source, created_at)
         VALUES (?1,?2,?3,'undo',?4)",
        params![story_id, new_rev, data, now],
    )
    .map_err(cmd_err)?;
    drop(conn);

    crate::commands::write_state_md(st, &story_id, &data);
    let conn = st.db.lock().map_err(cmd_err)?;
    let card = conn
        .query_row(
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
        .map_err(cmd_err)?;
    let _ = rev;
    Ok(card)
}

// ---------------------------------------------------------------------------
// Save points
// ---------------------------------------------------------------------------

fn characters_snapshot(conn: &Connection, world_id: &str, story_id: &str) -> String {
    let mut stmt = match conn.prepare(
        "SELECT id, world_id, story_id, name, kind, importance, preset_card_id,
                current_state, first_turn, inject, created_at, updated_at
         FROM characters WHERE world_id = ?1 AND (story_id = ?2 OR story_id IS NULL)",
    ) {
        Ok(s) => s,
        Err(_) => return "[]".into(),
    };
    let rows: Vec<Value> = stmt
        .query_map(params![world_id, story_id], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "world_id": r.get::<_, String>(1)?,
                "story_id": r.get::<_, Option<String>>(2)?,
                "name": r.get::<_, String>(3)?,
                "kind": r.get::<_, String>(4)?,
                "importance": r.get::<_, String>(5)?,
                "preset_card_id": r.get::<_, Option<String>>(6)?,
                "current_state": r.get::<_, String>(7)?,
                "first_turn": r.get::<_, Option<i64>>(8)?,
                "inject": r.get::<_, i64>(9)? != 0,
                "created_at": r.get::<_, String>(10)?,
                "updated_at": r.get::<_, String>(11)?,
            }))
        })
        .map(|it| it.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into())
}

/// Snapshot the memory layers at the END of `turn_index`. Passages are not
/// copied: restoring only has to delete the ones after this turn.
pub(crate) fn write_snapshot(
    conn: &Connection,
    story_id: &str,
    turn_index: i64,
    kind: &str,
    label: &str,
) -> rusqlite::Result<String> {
    let save_id = AppState::new_id();
    let world_id: String = conn
        .query_row(
            "SELECT world_id FROM stories WHERE id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let state_json: String = conn
        .query_row(
            "SELECT data_json FROM state_card WHERE story_id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "{}".into());
    let summary_md: String = conn
        .query_row(
            "SELECT content_md FROM summary_state WHERE story_id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    conn.execute(
        "INSERT INTO story_saves
            (id, story_id, turn_index, kind, label, state_json, summary_md, characters_json, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            save_id,
            story_id,
            turn_index,
            kind,
            label,
            state_json,
            summary_md,
            characters_snapshot(conn, &world_id, story_id),
            AppState::now()
        ],
    )?;
    // Keep the rolling autosaves bounded so the table cannot grow forever.
    if kind == "auto" {
        conn.execute(
            "DELETE FROM story_saves WHERE story_id = ?1 AND kind = 'auto' AND id NOT IN (
                SELECT id FROM story_saves WHERE story_id = ?1 AND kind = 'auto'
                ORDER BY turn_index DESC LIMIT 60)",
            params![story_id],
        )?;
    }
    Ok(save_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_saves(state: State<'_, AppState>, story_id: String) -> CmdResult<Vec<StorySave>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, story_id, turn_index, kind, label, created_at FROM story_saves
             WHERE story_id = ?1 ORDER BY turn_index DESC, created_at DESC",
        )
        .map_err(cmd_err)?;
    let rows = stmt
        .query_map(params![story_id], |r| {
            Ok(StorySave {
                id: r.get(0)?,
                story_id: r.get(1)?,
                turn_index: r.get(2)?,
                kind: r.get(3)?,
                label: r.get(4)?,
                created_at: r.get(5)?,
            })
        })
        .map_err(cmd_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(cmd_err)?;
    Ok(rows)
}

/// Manual save = "mark this moment as worth coming back to".
#[tauri::command(rename_all = "snake_case")]
pub fn create_save(
    state: State<'_, AppState>,
    story_id: String,
    label: String,
) -> CmdResult<StorySave> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let turn_index: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(turn_index), -1) FROM passages WHERE story_id = ?1
             AND parse_status != 'pending'",
            params![story_id],
            |r| r.get(0),
        )
        .unwrap_or(-1);
    let id = write_snapshot(&conn, &story_id, turn_index, "manual", label.trim()).map_err(cmd_err)?;
    let created_at: String = conn
        .query_row(
            "SELECT created_at FROM story_saves WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    Ok(StorySave {
        id,
        story_id,
        turn_index,
        kind: "manual".into(),
        label: label.trim().to_string(),
        created_at,
    })
}

/// Restore = drop every turn after the snapshot and put the three memory
/// layers back. Lore files are never touched, so settings survive a rewind.
/// Shared by "读档" and "从这里重开": drop later turns, put memory back.
pub(crate) fn apply_snapshot(
    conn: &Connection,
    story_id: &str,
    turn_index: i64,
    state_json: &str,
    summary_md: &str,
    chars_json: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM passages WHERE story_id = ?1 AND turn_index > ?2",
        params![story_id, turn_index],
    )?;
    conn.execute(
        "DELETE FROM story_saves WHERE story_id = ?1 AND turn_index > ?2",
        params![story_id, turn_index],
    )?;
    let now = AppState::now();
    conn.execute(
        "UPDATE state_card SET data_json = ?1, revision = revision + 1, updated_at = ?2,
            updated_by = 'restore' WHERE story_id = ?3",
        params![state_json, now, story_id],
    )?;
    conn.execute(
        "UPDATE summary_state SET content_md = ?1, covered_through_turn = ?2, updated_at = ?3
         WHERE story_id = ?4",
        params![summary_md, turn_index, now, story_id],
    )?;

    // Characters: drop the ones born after this point, then put every row of
    // the snapshot back exactly as it was (ids preserved, presets included).
    conn.execute(
        "DELETE FROM characters WHERE story_id = ?1 AND kind = 'emergent'",
        params![story_id],
    )?;
    if let Ok(rows) = serde_json::from_str::<Vec<Value>>(chars_json) {
        for c in rows {
            let text = |k: &str| c.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let num = |k: &str| c.get(k).and_then(|x| x.as_i64());
            let flag = c.get("inject").and_then(|x| x.as_bool()).unwrap_or(false);
            let _ = conn.execute(
                "INSERT OR REPLACE INTO characters
                    (id, world_id, story_id, name, kind, importance, preset_card_id,
                     current_state, first_turn, inject, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![
                    text("id"),
                    text("world_id"),
                    c.get("story_id").and_then(|x| x.as_str()),
                    text("name"),
                    text("kind"),
                    text("importance"),
                    c.get("preset_card_id").and_then(|x| x.as_str()),
                    text("current_state"),
                    num("first_turn"),
                    if flag { 1 } else { 0 },
                    text("created_at"),
                    text("updated_at"),
                ],
            );
        }
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn restore_save(state: State<'_, AppState>, save_id: String) -> CmdResult<()> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let (story_id, turn_index, state_json, summary_md, chars_json): (
        String,
        i64,
        String,
        String,
        String,
    ) = conn
        .query_row(
            "SELECT story_id, turn_index, state_json, summary_md, characters_json
             FROM story_saves WHERE id = ?1",
            params![save_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|_| "存档不存在".to_string())?;

    apply_snapshot(&conn, &story_id, turn_index, &state_json, &summary_md, &chars_json)
        .map_err(cmd_err)?;
    drop(conn);

    crate::commands::write_state_md(st, &story_id, &state_json);
    Ok(())
}

/// Used by `reopen_turn`: the newest snapshot at or before `turn_index`.
pub(crate) fn latest_snapshot_before(
    conn: &Connection,
    story_id: &str,
    turn_index: i64,
) -> Option<(i64, String, String, String)> {
    conn.query_row(
        "SELECT turn_index, state_json, summary_md, characters_json FROM story_saves
         WHERE story_id = ?1 AND turn_index <= ?2 ORDER BY turn_index DESC LIMIT 1",
        params![story_id, turn_index],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .ok()
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_save(state: State<'_, AppState>, save_id: String) -> CmdResult<()> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute("DELETE FROM story_saves WHERE id = ?1", params![save_id])
        .map_err(cmd_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Pinned lore cards (always injected, bypasses retrieval)
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn list_lore_pins(state: State<'_, AppState>, story_id: String) -> CmdResult<Vec<PinnedLore>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let mut stmt = conn
        .prepare(
            "SELECT d.id, d.title, d.rel_path FROM story_lore_pins p
             JOIN lore_docs d ON d.id = p.doc_id WHERE p.story_id = ?1 ORDER BY p.created_at",
        )
        .map_err(cmd_err)?;
    let rows = stmt
        .query_map(params![story_id], |r| {
            Ok(PinnedLore {
                doc_id: r.get(0)?,
                title: r.get(1)?,
                rel_path: r.get(2)?,
            })
        })
        .map_err(cmd_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(cmd_err)?;
    Ok(rows)
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_lore_pin(
    state: State<'_, AppState>,
    story_id: String,
    doc_id: String,
    pinned: bool,
) -> CmdResult<()> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    if pinned {
        conn.execute(
            "INSERT OR IGNORE INTO story_lore_pins (story_id, doc_id, created_at) VALUES (?1,?2,?3)",
            params![story_id, doc_id, AppState::now()],
        )
        .map_err(cmd_err)?;
    } else {
        conn.execute(
            "DELETE FROM story_lore_pins WHERE story_id = ?1 AND doc_id = ?2",
            params![story_id, doc_id],
        )
        .map_err(cmd_err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("db/schema.sql")).unwrap();
        conn.execute_batch(
            "INSERT INTO worlds (id,name,description,created_at,updated_at,mirror_path)
               VALUES ('w','w','','t','t','w');
             INSERT INTO stories (id,world_id,name,engine_mode,created_at,updated_at,mirror_path)
               VALUES ('s','w','s','local','t','t','w/stories/s');
             INSERT INTO story_settings (story_id,pov,show_scene_markers,updated_at)
               VALUES ('s','protagonist',1,'t');
             INSERT INTO state_card (story_id,revision,data_json,updated_at,updated_by)
               VALUES ('s',1,'{\"player\":{\"goal\":\"开局\"}}','t','manual');
             INSERT INTO summary_state (story_id,content_md,covered_through_turn,updated_at)
               VALUES ('s','','0','t');
             INSERT INTO characters
               (id,world_id,story_id,name,kind,importance,current_state,first_turn,inject,created_at,updated_at)
               VALUES ('c1','w','s','小舞','emergent','notable','{}',1,1,'t','t');
             INSERT INTO passages (id,story_id,turn_index,trigger_action_md,content_md,parse_status,engine_mode,created_at)
               VALUES ('p0','s',0,'a','第一段','ok','local','t'),
                      ('p1','s',1,'b','第二段','ok','local','t');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn snapshot_roundtrip_drops_later_turns_and_restores_memory() {
        let conn = seeded();
        let id = write_snapshot(&conn, "s", 0, "manual", "checkpoint").unwrap();

        // Play one more turn: state, summary and a new character all change.
        conn.execute("UPDATE state_card SET data_json = '{\"player\":{\"goal\":\"变了\"}}'", [])
            .unwrap();
        conn.execute("UPDATE summary_state SET content_md = '之后发生的事'", []).unwrap();
        conn.execute(
            "INSERT INTO characters
               (id,world_id,story_id,name,kind,importance,current_state,first_turn,inject,created_at,updated_at)
             VALUES ('c2','w','s','奥斯卡','emergent','notable','{}',2,1,'t','t')",
            [],
        )
        .unwrap();

        let (story_id, turn, state, summary, chars): (String, i64, String, String, String) = conn
            .query_row(
                "SELECT story_id, turn_index, state_json, summary_md, characters_json
                 FROM story_saves WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        apply_snapshot(&conn, &story_id, turn, &state, &summary, &chars).unwrap();

        let turns: i64 = conn
            .query_row("SELECT COUNT(*) FROM passages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(turns, 1, "the later turn is dropped");
        let goal: String = conn
            .query_row("SELECT data_json FROM state_card", [], |r| r.get(0))
            .unwrap();
        assert!(goal.contains("开局"), "state card goes back to the snapshot");
        let chars_now: i64 = conn
            .query_row("SELECT COUNT(*) FROM characters", [], |r| r.get(0))
            .unwrap();
        assert_eq!(chars_now, 1, "characters born later are removed");
    }

    #[test]
    fn snapshot_keeps_only_the_latest_sixty_autosaves() {
        let conn = seeded();
        for t in 0..70 {
            write_snapshot(&conn, "s", t, "auto", "").unwrap();
        }
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM story_saves WHERE kind = 'auto'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 60);
    }
}
