use crate::context;
use crate::domain::{EngineProfile, EnginesConfig, ParsedOutput};
use crate::state::AppState;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub fn load_engines(state: &AppState) -> Result<EnginesConfig> {
    let text = context::read_config_text(state, "engines.yaml")?;
    serde_yaml::from_str(&text).map_err(|e| anyhow!("engines.yaml 解析失败: {e}"))
}

pub fn profile_for(state: &AppState, mode: &str) -> Result<Option<EngineProfile>> {
    let cfg = load_engines(state)?;
    let profiles = cfg.engines.ok_or_else(|| anyhow!("engines.yaml 缺少 engines 段"))?;
    Ok(match mode {
        "api" => profiles.api,
        "local" => profiles.local,
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// Output contract: layered, forgiving parsing
// ---------------------------------------------------------------------------

fn normalize(raw: &str) -> String {
    let mut s = raw.replace("\r\n", "\n").replace('\r', "\n");
    // Remove code fences (whole-line ``` and ```markdown ...).
    let mut out = String::new();
    for line in s.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    s = out;
    // Common Chinese marker aliases -> canonical markers.
    let aliases = [
        ("【正文】", "<<<NARRATION>>>"),
        ("【/正文】", "<<<END_NARRATION>>>"),
        ("【选项】", "<<<CHOICES>>>"),
        ("【/选项】", "<<<END_CHOICES>>>"),
        ("【状态】", "<<<STATE_DELTA>>>"),
        ("【/状态】", "<<<END_STATE_DELTA>>>"),
        ("【摘要】", "<<<SUMMARY_DELTA>>>"),
        ("【/摘要】", "<<<END_SUMMARY_DELTA>>>"),
        ("【分析】", "<<<ANALYSIS>>>"),
        ("【/分析】", "<<<END_ANALYSIS>>>"),
        ("【停顿】", "<<<STOP>>>"),
        ("【/停顿】", "<<<END_STOP>>>"),
    ];
    for (from, to) in aliases {
        s = s.replace(from, to);
    }
    // Circled numbers often appear without a separator (①甲). Normalise them
    // into the canonical "A、" form so the same parser handles every variant.
    for (from, to) in [('①', "A、"), ('②', "B、"), ('③', "C、"), ('④', "D、")] {
        s = s.replace(from, to);
    }
    // Drop common courtesy preambles only when they are standalone lines.
    if let Ok(re) = Regex::new(r"(?m)^\s*(好的[，,。!！]?|以下是.*[:：]?|Sure[!,.]?|Here's .*[:：]?|Here is .*[:：]?)\s*$") {
        s = re.replace_all(&s, "").to_string();
    }
    s
}

fn extract_marker(raw: &str, name: &str) -> Option<String> {
    let pattern = format!(r"(?is)<<<{name}>>>(.*?)<<<END_{name}>>>");
    let re = Regex::new(&pattern).ok()?;
    re.captures(raw).map(|c| c[1].trim().to_string())
}

use crate::domain::CHOICES_SUSPECT;

const LABEL_CHARS: &str = r"[A-DＡ-Ｄa-d一二三四]";
const LABEL_SEP: &str = r"[\.、．:：]";

fn canonical_label(c: char) -> Option<char> {
    match c {
        'A'..='D' => Some(c),
        'a'..='d' => Some((c as u8 - b'a' + b'A') as char),
        'Ａ'..='Ｄ' => Some((b'A' + (c as u32 - 'Ａ' as u32) as u8) as char),
        '一' => Some('A'),
        '二' => Some('B'),
        '三' => Some('C'),
        '四' => Some('D'),
        _ => None,
    }
}

fn next_label(c: char) -> Option<char> {
    match c {
        'A' => Some('B'),
        'B' => Some('C'),
        'C' => Some('D'),
        _ => None,
    }
}

fn suspect_warning() -> String {
    format!("{CHOICES_SUSPECT}:检测到多个选项写在同一行，已自动拆分，请确认拆分结果")
}

/// Split a CHOICES block into individual options.
///
/// Pass 1 (preferred): one option per line; continuation lines are appended
/// to the previous option. This never splits a "B." that appears inside a
/// sentence. Pass 2 only runs when Pass 1 found fewer than two options:
/// it looks for an A->B->C->D sequence inside a single flattened line.
/// Returns (options, was_glued) where was_glued means Pass 2 fired.
pub fn split_choice_block(text: &str) -> (Vec<String>, bool) {
    let line_re = Regex::new(&format!(r"^\s*({LABEL_CHARS}){LABEL_SEP}\s*(.*)$")).unwrap();
    let mut entries: Vec<(char, String)> = Vec::new();

    for line in text.lines() {
        if let Some(cap) = line_re.captures(line) {
            let c = cap[1].chars().next().unwrap();
            if let Some(label) = canonical_label(c) {
                entries.push((label, cap[2].trim().to_string()));
                continue;
            }
        }
        if let Some(last) = entries.last_mut() {
            let t = line.trim();
            if !t.is_empty() {
                last.1.push('\n');
                last.1.push_str(t);
            }
        }
    }

    if entries.len() >= 2 {
        return (
            entries
                .into_iter()
                .filter(|(_, body)| !body.trim().is_empty())
                .map(|(label, body)| format!("{label}. {}", body.trim()))
                .collect(),
            false,
        );
    }

    // Pass 2: several options written on one line.
    let flat = text.replace('\n', " ");
    let inline_re = Regex::new(&format!(r"({LABEL_CHARS}){LABEL_SEP}\s*")).unwrap();
    let mut candidates: Vec<(usize, usize, char)> = Vec::new();
    for m in inline_re.find_iter(&flat) {
        let start = m.start();
        // regex crate has no lookbehind: check the preceding character manually.
        let boundary_ok = if start == 0 {
            true
        } else {
            flat[..start]
                .chars()
                .last()
                .map(|c| c.is_whitespace())
                .unwrap_or(false)
        };
        if !boundary_ok {
            continue;
        }
        let first = flat[start..].chars().next().unwrap();
        if let Some(label) = canonical_label(first) {
            candidates.push((start, m.end(), label));
        }
    }

    let mut expected = Some('A');
    let mut kept: Vec<(usize, usize, char)> = Vec::new();
    for cand in candidates {
        if let Some(want) = expected {
            if cand.2 == want {
                kept.push(cand);
                expected = next_label(want);
            }
        }
    }

    if kept.len() >= 2 {
        let mut out = Vec::new();
        for i in 0..kept.len() {
            let (_, body_start, label) = kept[i];
            let end = if i + 1 < kept.len() {
                kept[i + 1].0
            } else {
                flat.len()
            };
            let body = flat[body_start..end].trim();
            if !body.is_empty() {
                out.push(format!("{label}. {body}"));
            }
        }
        if out.len() >= 2 {
            return (out, true);
        }
    }

    // Final fallback: keep non-empty lines as-is (never dead-end).
    (
        text.lines()
            .map(|l| l.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        false,
    )
}

/// JSON mode fallback: a single array element that actually contains
/// several inline options gets expanded.
fn expand_choices(items: Vec<String>) -> (Vec<String>, bool) {
    let mut out = Vec::new();
    let mut glued = false;
    for item in items {
        let (split, was_glued) = split_choice_block(&item);
        if was_glued && split.len() >= 2 {
            glued = true;
            out.extend(split);
        } else {
            out.push(item);
        }
    }
    (out, glued)
}

/// Parse the STOP block. Accepts JSON, loose "stop: false", or plain keywords.
/// Missing/ambiguous always means STOP (never let a small model run away).
fn parse_stop(text: &str) -> (bool, String, String) {
    let t = text.trim();
    if t.is_empty() {
        return (true, String::new(), String::new());
    }
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        if let Some(obj) = v.as_object() {
            let stop = match obj.get("stop") {
                Some(Value::Bool(b)) => *b,
                Some(Value::String(s)) => {
                    matches!(s.to_ascii_lowercase().as_str(), "yes" | "true" | "y" | "是" | "停")
                }
                Some(Value::Number(n)) => n.as_i64().unwrap_or(1) != 0,
                _ => true,
            };
            let reason = obj.get("reason").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let trigger = obj.get("trigger").and_then(|x| x.as_str()).unwrap_or("").to_string();
            return (stop, reason, trigger);
        }
    }
    let lower = t.to_ascii_lowercase();
    let explicit_no = lower.contains("\"stop\": false")
        || lower.contains("\"stop\":false")
        || lower.contains("stop: false")
        || lower.contains("stop=false")
        || lower.contains("不停")
        || lower.contains("继续写")
        || lower.trim() == "no"
        || lower.trim() == "false";
    let stop = !explicit_no;
    let reason = t
        .lines()
        .find(|l| l.contains("原因") || l.to_ascii_lowercase().contains("reason"))
        .map(|l| l.trim().to_string())
        .unwrap_or_default();
    (stop, reason, String::new())
}

/// Extract an Analysis from an ANALYSIS block, tolerating prose / fences around it.
pub fn parse_analysis(text: &str) -> Option<crate::domain::Analysis> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let v: Value = match serde_json::from_str(t) {
        Ok(v) => v,
        Err(_) => {
            let start = t.find('{')?;
            let end = t.rfind('}')?;
            if end <= start {
                return None;
            }
            serde_json::from_str(&t[start..=end]).ok()?
        }
    };
    serde_json::from_value::<crate::domain::Analysis>(v).ok()
}

/// The analysis actually available on a parsed output (single-call block only).
pub fn analysis_for(p: &ParsedOutput) -> Option<crate::domain::Analysis> {
    if p.analysis_json.trim().is_empty() {
        return None;
    }
    parse_analysis(&p.analysis_json)
}

fn try_markers(raw: &str) -> Option<ParsedOutput> {
    let narration = extract_marker(raw, "NARRATION")?;
    if narration.is_empty() {
        return None;
    }
    let choices_text = extract_marker(raw, "CHOICES").unwrap_or_default();
    let (choices, glued) = split_choice_block(&choices_text);
    let mut warnings = Vec::new();
    if glued {
        warnings.push(suspect_warning());
    }
    let analysis_json = extract_marker(raw, "ANALYSIS").unwrap_or_default();
    let (stop, stop_reason, stop_trigger) =
        parse_stop(&extract_marker(raw, "STOP").unwrap_or_default());
    Some(ParsedOutput {
        narration,
        choices,
        state_delta: extract_marker(raw, "STATE_DELTA").unwrap_or_default(),
        summary_delta: extract_marker(raw, "SUMMARY_DELTA").unwrap_or_default(),
        analysis_json,
        stop,
        stop_reason,
        stop_trigger,
        warnings,
        raw_output: String::new(),
    })
}

fn try_json(raw: &str) -> Option<ParsedOutput> {
    // Find the first balanced-looking JSON object.
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    let v: Value = serde_json::from_str(&raw[start..=end]).ok()?;
    let narration = v["narration"].as_str()?.trim().to_string();
    if narration.is_empty() {
        return None;
    }
    let raw_choices: Vec<String> = v["choices"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let (choices, glued) = expand_choices(raw_choices);
    let mut warnings = Vec::new();
    if glued {
        warnings.push(suspect_warning());
    }
    let analysis_json = v
        .get("analysis")
        .filter(|a| !a.is_null())
        .map(|a| a.to_string())
        .unwrap_or_default();
    let (stop, stop_reason, stop_trigger) = match v.get("stop") {
        Some(Value::Object(_)) => parse_stop(&v["stop"].to_string()),
        Some(Value::Bool(b)) => (*b, String::new(), String::new()),
        Some(Value::String(s)) => (
            !matches!(s.to_ascii_lowercase().as_str(), "no" | "false" | "continue" | "继续"),
            String::new(),
            String::new(),
        ),
        _ => (true, String::new(), String::new()),
    };
    Some(ParsedOutput {
        narration,
        choices,
        state_delta: v["state_delta"].as_str().unwrap_or("").to_string(),
        summary_delta: v["summary_delta"].as_str().unwrap_or("").to_string(),
        analysis_json,
        stop,
        stop_reason,
        stop_trigger,
        warnings,
        raw_output: String::new(),
    })
}

fn heuristic(raw: &str) -> ParsedOutput {
    let inline_re = Regex::new(&format!(r"({LABEL_CHARS}){LABEL_SEP}\s*")).unwrap();
    let mut split_at = None;
    for m in inline_re.find_iter(raw) {
        let boundary_ok = if m.start() == 0 {
            true
        } else {
            raw[..m.start()]
                .chars()
                .last()
                .map(|c| c.is_whitespace())
                .unwrap_or(false)
        };
        if boundary_ok {
            split_at = Some(m.start());
            break;
        }
    }
    if let Some(split) = split_at {
        let narration = raw[..split].trim().to_string();
        if !narration.is_empty() {
            let (choices, glued) = split_choice_block(&raw[split..]);
            let mut warnings =
                vec!["未检测到标准标记，已按选项行启发式切分；状态与摘要增量未更新".into()];
            if glued {
                warnings.push(suspect_warning());
            }
            return ParsedOutput {
                narration,
                choices,
                state_delta: String::new(),
                summary_delta: String::new(),
                analysis_json: String::new(),
                stop: true,
                stop_reason: String::new(),
                stop_trigger: String::new(),
                warnings,
                raw_output: String::new(),
            };
        }
    }
    ParsedOutput {
        narration: raw.trim().to_string(),
        choices: Vec::new(),
        state_delta: String::new(),
        summary_delta: String::new(),
        analysis_json: String::new(),
        stop: true,
        stop_reason: String::new(),
        stop_trigger: String::new(),
        warnings: vec!["未检测到任何结构化标记，已将整段回贴作为正文".into()],
        raw_output: String::new(),
    }
}

fn finalize(mut p: ParsedOutput, raw: &str) -> ParsedOutput {
    p.raw_output = raw.to_string();
    if p.narration.trim().is_empty() {
        p.warnings.push("未能解析出正文，请人工修正".into());
    }
    // Only a STOP turn needs options. A "keep writing" turn has none by design.
    if p.choices.is_empty() && p.stop {
        p.choices.push("继续".into());
        p.warnings.push("选项解析失败，已补一个默认项；自由输入始终可用".into());
    }
    if p.state_delta.trim().is_empty() && p.analysis_json.trim().is_empty() {
        p.warnings.push("未检测到分析段，本次不更新分层状态卡".into());
    }
    let has_summary = analysis_for(&p)
        .map(|a| !a.summary.trim().is_empty())
        .unwrap_or(false);
    if p.summary_delta.trim().is_empty() && !has_summary {
        p.warnings.push("摘要增量缺失，本次不更新滚动摘要".into());
    }
    p
}

pub fn parse_output(raw: &str, mode: &str) -> ParsedOutput {
    let cleaned = normalize(raw);
    if mode == "api" {
        if let Some(p) = try_json(&cleaned) {
            return finalize(p, raw);
        }
    }
    if let Some(p) = try_markers(&cleaned) {
        return finalize(p, raw);
    }
    finalize(heuristic(&cleaned), raw)
}

// ---------------------------------------------------------------------------
// OpenAI-compatible HTTP + SSE (shared by api and local)
// ---------------------------------------------------------------------------

/// Decode as much valid UTF-8 as possible out of `pending`, leaving any
/// incomplete trailing sequence in place for the next network chunk.
///
/// This is why Chinese text used to arrive as `�`: a 3-byte character split
/// across two TCP chunks was decoded per-chunk with `from_utf8_lossy`.
fn drain_utf8(pending: &mut Vec<u8>) -> String {
    let mut out = String::new();
    loop {
        match std::str::from_utf8(pending) {
            Ok(s) => {
                out.push_str(s);
                pending.clear();
                return out;
            }
            Err(e) => {
                let valid = e.valid_up_to();
                if valid > 0 {
                    if let Ok(s) = std::str::from_utf8(&pending[..valid]) {
                        out.push_str(s);
                    }
                    pending.drain(..valid);
                    continue;
                }
                match e.error_len() {
                    // A complete but invalid sequence: skip the bad bytes.
                    Some(n) => {
                        pending.drain(..n);
                    }
                    // Incomplete tail: wait for the rest of the character.
                    None => return out,
                }
            }
        }
    }
}

/// Pull the text of one `data:` line out of an SSE chunk. Returns false when
/// the line is not an SSE data line at all (so callers know the body isn't SSE).
fn sse_delta(line: &str) -> Option<Option<String>> {
    let line = line.trim();
    if !line.starts_with("data:") {
        return None;
    }
    let payload = line["data:".len()..].trim();
    if payload.is_empty() || payload == "[DONE]" {
        return Some(None);
    }
    let v: Value = match serde_json::from_str(payload) {
        Ok(v) => v,
        // Some runtimes chunk JSON across `data:` lines; skip the fragment
        // instead of aborting the whole generation.
        Err(_) => return Some(None),
    };
    let choice = v.get("choices").and_then(|c| c.get(0));
    // reasoning_content (DeepSeek-R1 style) is deliberately dropped.
    let text = choice
        .and_then(|c| c.get("delta"))
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
        .or_else(|| {
            choice
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
        })
        .or_else(|| choice.and_then(|c| c.get("text")).and_then(|c| c.as_str()))
        .unwrap_or("");
    Some(if text.is_empty() { None } else { Some(text.to_string()) })
}

pub async fn stream_chat<F>(
    profile: &EngineProfile,
    prompt: &str,
    cancel: CancellationToken,
    on_delta: F,
) -> Result<String>
where
    F: Fn(String) + Send + 'static,
{
    // Total-request timeout is wrong for streaming: a 2000-token cloud reply can
    // legitimately outlive any fixed wall clock. Bound connect + per-chunk idle.
    let idle = Duration::from_millis(profile.timeout_ms.max(1000));
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .build()?;
    let url = format!("{}/chat/completions", profile.base_url.trim_end_matches('/'));
    let mut body = json!({
        "model": profile.model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": profile.params.temperature,
        "max_tokens": profile.params.max_tokens,
        "stream": true
    });
    if let Some(tp) = profile.params.top_p {
        body["top_p"] = json!(tp);
    }

    let mut req = client.post(&url).json(&body);
    if let Some(key) = profile.api_key.as_ref().filter(|k| !k.is_empty()) {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?.error_for_status()?;
    let mut stream = resp.bytes_stream();

    let mut full = String::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut line_buf = String::new();
    let mut raw_all = String::new();
    let mut saw_sse = false;

    loop {
        let next = match tokio::time::timeout(idle, stream.next()).await {
            Err(_) => {
                return Err(anyhow!(
                    "模型 {} 超过 {} 秒没有任何输出，已中断",
                    profile.model,
                    idle.as_secs()
                ))
            }
            Ok(None) => break,
            Ok(Some(Err(e))) => return Err(e.into()),
            Ok(Some(Ok(bytes))) => bytes,
        };
        if cancel.is_cancelled() {
            break;
        }
        pending.extend_from_slice(&next);
        let text = drain_utf8(&mut pending);
        if text.is_empty() {
            continue;
        }
        raw_all.push_str(&text);
        line_buf.push_str(&text);
        while let Some(pos) = line_buf.find('\n') {
            let rest = line_buf.split_off(pos + 1);
            let line = std::mem::replace(&mut line_buf, rest);
            if let Some(delta) = sse_delta(&line) {
                saw_sse = true;
                if let Some(d) = delta {
                    full.push_str(&d);
                    on_delta(d);
                }
            }
        }
    }

    // A last data line that arrived without its trailing newline.
    if !line_buf.trim().is_empty() {
        if let Some(Some(d)) = sse_delta(&line_buf) {
            full.push_str(&d);
            on_delta(d);
        }
    }

    // Some OpenAI-compatible servers (or a proxy in front of them) ignore
    // `stream: true` and send one plain JSON body instead of SSE.
    if !saw_sse && !raw_all.trim().is_empty() {        if let Ok(v) = serde_json::from_str::<Value>(raw_all.trim()) {
            let text = v
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| {
                    c.get("message")
                        .and_then(|m| m.get("content"))
                        .or_else(|| c.get("text"))
                })
                .and_then(|c| c.as_str())
                .unwrap_or("");
            if !text.is_empty() {
                full.push_str(text);
                on_delta(text.to_string());
            }
        }
    }

    if full.trim().is_empty() && !cancel.is_cancelled() {
        return Err(anyhow!(
            "模型没有返回任何正文（可能命中了上下文长度上限，或该服务不支持流式）"
        ));
    }
    Ok(full)
}

/// The analyser's prompt, exposed separately so the context inspector can show
/// exactly what the second pass was given (and the UI can re-run it verbatim).
pub fn analysis_prompt(narration: &str, action: &str, memory: &str) -> String {
    format!(
        "你是小说状态记录员，不是作者。只依据下面的正文片段，输出一个 JSON 对象，不要解释、不要代码块围栏。\n\
         字段：{{\"world\":{{\"time\":\"\",\"place\":\"\",\"situation\":\"\",\"threads\":[]}},\
         \"player\":{{\"body\":\"\",\"mind\":\"\",\"mood\":\"\",\"goal\":\"\",\"abilities\":[],\"items\":[],\"conditions\":[]}},\
         \"characters\":[{{\"name\":\"\",\"current_state\":\"\",\"importance\":\"passerby\",\"is_new\":false}}],\
         \"summary\":\"一两句话\",\"new_character_candidates\":[]}}\n\
         只写这段正文引起的变化；没变化的字段留空，不要复述已知状态。\n\
         以「〔暗场〕」开头的段落是主角看不见、但真实发生的事：据此更新相关角色的心思与状态（读者暂时看不到这些内容）。\n\
         abilities 写功法/技能/等级的变化，items 写获得或失去的物品，conditions 写受伤、中毒、诅咒这类持续状态，mood 写当前情绪。\n\
         importance 只能是 passerby（路过、不再出现）或 notable（会再次登场、对剧情有影响）。\n\n\
         {memory}\n\n【本段正文】\n{narration}\n\n【主角行动】\n{action}"
    )
}

/// Optional second (cheap, short) analysis call for api/local. Default OFF:
/// the main contract already returns an ANALYSIS block in the same response,
/// so this only runs when the user explicitly wants a cleaner structured pass.
pub async fn analyze_second_pass(
    profile: &EngineProfile,
    narration: &str,
    action: &str,
    memory: &str,
) -> Result<String> {
    let prompt = analysis_prompt(narration, action, memory);
    analyze_with_prompt(profile, &prompt).await
}

/// 通用一次性补全（设定辅助创作这类"要一段正文"的场景）。
pub async fn complete(
    profile: &EngineProfile,
    prompt: &str,
    max_tokens: usize,
    temperature: f64,
) -> Result<String> {
    let client = Client::builder()
        .timeout(Duration::from_millis(profile.timeout_ms.max(1000)))
        .build()?;
    let url = format!("{}/chat/completions", profile.base_url.trim_end_matches('/'));
    let body = json!({
        "model": profile.model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": false
    });
    let mut req = client.post(&url).json(&body);
    if let Some(key) = profile.api_key.as_ref().filter(|k| !k.is_empty()) {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?.error_for_status()?;
    let v: Value = resp.json().await.unwrap_or(json!({}));
    Ok(v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string())
}

pub async fn analyze_with_prompt(profile: &EngineProfile, prompt: &str) -> Result<String> {
    let client = Client::builder()
        .timeout(Duration::from_millis(profile.timeout_ms.max(1000)))
        .build()?;
    let url = format!("{}/chat/completions", profile.base_url.trim_end_matches('/'));
    let body = json!({
        "model": profile.model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0.2,
        "max_tokens": 600,
        "stream": false
    });
    let mut req = client.post(&url).json(&body);
    if let Some(key) = profile.api_key.as_ref().filter(|k| !k.is_empty()) {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?.error_for_status()?;
    let v: Value = resp.json().await.unwrap_or(json!({}));
    Ok(v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string())
}

pub async fn test_connection(profile: &EngineProfile) -> Result<String> {
    let client = Client::builder().timeout(Duration::from_secs(20)).build()?;
    let url = format!("{}/chat/completions", profile.base_url.trim_end_matches('/'));
    let body = json!({
        "model": profile.model,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1,
        "stream": false
    });
    let mut req = client.post(&url).json(&body);
    if let Some(key) = profile.api_key.as_ref().filter(|k| !k.is_empty()) {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?.error_for_status()?;
    let v: Value = resp.json().await.unwrap_or(json!({}));
    let model = v["model"].as_str().unwrap_or(&profile.model).to_string();
    Ok(format!("连接成功 · 服务端返回模型 {model}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_markers_through_courtesy_and_fences() {
        let raw = "好的，以下是续写：\n```\n<<<NARRATION>>>\n他停住脚，指尖碰到冰凉的门环。\n<<<END_NARRATION>>>\n<<<CHOICES>>>\nA. 推门进去\nB. 贴耳听一听\nC. 转身离开\n<<<END_CHOICES>>>\n<<<STATE_DELTA>>>\n~ 地点: 旧宅门前\n<<<END_STATE_DELTA>>>\n<<<SUMMARY_DELTA>>>\n主角抵达门前。\n<<<END_SUMMARY_DELTA>>>\n```";
        let p = parse_output(raw, "local");
        assert!(p.narration.contains("门环"));
        assert_eq!(p.choices.len(), 3);
        assert!(p.state_delta.contains("旧宅门前"));
        assert!(p.summary_delta.contains("抵达"));
    }

    #[test]
    fn utf8_split_across_network_chunks_is_reassembled() {
        let text = "<<<NARRATION>>>\n他停住脚，指尖碰到冰凉的门环。\n";
        let mut pending: Vec<u8> = Vec::new();
        let mut out = String::new();
        // One byte at a time: every CJK character is split across "chunks".
        for b in text.as_bytes() {
            pending.push(*b);
            out.push_str(&drain_utf8(&mut pending));
        }
        assert_eq!(out, text);
        assert!(pending.is_empty(), "incomplete bytes must not linger");
        assert!(!out.contains('\u{fffd}'), "no replacement characters");
    }

    #[test]
    fn sse_lines_yield_text_and_ignore_reasoning() {
        let content = "data: {\"choices\":[{\"delta\":{\"content\":\"他停\"}}]}";
        assert_eq!(sse_delta(content), Some(Some("他停".into())));
        let reasoning = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"想\"}}]}";
        assert_eq!(sse_delta(reasoning), Some(None));
        assert_eq!(sse_delta("data: [DONE]"), Some(None));
        // A JSON object cut in half by a chunk boundary is skipped, not fatal.
        assert_eq!(sse_delta("data: {\"choices\":[{\"delta\":{\"cont"), Some(None));
        // Not SSE at all: the caller falls back to a single JSON body.
        assert_eq!(sse_delta("{\"choices\":[]}"), None);
        assert_eq!(sse_delta("event: ping"), None);
    }

    #[test]
    fn heuristic_fallback_when_no_markers() {        let raw = "他走进院子，四下无人。\nA. 喊一声\nB. 躲起来";
        let p = parse_output(raw, "local");
        assert_eq!(p.narration, "他走进院子，四下无人。");
        assert_eq!(p.choices.len(), 2);
        assert!(!p.warnings.is_empty());
    }

    #[test]
    fn api_mode_prefers_json() {
        let raw = r#"{"narration":"正文","choices":["A. 一","B. 二"],"state_delta":"无","summary_delta":"推进"}"#;
        let p = parse_output(raw, "api");
        assert_eq!(p.narration, "正文");
        assert_eq!(p.choices.len(), 2);
    }

    #[test]
    fn never_dead_ends_on_garbage() {
        let p = parse_output("模型只回了一句莫名其妙的话", "local");
        assert!(!p.narration.is_empty());
        assert!(!p.choices.is_empty()); // default "继续" is always present
    }

    fn marked(choices: &str) -> String {
        format!(
            "<<<NARRATION>>>\n他停住脚。\n<<<END_NARRATION>>>\n<<<CHOICES>>>\n{choices}\n<<<END_CHOICES>>>\n<<<STATE_DELTA>>>\n无\n<<<END_STATE_DELTA>>>\n<<<SUMMARY_DELTA>>>\n抵达门前。\n<<<END_SUMMARY_DELTA>>>"
        )
    }

    /// The exact bug from the user's screenshot.
    #[test]
    fn splits_glued_single_line_options() {
        let raw = marked("A. 沿着土路往村子深处走 B. 站在原地打量四周的房屋 C. 心里想着这里会不会有其他人");
        let p = parse_output(&raw, "local");
        assert_eq!(p.choices.len(), 3);
        assert_eq!(p.choices[0], "A. 沿着土路往村子深处走");
        assert_eq!(p.choices[1], "B. 站在原地打量四周的房屋");
        assert_eq!(p.choices[2], "C. 心里想着这里会不会有其他人");
        assert!(p.warnings.iter().any(|w| w.starts_with(CHOICES_SUSPECT)));
        assert!(p.needs_fix(), "glued options must trigger the manual-fix dialog");
    }

    #[test]
    fn multiline_options_do_not_regress() {
        let raw = marked("A. 推门进去\nB. 贴耳听一听\nC. 转身离开");
        let p = parse_output(&raw, "local");
        assert_eq!(p.choices.len(), 3);
        assert_eq!(p.choices[0], "A. 推门进去");
        assert_eq!(p.choices[2], "C. 转身离开");
        assert!(!p.warnings.iter().any(|w| w.starts_with(CHOICES_SUSPECT)));
    }

    #[test]
    fn does_not_split_label_inside_a_quote() {
        let raw = marked("A. 他说\"B. 不对\"，然后沉默\nB. 站在原地");
        let p = parse_output(&raw, "local");
        assert_eq!(p.choices.len(), 2);
        assert!(p.choices[0].contains("B. 不对"));
        assert_eq!(p.choices[1], "B. 站在原地");
    }

    #[test]
    fn supports_chinese_fullwidth_and_circled_labels() {
        for (a, b, c) in [
            ("一、甲", "二、乙", "三、丙"),
            ("Ａ．甲", "Ｂ．乙", "Ｃ．丙"),
            ("①甲", "②乙", "③丙"),
        ] {
            let raw = marked(&format!("{a}\n{b}\n{c}"));
            let p = parse_output(&raw, "local");
            assert_eq!(p.choices.len(), 3, "variant {a}");
            assert_eq!(p.choices[0], "A. 甲", "variant {a}");
            assert_eq!(p.choices[2], "C. 丙", "variant {a}");
        }
    }

    #[test]
    fn continuation_lines_join_previous_option() {
        let raw = marked("A. 沿着土路往村子深处走，\n一边打量四周的房屋\nB. 站在原地不动");
        let p = parse_output(&raw, "local");
        assert_eq!(p.choices.len(), 2);
        assert!(p.choices[0].contains("一边打量四周的房屋"));
        assert_eq!(p.choices[1], "B. 站在原地不动");
    }

    #[test]
    fn expands_glued_json_array_element() {
        let raw = r#"{"narration":"正文","choices":["A. 一 B. 二 C. 三"],"state_delta":"无","summary_delta":"推进"}"#;
        let p = parse_output(raw, "api");
        assert_eq!(p.choices.len(), 3);
        assert_eq!(p.choices[1], "B. 二");
        assert!(p.warnings.iter().any(|w| w.starts_with(CHOICES_SUSPECT)));
    }

    #[test]
    fn normal_json_array_stays_untouched() {
        let raw = r#"{"narration":"正文","choices":["A. 一","B. 二","C. 三"],"state_delta":"无","summary_delta":"推进"}"#;
        let p = parse_output(raw, "api");
        assert_eq!(p.choices.len(), 3);
        assert_eq!(p.choices[0], "A. 一");
        assert!(!p.warnings.iter().any(|w| w.starts_with(CHOICES_SUSPECT)));
    }

    // ---------- v2 four-block contract ----------

    fn four_blocks(stop: &str) -> String {
        format!(
            "<<<NARRATION>>>\n他推开门，屋里没有人。\n<<<END_NARRATION>>>\n\
             <<<ANALYSIS>>>\n{{\"world\":{{\"time\":\"傍晚\",\"place\":\"旧宅\"}},\"player\":{{\"mind\":\"警惕\"}},\"characters\":[{{\"name\":\"守门人\",\"current_state\":\"不在\",\"importance\":\"notable\",\"is_new\":true}}],\"summary\":\"主角进入旧宅。\",\"new_character_candidates\":[\"守门人\"]}}\n<<<END_ANALYSIS>>>\n\
             <<<STOP>>>\n{stop}\n<<<END_STOP>>>\n\
             <<<CHOICES>>>\nA. 喊一声\nB. 退回去\nC. 点灯\n<<<END_CHOICES>>>"
        )
    }

    #[test]
    fn parses_analysis_and_stop_true() {
        let p = parse_output(&four_blocks(r#"{"stop":true,"reason":"屋内有人要对话","trigger":"dialogue"}"#), "local");
        assert!(p.stop);
        assert_eq!(p.stop_trigger, "dialogue");
        assert_eq!(p.choices.len(), 3);
        let a = analysis_for(&p).unwrap();
        assert_eq!(a.world["place"].as_str(), Some("旧宅"));
        assert_eq!(a.player["mind"].as_str(), Some("警惕"));
        assert_eq!(a.characters.len(), 1);
        assert_eq!(a.characters[0].name, "守门人");
        assert_eq!(a.summary, "主角进入旧宅。");
    }

    #[test]
    fn keep_writing_turn_has_no_forced_choice() {
        let p = parse_output(&four_blocks(r#"{"stop":false,"reason":"继续描写环境"}"#), "local");
        assert!(!p.stop);
        // Choices are still parsed (the model may provide them), but no
        // synthetic "继续" is injected when options are genuinely absent.
        let empty = "<<<NARRATION>>>\n风从窗缝里钻进来。\n<<<END_NARRATION>>>\n<<<STOP>>>\n{\"stop\":false}\n<<<END_STOP>>>";
        let p2 = parse_output(empty, "local");
        assert!(!p2.stop);
        assert!(p2.choices.is_empty(), "no default option on a continue turn");
    }

    #[test]
    fn missing_stop_means_stop_conservatively() {
        let p = parse_output(&marked("A. 走\nB. 留"), "local");
        assert!(p.stop);
    }

    #[test]
    fn stop_accepts_loose_chinese_keywords() {
        let yes = "<<<NARRATION>>>\n他抬头。\n<<<END_NARRATION>>>\n<<<STOP>>>\n是，该停下来\n<<<END_STOP>>>\n<<<CHOICES>>>\nA. 走\nB. 留\n<<<END_CHOICES>>>";
        assert!(parse_output(yes, "local").stop);
        let no = "<<<NARRATION>>>\n他抬头。\n<<<END_NARRATION>>>\n<<<STOP>>>\n不停，继续写\n<<<END_STOP>>>";
        assert!(!parse_output(no, "local").stop);
    }

    #[test]
    fn api_json_analysis_object_is_preserved() {
        let raw = r#"{"narration":"正文","choices":["A. 一","B. 二"],"analysis":{"world":{"time":"夜"},"player":{},"summary":"推进"},"stop":{"stop":false,"trigger":"word_limit"}}"#;
        let p = parse_output(raw, "api");
        assert!(!p.stop);
        assert_eq!(p.stop_trigger, "word_limit");
        let a = analysis_for(&p).unwrap();
        assert_eq!(a.world["time"].as_str(), Some("夜"));
        assert_eq!(a.summary, "推进");
    }

    #[test]
    fn analysis_tolerates_fences_and_prose() {
        let a = parse_analysis("这是分析结果：\n```json\n{\"world\":{\"time\":\"夜\"},\"summary\":\"推进\"}\n```\n以上。").unwrap();
        assert_eq!(a.world["time"].as_str(), Some("夜"));
    }
}
