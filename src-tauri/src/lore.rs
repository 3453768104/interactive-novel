use crate::defaults;
use crate::domain::LoreDoc;
use crate::error::CONFLICT_PREFIX;
use crate::state::AppState;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn world_dir(state: &AppState, world_id: &str) -> PathBuf {
    state.worlds_dir.join(world_id)
}

pub fn lore_dir(state: &AppState, world_id: &str) -> PathBuf {
    world_dir(state, world_id).join("lore")
}

/// Create the world folder and its first (empty) lore document.
pub fn init_world_files(state: &AppState, world_id: &str) -> Result<(String, String)> {
    let dir = lore_dir(state, world_id);
    fs::create_dir_all(&dir)?;
    let rel = "lore/设定.md".to_string();
    let abs = dir.join("设定.md");
    fs::write(&abs, defaults::LORE_TEMPLATE)?;
    Ok((rel, sha256_hex(defaults::LORE_TEMPLATE.as_bytes())))
}

pub fn list_docs(conn: &Connection, world_id: &str) -> Result<Vec<LoreDoc>> {
    let mut stmt = conn.prepare(
        "SELECT id, world_id, title, rel_path, sha256, chunk_count, origin, created_at
         FROM lore_docs WHERE world_id = ?1 ORDER BY created_at, title",
    )?;
    let rows = stmt.query_map(params![world_id], |r| {
        Ok(LoreDoc {
            id: r.get(0)?,
            world_id: r.get(1)?,
            title: r.get(2)?,
            rel_path: r.get(3)?,
            sha256: r.get(4)?,
            chunk_count: r.get(5)?,
            origin: r.get(6)?,
            created_at: r.get(7)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

fn doc_by_id(conn: &Connection, id: &str) -> Result<LoreDoc> {
    conn.query_row(
        "SELECT id, world_id, title, rel_path, sha256, chunk_count, origin, created_at
         FROM lore_docs WHERE id = ?1",
        params![id],
        |r| {
            Ok(LoreDoc {
                id: r.get(0)?,
                world_id: r.get(1)?,
                title: r.get(2)?,
                rel_path: r.get(3)?,
                sha256: r.get(4)?,
                chunk_count: r.get(5)?,
                origin: r.get(6)?,
                created_at: r.get(7)?,
            })
        },
    )
    .map_err(|e| anyhow!("设定文档不存在: {e}"))
}

pub fn doc_abs_path(state: &AppState, doc: &LoreDoc) -> PathBuf {
    world_dir(state, &doc.world_id).join(&doc.rel_path)
}

pub fn read_doc(state: &AppState, id: &str) -> Result<(LoreDoc, String, String)> {
    let conn = state.db.lock()?;
    let doc = doc_by_id(&conn, id)?;
    drop(conn);
    let path = doc_abs_path(state, &doc);
    let content = fs::read_to_string(&path).unwrap_or_default();
    let current_sha = sha256_hex(content.as_bytes());
    Ok((doc, content, current_sha))
}

/// Atomic save. `base_sha` is the sha the editor saw when it opened the file.
/// If the file changed outside the app in the meantime, return a stable conflict
/// error carrying the external content so the UI can offer a three-way choice.
pub fn save_doc(
    state: &AppState,
    id: &str,
    content: &str,
    base_sha: &str,
    force: bool,
) -> Result<String> {
    let conn = state.db.lock()?;
    let doc = doc_by_id(&conn, id)?;
    drop(conn);

    let path = doc_abs_path(state, &doc);
    let on_disk = fs::read_to_string(&path).unwrap_or_default();
    let disk_sha = sha256_hex(on_disk.as_bytes());

    if !force && disk_sha != base_sha {
        // Back up the external version before the user decides to overwrite.
        if !on_disk.is_empty() {
            let backup = state
                .backups_dir
                .join(format!("lore-external-{}-{}.md", id, chrono::Local::now().format("%Y%m%d-%H%M%S")));
            let _ = fs::write(backup, &on_disk);
        }
        return Err(anyhow!(
            "{CONFLICT_PREFIX}{}",
            serde_json::json!({ "sha256": disk_sha, "content": on_disk }).to_string()
        ));
    }

    // Atomic write: temp file + rename, so the watcher never sees a partial file.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("md.tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, &path)?;

    let new_sha = sha256_hex(content.as_bytes());
    let conn = state.db.lock()?;
    reslice(&conn, &doc.id, content)?;
    conn.execute(
        "UPDATE lore_docs SET sha256 = ?1, chunk_count = (SELECT COUNT(*) FROM lore_chunks WHERE doc_id = ?2) WHERE id = ?2",
        params![new_sha, doc.id],
    )?;
    drop(conn);
    Ok(new_sha)
}

pub fn create_doc(state: &AppState, world_id: &str, title: &str) -> Result<LoreDoc> {
    let safe = slugify(title);
    let rel = format!("lore/{safe}.md");
    let abs = world_dir(state, world_id).join(&rel);
    if abs.exists() {
        return Err(anyhow!("同名设定文档已存在: {title}"));
    }
    fs::create_dir_all(abs.parent().unwrap_or(Path::new(".")))?;
    let body = format!("# {title}\n\n");
    fs::write(&abs, &body)?;

    let id = AppState::new_id();
    let now = AppState::now();
    let sha = sha256_hex(body.as_bytes());
    let conn = state.db.lock()?;
    conn.execute(
        "INSERT INTO lore_docs (id, world_id, title, rel_path, sha256, chunk_count, origin, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, 'manual', ?6)",
        params![id, world_id, title, rel, sha, now],
    )?;
    drop(conn);
    reslice_and_update(state, &id, &body)?;
    let conn = state.db.lock()?;
    conn.execute(
        "INSERT OR IGNORE INTO lore_cards
            (doc_id, world_id, category_id, tags, summary, char_count, updated_at)
         VALUES (?1, ?2, NULL, '[]', '', 0, ?3)",
        params![id, world_id, now],
    )?;
    doc_by_id(&conn, &id)
}

pub fn delete_doc(state: &AppState, id: &str) -> Result<()> {
    let conn = state.db.lock()?;
    let doc = doc_by_id(&conn, id)?;
    drop(conn);
    let path = doc_abs_path(state, &doc);
    if path.exists() {
        let backup = state
            .backups_dir
            .join(format!("lore-deleted-{}-{}.md", id, chrono::Local::now().format("%Y%m%d-%H%M%S")));
        let _ = fs::copy(&path, backup);
        fs::remove_file(&path)?;
    }
    let conn = state.db.lock()?;
    // Delete FTS/vector rows first, while the chunk ids still exist.
    conn.execute(
        "DELETE FROM lore_fts WHERE chunk_id IN (SELECT id FROM lore_chunks WHERE doc_id = ?1)",
        params![id],
    )?;
    conn.execute(
        "DELETE FROM chunk_vectors WHERE chunk_id IN (SELECT id FROM lore_chunks WHERE doc_id = ?1)",
        params![id],
    )?;
    conn.execute("DELETE FROM lore_chunks WHERE doc_id = ?1", params![id])?;
    conn.execute("DELETE FROM lore_docs WHERE id = ?1", params![id])?;
    Ok(())
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
            out.push(c);
        }
    }
    let trimmed = out.trim().replace(' ', "-");
    if trimmed.is_empty() {
        "未命名".into()
    } else {
        trimmed
    }
}

// ---------- Minimal chunker (MVP) ----------

const MAX_CHUNK_CHARS: usize = 600;

/// Split by markdown headings, then by length when a section is too long.
/// Returns (heading_path, content) pairs. Empty sections are skipped.
pub fn chunk_markdown(content: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut headings: Vec<(usize, String)> = Vec::new();
    let mut buf = String::new();

    let flush = |buf: &mut String, headings: &Vec<(usize, String)>, out: &mut Vec<(String, String)>| {
        let text = buf.trim().to_string();
        if text.is_empty() {
            return;
        }
        let path = headings
            .iter()
            .map(|(_, h)| h.as_str())
            .collect::<Vec<_>>()
            .join(" > ");
        for piece in split_long(&text) {
            out.push((path.clone(), piece));
        }
        buf.clear();
    };

    for line in content.lines() {
        let trimmed = line.trim_end();
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if level > 0 && level <= 6 && trimmed[level..].starts_with(' ') {
            flush(&mut buf, &headings, &mut result);
            let title = trimmed[level..].trim().to_string();
            while headings.last().map(|(l, _)| *l >= level).unwrap_or(false) {
                headings.pop();
            }
            headings.push((level, title));
            continue;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    flush(&mut buf, &headings, &mut result);
    result
}

fn split_long(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_CHUNK_CHARS {
        return vec![text.to_string()];
    }
    // Prefer paragraph boundaries, then sentence endings; never cut mid-sentence.
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let end = (start + MAX_CHUNK_CHARS).min(chars.len());
        if end == chars.len() {
            out.push(chars[start..].iter().collect());
            break;
        }
        let window: String = chars[start..end].iter().collect();
        let cut = window
            .rfind("\n\n")
            .or_else(|| window.rfind('。'))
            .or_else(|| window.rfind('！'))
            .or_else(|| window.rfind('？'))
            .or_else(|| window.rfind('\n'))
            .map(|b| {
                // byte offset -> char offset
                chars[start..end]
                    .iter()
                    .collect::<String>()
                    .get(..b)
                    .map(|s| s.chars().count())
                    .unwrap_or(0)
            })
            .filter(|c| *c > MAX_CHUNK_CHARS / 3)
            .unwrap_or(MAX_CHUNK_CHARS);
        let stop = (start + cut).min(chars.len());
        out.push(chars[start..stop].iter().collect::<String>().trim().to_string());
        start = stop;
    }
    out.into_iter().filter(|s| !s.trim().is_empty()).collect()
}

pub fn rough_token_est(text: &str) -> usize {
    let mut cjk = 0f64;
    let mut ascii = 0f64;
    let mut other = 0f64;
    for c in text.chars() {
        if ('\u{4e00}'..='\u{9fff}').contains(&c) {
            cjk += 1.0;
        } else if c.is_ascii() {
            ascii += 1.0;
        } else {
            other += 1.0;
        }
    }
    (cjk + ascii * 0.28 + other * 0.5).ceil() as usize
}

/// Replace all chunks of a document. In MVP the retrieval tables stay empty;
/// phase 2 will additionally refresh FTS/vectors here when retrieval is enabled.
pub fn reslice(conn: &Connection, doc_id: &str, content: &str) -> Result<()> {
    conn.execute("DELETE FROM lore_chunks WHERE doc_id = ?1", params![doc_id])?;
    for (idx, (heading, text)) in chunk_markdown(content).into_iter().enumerate() {
        let sha = sha256_hex(text.as_bytes());
        conn.execute(
            "INSERT INTO lore_chunks (doc_id, ordinal, heading_path, content, char_len, token_est, sha256)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                doc_id,
                idx as i64,
                heading,
                text,
                text.chars().count() as i64,
                rough_token_est(&text) as i64,
                sha
            ],
        )?;
    }
    // New chunks need search tokens; old rows for this doc are already gone.
    let _ = crate::retrieval::sync_fts(conn);
    Ok(())
}

pub fn reslice_and_update(state: &AppState, doc_id: &str, content: &str) -> Result<()> {
    let conn = state.db.lock()?;
    reslice(&conn, doc_id, content)?;
    conn.execute(
        "UPDATE lore_docs SET sha256 = ?1, chunk_count = (SELECT COUNT(*) FROM lore_chunks WHERE doc_id = ?2) WHERE id = ?2",
        params![sha256_hex(content.as_bytes()), doc_id],
    )?;
    Ok(())
}

/// Called by the watcher when an external editor changed a lore file.
/// Idempotent: if the on-disk sha already matches the DB, it is a no-op
/// (this also makes our own atomic saves harmless).
pub fn sync_external(
    state: &AppState,
    world_id: &str,
    rel_path: &str,
) -> Result<bool> {
    let conn = state.db.lock()?;
    let found: Option<(String, String)> = conn
        .query_row(
            "SELECT id, sha256 FROM lore_docs WHERE world_id = ?1 AND rel_path = ?2",
            params![world_id, rel_path],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    drop(conn);
    let Some((doc_id, db_sha)) = found else { return Ok(false) };
    let abs = world_dir(state, world_id).join(rel_path);
    let content = fs::read_to_string(&abs).unwrap_or_default();
    let sha = sha256_hex(content.as_bytes());
    if sha == db_sha {
        return Ok(false);
    }
    reslice_and_update(state, &doc_id, &content)?;
    Ok(true)
}
