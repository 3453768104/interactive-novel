//! 正文全文检索（回合级），与设定检索互不干扰。
//!
//! 复用 `retrieval::tokenize` 的中文分词写进 FTS5，再用 bm25() 排序。
//! 索引在启动时补齐、每回合应用后增量更新。

use crate::error::{cmd_err, CmdResult};
use crate::state::AppState;
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct StoryHit {
    pub passage_id: String,
    pub turn_index: i64,
    pub snippet: String,
    pub score: f64,
}

fn index_text(content: &str) -> String {
    crate::retrieval::tokenize(content).join(" ")
}

/// 补齐缺失的索引、清掉已删除回合的孤儿行。幂等。
pub fn sync_passage_fts(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM passage_fts WHERE passage_id NOT IN (SELECT id FROM passages)",
        [],
    )?;
    let mut stmt = conn.prepare(
        "SELECT p.id, p.content_md FROM passages p
         WHERE p.parse_status != 'pending'
           AND p.id NOT IN (SELECT passage_id FROM passage_fts)",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    let mut n = 0usize;
    for (id, content) in rows {
        if content.trim().is_empty() {
            continue;
        }
        conn.execute(
            "INSERT INTO passage_fts (tokens, passage_id) VALUES (?1, ?2)",
            params![index_text(&content), id],
        )?;
        n += 1;
    }
    Ok(n)
}

/// 单回合重新索引（内容变化时调用）。
pub fn reindex_passage(conn: &Connection, passage_id: &str) {
    let _ = conn.execute(
        "DELETE FROM passage_fts WHERE passage_id = ?1",
        params![passage_id],
    );
    let content: Option<String> = conn
        .query_row(
            "SELECT content_md FROM passages WHERE id = ?1",
            params![passage_id],
            |r| r.get(0),
        )
        .ok();
    if let Some(text) = content {
        if !text.trim().is_empty() {
            let _ = conn.execute(
                "INSERT INTO passage_fts (tokens, passage_id) VALUES (?1, ?2)",
                params![index_text(&text), passage_id],
            );
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn search_story(
    state: State<'_, AppState>,
    story_id: String,
    query: String,
    limit: Option<usize>,
) -> CmdResult<Vec<StoryHit>> {
    let st = &*state;
    let terms = crate::retrieval::tokenize(&query);
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let q = terms
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ");
    let limit = limit.unwrap_or(30).clamp(1, 200);

    let conn = st.db.lock().map_err(cmd_err)?;
    let mut stmt = conn
        .prepare(
            "SELECT f.passage_id, p.turn_index, p.content_md, bm25(passage_fts) AS score
             FROM passage_fts f
             JOIN passages p ON p.id = f.passage_id
             WHERE passage_fts MATCH ?1 AND p.story_id = ?2
             ORDER BY score LIMIT ?3",
        )
        .map_err(cmd_err)?;
    let rows: Vec<(String, i64, String, f64)> = stmt
        .query_map(params![q, story_id, limit as i64], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .map_err(cmd_err)?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows
        .into_iter()
        .map(|(passage_id, turn_index, content, score)| StoryHit {
            passage_id,
            turn_index,
            snippet: snippet_around(&content, &terms, 48),
            score,
        })
        .collect())
}

/// 截取命中词前后的一段文本，给搜索结果当预览。
fn snippet_around(content: &str, terms: &[String], radius: usize) -> String {
    let flat = content.replace('\n', " ");
    let chars: Vec<char> = flat.chars().collect();
    let pos = terms
        .iter()
        .filter_map(|t| {
            let needle: Vec<char> = t.chars().collect();
            if needle.is_empty() || needle.len() > chars.len() {
                return None;
            }
            chars
                .windows(needle.len())
                .position(|w| w == needle.as_slice())
        })
        .min()
        .unwrap_or(0);
    let start = pos.saturating_sub(radius);
    let end = (pos + radius).min(chars.len());
    let mut s: String = chars[start..end].iter().collect();
    if start > 0 {
        s.insert(0, '…');
    }
    if end < chars.len() {
        s.push('…');
    }
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_centers_on_the_first_hit() {
        let content = "圣魂村的清晨来得比别处早一些。唐三站在老槐树底下，等着武魂觉醒。";
        let s = snippet_around(content, &["唐三".to_string()], 10);
        assert!(s.contains("唐三"), "预览必须包含命中词: {s}");
        assert!(s.starts_with('…') || s.len() <= content.chars().count());
    }

    #[test]
    fn passage_index_finds_the_right_turn() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("db/schema.sql")).unwrap();
        conn.execute_batch(
            "INSERT INTO worlds (id,name,description,created_at,updated_at,mirror_path)
               VALUES ('w','w','','t','t','w');
             INSERT INTO stories (id,world_id,name,engine_mode,created_at,updated_at,mirror_path)
               VALUES ('s','w','s','local','t','t','w/s');
             INSERT INTO passages (id,story_id,turn_index,trigger_action_md,content_md,parse_status,engine_mode,created_at)
               VALUES ('p1','s',0,'a','唐三举起了昊天锤。','ok','local','t'),
                      ('p2','s',1,'b','小舞在树林里等他。','ok','local','t');",
        )
        .unwrap();
        sync_passage_fts(&conn).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM passage_fts f JOIN passages p ON p.id = f.passage_id
                 WHERE passage_fts MATCH '\"昊天\"'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }
}
