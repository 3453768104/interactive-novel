use crate::config_migrate;
use crate::domain::*;
use crate::error::{cmd_err, CmdResult};
use crate::lore;
use crate::state::AppState;
use rusqlite::params;
use std::fs;
use tauri::State;

fn now() -> String {
    AppState::now()
}

fn stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

// ---------------------------------------------------------------------------
// Factory config
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn reset_config_to_defaults(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    let st = &*state;
    let written = config_migrate::reset_to_defaults(st).map_err(cmd_err)?;
    *st.needs_merge
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = Vec::new();
    Ok(written)
}

// ---------------------------------------------------------------------------
// Event background (World level, with per-story override)
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn get_world_event_background(
    state: State<'_, AppState>,
    world_id: String,
) -> CmdResult<WorldEventBackground> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let (content_md, updated_at) = conn
        .query_row(
            "SELECT content_md, updated_at FROM world_event_background WHERE world_id = ?1",
            params![world_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .unwrap_or((String::new(), String::new()));
    Ok(WorldEventBackground { content_md, updated_at })
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_world_event_background(
    state: State<'_, AppState>,
    world_id: String,
    content_md: String,
) -> CmdResult<WorldEventBackground> {
    let st = &*state;
    let ts = now();
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO world_event_background (world_id, content_md, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(world_id) DO UPDATE SET content_md = excluded.content_md, updated_at = excluded.updated_at",
        params![world_id, content_md, ts],
    )
    .map_err(cmd_err)?;
    Ok(WorldEventBackground { content_md, updated_at: ts })
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_story_event_background(
    state: State<'_, AppState>,
    story_id: String,
) -> CmdResult<StoryEventBackground> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let world_id: String = conn
        .query_row(
            "SELECT world_id FROM stories WHERE id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .map_err(cmd_err)?;
    let inherited_content_md: String = conn
        .query_row(
            "SELECT content_md FROM world_event_background WHERE world_id = ?1",
            params![world_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let (enabled, content_md, updated_at) = conn
        .query_row(
            "SELECT enabled, content_md, updated_at FROM story_event_background WHERE story_id = ?1",
            params![story_id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
        )
        .unwrap_or((0, String::new(), String::new()));
    Ok(StoryEventBackground {
        enabled: enabled != 0,
        content_md,
        inherited_content_md,
        updated_at,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_story_event_background(
    state: State<'_, AppState>,
    story_id: String,
    enabled: bool,
    content_md: String,
) -> CmdResult<()> {
    let st = &*state;
    let ts = now();
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO story_event_background (story_id, content_md, enabled, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(story_id) DO UPDATE SET
            content_md = excluded.content_md, enabled = excluded.enabled,
            updated_at = excluded.updated_at",
        params![story_id, content_md, enabled as i64, ts],
    )
    .map_err(cmd_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Categories
// ---------------------------------------------------------------------------

const CATEGORY_SELECT: &str = "SELECT c.id, c.world_id, c.parent_id, c.name, c.sort_order,
       (SELECT COUNT(*) FROM lore_cards lc WHERE lc.category_id = c.id)
       FROM lore_categories c";

fn category_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LoreCategory> {
    Ok(LoreCategory {
        id: r.get(0)?,
        world_id: r.get(1)?,
        parent_id: r.get(2)?,
        name: r.get(3)?,
        sort_order: r.get(4)?,
        card_count: r.get(5)?,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_lore_categories(
    state: State<'_, AppState>,
    world_id: String,
) -> CmdResult<Vec<LoreCategory>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let sql = format!("{CATEGORY_SELECT} WHERE c.world_id = ?1 ORDER BY c.sort_order, c.name");
    let mut stmt = conn.prepare(&sql).map_err(cmd_err)?;
    let rows = stmt
        .query_map(params![world_id], category_from_row)
        .map_err(cmd_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(cmd_err)?;
    Ok(rows)
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_lore_category(
    state: State<'_, AppState>,
    world_id: String,
    name: String,
    parent_id: Option<String>,
) -> CmdResult<LoreCategory> {
    if name.trim().is_empty() {
        return Err("分类名称不能为空".into());
    }
    let st = &*state;
    let id = AppState::new_id();
    let conn = st.db.lock().map_err(cmd_err)?;
    let sort_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM lore_categories WHERE world_id = ?1",
            params![world_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO lore_categories (id, world_id, parent_id, name, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, world_id, parent_id, name.trim(), sort_order],
    )
    .map_err(|e| format!("创建分类失败（同级可能已存在同名分类）：{e}"))?;
    Ok(LoreCategory {
        id,
        world_id,
        parent_id,
        name: name.trim().to_string(),
        sort_order,
        card_count: 0,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn rename_lore_category(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> CmdResult<()> {
    if name.trim().is_empty() {
        return Err("分类名称不能为空".into());
    }
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "UPDATE lore_categories SET name = ?1 WHERE id = ?2",
        params![name.trim(), id],
    )
    .map_err(|e| format!("重命名失败（同级可能已存在同名分类）：{e}"))?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_lore_category(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "UPDATE lore_categories SET parent_id = NULL WHERE parent_id = ?1",
        params![id],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "UPDATE lore_cards SET category_id = NULL WHERE category_id = ?1",
        params![id],
    )
    .map_err(cmd_err)?;
    conn.execute("DELETE FROM lore_categories WHERE id = ?1", params![id])
        .map_err(cmd_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cards (1 card = 1 .md = 1 lore_docs row)
// ---------------------------------------------------------------------------

const CARD_SELECT: &str = "SELECT d.id, d.world_id, d.title, d.rel_path, lc.category_id,
       cat.name, lc.tags, lc.summary, lc.char_count, d.chunk_count, d.sha256, lc.updated_at
       FROM lore_docs d
       JOIN lore_cards lc ON lc.doc_id = d.id
       LEFT JOIN lore_categories cat ON cat.id = lc.category_id";

fn card_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LoreCard> {
    let tags_json: String = r.get(6)?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    Ok(LoreCard {
        doc_id: r.get(0)?,
        world_id: r.get(1)?,
        title: r.get(2)?,
        rel_path: r.get(3)?,
        category_id: r.get(4)?,
        category_name: r.get(5)?,
        tags,
        summary: r.get(7)?,
        char_count: r.get(8)?,
        chunk_count: r.get(9)?,
        sha256: r.get(10)?,
        updated_at: r.get(11)?,
    })
}

fn cards_for_world(conn: &rusqlite::Connection, world_id: &str) -> CmdResult<Vec<LoreCard>> {
    let sql = format!("{CARD_SELECT} WHERE d.world_id = ?1 ORDER BY lc.updated_at DESC, d.title");
    let mut stmt = conn.prepare(&sql).map_err(cmd_err)?;
    let rows = stmt
        .query_map(params![world_id], card_from_row)
        .map_err(cmd_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(cmd_err)?;
    Ok(rows)
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_lore_cards(
    state: State<'_, AppState>,
    world_id: String,
) -> CmdResult<Vec<LoreCard>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    cards_for_world(&conn, &world_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_lore_card(
    state: State<'_, AppState>,
    doc_id: String,
) -> CmdResult<LoreCardDetail> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let sql = format!("{CARD_SELECT} WHERE d.id = ?1");
    let card = conn
        .query_row(&sql, params![doc_id], card_from_row)
        .map_err(cmd_err)?;
    let mut stmt = conn
        .prepare("SELECT event_id FROM card_timeline WHERE card_id = ?1")
        .map_err(cmd_err)?;
    let timeline_event_ids = stmt
        .query_map(params![doc_id], |r| r.get(0))
        .map_err(cmd_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(cmd_err)?;
    drop(stmt);
    drop(conn);

    // read_doc takes the db lock itself, so it must run after we release it.
    let (_, content, _) = lore::read_doc(st, &doc_id).map_err(cmd_err)?;
    Ok(LoreCardDetail { card, content, timeline_event_ids })
}

/// Metadata lives only in SQLite. Writing through this command NEVER touches
/// the .md file, so sha / watcher / external-editor conflicts are unaffected.
#[tauri::command(rename_all = "snake_case")]
pub fn update_lore_card(
    state: State<'_, AppState>,
    doc_id: String,
    patch: LoreCardPatch,
) -> CmdResult<LoreCard> {
    if patch.title.trim().is_empty() {
        return Err("卡片标题不能为空".into());
    }
    let st = &*state;
    let ts = now();
    let tags_json = serde_json::to_string(&patch.tags).map_err(cmd_err)?;
    let conn = st.db.lock().map_err(cmd_err)?;
    let world_id: String = conn
        .query_row(
            "SELECT world_id FROM lore_docs WHERE id = ?1",
            params![doc_id],
            |r| r.get(0),
        )
        .map_err(cmd_err)?;

    conn.execute(
        "UPDATE lore_docs SET title = ?1 WHERE id = ?2",
        params![patch.title.trim(), doc_id],
    )
    .map_err(cmd_err)?;
    conn.execute(
        "UPDATE lore_cards SET category_id = ?1, tags = ?2, summary = ?3, updated_at = ?4
         WHERE doc_id = ?5",
        params![patch.category_id, tags_json, patch.summary, ts, doc_id],
    )
    .map_err(cmd_err)?;

    conn.execute("DELETE FROM card_tags WHERE card_id = ?1", params![doc_id])
        .map_err(cmd_err)?;
    for tag in &patch.tags {
        if !tag.trim().is_empty() {
            conn.execute(
                "INSERT OR IGNORE INTO card_tags (card_id, tag) VALUES (?1, ?2)",
                params![doc_id, tag.trim()],
            )
            .map_err(cmd_err)?;
        }
    }

    conn.execute("DELETE FROM card_timeline WHERE card_id = ?1", params![doc_id])
        .map_err(cmd_err)?;
    for event_id in &patch.timeline_event_ids {
        conn.execute(
            "INSERT OR IGNORE INTO card_timeline (card_id, event_id) VALUES (?1, ?2)",
            params![doc_id, event_id],
        )
        .map_err(cmd_err)?;
    }

    let sql = format!("{CARD_SELECT} WHERE d.id = ?1");
    let card = conn
        .query_row(&sql, params![doc_id], card_from_row)
        .map_err(cmd_err)?;
    let _ = world_id;
    Ok(card)
}

/// One-shot helper: turn a long legacy .md with several ## sections into
/// independent cards. The original file is backed up before deletion.
#[tauri::command(rename_all = "snake_case")]
pub fn split_lore_doc_by_headings(
    state: State<'_, AppState>,
    doc_id: String,
    category_id: Option<String>,
) -> CmdResult<Vec<LoreCard>> {
    let st = &*state;
    let (doc, content, _) = lore::read_doc(st, &doc_id).map_err(cmd_err)?;
    let sections = split_h2(&content);
    if sections.len() < 2 {
        return Err("这个文档没有可拆分的 ## 章节（至少需要 2 个非空章节）".into());
    }

    let abs = lore::doc_abs_path(st, &doc);
    let backup = st
        .backups_dir
        .join(format!("lore-split-{}-{}", doc_id, stamp()));
    fs::copy(&abs, backup).map_err(cmd_err)?;

    let mut new_ids = Vec::new();
    for (idx, (title, body)) in sections.iter().enumerate() {
        let created = lore::create_doc(
            st,
            &doc.world_id,
            &format!("{} {} {}", doc.title, idx + 1, title),
        )
        .map_err(cmd_err)?;
        let body_md = format!("# {title}\n\n{body}\n");
        lore::save_doc(st, &created.id, &body_md, &created.sha256, true)
            .map_err(cmd_err)?;
        let conn = st.db.lock().map_err(cmd_err)?;
        conn.execute(
            "UPDATE lore_cards SET category_id = ?1 WHERE doc_id = ?2",
            params![category_id, created.id],
        )
        .map_err(cmd_err)?;
        drop(conn);
        new_ids.push(created.id);
    }
    lore::delete_doc(st, &doc_id).map_err(cmd_err)?;

    let conn = st.db.lock().map_err(cmd_err)?;
    let cards = cards_for_world(&conn, &doc.world_id)?;
    Ok(cards
        .into_iter()
        .filter(|c| new_ids.contains(&c.doc_id))
        .collect())
}

/// Bulk import a hand-written setting book: `# 编` becomes a category,
/// `## 条目` becomes one card each. Idempotent by card title.
#[tauri::command(rename_all = "snake_case")]
pub fn import_lore_markdown(
    state: State<'_, AppState>,
    world_id: String,
    path: String,
) -> CmdResult<crate::importer::ImportReport> {
    let st = &*state;
    let md = fs::read_to_string(&path).map_err(|e| format!("读取 {path} 失败：{e}"))?;
    crate::importer::import_bundle(st, &world_id, &md).map_err(cmd_err)
}

fn split_h2(content: &str) -> Vec<(String, String)> {    let mut out: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in content.lines() {
        let t = line.trim_end();
        if t.starts_with("## ") {
            if let Some((title, body)) = current.take() {
                if !body.trim().is_empty() {
                    out.push((title, body.trim().to_string()));
                }
            }
            current = Some((t[3..].trim().to_string(), String::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some((title, body)) = current {
        if !body.trim().is_empty() {
            out.push((title, body.trim().to_string()));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Timeline (world history)
// ---------------------------------------------------------------------------

const TIMELINE_SELECT: &str = "SELECT id, world_id, era, year_label, title, body_md,
       ordinal, inject_into_context, created_at, updated_at FROM timeline_events";

fn timeline_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TimelineEvent> {
    Ok(TimelineEvent {
        id: r.get(0)?,
        world_id: r.get(1)?,
        era: r.get(2)?,
        year_label: r.get(3)?,
        title: r.get(4)?,
        body_md: r.get(5)?,
        ordinal: r.get(6)?,
        inject_into_context: r.get::<_, i64>(7)? != 0,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_timeline_events(
    state: State<'_, AppState>,
    world_id: String,
) -> CmdResult<Vec<TimelineEvent>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let sql = format!("{TIMELINE_SELECT} WHERE world_id = ?1 ORDER BY ordinal, created_at");
    let mut stmt = conn.prepare(&sql).map_err(cmd_err)?;
    let rows = stmt
        .query_map(params![world_id], timeline_from_row)
        .map_err(cmd_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(cmd_err)?;
    Ok(rows)
}

#[tauri::command(rename_all = "snake_case")]
pub fn upsert_timeline_event(
    state: State<'_, AppState>,
    world_id: String,
    event: TimelineEvent,
) -> CmdResult<TimelineEvent> {
    if event.title.trim().is_empty() {
        return Err("时间线事件标题不能为空".into());
    }
    let st = &*state;
    let ts = now();
    let id = if event.id.is_empty() {
        AppState::new_id()
    } else {
        event.id.clone()
    };
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO timeline_events
            (id, world_id, era, year_label, title, body_md, ordinal, inject_into_context, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
         ON CONFLICT(id) DO UPDATE SET
            era = excluded.era, year_label = excluded.year_label, title = excluded.title,
            body_md = excluded.body_md, ordinal = excluded.ordinal,
            inject_into_context = excluded.inject_into_context, updated_at = excluded.updated_at",
        params![
            id,
            world_id,
            event.era,
            event.year_label,
            event.title.trim(),
            event.body_md,
            event.ordinal,
            event.inject_into_context as i64,
            ts
        ],
    )
    .map_err(cmd_err)?;
    let sql = format!("{TIMELINE_SELECT} WHERE id = ?1");
    conn.query_row(&sql, params![id], timeline_from_row)
        .map_err(cmd_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_timeline_event(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute("DELETE FROM timeline_events WHERE id = ?1", params![id])
        .map_err(cmd_err)?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_card_timeline(
    state: State<'_, AppState>,
    card_id: String,
    event_ids: Vec<String>,
) -> CmdResult<()> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute("DELETE FROM card_timeline WHERE card_id = ?1", params![card_id])
        .map_err(cmd_err)?;
    for event_id in event_ids {
        conn.execute(
            "INSERT OR IGNORE INTO card_timeline (card_id, event_id) VALUES (?1, ?2)",
            params![card_id, event_id],
        )
        .map_err(cmd_err)?;
    }
    let ts = now();
    conn.execute(
        "UPDATE lore_cards SET updated_at = ?1 WHERE doc_id = ?2",
        params![ts, card_id],
    )
    .map_err(cmd_err)?;
    Ok(())
}
