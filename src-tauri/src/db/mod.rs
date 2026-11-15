use anyhow::{anyhow, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(include_str!("schema.sql"))?;
        // All post-schema migrations are idempotent and safe to replay.
        migrate_mirror_paths(&conn)?;
        ensure_lore_categories(&conn)?;
        ensure_lore_cards(&conn)?;
        ensure_story_settings(&conn)?;
        ensure_preset_characters(&conn)?;
        // Keep the search index in step with the chunk table (idempotent).
        let _ = crate::retrieval::sync_fts(&conn);
        let _ = crate::search::sync_passage_fts(&conn);
        // v3: "喜欢" was folded into "重要" — there is only one elevated tier
        // now, so an old favourite must not become invisible to injection.
        conn.execute(
            "UPDATE characters SET importance = 'notable' WHERE importance = 'favorite'",
            [],
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| anyhow!("database lock was poisoned"))
    }
}

fn flag_set(conn: &Connection, key: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM app_config WHERE key = ?1",
        rusqlite::params![key],
        |_| Ok(()),
    )
    .is_ok()
}

/// Old builds stored absolute mirror_path values; after switching to a
/// portable data dir they become stale. Store relative paths instead.
pub fn migrate_mirror_paths(conn: &Connection) -> Result<()> {
    if flag_set(conn, "mirror_path_rel_v1") {
        return Ok(());
    }
    conn.execute("UPDATE worlds SET mirror_path = id", [])?;
    conn.execute(
        "UPDATE stories SET mirror_path = world_id || '/stories/' || id",
        [],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO app_config(key, value) VALUES ('mirror_path_rel_v1', '1')",
        [],
    )?;
    Ok(())
}

const SEED_CATEGORIES: &[&str] = &["角色", "物品", "组织", "地理", "能力", "魔法", "事件", "其他"];

/// Seed a small starter taxonomy for worlds that have no categories yet.
pub fn ensure_lore_categories(conn: &Connection) -> Result<()> {
    for world_id in world_ids(conn)? {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM lore_categories WHERE world_id = ?1",
            rusqlite::params![world_id],
            |r| r.get(0),
        )?;
        if count > 0 {
            continue;
        }
        for (idx, name) in SEED_CATEGORIES.iter().enumerate() {
            conn.execute(
                "INSERT OR IGNORE INTO lore_categories (id, world_id, parent_id, name, sort_order)
                 VALUES (?1, ?2, NULL, ?3, ?4)",
                rusqlite::params![crate::state::AppState::new_id(), world_id, name, idx as i64],
            )?;
        }
    }
    Ok(())
}

/// Every existing lore_docs becomes exactly one card. This only writes the
/// side-car table: no .md file is touched, so no sha / watcher event fires.
pub fn ensure_lore_cards(conn: &Connection) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO lore_cards
            (doc_id, world_id, category_id, tags, summary, char_count, updated_at)
         SELECT d.id, d.world_id, NULL, '[]', '',
                COALESCE((SELECT SUM(c.char_len) FROM lore_chunks c WHERE c.doc_id = d.id), 0),
                d.created_at
         FROM lore_docs d",
        [],
    )?;
    Ok(())
}

/// Existing stories predate the POV setting; a missing row means "protagonist".
pub fn ensure_story_settings(conn: &Connection) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO story_settings (story_id, pov, show_scene_markers, updated_at)
         SELECT id, 'protagonist', 1, updated_at FROM stories",
        [],
    )?;
    Ok(())
}

/// Seed one preset character per lore card in the 角色 category. This is a
/// read-only projection of the lore library; editing the .md and re-saving the
/// card title is what updates it (no second source of truth for the body).
/// 角色 cards become an opt-in cast list: passerby until the story actually
/// introduces them (the analysis then promotes them to notable) or the reader
/// marks them. Otherwise every name in a 400-card setting book would be
/// injected every turn.
pub fn ensure_preset_characters(conn: &Connection) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO characters
            (id, world_id, story_id, name, kind, importance, preset_card_id,
             current_state, first_turn, inject, created_at, updated_at)
         SELECT d.id, c.world_id, NULL, d.title, 'preset', 'passerby', c.doc_id,
                '{}', NULL, 0, d.created_at, d.created_at
         FROM lore_cards c
         JOIN lore_docs d ON d.id = c.doc_id
         JOIN lore_categories cat ON cat.id = c.category_id
         WHERE cat.name = '角色' AND d.title != ''",
        [],
    )?;
    Ok(())
}

