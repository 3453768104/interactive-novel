//! 设定集 AI 辅助创作：给一段要求，生成 / 补全 / 扩写一张设定卡。
//!
//! 设计上只做两件事：**拼提示词**与**写回卡片**。生成结果永远先落到可编辑的
//! 预览框里，由用户确认后才落库——不做"AI 直接改你的设定"。

use crate::domain::LoreDoc;
use crate::engine;
use crate::error::{cmd_err, CmdResult};
use crate::lore;
use crate::state::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Clone, Deserialize)]
pub struct LoreAiRequest {
    pub world_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub brief: String,
    #[serde(default)]
    pub category_id: Option<String>,
    /// 扩写模式：要扩写哪张卡
    #[serde(default)]
    pub expand_doc_id: Option<String>,
    /// short | medium | long
    #[serde(default)]
    pub length: Option<String>,
    /// 考据 / 通俗 / 热血
    #[serde(default)]
    pub tone: Option<String>,
    /// local | api | paste
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoreAiResult {
    pub prompt: String,
    pub text: String,
    pub mode: String,
}

fn world_context(state: &AppState, world_id: &str) -> (String, String, Vec<String>, Vec<String>) {
    let name: String = state
        .db
        .lock()
        .ok()
        .and_then(|c| {
            c.query_row("SELECT name FROM worlds WHERE id = ?1", params![world_id], |r| {
                r.get(0)
            })
            .ok()
        })
        .unwrap_or_default();
    let bg: String = state
        .db
        .lock()
        .ok()
        .and_then(|c| {
            c.query_row(
                "SELECT content_md FROM world_event_background WHERE world_id = ?1",
                params![world_id],
                |r| r.get(0),
            )
            .ok()
        })
        .unwrap_or_default();
    let mut categories: Vec<String> = Vec::new();
    let mut titles: Vec<String> = Vec::new();
    if let Ok(conn) = state.db.lock() {
        if let Ok(mut stmt) =
            conn.prepare("SELECT name FROM lore_categories WHERE world_id = ?1 ORDER BY sort_order")
        {
            if let Ok(it) = stmt.query_map(params![world_id], |r| r.get::<_, String>(0)) {
                categories = it.flatten().collect();
            }
        }
        if let Ok(mut stmt) = conn.prepare(
            "SELECT d.title FROM lore_docs d WHERE d.world_id = ?1 ORDER BY d.created_at LIMIT 60",
        ) {
            if let Ok(it) = stmt.query_map(params![world_id], |r| r.get::<_, String>(0)) {
                titles = it.flatten().collect();
            }
        }
    }
    (name, bg, categories, titles)
}

fn existing_card(state: &AppState, doc_id: &str) -> Option<(String, String)> {
    lore::read_doc(state, doc_id).ok().map(|(d, c, _)| (d.title, c))
}

/// 纯函数式地拼提示词：也用于粘贴模式（用户自己复制到网页 AI）。
pub fn build_prompt(state: &AppState, req: &LoreAiRequest) -> Result<String, String> {
    let (world, bg, cats, titles) = world_context(state, &req.world_id);
    if world.is_empty() {
        return Err("世界观不存在".into());
    }
    let expanding = req.expand_doc_id.as_deref().filter(|s| !s.is_empty());
    if expanding.is_none() && req.title.trim().is_empty() {
        return Err("新建卡片时标题不能为空".into());
    }

    let length = match req.length.as_deref().unwrap_or("medium") {
        "short" => "300–500 字",
        "long" => "1000–1500 字",
        _ => "600–900 字",
    };
    let tone = match req.tone.as_deref().unwrap_or("canon") {
        "plain" => "通俗易读，像给朋友讲设定",
        "epic" => "热血、有画面感，但仍然克制",
        _ => "考据严谨，术语前后一致",
    };

    let mut p = String::new();
    p.push_str("你是中文小说的设定编辑，只输出 Markdown 正文。不要客套话、不要代码围栏、不要解释你的思路。\n\n");
    p.push_str(&format!("【世界观】{world}\n"));
    if !bg.trim().is_empty() {
        let trimmed: String = bg.chars().take(1200).collect();
        p.push_str(&format!("【既有背景】\n{trimmed}\n"));
    }
    if !cats.is_empty() {
        p.push_str(&format!("【已有分类】{}\n", cats.join("、")));
    }
    if !titles.is_empty() {
        let head: Vec<&str> = titles.iter().take(40).map(|s| s.as_str()).collect();
        p.push_str(&format!("【已有条目】{}\n", head.join("、")));
    }

    match expanding {
        Some(doc_id) => {
            if let Some((title, content)) = existing_card(state, doc_id) {
                let body: String = content.chars().take(3000).collect();
                p.push_str(&format!("\n【待扩写的条目】《{title}》\n{body}\n"));
                p.push_str("\n【本次任务】在保留原有全部信息的前提下扩写这张卡：补细节、补因果、补可用的具体名词。");
            } else {
                return Err("要扩写的卡片不存在".into());
            }
        }
        None => {
            p.push_str(&format!("\n【本次任务】新写一张设定卡，标题《{}》。", req.title.trim()));
            if let Some(cid) = req.category_id.as_deref() {
                if let Ok(conn) = state.db.lock() {
                    if let Ok(name) = conn.query_row(
                        "SELECT name FROM lore_categories WHERE id = ?1",
                        params![cid],
                        |r| r.get::<_, String>(0),
                    ) {
                        p.push_str(&format!("\n所属分类：{name}（内容要与该分类的口径一致）。"));
                    }
                }
            }
        }
    }
    if !req.brief.trim().is_empty() {
        p.push_str(&format!("\n作者的要求：{}", req.brief.trim()));
    }
    p.push_str(&format!("\n篇幅：{length}。语气：{tone}。"));
    p.push_str(
        "\n\n【格式】直接给正文；可用 ### 小标题、列表、粗体；不要写一级/二级标题（标题已单独保存）；\
         不要出现「以下是」「希望你喜欢」这类话，也不要重复题目。",
    );
    Ok(p)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn lore_ai_generate(
    app: AppHandle,
    state: State<'_, AppState>,
    req: LoreAiRequest,
) -> CmdResult<LoreAiResult> {
    let st = &*state;
    let prompt = build_prompt(st, &req).map_err(|e| e)?;
    let mode = req.mode.clone().unwrap_or_else(|| "local".into());
    if mode == "paste" {
        return Ok(LoreAiResult { prompt, text: String::new(), mode });
    }
    let profile = engine::profile_for(st, &mode)
        .map_err(cmd_err)?
        .ok_or_else(|| format!("engines.yaml 里没有 {mode} 模式的配置"))?;
    let max_tokens = match req.length.as_deref().unwrap_or("medium") {
        "short" => 700,
        "long" => 2200,
        _ => 1300,
    };
    drop(state);
    let raw = engine::complete(&profile, &prompt, max_tokens, 0.8)
        .await
        .map_err(|e| e.to_string())?;
    let text = tidy(&raw);
    if text.is_empty() {
        return Err("模型返回了空内容，换个要求或降低篇幅再试".into());
    }
    crate::diagnostics::log(
        &app.state::<AppState>(),
        "lore_ai",
        json!({ "mode": mode, "title": req.title, "chars": text.chars().count() }),
    );
    Ok(LoreAiResult { prompt, text, mode })
}

/// 去掉代码围栏与"以下是"之类的开场，最多留 8000 字。
fn tidy(raw: &str) -> String {
    let mut out = String::new();
    for line in raw.replace("\r\n", "\n").lines() {
        if line.trim_start().starts_with("```") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    let mut text = out.trim().to_string();
    for prefix in ["以下是", "这是", "好的，", "好的,", "Sure", "Here"] {
        if text.starts_with(prefix) {
            if let Some(pos) = text.find('\n') {
                text = text[pos + 1..].trim_start().to_string();
            }
            break;
        }
    }
    text.chars().take(8000).collect()
}

/// 把 AI 结果落成一张新卡：写 .md、切片段、建卡元数据，一次完成。
#[tauri::command(rename_all = "snake_case")]
pub fn create_lore_card(
    state: State<'_, AppState>,
    world_id: String,
    category_id: Option<String>,
    title: String,
    content: String,
) -> CmdResult<String> {
    let st = &*state;
    let title = title.trim();
    if title.is_empty() {
        return Err("标题不能为空".into());
    }
    if content.trim().is_empty() {
        return Err("正文为空，没什么可保存的".into());
    }
    // 同名文件会被 create_doc 拒绝；自动加序号，避免打断用户。
    let mut final_title = title.to_string();
    let mut doc: Option<LoreDoc> = None;
    for n in 0..20 {
        let candidate = if n == 0 {
            title.to_string()
        } else {
            format!("{title}（{}）", n + 1)
        };
        match lore::create_doc(st, &world_id, &candidate) {
            Ok(d) => {
                final_title = candidate;
                doc = Some(d);
                break;
            }
            Err(e) => {
                if n == 19 {
                    return Err(e.to_string());
                }
            }
        }
    }
    let doc = doc.ok_or_else(|| "创建卡片失败".to_string())?;
    let body = format!("# {final_title}\n\n{}\n", content.trim());
    lore::save_doc(st, &doc.id, &body, &doc.sha256, true).map_err(cmd_err)?;

    let summary: String = content
        .trim()
        .trim_start_matches('#')
        .trim()
        .chars()
        .take(80)
        .collect();
    if let Ok(conn) = st.db.lock() {
        let _ = conn.execute(
            "UPDATE lore_cards SET category_id = ?1, summary = ?2, char_count = ?3, updated_at = ?4
             WHERE doc_id = ?5",
            params![
                category_id,
                summary.replace('\n', " "),
                content.chars().count() as i64,
                AppState::now(),
                doc.id
            ],
        );
    }
    Ok(doc.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tidy_strips_fences_and_preamble() {
        let raw = "```markdown\n以下是正文：\n# 标题\n### 一、来历\n内容。\n```";
        let out = tidy(raw);
        assert!(!out.contains("```"));
        assert!(!out.contains("以下是"));
        assert!(out.contains("### 一、来历"));
    }
}
