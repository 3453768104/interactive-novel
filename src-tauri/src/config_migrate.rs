use crate::defaults;
use crate::lore::sha256_hex;
use crate::state::AppState;
use anyhow::Result;
use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};

fn flag_set(conn: &Connection, key: &str) -> bool {
    conn.query_row("SELECT 1 FROM app_config WHERE key = ?1", params![key], |_| Ok(()))
        .is_ok()
}

fn set_flag(conn: &Connection, key: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO app_config(key, value) VALUES (?1, '1')",
        params![key],
    )?;
    Ok(())
}

fn stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Recursively copy a directory (used for both legacy migration and config backup).
pub fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&from, &to)?;
        } else if ty.is_file() {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn backup_file(state: &AppState, rel: &str, content: &str) -> Result<()> {
    let dst = state
        .backups_dir
        .join(format!("config-{}", stamp()))
        .join(rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dst, content)?;
    Ok(())
}

/// Three-way upgrade of shipped config files.
///
/// cur == old  -> user never touched it, safe to overwrite with v2
/// cur != old  -> user edited it, keep it and drop a `<rel>.new` for manual merge
/// README      -> documentation, always refreshed (with a backup first)
/// Returns the list of files that need a manual merge.
pub fn ensure(state: &AppState) -> Result<Vec<String>> {
    let conn = state.db.lock()?;
    let version: i64 = conn
        .query_row(
            "SELECT value FROM app_config WHERE key = 'defaults_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    conn.execute(
        "INSERT OR REPLACE INTO app_config(key, value) VALUES ('defaults_version', ?1)",
        params![defaults::DEFAULTS_VERSION.to_string()],
    )?;
    drop(conn);

    let mut needs_merge = Vec::new();
    if version >= defaults::DEFAULTS_VERSION {
        return Ok(needs_merge);
    }

    let prev = defaults::prev_all();
    for (rel, new_content) in defaults::all() {
        let target = state.config_dir.join(rel);
        if !target.exists() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target, new_content)?;
            continue;
        }
        let old_content = prev
            .iter()
            .find(|(r, _)| *r == rel)
            .map(|(_, c)| *c)
            .unwrap_or("");
        let cur = fs::read_to_string(&target).unwrap_or_default();
        let cur_sha = sha256_hex(cur.as_bytes());
        let old_sha = sha256_hex(old_content.as_bytes());
        let new_sha = sha256_hex(new_content.as_bytes());

        if rel.ends_with("README.md") {
            if cur_sha != new_sha {
                backup_file(state, rel, &cur)?;
                fs::write(&target, new_content)?;
            }
            continue;
        }
        if old_sha == new_sha || cur_sha == new_sha {
            continue;
        }
        if cur_sha == old_sha {
            fs::write(&target, new_content)?;
        } else {
            let new_path: PathBuf = state.config_dir.join(format!("{rel}.new"));
            if let Some(parent) = new_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&new_path, new_content)?;
            needs_merge.push(rel.to_string());
        }
    }
    Ok(needs_merge)
}

/// Explicit "restore factory defaults": back up the whole config/ first,
/// then overwrite everything and clear pending .new files.
pub fn reset_to_defaults(state: &AppState) -> Result<Vec<String>> {
    let dst = state.backups_dir.join(format!("config-{}", stamp()));
    copy_dir_all(&state.config_dir, &dst)?;

    let mut written = Vec::new();
    for (rel, content) in defaults::all() {
        let path = state.config_dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        let pending = state.config_dir.join(format!("{rel}.new"));
        if pending.exists() {
            let _ = fs::remove_file(pending);
        }
        written.push(rel.to_string());
    }
    let conn = state.db.lock()?;
    conn.execute(
        "INSERT OR REPLACE INTO app_config(key, value) VALUES ('defaults_version', ?1)",
        params![defaults::DEFAULTS_VERSION.to_string()],
    )?;
    Ok(written)
}

