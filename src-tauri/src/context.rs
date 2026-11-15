use crate::domain::{
    AppSettings, BudgetItem, BudgetReport, ContextBundle, EstimatorCfg, Policy, Protagonist, Story,
};
use crate::state::AppState;
use anyhow::{anyhow, Result};
use rusqlite::params;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Resolve a path inside config/, rejecting traversal.
pub fn config_path(state: &AppState, rel: &str) -> Result<PathBuf> {
    let clean = rel.trim_start_matches('/').replace('\\', "/");
    if clean.contains("..") || clean.is_empty() {
        return Err(anyhow!("非法配置路径: {rel}"));
    }
    Ok(state.config_dir.join(clean))
}

pub fn read_config_text(state: &AppState, rel: &str) -> Result<String> {
    Ok(fs::read_to_string(config_path(state, rel)?)?)
}

pub fn write_config_text(state: &AppState, rel: &str, content: &str) -> Result<()> {
    let path = config_path(state, rel)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

fn default_policy_text(mode: &str) -> Option<&'static str> {
    match mode {
        "api" => Some(crate::defaults::POLICY_API),
        "local" => Some(crate::defaults::POLICY_LOCAL),
        "paste" => Some(crate::defaults::POLICY_PASTE),
        _ => None,
    }
}

/// `Some(msg)` when the file on disk is broken. Generation keeps working (it
/// falls back to the shipped policy) but the UI must show this instead of
/// silently pretending everything is fine.
pub fn policy_parse_error(state: &AppState, mode: &str) -> Option<String> {
    let text = read_config_text(state, &format!("policies/{mode}.yaml")).ok()?;
    serde_yaml::from_str::<Policy>(&text)
        .err()
        .map(|e| format!("策略文件 policies/{mode}.yaml 解析失败（本次临时使用出厂配置）: {e}"))
}

pub fn load_policy(state: &AppState, mode: &str) -> Result<Policy> {
    let text = read_config_text(state, &format!("policies/{mode}.yaml"))?;
    match serde_yaml::from_str::<Policy>(&text) {
        Ok(p) => Ok(p),
        Err(e) => {
            // One bad line must never brick the turn pipeline: fall back to the
            // compiled-in policy. `policy_parse_error` surfaces the reason.
            if let Some(fallback) = default_policy_text(mode)
                .and_then(|t| serde_yaml::from_str::<Policy>(t).ok())
            {
                return Ok(fallback);
            }
            Err(anyhow!("策略文件解析失败（已保留上一份可用配置）: {e}"))
        }
    }
}

pub fn read_app_yaml(state: &AppState) -> serde_yaml::Value {
    read_config_text(state, "app.yaml")
        .ok()
        .and_then(|s| serde_yaml::from_str(&s).ok())
        .unwrap_or_else(|| serde_yaml::Value::Mapping(Default::default()))
}

pub fn app_str(state: &AppState, key: &str) -> String {
    read_app_yaml(state)
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Structured read model of app.yaml for the GUI panel. The file stays the
/// single source of truth; this only mirrors it into typed fields.
pub fn app_settings(state: &AppState) -> AppSettings {
    let v = read_app_yaml(state);
    let d = AppSettings::default();
    let default_inject = d.character_inject.clone();
    let default_triggers = d.stop_triggers.clone();
    let usize_of = |key: &str, fallback: usize| {
        v.get(key)
            .and_then(|x| x.as_u64())
            .map(|n| n as usize)
            .unwrap_or(fallback)
    };
    let bool_of = |key: &str, fallback: bool| {
        v.get(key).and_then(|x| x.as_bool()).unwrap_or(fallback)
    };
    AppSettings {
        retrieval_enabled: bool_of("retrieval_enabled", d.retrieval_enabled),
        event_background_soft_cap: usize_of(
            "event_background_soft_cap",
            d.event_background_soft_cap,
        ),
        persona_soft_cap: usize_of("persona_soft_cap", d.persona_soft_cap),
        opening_scene_turns: usize_of("opening_scene_turns", d.opening_scene_turns),
        stop_triggers: v
            .get("stop_triggers")
            .and_then(|x| x.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|i| i.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or(default_triggers),
        max_consecutive_turns: usize_of("max_consecutive_turns", d.max_consecutive_turns),
        analysis_second_pass: bool_of("analysis_second_pass", d.analysis_second_pass),
        character_inject: v
            .get("character_inject")
            .and_then(|x| x.as_str())
            .unwrap_or(&default_inject)
            .to_string(),
        show_scene_markers: bool_of("show_scene_markers", d.show_scene_markers),
    }
}

/// 配置里认得哪些键。用于"你写了个不会生效的项"这种提醒。
const APP_KEYS: &[&str] = &[
    "retrieval_enabled", "event_background_soft_cap", "persona_soft_cap",
    "opening_scene_turns", "stop_triggers", "max_consecutive_turns",
    "analysis_second_pass", "character_inject", "show_scene_markers",
    "fixed_background", "editor_external_hint",
];
const POLICY_KEYS: &[&str] = &[
    "mode", "estimator", "context_window", "max_output_tokens", "modules",
    "retrieval", "throttle", "templates", "output_template",
];
const MODULE_KEYS: &[&str] = &[
    "system_contract", "protagonist", "opening_scene", "state_card",
    "rolling_summary", "characters", "lore_pins", "lore_index", "lore_chunks",
    "recent_passages", "action_choices", "stop_policy", "director_outline", "buffer",
];

/// 拼错的键不会报错，只会"改了没反应"——这里主动指出来。
pub fn config_warnings(state: &AppState) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(text) = read_config_text(state, "app.yaml") {
        if let Ok(serde_yaml::Value::Mapping(map)) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
            for k in map.keys().filter_map(|k| k.as_str()) {
                if !APP_KEYS.contains(&k) {
                    out.push(format!("app.yaml 的 `{k}` 不会被读取（拼写错误或旧版残留）"));
                }
            }
        }
    }
    for mode in ["api", "local", "paste"] {
        let Ok(text) = read_config_text(state, &format!("policies/{mode}.yaml")) else { continue };
        let Ok(serde_yaml::Value::Mapping(map)) = serde_yaml::from_str::<serde_yaml::Value>(&text) else { continue };
        for k in map.keys().filter_map(|k| k.as_str()) {
            if !POLICY_KEYS.contains(&k) {
                out.push(format!("policies/{mode}.yaml 的 `{k}` 不会被读取"));
            }
        }
        if let Some(serde_yaml::Value::Mapping(m)) = map.get("modules") {
            for k in m.keys().filter_map(|k| k.as_str()) {
                if !MODULE_KEYS.contains(&k) {
                    out.push(format!("policies/{mode}.yaml 的模块 `{k}` 不存在，不会被注入"));
                }
            }
        }
    }
    out
}