fn world_ids(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM worlds")?;
    let rows = stmt
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_applies_and_fts5_works() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("schema.sql")).unwrap();
        // Proves the bundled SQLite really has FTS5 and our virtual table works.
        conn.execute(
            "INSERT INTO lore_fts (rowid, tokens, chunk_id) VALUES (1, '北境 气候 严寒', 1)",
            [],
        )
        .unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lore_fts WHERE lore_fts MATCH '北境'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn side_car_tables_and_seeding_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("schema.sql")).unwrap();
        conn.execute(
            "INSERT INTO worlds (id,name,description,created_at,updated_at,mirror_path)
             VALUES ('w1','测试','','t','t','/old/absolute/w1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lore_docs (id,world_id,title,rel_path,sha256,origin,created_at)
             VALUES ('d1','w1','设定','lore/设定.md','abc','empty','t')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lore_chunks (doc_id,ordinal,heading_path,content,char_len,token_est,sha256)
             VALUES ('d1',0,'设定 > 地理','北境很冷',4,4,'x')",
            [],
        )
        .unwrap();

        migrate_mirror_paths(&conn).unwrap();
        let mirror: String = conn
            .query_row("SELECT mirror_path FROM worlds WHERE id='w1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mirror, "w1");

        ensure_lore_categories(&conn).unwrap();
        ensure_lore_cards(&conn).unwrap();
        let cats: i64 = conn
            .query_row("SELECT COUNT(*) FROM lore_categories WHERE world_id='w1'", [], |r| r.get(0))
            .unwrap();
        let cards: i64 = conn
            .query_row("SELECT COUNT(*) FROM lore_cards", [], |r| r.get(0))
            .unwrap();
        let chars: i64 = conn
            .query_row("SELECT char_count FROM lore_cards WHERE doc_id='d1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cats, 8);
        assert_eq!(cards, 1);
        assert_eq!(chars, 4);

        // Replaying on every startup must not duplicate anything.
        migrate_mirror_paths(&conn).unwrap();
        ensure_lore_categories(&conn).unwrap();
        ensure_lore_cards(&conn).unwrap();
        let cats2: i64 = conn
            .query_row("SELECT COUNT(*) FROM lore_categories WHERE world_id='w1'", [], |r| r.get(0))
            .unwrap();
        let cards2: i64 = conn
            .query_row("SELECT COUNT(*) FROM lore_cards", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cats2, 8);
        assert_eq!(cards2, 1);
    }

    #[test]
    fn v2_tables_seed_pov_presets_and_stop_meta_idempotently() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("schema.sql")).unwrap();
        conn.execute_batch(
            "INSERT INTO worlds (id,name,description,created_at,updated_at,mirror_path)
             VALUES ('w1','测试','','t','t','w1');
             INSERT INTO stories (id,world_id,name,engine_mode,created_at,updated_at,mirror_path)
             VALUES ('s1','w1','故事','local','t','t','w1/stories/s1');
             INSERT INTO lore_categories (id,world_id,parent_id,name,sort_order)
             VALUES ('c1','w1',NULL,'角色',0);
             INSERT INTO lore_docs (id,world_id,title,rel_path,sha256,origin,created_at)
             VALUES ('d1','w1','守门人','lore/守门人.md','x','empty','t');
             INSERT INTO lore_cards (doc_id,world_id,category_id,tags,summary,char_count,updated_at)
             VALUES ('d1','w1','c1','[]','',0,'t');
             INSERT INTO passages (id,story_id,turn_index,trigger_action_md,content_md,parse_status,created_at)
             VALUES ('p1','s1',0,'开始','正文','ok','t');",
        )
        .unwrap();

        ensure_story_settings(&conn).unwrap();
        ensure_preset_characters(&conn).unwrap();
        let pov: String = conn
            .query_row("SELECT pov FROM story_settings WHERE story_id='s1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(pov, "protagonist");
        let presets: i64 = conn
            .query_row("SELECT COUNT(*) FROM characters WHERE kind='preset'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(presets, 1);

        conn.execute(
            "INSERT INTO passage_meta (passage_id, stop_decision, stop_reason, stop_trigger, pov, analysis_json, created_at)
             VALUES ('p1',0,'继续写','','omniscient','{}','t')",
            [],
        )
        .unwrap();

        // Replaying on the next startup must not duplicate or overwrite.
        ensure_story_settings(&conn).unwrap();
        ensure_preset_characters(&conn).unwrap();
        let presets2: i64 = conn
            .query_row("SELECT COUNT(*) FROM characters WHERE kind='preset'", [], |r| r.get(0))
            .unwrap();
        let meta: i64 = conn
            .query_row("SELECT stop_decision FROM passage_meta WHERE passage_id='p1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(presets2, 1);
        assert_eq!(meta, 0, "user/AI stop metadata must survive a restart");
    }
}
