//! Bulk import of a single Markdown "setting book" into the lore library.
//!
//! Structure assumed (this is how most hand-written 设定集 are organised):
//!
//! ```text
//! # 斗罗大陆设定集        <- document title
//! ## 目录                 <- skipped
//! # 第一编·世界与地理      <- becomes a category
//! ## 斗罗大陆（主世界）     <- becomes one card (one .md file)
//! ```
//!
//! H1 = category, H2 = card. Anything before the first H1 (front matter,
//! table of contents) is ignored. Import is idempotent by card title, so
//! re-running it after editing the source only adds what is missing.

use crate::lore;
use crate::state::AppState;
use anyhow::{anyhow, Result};
use rusqlite::params;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub doc_title: String,
    pub categories_created: usize,
    pub cards_created: usize,
    pub cards_skipped: usize,
    pub samples: Vec<String>,
}

struct Card {
    part: String,
    title: String,
    body: String,
}

/// Split the book into (document title, cards). Never fails: a malformed file
/// simply yields fewer cards.
fn parse_cards(md: &str) -> (String, Vec<Card>) {
    let mut doc_title = String::new();
    let mut part = String::new();
    let mut cards: Vec<Card> = Vec::new();
    let mut cur: Option<Card> = None;
    let mut in_toc = false;
    let mut seen_h1 = false;

    let flush = |cur: &mut Option<Card>, out: &mut Vec<Card>| {
        if let Some(card) = cur.take() {
            if !card.body.trim().is_empty() {
                out.push(card);
            }
        }
    };

    for raw in md.lines() {
        let line = raw.trim_end();
        if let Some(rest) = line.strip_prefix("## ") {
            flush(&mut cur, &mut cards);
            let title = rest.trim().to_string();
            if title.contains("目录") {
                in_toc = true;
                continue;
            }
            in_toc = false;
            cur = Some(Card { part: part.clone(), title, body: String::new() });
            continue;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            flush(&mut cur, &mut cards);
            in_toc = false;
            let title = rest.trim().to_string();
            if !seen_h1 {
                doc_title = title.clone();
                seen_h1 = true;
            }
            part = title;
            continue;
        }
        if in_toc {
            continue;
        }
        if let Some(card) = cur.as_mut() {
            card.body.push_str(raw);
            card.body.push('\n');
        }
    }
    flush(&mut cur, &mut cards);
    (doc_title, cards)
}

fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "card".into()
    } else {
        trimmed.chars().take(40).collect()
    }
}

/// First meaningful line, markdown stripped, for the card grid.
fn summary_of(body: &str) -> String {
    let line = body
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && !l.starts_with("---") && !l.starts_with("##"))
        .unwrap_or("");
    let cleaned = line
        .replace("**", "")
        .replace("__", "")
        .replace("【", "")
        .replace("】", "");
    cleaned.chars().take(70).collect()
}

/// Import every H2 section as its own card, grouped under H1 categories.
pub fn import_bundle(state: &AppState, world_id: &str, md: &str) -> Result<ImportReport> {
    let (doc_title, cards) = parse_cards(md);
    if cards.is_empty() {
        return Err(anyhow!("没有解析到任何 「## 标题」 条目，无法导入"));
    }
    fs::create_dir_all(lore::lore_dir(state, world_id))?;

    let now = AppState::now();
    let conn = state.db.lock()?;
    // One transaction: 400+ documents otherwise cost 400+ fsyncs.
    let tx = conn.unchecked_transaction()?;

    let mut existing: Vec<String> = {
        let mut stmt = tx.prepare("SELECT title FROM lore_docs WHERE world_id = ?1")?;
        let rows = stmt.query_map(params![world_id], |r| r.get::<_, String>(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };
    existing.sort();

    let mut report = ImportReport { doc_title, ..Default::default() };
    let mut cat_ids: HashMap<String, String> = HashMap::new();
    let mut next_order: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) FROM lore_categories WHERE world_id = ?1",
            params![world_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    for (idx, card) in cards.iter().enumerate() {
        if existing.iter().any(|t| t == &card.title) {
            report.cards_skipped += 1;
            continue;
        }
        let part = if card.part.trim().is_empty() {
            report.doc_title.clone()
        } else {
            card.part.clone()
        };
        let category_id = match cat_ids.get(&part) {
            Some(id) => id.clone(),
            None => {
                let found: Option<String> = tx
                    .query_row(
                        "SELECT id FROM lore_categories WHERE world_id = ?1 AND name = ?2",
                        params![world_id, part],
                        |r| r.get(0),
                    )
                    .ok();
                let id = match found {
                    Some(id) => id,
                    None => {
                        let id = AppState::new_id();
                        next_order += 1;
                        tx.execute(
                            "INSERT INTO lore_categories (id, world_id, parent_id, name, sort_order)
                             VALUES (?1, ?2, NULL, ?3, ?4)",
                            params![id, world_id, part, next_order],
                        )?;
                        report.categories_created += 1;
                        id
                    }
                };
                cat_ids.insert(part.clone(), id.clone());
                id
            }
        };

        let doc_id = AppState::new_id();
        let rel = format!("lore/{:03}-{}.md", idx + 1, slug(&card.title));
        let body_md = format!("# {}\n\n{}\n", card.title, card.body.trim());
        fs::write(lore::lore_dir(state, world_id).join(rel.trim_start_matches("lore/")), &body_md)?;
        let sha = lore::sha256_hex(body_md.as_bytes());

        tx.execute(
            "INSERT INTO lore_docs (id, world_id, title, rel_path, sha256, chunk_count, origin, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, 'import', ?6)",
            params![doc_id, world_id, card.title, rel, sha, now],
        )?;
        lore::reslice(&tx, &doc_id, &body_md)?;
        tx.execute(
            "UPDATE lore_docs SET chunk_count = (SELECT COUNT(*) FROM lore_chunks WHERE doc_id = ?1)
             WHERE id = ?1",
            params![doc_id],
        )?;
        tx.execute(
            "INSERT INTO lore_cards (doc_id, world_id, category_id, tags, summary, char_count, updated_at)
             VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?6)",
            params![
                doc_id,
                world_id,
                category_id,
                summary_of(&card.body),
                body_md.chars().count() as i64,
                now
            ],
        )?;
        report.cards_created += 1;
        if report.samples.len() < 3 {
            report.samples.push(card.title.clone());
        }
        existing.push(card.title.clone());
    }

    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# 测试设定集

> 序言会被忽略

## 目录
- 一篇

# 第一编·地理
## 主城
主城是**首都**，人口百万。

## 森林
北方森林。

# 第二编·人物
## 主角
他姓唐。
"#;

    #[test]
    fn parses_h1_as_category_and_h2_as_card() {
        let (title, cards) = parse_cards(SAMPLE);
        assert_eq!(title, "测试设定集");
        assert_eq!(cards.len(), 3, "目录 must not become a card");
        assert_eq!(cards[0].part, "第一编·地理");
        assert_eq!(cards[0].title, "主城");
        assert!(cards[0].body.contains("首都"));
        assert_eq!(cards[2].part, "第二编·人物");
        assert!(!cards[0].body.contains("目录"));
    }

    #[test]
    fn summary_strips_markup_and_is_short() {
        let s = summary_of("\n\n主城是**首都**，人口百万。\n\n更多内容");
        assert_eq!(s, "主城是首都，人口百万。");
    }

    #[test]
    fn slug_is_file_safe() {
        assert_eq!(slug("斗罗大陆（主世界）"), "斗罗大陆-主世界");
        assert_eq!(slug("!!!"), "card");
    }
}