// ---------- Chinese-aware token estimation ----------

pub fn estimate(text: &str, cfg: &EstimatorCfg) -> usize {
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
    let raw = cjk * cfg.zh_ratio + ascii * cfg.ascii_ratio + other * cfg.other_ratio;
    (raw * cfg.safety_margin).ceil() as usize
}

/// Trim from the FRONT, keeping the most recent tail (oldest text is dropped first).
fn trim_to_budget(text: &str, budget: usize, cfg: &EstimatorCfg) -> (String, bool) {
    if budget == 0 {
        return (String::new(), !text.is_empty());
    }
    if estimate(text, cfg) <= budget {
        return (text.to_string(), false);
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let suffix: String = chars[chars.len() - mid..].iter().collect();
        if estimate(&suffix, cfg) <= budget {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    (chars[chars.len() - lo..].iter().collect(), true)
}

fn render(template: &str, values: &HashMap<String, String>) -> String {
    let mut out = template.to_string();
    for (key, value) in values {
        out = out.replace(&format!("{{{{{key}}}}}"), value);
    }
    let re = regex::Regex::new(r"\{\{[a-zA-Z0-9_]+\}\}").unwrap();
    re.replace_all(&out, "").to_string()
}

fn get_story(conn: &rusqlite::Connection, story_id: &str) -> Result<Story> {
    conn.query_row(
        "SELECT id, world_id, name, engine_mode, created_at, updated_at, mirror_path
         FROM stories WHERE id = ?1",
        params![story_id],
        |r| {
            Ok(Story {
                id: r.get(0)?,
                world_id: r.get(1)?,
                name: r.get(2)?,
                engine_mode: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
                mirror_path: r.get(6)?,
            })
        },
    )
    .map_err(Into::into)
}

fn get_protagonist(conn: &rusqlite::Connection, story_id: &str) -> Result<Protagonist> {
    conn.query_row(
        "SELECT story_id, name, persona_md, opening_scene_md FROM protagonists WHERE story_id = ?1",
        params![story_id],
        |r| {
            Ok(Protagonist {
                story_id: r.get(0)?,
                name: r.get(1)?,
                persona_md: r.get(2)?,
                opening_scene_md: r.get(3)?,
            })
        },
    )
    .map_err(Into::into)
}

fn story_pov(conn: &rusqlite::Connection, story_id: &str) -> String {
    conn.query_row(
        "SELECT pov FROM story_settings WHERE story_id = ?1",
        params![story_id],
        |r| r.get::<_, String>(0),
    )
    .unwrap_or_else(|_| "protagonist".into())
}

pub const STOP_TRIGGER_LABELS: &[(&str, &str)] = &[
    ("dialogue", "对话即将开始，或对方正等着主角回应"),
    ("major_decision", "主角面临重大抉择、需要读者决定"),
    ("new_character", "有新角色登场、值得读者留意"),
    ("scene_change", "场景或时间发生明显转换"),
    ("word_limit", "本段已经写得足够长"),
    ("every_n_turns", "已连写多段，该让读者介入"),
];

fn stop_policy_text(settings: &AppSettings, consecutive: usize, auto_continue: bool) -> String {
    if auto_continue {
        // 看着就行：不打断，也不给选项，让这一场自己走完。
        return format!(
            "【自动连写 · 本回合不要停】\n读者此刻只想看剧情推进，不要向他提问、不要给选项。\n\
             STOP 段写 stop=false；正文自然往下写，约 {} 字，写到一个可以喘口气的地方收住。\n\
             主角的行动由你按人设与当前局势替他决定，不要写「你会怎么做」这类话。\n",
            600
        );
    }
    let mut s = String::from("【何时停下让我行动】\n");
    if settings.stop_triggers.is_empty() {
        s.push_str("本回合请停下，等待读者给出主角的行动。\n");
    } else {
        s.push_str(
            "命中下列任一条件时，在 STOP 段写 stop=true 并给出 3 个主角行动选项；\
             否则写 stop=false 继续往下写（此时不要写 CHOICES 段）：\n",
        );
        for t in &settings.stop_triggers {
            let label = STOP_TRIGGER_LABELS
                .iter()
                .find(|(k, _)| *k == t.as_str())
                .map(|(_, v)| *v)
                .unwrap_or(t.as_str());
            s.push_str(&format!("- {t}：{label}\n"));
        }
    }
    s.push_str(&format!(
        "此前已连续写了 {consecutive} 段；最多连写 {} 段，到上限必须停下。\n",
        settings.max_consecutive_turns
    ));
    s
}

fn consecutive_continue_turns(conn: &rusqlite::Connection, story_id: &str) -> usize {
    let mut stmt = match conn.prepare(
        "SELECT m.stop_decision FROM passage_meta m
         JOIN passages p ON p.id = m.passage_id
         WHERE p.story_id = ?1 ORDER BY p.turn_index DESC LIMIT 30",
    ) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let flags: Vec<i64> = stmt
        .query_map(params![story_id], |r| r.get(0))
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    flags.iter().take_while(|n| **n == 0).count()
}

/// Render the layered state card (world / player) as readable text.
/// Old flat state cards (no world/player keys) still render their scalar keys.
fn layered_state_text(data_json: &str) -> String {
    let v: Value = serde_json::from_str(data_json).unwrap_or_else(|_| serde_json::json!({}));
    let mut out = String::new();
    let render_map = |title: &str, obj: Option<&serde_json::Map<String, Value>>| -> String {
        let mut part = format!("【{title}】\n");
        if let Some(map) = obj {
            for (k, val) in map {
                if k == "delta_log" || val.is_null() {
                    continue;
                }
                let text = match val {
                    Value::String(s) => s.clone(),
                    Value::Array(a) => a
                        .iter()
                        .map(|x| match x {
                            Value::String(s) => s.clone(),
                            Value::Object(o) => o
                                .iter()
                                .map(|(kk, vv)| format!("{kk}={}", display_scalar(vv)))
                                .collect::<Vec<_>>()
                                .join(" "),
                            other => display_scalar(other),
                        })
                        .filter(|x| !x.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join("、"),
                    _ => val.to_string(),
                };
                if !text.trim().is_empty() {
                    part.push_str(&format!("- {k}: {text}\n"));
                }
            }
        }
        part
    };
    let obj = v.as_object();
    let has_layers = obj
        .map(|o| o.contains_key("world") || o.contains_key("player"))
        .unwrap_or(false);
    if has_layers {
        out.push_str(&render_map("世界状态", v.get("world").and_then(|x| x.as_object())));
        out.push('\n');
        out.push_str(&render_map("玩家状态", v.get("player").and_then(|x| x.as_object())));
    } else if let Some(o) = obj {
        out.push_str(&render_map("当前状态", Some(o)));
    }
    if out.trim().is_empty() {
        "（暂无）".into()
    } else {
        out.trim().to_string()
    }
}

/// Characters marked for long-term injection. Only two tiers exist now:
/// passerby (stays out of the prompt) and notable (injected as name + state).
fn characters_text(
    conn: &rusqlite::Connection,
    world_id: &str,
    story_id: &str,
    inject_mode: &str,
) -> String {
    if inject_mode == "none" {
        return String::new();
    }
    let sql = "SELECT name, kind, importance, current_state, first_turn FROM characters
         WHERE world_id = ?1 AND (story_id = ?2 OR story_id IS NULL)
           AND inject = 1 AND importance = 'notable'
         ORDER BY CASE kind WHEN 'preset' THEN 0 ELSE 1 END, name LIMIT 20";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let rows: Vec<(String, String, String, String, Option<i64>)> = stmt
        .query_map(params![world_id, story_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .ok()
        .map(|it| it.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    if rows.is_empty() {
        return "（暂无重点角色）".into();
    }
    let mut out = String::new();
    for (name, kind, _importance, state, first_turn) in rows {
        let tag = if kind == "preset" { "预设" } else { "登场" };
        let state_text = readable_state(&state);
        let suffix = if state_text.is_empty() {
            String::new()
        } else {
            format!("：{state_text}")
        };
        let origin = match first_turn {
            Some(t) if kind != "preset" => format!("（第 {} 回合起）", t + 1),
            _ => String::new(),
        };
        out.push_str(&format!("- {name}{origin}〔{tag}〕{suffix}\n"));
    }
    out.trim().to_string()
}

/// Character / player state is a free-form string from the analyser, but it is
/// usually a small JSON object. Render it as readable prose instead of raw JSON.
fn readable_state(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() || raw == "{}" || raw == "null" {
        return String::new();
    }
    if !raw.starts_with('{') {
        return raw.to_string();
    }
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return raw.to_string();
    };
    let Some(obj) = v.as_object() else {
        return raw.to_string();
    };
    let mut parts = Vec::new();
    for (k, val) in obj {
        let text = match val {
            Value::String(s) => s.clone(),
            Value::Array(a) => a
                .iter()
                .filter_map(|x| {
                    x.as_str().map(String::from).or_else(|| {
                        x.as_object().map(|o| {
                            o.iter()
                                .map(|(kk, vv)| format!("{kk}={}", display_scalar(vv)))
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                    })
                })
                .collect::<Vec<_>>()
                .join("、"),
            other => display_scalar(other),
        };
        if !text.trim().is_empty() {
            parts.push(format!("{k}={text}"));
        }
    }
    parts.join("；")
}

fn display_scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// A one-line catalogue of every lore card: category -> titles. This is the
/// answer to "如果设定没被注入，模型就不知道有这条设定". It costs a few hundred
/// tokens and makes the world's vocabulary reachable even when retrieval misses.
fn lore_index_text(
    conn: &rusqlite::Connection,
    world_id: &str,
    budget: usize,
    cfg: &EstimatorCfg,
) -> (String, usize) {
    if budget == 0 {
        return (String::new(), 0);
    }
    let mut stmt = match conn.prepare(
        "SELECT COALESCE(c.name, '未分类'),
                COALESCE(p.name, ''),
                d.title
         FROM lore_docs d
         LEFT JOIN lore_cards lc ON lc.doc_id = d.id
         LEFT JOIN lore_categories c ON c.id = lc.category_id
         LEFT JOIN lore_categories p ON p.id = c.parent_id
         WHERE d.world_id = ?1
         ORDER BY COALESCE(p.sort_order, 99), COALESCE(c.sort_order, 99), d.title",
    ) {
        Ok(s) => s,
        Err(_) => return (String::new(), 0),
    };
    let rows: Vec<(String, String, String)> = match stmt.query_map(params![world_id], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    }) {
        Ok(it) => it.filter_map(|r| r.ok()).collect(),
        Err(_) => return (String::new(), 0),
    };
    if rows.is_empty() {
        return (String::new(), 0);
    }
    // Group while preserving the SQL order.
    let mut order: Vec<(String, String)> = Vec::new();
    let mut grouped: HashMap<(String, String), Vec<String>> = HashMap::new();
    for (cat, parent, title) in rows {
        let key = (parent, cat);
        if !grouped.contains_key(&key) {
            order.push(key.clone());
        }
        grouped.entry(key).or_default().push(title);
    }
    let total_cards: usize = grouped.values().map(|v| v.len()).sum();
    let header = format!("【设定集目录】共 {total_cards} 条，按分类列出标题；需要细节时以检索或置顶卡片为准。\n");
    let mut out = header.clone();
    let mut emitted = 0usize;
    let mut omitted = 0usize;
    for key in &order {
        let titles = &grouped[key];
        let name = if key.0.is_empty() {
            key.1.clone()
        } else {
            format!("{} / {}", key.0, key.1)
        };
        // Cap titles per category: a 400-card book must still fit a few hundred
        // tokens, otherwise the catalogue starves every other module.
        const PER_CATEGORY: usize = 6;
        let shown = if titles.len() > PER_CATEGORY {
            format!("{}…等 {} 条", titles[..PER_CATEGORY].join("、"), titles.len())
        } else {
            titles.join("、")
        };
        let line = format!("- {name}：{shown}\n");
        let candidate = format!("{out}{line}");
        if estimate(&candidate, cfg) > budget {
            omitted += titles.len();
            continue;
        }
        out = candidate;
        emitted += titles.len();
    }
    if omitted > 0 {
        out.push_str(&format!(
            "（还有 {omitted} 条未列出；用关键词会让检索自动带上对应条目，或在设定集里「置顶」它们。）\n"
        ));
    }
    (out.trim_end().to_string(), emitted)
}

/// Cards the user pinned for this story: full text, injected every turn.
fn pinned_lore_text(
    conn: &rusqlite::Connection,
    worlds_dir: &std::path::Path,
    world_id: &str,
    story_id: &str,
    budget: usize,
    cfg: &EstimatorCfg,
) -> (String, usize) {
    if budget == 0 {
        return (String::new(), 0);
    }
    let mut stmt = match conn.prepare(
        "SELECT d.title, d.rel_path FROM story_lore_pins p
         JOIN lore_docs d ON d.id = p.doc_id
         WHERE p.story_id = ?1 ORDER BY p.created_at",
    ) {
        Ok(s) => s,
        Err(_) => return (String::new(), 0),
    };
    let rows: Vec<(String, String)> = match stmt.query_map(params![story_id], |r| {
        Ok((r.get(0)?, r.get(1)?))
    }) {
        Ok(it) => it.filter_map(|r| r.ok()).collect(),
        Err(_) => return (String::new(), 0),
    };
    if rows.is_empty() {
        return (String::new(), 0);
    }
    let mut out = String::from("【常驻设定卡（作者置顶，必须遵守）】\n");
    let mut used = 0usize;
    for (title, rel) in rows {
        let path = worlds_dir.join(world_id).join(&rel);
        let body = match fs::read_to_string(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let block = format!("### {title}\n{}\n\n", body.trim());
        if estimate(&format!("{out}{block}"), cfg) > budget {
            out.push_str(&format!("（{title} 超预算未注入，可提高该模块额度）\n"));
            continue;
        }
        out.push_str(&block);
        used += 1;
    }
    if used == 0 {
        return (String::new(), 0);
    }
    (out.trim_end().to_string(), used)
}

/// Director mode: long-range outline + the one-shot instruction for this turn.
fn director_text(conn: &rusqlite::Connection, story_id: &str) -> (String, String) {
    let row: Option<(i64, String, String)> = conn
        .query_row(
            "SELECT enabled, outline_md, next_note FROM story_director WHERE story_id = ?1",
            params![story_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    match row {
        // The one-shot note is an explicit order for THIS turn, so it does not
        // need the master switch; the outline is gated by `enabled`.
        Some((enabled, outline, note)) => (
            if enabled == 1 {
                outline.trim().to_string()
            } else {
                String::new()
            },
            note.trim().to_string(),
        ),
        None => (String::new(), String::new()),
    }
}


/// Assemble the budget-capped modules. `protagonist` / event_background are
/// deliberately NOT part of this list: they bypass trimming entirely.
fn assemble_modules(
    specs: &[(&str, String)],
    effective: &HashMap<String, usize>,
    policy: &Policy,
    cfg: &EstimatorCfg,
) -> (HashMap<String, String>, Vec<BudgetItem>) {
    let mut values = HashMap::new();
    let mut items = Vec::new();
    for (name, text) in specs {
        let m = policy.modules.get(*name);
        let on = m.map(|x| x.on).unwrap_or(true);
        let budget = effective
            .get(*name)
            .copied()
            .unwrap_or(usize::MAX / 4);
        let (final_text, trimmed) = if on {
            trim_to_budget(text, budget, cfg)
        } else {
            (String::new(), false)
        };
        let est = if on { estimate(&final_text, cfg) } else { 0 };
        let key = if *name == "action_choices" { "action" } else { *name };
        values.insert(key.to_string(), final_text);
        items.push(BudgetItem {
            name: (*name).into(),
            enabled: on,
            budget: m.map(|x| x.budget).unwrap_or(0),
            est,
            trimmed,
        });
    }
    (values, items)
}

const FALLBACK_SYSTEM: &str = "续写小说，不替主角做决定。\n{{fixed_background}}\n{{event_background}}\n\n【主角人设】\n{{protagonist}}\n\n{{opening_scene}}\n\n【重要角色】\n{{characters}}\n\n【当前状态】\n{{state_card}}\n\n【前情】\n{{rolling_summary}}\n\n{{lore_pins}}\n\n{{lore_index}}\n\n{{lore_chunks}}\n\n{{director_outline}}\n\n【最近正文】\n{{recent_passages}}\n\n【主角行动】{{action}}\n\n{{director_note}}\n\n{{stop_policy}}\n\n{{system_contract}}";
const FALLBACK_CONTRACT: &str = "严格按四段格式回复，每个选项必须独占一行：\n<<<NARRATION>>>\n正文\n<<<END_NARRATION>>>\n<<<ANALYSIS>>>\n{\"world\":{},\"player\":{},\"characters\":[],\"summary\":\"\",\"new_character_candidates\":[]}\n<<<END_ANALYSIS>>>\n<<<STOP>>>\n{\"stop\":true,\"reason\":\"\",\"trigger\":\"\"}\n<<<END_STOP>>>\n<<<CHOICES>>>\nA. ...\nB. ...\nC. ...\n<<<END_CHOICES>>>";

/// Compact memory for the optional independent analysis pass. Deltas can only
/// be computed against a known baseline, so the state card and the injected
/// characters must be present. Lore is deliberately left out: the analyser
/// records what the passage did, it does not need world exposition.
pub fn analysis_context(state: &AppState, story_id: &str) -> String {
    let settings = app_settings(state);
    let conn = match state.db.lock() {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let story = match get_story(&conn, story_id) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let state_json: String = conn
        .query_row(
            "SELECT data_json FROM state_card WHERE story_id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "{}".into());
    let chars = characters_text(&conn, &story.world_id, story_id, &settings.character_inject);
    drop(conn);

    let mut out = String::from("【当前状态】\n");
    out.push_str(&layered_state_text(&state_json));
    if !chars.trim().is_empty() && chars.trim() != "（暂无重点角色）" {
        out.push_str("\n\n【已知角色】\n");
        out.push_str(&chars);
    }
    out
}

/// Assemble the whole ContextBundle: memory -> budget trim -> template render -> preview data.
pub fn build_bundle(
    state: &AppState,
    story_id: &str,
    action_md: &str,
    auto_continue: bool,
) -> Result<ContextBundle> {
    let settings = app_settings(state);
    let conn = state.db.lock()?;
    let story = get_story(&conn, story_id)?;
    let protagonist = get_protagonist(&conn, story_id)?;
    let pov = story_pov(&conn, story_id);

    let state_json: String = conn
        .query_row(
            "SELECT data_json FROM state_card WHERE story_id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "{}".to_string());

    let summary: String = conn
        .query_row(
            "SELECT content_md FROM summary_state WHERE story_id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .unwrap_or_default();

    let turn_index: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(turn_index), -1) + 1 FROM passages WHERE story_id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT content_md FROM passages WHERE story_id = ?1 AND parse_status != 'pending'
         ORDER BY turn_index DESC LIMIT 30",
    )?;
    let mut recent: Vec<String> = stmt
        .query_map(params![story_id], |r| r.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);
    recent.reverse();

    let consecutive = consecutive_continue_turns(&conn, story_id);

    // Lore: an index of what exists, the cards the author pinned, and keyword
    // retrieval of the chunks that the current action actually mentions.
    let mut lore_text = String::new();
    let mut lore_hits = 0usize;
    let policy = load_policy(state, &story.engine_mode)?;
    let policy_warning = policy_parse_error(state, &story.engine_mode);
    let cfg = &policy.estimator;
    let mod_cfg = |name: &str| policy.modules.get(name).copied();
    let lore_cfg_on = policy
        .modules
        .get("lore_chunks")
        .map(|m| (m.on, m.top_n))
        .unwrap_or((false, 0));
    let (director_outline, director_note) = director_text(&conn, story_id);
    let _ = auto_continue;

    let (index_text, idx_count) = match mod_cfg("lore_index") {
        Some(m) if m.on => lore_index_text(&conn, &story.world_id, m.budget, cfg),
        _ => (String::new(), 0),
    };
    let index_hits = idx_count;

    let (pins_text, _pin_count) = match mod_cfg("lore_pins") {
        Some(m) if m.on => pinned_lore_text(
            &conn,
            &state.worlds_dir,
            &story.world_id,
            story_id,
            m.budget,
            cfg,
        ),
        _ => (String::new(), 0),
    };

    // Keyword phase runs under the DB lock; the optional embedding call must
    // NOT (a slow embedding server would freeze every other command).
    let mut lore_query = String::new();
    let mut lore_pool: Option<(crate::retrieval::Pool, Vec<(i64, f64)>)> = None;
    if settings.retrieval_enabled && lore_cfg_on.0 {
        let tail_start = recent.len().saturating_sub(3);
        lore_query = format!(
            "{action_md}\n{director_note}\n{}",
            recent[tail_start..].join("\n")
        );
        let pool = crate::retrieval::prepare(&conn, &story.world_id);
        let bm25 = crate::retrieval::bm25_search(
            &conn,
            &story.world_id,
            &lore_query,
            policy.retrieval.bm25_topk.max(10),
        );
        lore_pool = Some((pool, bm25));
    }

    // Event background: a story-level override (enabled=1) wins, including an
    // empty string (an explicit "no background for this story").
    let story_bg: Option<String> = conn
        .query_row(
            "SELECT content_md FROM story_event_background WHERE story_id = ?1 AND enabled = 1",
            params![story_id],
            |r| r.get(0),
        )
        .ok();
    let event_bg = match story_bg {
        Some(v) => v,
        None => conn
            .query_row(
                "SELECT content_md FROM world_event_background WHERE world_id = ?1",
                params![story.world_id],
                |r| r.get(0),
            )
            .unwrap_or_default(),
    };

    let mut tstmt = conn.prepare(
        "SELECT era, year_label, title, body_md FROM timeline_events
         WHERE world_id = ?1 AND inject_into_context = 1
         ORDER BY ordinal, created_at",
    )?;
    let timeline_rows: Vec<(String, String, String, String)> = tstmt
        .query_map(params![story.world_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(tstmt);

    let characters_md = characters_text(
        &conn,
        &story.world_id,
        story_id,
        &settings.character_inject,
    );
    drop(conn);

    // Phase 2 of retrieval: embed the query (network, lock released), fuse with
    // BM25 by RRF, and render the selected chunks.
    let mut lore_warn: Option<String> = None;
    if let Some((pool, bm25)) = lore_pool {
        if !pool.is_empty() {
            let (hits, warn) =
                crate::retrieval::finish(state, bm25, &pool, &lore_query, &policy);
            lore_hits = hits.len();
            lore_text = hits
                .into_iter()
                .map(|h| {
                    if h.heading.trim().is_empty() {
                        format!("【{}】\n{}", h.title, h.content)
                    } else {
                        format!("【{} · {}】\n{}", h.title, h.heading, h.content)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            lore_warn = warn;
        }
    }

    let mut bg_text = String::new();
    if !event_bg.trim().is_empty() {
        bg_text.push_str("【事件背景】\n");
        bg_text.push_str(event_bg.trim());
    }
    if !timeline_rows.is_empty() {
        if !bg_text.is_empty() {
            bg_text.push_str("\n\n");
        }
        bg_text.push_str("【时间线】\n");
        for (era, year, title, body) in timeline_rows {
            let when = match (era.trim(), year.trim()) {
                ("", "") => String::new(),
                ("", y) => y.to_string(),
                (e, "") => e.to_string(),
                (e, y) => format!("{e}·{y}"),
            };
            if when.is_empty() {
                bg_text.push_str(&format!("- {title}：{body}\n"));
            } else {
                bg_text.push_str(&format!("- 【{when}】{title}：{body}\n"));
            }
        }
    }

    let system_rel = if pov == "omniscient" {
        policy
            .templates
            .get("system_omniscient")
            .cloned()
            .unwrap_or_else(|| format!("templates/system.omni.{}.md", story.engine_mode))
    } else {
        policy
            .templates
            .get("system")
            .cloned()
            .unwrap_or_else(|| format!("templates/system.{}.md", story.engine_mode))
    };
    let output_rel = if policy.output_template.is_empty() {
        format!("templates/output.{}.md", story.engine_mode)
    } else {
        policy.output_template.clone()
    };
    let system_tpl = read_config_text(state, &system_rel).unwrap_or_else(|_| FALLBACK_SYSTEM.into());
    let contract = read_config_text(state, &output_rel).unwrap_or_else(|_| FALLBACK_CONTRACT.into());

    let recent_n = policy
        .modules
        .get("recent_passages")
        .map(|m| m.n)
        .unwrap_or(3);
    let recent_text = recent
        .into_iter()
        .rev()
        .take(recent_n.max(1))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    // The protagonist persona is the ONE module that can never be trimmed:
    // local 10K mode used to cut it from the head and leave only the opening.
    let persona_text = format!("姓名：{}\n{}", protagonist.name, protagonist.persona_md).trim().to_string();
    let opening_text = if turn_index < settings.opening_scene_turns as i64
        && !protagonist.opening_scene_md.trim().is_empty()
    {
        format!("【开场情境】\n{}", protagonist.opening_scene_md.trim())
    } else {
        String::new()
    };
    let state_text = layered_state_text(&state_json);
    let summary_text = if summary.trim().is_empty() {
        "（暂无）".to_string()
    } else {
        summary
    };
    let fixed_bg = app_str(state, "fixed_background");
    let stop_text = stop_policy_text(&settings, consecutive, auto_continue);

    let specs: Vec<(&str, String)> = vec![
        ("system_contract", contract),
        ("opening_scene", opening_text.clone()),
        ("state_card", state_text),
        ("rolling_summary", summary_text),
        ("characters", characters_md),
        ("lore_pins", pins_text.clone()),
        ("lore_index", index_text.clone()),
        ("lore_chunks", lore_text.clone()),
        ("recent_passages", recent_text),
        ("action_choices", action_md.to_string()),
        ("stop_policy", stop_text),
        ("director_outline", director_outline.clone()),
    ];

    let mut effective: HashMap<String, usize> = HashMap::new();
    for (name, _) in &specs {
        let b = policy
            .modules
            .get(*name)
            .map(|m| m.budget)
            .unwrap_or(usize::MAX / 4);
        effective.insert((*name).to_string(), b);
    }

    // Values injected verbatim regardless of budget. They are reported but
    // never silently cut (the user explicitly asked for 100% persona + bg).
    let inject_always = |values: &mut HashMap<String, String>| {
        values.insert("protagonist".into(), persona_text.clone());
        values.insert("persona_opening".into(), {
            let mut p = persona_text.clone();
            if !opening_text.is_empty() {
                p.push_str("\n\n");
                p.push_str(&opening_text);
            }
            p
        });
        values.insert("fixed_background".into(), fixed_bg.clone());
        values.insert("event_background".into(), bg_text.clone());
        // Director note is a direct order for THIS turn: never trimmed.
        values.insert(
            "director_note".into(),
            if director_note.is_empty() {
                String::new()
            } else {
                format!("【导演指令 · 本回合必须执行】\n{director_note}")
            },
        );
    };

    let (mut values, mut items) = assemble_modules(&specs, &effective, &policy, cfg);
    inject_always(&mut values);

    let bg_est = estimate(&bg_text, cfg);
    let persona_est = estimate(&persona_text, cfg);
    let soft_cap = settings.event_background_soft_cap;
    let persona_cap = settings.persona_soft_cap;

    let cap = if story.engine_mode == "paste" {
        policy.context_window
    } else {
        policy
            .context_window
            .saturating_sub(policy.max_output_tokens)
    };

    let mut bundle_text = render(&system_tpl, &values);
    let mut real_est = estimate(&bundle_text, cfg);

    // ModuleCfg.trim is now live: when the assembled prompt is over the cap,
    // shrink the highest-trim modules first (oldest context goes before persona).
    let mut trimmable: Vec<(String, i32)> = specs
        .iter()
        .filter_map(|(n, _)| {
            policy
                .modules
                .get(*n)
                .filter(|m| m.on && m.trim > 0)
                .map(|m| ((*n).to_string(), m.trim))
        })
        .collect();
    trimmable.sort_by(|a, b| b.1.cmp(&a.1));
    for (name, _) in trimmable {
        if real_est <= cap {
            break;
        }
        let cur = effective.get(&name).copied().unwrap_or(0);
        if cur == 0 {
            continue;
        }
        effective.insert(name.clone(), (cur as f64 * 0.65) as usize);
        let (v, it) = assemble_modules(&specs, &effective, &policy, cfg);
        values = v;
        inject_always(&mut values);
        items = it;
        bundle_text = render(&system_tpl, &values);
        real_est = estimate(&bundle_text, cfg);
    }

    // buffer is a reported reserve, not injected text.
    let buffer = policy.modules.get("buffer");
    items.push(BudgetItem {
        name: "buffer".into(),
        enabled: buffer.map(|m| m.on).unwrap_or(true),
        budget: buffer.map(|m| m.budget).unwrap_or(0),
        est: 0,
        trimmed: false,
    });
    items.push(BudgetItem {
        name: "protagonist".into(),
        enabled: true,
        budget: persona_cap,
        est: persona_est,
        trimmed: false,
    });
    items.push(BudgetItem {
        name: "event_background".into(),
        enabled: true,
        budget: soft_cap,
        est: bg_est,
        trimmed: false,
    });
    items.push(BudgetItem {
        name: "director_note".into(),
        enabled: !director_note.is_empty(),
        budget: 0,
        est: if director_note.is_empty() {
            0
        } else {
            estimate(&director_note, cfg)
        },
        trimmed: false,
    });

    let mut warnings = Vec::new();
    if let Some(w) = policy_warning {
        warnings.push(w);
    }
    if let Some(w) = lore_warn {
        warnings.push(w);
    }
    if settings.retrieval_enabled && lore_cfg_on.0 && lore_hits == 0 {
        warnings.push(
            "设定检索已启用，但这次没有关键词命中任何设定片段（目录索引仍然注入了，模型知道有哪些条目）"
                .to_string(),
        );
    }
    if index_hits == 0 && mod_cfg("lore_index").map(|m| m.on).unwrap_or(false) {
        warnings.push("世界观里还没有设定卡，或「设定集目录」模块被关闭；模型目前完全不知道设定集的存在".into());
    }
    if bg_est > soft_cap {
        warnings.push(format!(
            "事件背景+时间线约 {bg_est} tokens，超过建议上限 {soft_cap}；建议精简或把细节移入设定集（这部分不会被自动裁剪）"
        ));
    }
    if persona_est > persona_cap {
        warnings.push(format!(
            "主角人设约 {persona_est} tokens，超过建议上限 {persona_cap}；它按你的要求 100% 注入、不裁剪，请自行精简"
        ));
    }
    if real_est > cap {
        warnings.push(format!(
            "整块提示词估算 {real_est} tokens 超过上限 {cap}；事件背景、时间线与主角人设不参与自动裁剪，请手动缩短"
        ));
    }

    Ok(ContextBundle {
        text: bundle_text,
        report: BudgetReport {
            items,
            total_est: real_est,
            cap,
            retrieval_enabled: settings.retrieval_enabled,
            warnings,
        },
        mode: story.engine_mode.clone(),
        template: system_rel,
        pov,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults;

    #[test]
    fn shipped_yaml_configs_parse() {
        let engines: crate::domain::EnginesConfig =
            serde_yaml::from_str(defaults::ENGINES_YAML).unwrap();
        assert!(engines.engines.and_then(|e| e.local).is_some());
        for text in [defaults::POLICY_API, defaults::POLICY_LOCAL, defaults::POLICY_PASTE] {
            let policy: Policy = serde_yaml::from_str(text).unwrap();
            assert!(!policy.modules.is_empty());
            assert_eq!(policy.modules.get("lore_chunks").map(|m| m.on), Some(true));
            assert!(policy.modules.contains_key("lore_index"));
            assert!(policy.modules.contains_key("lore_pins"));
            assert!(policy.modules.contains_key("director_outline"));
            assert!(policy.modules.contains_key("protagonist"));
            assert!(policy.modules.contains_key("characters"));
            assert!(policy.modules.contains_key("stop_policy"));
            assert!(policy.templates.contains_key("system_omniscient"));
        }
    }

    #[test]
    fn estimator_overestimates_chinese() {
        let cfg = EstimatorCfg::default();
        let text = "北境的冬天很长，风雪封住了旧宅的门。";
        assert!(estimate(text, &cfg) >= text.chars().count());
    }

    #[test]
    fn layered_state_renders_both_layers_and_tolerates_flat_json() {
        let layered = r#"{"world":{"place":"旧宅","threads":["封印"]},"player":{"mind":"警惕"},"delta_log":"x"}"#;
        let s = layered_state_text(layered);
        assert!(s.contains("世界状态"));
        assert!(s.contains("place: 旧宅"));
        assert!(s.contains("玩家状态"));
        assert!(s.contains("mind: 警惕"));
        assert!(!s.contains("delta_log"));
        let flat = r#"{"characters":"","time":""}"#;
        assert!(layered_state_text(flat).contains("当前状态"));
    }

    #[test]
    fn stop_policy_lists_enabled_triggers() {
        let mut settings = AppSettings::default();
        settings.stop_triggers = vec!["dialogue".into(), "major_decision".into()];
        settings.max_consecutive_turns = 3;
        let s = stop_policy_text(&settings, 2, false);
        assert!(s.contains("dialogue"));
        assert!(s.contains("stop=false"));
        assert!(s.contains("最多连写 3 段"));
    }

    #[test]
    fn lore_index_lists_titles_and_reports_what_it_dropped() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("db/schema.sql")).unwrap();
        conn.execute_batch(
            "INSERT INTO worlds (id,name,description,created_at,updated_at,mirror_path)
               VALUES ('w','w','','t','t','w');
             INSERT INTO lore_categories (id,world_id,parent_id,name,sort_order)
               VALUES ('c1','w',NULL,'势力',1);
             INSERT INTO lore_docs (id,world_id,title,rel_path,sha256,chunk_count,origin,created_at)
               VALUES ('d1','w','武魂殿','a.md','',0,'import','t'),
                      ('d2','w','史莱克学院','b.md','',0,'import','t');
             INSERT INTO lore_cards (doc_id,world_id,category_id,tags,summary,char_count,updated_at)
               VALUES ('d1','w','c1','[]','',0,'t'), ('d2','w','c1','[]','',0,'t');",
        )
        .unwrap();
        let cfg = EstimatorCfg::default();
        let (text, n) = lore_index_text(&conn, "w", 2000, &cfg);
        assert_eq!(n, 2);
        assert!(text.contains("势力"));
        assert!(text.contains("武魂殿") && text.contains("史莱克学院"));

        // A budget too small for both lines still yields a partial catalogue
        // instead of silently injecting nothing.
        let (tiny, small_n) = lore_index_text(&conn, "w", 12, &cfg);
        assert!(small_n <= 1);
        assert!(tiny.contains("未列出") || tiny.is_empty());
    }

    #[test]
    fn characters_text_skips_passers_by_and_reads_json_state() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("db/schema.sql")).unwrap();
        conn.execute_batch(
            "INSERT INTO worlds (id,name,description,created_at,updated_at,mirror_path)
               VALUES ('w','w','','t','t','w');
             INSERT INTO characters
               (id,world_id,story_id,name,kind,importance,current_state,first_turn,inject,created_at,updated_at)
               VALUES ('a','w',NULL,'唐三','emergent','notable','{\"body\":\"轻伤\"}',3,1,'t','t'),
                      ('b','w',NULL,'路人甲','emergent','passerby','{}',NULL,0,'t','t');",
        )
        .unwrap();
        let text = characters_text(&conn, "w", "s", "notable");
        assert!(text.contains("唐三"));
        assert!(text.contains("body=轻伤"), "json state is rendered readably");
        assert!(!text.contains("路人甲"), "passerby never enters the prompt");
    }

    #[test]
    fn auto_continue_replaces_the_stop_policy() {
        let settings = AppSettings::default();
        let s = stop_policy_text(&settings, 2, true);
        assert!(s.contains("不要停"));
        assert!(s.contains("stop=false"));
        assert!(!s.contains("何时停下"), "自动连写时不谈'何时停下'");
    }

    #[test]
    fn omnniscient_templates_are_not_empty() {
        assert!(defaults::SYSTEM_OMNI_LOCAL.contains("全知读者视角"));
        assert!(defaults::SYSTEM_OMNI_LOCAL.contains("{{characters}}"));
        assert!(defaults::SYSTEM_OMNI_LOCAL.contains("{{stop_policy}}"));
    }
}
