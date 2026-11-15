//! 章节：按字数 + 场景切换把回合聚成章，允许改名（只存覆盖值）。

use crate::error::{cmd_err, CmdResult};
use crate::state::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub ordinal: i64,
    /// 1-based，给界面显示
    pub index: i64,
    pub title: String,
    pub start_turn: i64,
    pub end_turn: i64,
    pub chars: i64,
    pub turns: i64,
    /// 本章第一句，目录里当摘要用
    pub lead: String,
}

fn lead_of(content: &str) -> String {
    let first = content
        .split('\n')
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("〔暗场〕"))
        .unwrap_or("");
    let clean = first
        .trim_start_matches('◇')
        .trim_start_matches('◆')
        .trim();
    clean.chars().take(36).collect()
}

/// 默认 3000 字一章；段落以「◇」开场的场景切换也可以提前分章（≥1200 字时）。
pub fn derive_chapters(
    passages: &[(i64, String)],
    target_chars: usize,
) -> Vec<(i64, i64, i64, i64, String)> {
    // (ordinal, start_turn, end_turn, chars, lead)
    let target = target_chars.max(300);
    let mut out = Vec::new();
    if passages.is_empty() {
        return out;
    }
    let mut start_turn = passages[0].0;
    let mut last_turn = passages[0].0;
    let mut chars = 0i64;
    let mut lead = lead_of(&passages[0].1);

    for (idx, (turn, content)) in passages.iter().enumerate() {
        let is_scene_start = content.trim_start().starts_with('◇') || content.trim_start().starts_with('◆');
        // 场景切换 + 已经写得够长 → 在上一段收尾处断章
        if idx > 0 && is_scene_start && chars as usize >= target / 2 {
            out.push((out.len() as i64, start_turn, last_turn, chars, lead.clone()));
            start_turn = *turn;
            chars = 0;
            lead = lead_of(content);
        }
        chars += content.chars().count() as i64;
        last_turn = *turn;
        if chars as usize >= target {
            out.push((out.len() as i64, start_turn, last_turn, chars, lead.clone()));
            chars = 0;
            if let Some((next_turn, next_content)) = passages.get(idx + 1) {
                start_turn = *next_turn;
                lead = lead_of(next_content);
            }
        }
    }
    if chars > 0 {
        out.push((out.len() as i64, start_turn, last_turn, chars, lead));
    }
    out
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_chapters(
    state: State<'_, AppState>,
    story_id: String,
    target_chars: Option<usize>,
) -> CmdResult<Vec<Chapter>> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let mut stmt = conn
        .prepare(
            "SELECT turn_index, content_md FROM passages
             WHERE story_id = ?1 AND parse_status != 'pending' ORDER BY turn_index",
        )
        .map_err(cmd_err)?;
    let passages: Vec<(i64, String)> = stmt
        .query_map(params![story_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(cmd_err)?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let mut overrides: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    if let Ok(mut s) = conn.prepare("SELECT ordinal, title FROM story_chapters WHERE story_id = ?1") {
        if let Ok(it) = s.query_map(params![story_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        }) {
            for (o, t) in it.flatten() {
                if !t.trim().is_empty() {
                    overrides.insert(o, t);
                }
            }
        }
    }
    drop(conn);

    let derived = derive_chapters(&passages, target_chars.unwrap_or(3000));
    Ok(derived
        .into_iter()
        .map(|(ordinal, start_turn, end_turn, chars, lead)| Chapter {
            ordinal,
            index: ordinal + 1,
            title: overrides
                .get(&ordinal)
                .cloned()
                .unwrap_or_else(|| format!("第 {} 章", ordinal + 1)),
            start_turn,
            end_turn,
            chars,
            turns: end_turn - start_turn + 1,
            lead,
        })
        .collect())
}

#[tauri::command(rename_all = "snake_case")]
pub fn rename_chapter(
    state: State<'_, AppState>,
    story_id: String,
    ordinal: i64,
    title: String,
) -> CmdResult<()> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    conn.execute(
        "INSERT INTO story_chapters (story_id, ordinal, title, updated_at) VALUES (?1,?2,?3,?4)
         ON CONFLICT(story_id, ordinal) DO UPDATE SET title = excluded.title,
             updated_at = excluded.updated_at",
        params![story_id, ordinal, title.trim(), AppState::now()],
    )
    .map_err(cmd_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(turn: i64, text: &str) -> (i64, String) {
        (turn, text.to_string())
    }

    #[test]
    fn splits_by_length() {
        let passages = vec![p(0, "甲"), p(1, "乙"), p(2, "丙"), p(3, "丁")];
        let chapters = derive_chapters(&passages, 300);
        assert_eq!(chapters.len(), 1, "太短的内容不该被切成多章");
        assert_eq!(chapters[0].2, 3, "最后一章的 end_turn 是最后一段");

        let long = vec![p(0, &"字".repeat(400)), p(1, &"字".repeat(400))];
        let chapters = derive_chapters(&long, 300);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].1, 0);
        assert_eq!(chapters[1].1, 1);
    }

    #[test]
    fn scene_shift_can_break_earlier() {
        let long = vec![p(0, &"字".repeat(200)), p(1, "◇ 与此同时，另一个地方")];
        let chapters = derive_chapters(&long, 300);
        assert_eq!(chapters.len(), 2, "场景切换且已过半，可以在切换处断章");
        assert_eq!(chapters[0].2, 0);
    }
}