/// Online SQLite backup (WAL-safe) before we touch historical choices.
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_state() -> (AppState, PathBuf) {
        let dir = std::env::temp_dir()
            .join(format!("novel-migrate-test-{}", uuid::Uuid::new_v4()));
        let st = AppState::new(dir.clone(), None).unwrap();
        (st, dir)
    }

    #[test]
    fn factory_upgrade_never_overwrites_user_edits() {
        let (st, dir) = temp_state();
        let path = st.config_dir.join("app.yaml");
        let original = fs::read_to_string(&path).unwrap();
        fs::write(&path, format!("{original}\n# 用户手改\n")).unwrap();

        let merge = ensure(&st).unwrap();
        assert!(merge.iter().any(|f| f == "app.yaml"));
        assert!(st.config_dir.join("app.yaml.new").exists());
        assert!(fs::read_to_string(&path).unwrap().contains("用户手改"));
        drop(st);
        let _ = fs::remove_dir_all(dir);
    }

    /// The v3 factory snapshot, used to prove a user who never hand-edited a
    /// file is upgraded automatically (not misdetected as "modified").
    const PREV_APP: &str = include_str!("../../config/defaults_v3/app.yaml");

    #[test]
    fn fresh_v3_defaults_are_complete() {
        let (st, dir) = temp_state();
        let app = fs::read_to_string(st.config_dir.join("app.yaml")).unwrap();
        assert!(app.contains("stop_triggers"));
        assert!(app.contains("persona_soft_cap"));
        for rel in [
            "templates/system.omni.local.md",
            "templates/system.omni.api.md",
            "templates/system.omni.paste.md",
        ] {
            assert!(st.config_dir.join(rel).exists(), "missing factory file {rel}");
        }
        let merge = ensure(&st).unwrap();
        assert!(!merge.iter().any(|f| f == "app.yaml"));
        assert_eq!(defaults_version(&st), defaults::DEFAULTS_VERSION);
        drop(st);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn untouched_prev_config_is_auto_upgraded() {
        let (st, dir) = temp_state();
        let path = st.config_dir.join("app.yaml");
        fs::write(&path, PREV_APP).unwrap();
        let merge = ensure(&st).unwrap();
        let upgraded = fs::read_to_string(&path).unwrap();
        assert!(
            upgraded.contains("character_inject: notable"),
            "untouched previous-version file must be upgraded"
        );
        assert!(!merge.iter().any(|f| f == "app.yaml"));
        assert!(!st.config_dir.join("app.yaml.new").exists());
        drop(st);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn historical_glued_choices_are_reparsed_once() {
        let (st, dir) = temp_state();
        {
            let conn = st.db.lock().unwrap();
            conn.execute_batch(
                "INSERT INTO worlds (id,name,description,created_at,updated_at,mirror_path)
                 VALUES ('w1','w','','t','t','w1');
                 INSERT INTO stories (id,world_id,name,engine_mode,created_at,updated_at,mirror_path)
                 VALUES ('s1','w1','s','paste','t','t','w1/stories/s1');
                 INSERT INTO passages
                   (id,story_id,turn_index,trigger_action_md,content_md,raw_output,parse_status,engine_mode,created_at)
                 VALUES ('p1','s1',0,'开始','正文未改动',
                   '<<<NARRATION>>>\n他停住脚。\n<<<END_NARRATION>>>\n<<<CHOICES>>>\nA. 沿着土路走 B. 站在原地 C. 心里想着别人\n<<<END_CHOICES>>>',
                   'ok','paste','t');
                 INSERT INTO choices (id,passage_id,ordinal,label,action_md,is_free_input)
                 VALUES ('c1','p1',0,'A. 沿着土路走 B. 站在原地 C. 心里想着别人',
                   'A. 沿着土路走 B. 站在原地 C. 心里想着别人',0);",
            )
            .unwrap();
        }

        assert_eq!(reparse_legacy_choices(&st).unwrap(), 1);
        {
            let conn = st.db.lock().unwrap();
            let n: i64 = conn
                .query_row("SELECT COUNT(*) FROM choices WHERE passage_id='p1'", [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 3);
            let narration: String = conn
                .query_row("SELECT content_md FROM passages WHERE id='p1'", [], |r| r.get(0))
                .unwrap();
            assert_eq!(narration, "正文未改动");
        }
        // The flag makes it a no-op on every later startup.
        assert_eq!(reparse_legacy_choices(&st).unwrap(), 0);
        drop(st);
        let _ = fs::remove_dir_all(dir);
    }
}

fn backup_database(state: &AppState) -> Result<PathBuf> {
    let path = state
        .backups_dir
        .join(format!("novel-before-reparse-{}.db", stamp()));
    let conn = state.db.lock()?;
    let mut dst = Connection::open(&path)?;
    {
        let bk = rusqlite::backup::Backup::new(&*conn, &mut dst)?;
        bk.run_to_completion(100, std::time::Duration::from_millis(10), None)?;
    }
    Ok(path)
}

fn inline_label_count(text: &str) -> usize {
    let re = regex::Regex::new(r"[A-DＡ-Ｄa-d一二三四①②③④][\.、．:：]").unwrap();
    re.find_iter(text).count()
}

/// One-shot repair of passages whose options were all glued into a single
/// choice by the old parser. Only the `choices` rows are rewritten;
/// narration / state card / summary are never touched.
pub fn reparse_legacy_choices(state: &AppState) -> Result<usize> {
    let conn = state.db.lock()?;
    if flag_set(&conn, "choices_reparse_v1") {
        return Ok(0);
    }

    let mut stmt = conn.prepare(
        "SELECT p.id, p.raw_output,
                (SELECT c.label FROM choices c WHERE c.passage_id = p.id LIMIT 1)
         FROM passages p
         WHERE p.parse_status = 'ok' AND p.raw_output != ''
           AND (SELECT COUNT(*) FROM choices c WHERE c.passage_id = p.id) = 1",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    let mut fixes: Vec<(String, Vec<String>)> = Vec::new();
    for (passage_id, raw, label) in rows {
        if inline_label_count(&label) < 2 {
            continue;
        }
        // Local-style tolerant parsing works for markers produced by every mode.
        let parsed = crate::engine::parse_output(&raw, "local");
        if parsed.choices.len() >= 2 {
            fixes.push((passage_id, parsed.choices));
        }
    }

    if fixes.is_empty() {
        set_flag(&conn, "choices_reparse_v1")?;
        return Ok(0);
    }

    drop(conn);
    backup_database(state)?;
    let conn = state.db.lock()?;
    for (passage_id, choices) in &fixes {
        conn.execute(
            "DELETE FROM choices WHERE passage_id = ?1",
            params![passage_id],
        )?;
        for (idx, label) in choices.iter().enumerate() {
            conn.execute(
                "INSERT INTO choices (id, passage_id, ordinal, label, action_md, is_free_input)
                 VALUES (?1, ?2, ?3, ?4, ?4, 0)",
                params![AppState::new_id(), passage_id, idx as i64, label],
            )?;
        }
    }
    set_flag(&conn, "choices_reparse_v1")?;
    Ok(fixes.len())
}

pub fn defaults_version(state: &AppState) -> i64 {
    state
        .db
        .lock()
        .ok()
        .and_then(|c| {
            c.query_row(
                "SELECT value FROM app_config WHERE key = 'defaults_version'",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
        })
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

