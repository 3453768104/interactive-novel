//! 设定检索：真 BM25（预分词 + FTS5）＋ 向量余弦 ＋ RRF 融合。
//!
//! 为什么自己做分词：FTS5 的 unicode61 会把一整段中文当成一个 token，
//! trigram 又只适合子串匹配。这里在**索引时**把中文切成「单字 + 二元组」、
//! ASCII 切成小写单词，用空格连起来存进 `lore_fts.tokens`，于是 unicode61
//! 按空格切分后就得到了可用词项，`bm25()` 也就能给出真正的相关性排序。
//!
//! 向量那一路是可选的：只有 `engines.yaml` 配了 `embedding.model` 才会走，
//! 否则自动退化成 BM25-only（RRF 对单路也成立）。

use crate::domain::{EmbeddingConfig, EnginesConfig, EngineProfile, Policy};
use crate::state::AppState;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/// 中文里出现频率极高、区分度为 0 的单字，索引与查询都丢掉。
const STOP_CHARS: &[char] = &[
    '的', '了', '是', '在', '和', '不', '有', '也', '就', '都', '而', '及', '与', '着', '或',
    '一', '个', '上', '中', '下', '为', '以', '对', '到', '被', '把', '让', '他', '她', '它',
    '我', '你', '们', '这', '那', '之', '其', '所', '又', '但', '却', '很', '会', '能', '要',
];

/// 中英混排文本 → 去重词项：ASCII 词（≥2 字符，小写）+ 中文字 + 中文二元组。
pub fn tokenize(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            word.push(c.to_ascii_lowercase());
        } else {
            if word.chars().count() >= 2 {
                out.push(word.clone());
            }
            word.clear();
        }
    }
    if word.chars().count() >= 2 {
        out.push(word);
    }
    let cjk: Vec<char> = text
        .chars()
        .filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c))
        .collect();
    for ch in &cjk {
        if !STOP_CHARS.contains(ch) {
            out.push(ch.to_string());
        }
    }
    for w in cjk.windows(2) {
        if STOP_CHARS.contains(&w[0]) && STOP_CHARS.contains(&w[1]) {
            continue;
        }
        out.push(w.iter().collect());
    }
    out.sort();
    out.dedup();
    out
}

fn index_text(title: &str, heading: &str, content: &str) -> String {
    // Title is indexed twice: cheap way to weight it above body text.
    let mut tokens = tokenize(title);
    tokens.extend(tokenize(title));
    tokens.extend(tokenize(heading));
    tokens.extend(tokenize(content));
    tokens.join(" ")
}

fn match_query(terms: &[String]) -> String {
    terms
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

// ---------------------------------------------------------------------------
// FTS index maintenance
// ---------------------------------------------------------------------------

/// Bring `lore_fts` in line with `lore_chunks`: drop orphan rows, index new
/// chunks. Cheap enough (a few hundred rows) to run on every startup.
pub fn sync_fts(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM lore_fts WHERE chunk_id NOT IN (SELECT id FROM lore_chunks)",
        [],
    )?;
    let mut stmt = conn.prepare(
        "SELECT c.id, d.title, c.heading_path, c.content
         FROM lore_chunks c JOIN lore_docs d ON d.id = c.doc_id
         WHERE c.id NOT IN (SELECT chunk_id FROM lore_fts)",
    )?;
    let rows: Vec<(i64, String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    let mut inserted = 0usize;
    for (id, title, heading, content) in rows {
        conn.execute(
            "INSERT INTO lore_fts (rowid, tokens, chunk_id) VALUES (?1, ?2, ?1)",
            params![id, index_text(&title, &heading, &content)],
        )?;
        inserted += 1;
    }
    Ok(inserted)
}

// ---------------------------------------------------------------------------
// BM25
// ---------------------------------------------------------------------------

/// Returns `(chunk_id, bm25)` best-first. FTS5's bm25() is negative-is-better.
pub fn bm25_search(
    conn: &Connection,
    world_id: &str,
    query: &str,
    top_k: usize,
) -> Vec<(i64, f64)> {
    let terms = tokenize(query);
    if terms.is_empty() || top_k == 0 {
        return Vec::new();
    }
    let q = match_query(&terms);
    let mut stmt = match conn.prepare(
        "SELECT chunk_id, bm25(lore_fts) AS score FROM lore_fts
         WHERE lore_fts MATCH ?1 ORDER BY score LIMIT ?2",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    // Ask for a wider window, then keep only this world's chunks.
    let rows: Vec<(i64, f64)> = match stmt.query_map(params![q, (top_k * 8) as i64], |r| {
        Ok((r.get(0)?, r.get(1)?))
    }) {
        Ok(it) => it.filter_map(|r| r.ok()).collect(),
        Err(_) => return Vec::new(),
    };
    let owned: Vec<(i64, f64)> = rows
        .into_iter()
        .filter(|(id, _)| chunk_in_world(conn, *id, world_id))
        .take(top_k)
        .collect();
    owned
}

fn chunk_in_world(conn: &Connection, chunk_id: i64, world_id: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM lore_chunks c JOIN lore_docs d ON d.id = c.doc_id
         WHERE c.id = ?1 AND d.world_id = ?2",
        params![chunk_id, world_id],
        |_| Ok(()),
    )
    .is_ok()
}

// ---------------------------------------------------------------------------
// Embeddings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EmbeddingProfile {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub dimensions: usize,
}

/// Resolve `engines.yaml` → embedding profile. `None` means vector search is
/// off (no model configured or no base_url).
pub fn embedding_profile(state: &AppState) -> Option<EmbeddingProfile> {
    let text = std::fs::read_to_string(state.config_dir.join("engines.yaml")).ok()?;
    let cfg: EnginesConfig = serde_yaml::from_str(&text).ok()?;
    let emb: EmbeddingConfig = cfg.embedding.unwrap_or_default();
    if emb.model.trim().is_empty() {
        return None;
    }
    let provider = emb.provider.trim().to_lowercase();
    // 缺省值按 provider 借：local 借用本地 chat 的地址，api / custom 借用联网 chat。
    let borrowed: Option<EngineProfile> = match provider.as_str() {
        "api" | "online" | "cloud" => cfg.engines.clone().and_then(|e| e.api),
        _ => cfg.engines.clone().and_then(|e| e.local),
    };

    let mut base = emb.base_url.trim().to_string();
    if base.is_empty() {
        base = borrowed.as_ref().map(|p| p.base_url.clone()).unwrap_or_default();
    }
    if base.is_empty() {
        return None;
    }
    // 显式配置优先；否则沿用同 provider 的 key（在线向量服务几乎都要 key）。
    let key = if !emb.api_key.trim().is_empty() {
        Some(emb.api_key.trim().to_string())
    } else {
        borrowed.as_ref().and_then(|p| p.api_key.clone())
    };

    Some(EmbeddingProfile {
        base_url: base.trim_end_matches('/').to_string(),
        api_key: key.filter(|k| !k.is_empty()),
        model: emb.model.trim().to_string(),
        dimensions: emb.dimensions,
    })
}

fn embed_texts(profile: &EmbeddingProfile, texts: &[String]) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()?;
    let url = format!("{}/embeddings", profile.base_url);
    let mut body = json!({ "model": profile.model, "input": texts });
    if profile.dimensions > 0 {
        body["dimensions"] = json!(profile.dimensions);
    }
    let mut req = client.post(&url).json(&body);
    if let Some(key) = profile.api_key.as_ref() {
        req = req.bearer_auth(key);
    }
    let resp = req.send()?.error_for_status()?;
    let v: Value = resp.json()?;
    let arr = v["data"]
        .as_array()
        .ok_or_else(|| anyhow!("embedding 返回体缺少 data 字段"))?;
    let mut out: Vec<Vec<f32>> = Vec::with_capacity(arr.len());
    for item in arr {
        let vec: Vec<f32> = item["embedding"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_f64().map(|f| f as f32))
                    .collect()
            })
            .unwrap_or_default();
        if vec.is_empty() {
            return Err(anyhow!("embedding 返回空向量"));
        }
        out.push(vec);
    }
    if out.len() != texts.len() {
        return Err(anyhow!(
            "embedding 数量不匹配：请求 {} 条，返回 {} 条",
            texts.len(),
            out.len()
        ));
    }
    Ok(out)
}

pub fn embed_query(profile: &EmbeddingProfile, query: &str) -> Result<Vec<f32>> {
    let mut v = embed_texts(profile, &[query.to_string()])?;
    let mut vec = v.pop().ok_or_else(|| anyhow!("embedding 为空"))?;
    normalize(&mut vec);
    Ok(vec)
}

fn normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn blob_to_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Build (or refresh) vectors for every chunk of a world that does not have
/// one for the current model. Returns `(embedded, skipped, model)`.
pub fn build_vectors(state: &AppState, world_id: &str) -> Result<(usize, usize, String)> {
    let profile = embedding_profile(state).ok_or_else(|| {
        anyhow!("engines.yaml 没有配置 embedding.model，向量检索不可用（BM25 仍可用）")
    })?;
    let conn = state.db.lock()?;
    let mut stmt = conn.prepare(
        "SELECT c.id, d.title, c.heading_path, c.content FROM lore_chunks c
         JOIN lore_docs d ON d.id = c.doc_id
         LEFT JOIN chunk_vectors v ON v.chunk_id = c.id
         WHERE d.world_id = ?1 AND (v.chunk_id IS NULL OR v.model_name <> ?2)",
    )?;
    let rows: Vec<(i64, String, String, String)> = stmt
        .query_map(params![world_id, profile.model], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    if rows.is_empty() {
        return Ok((0, 0, profile.model));
    }
    let mut embedded = 0usize;
    let now = AppState::now();
    for batch in rows.chunks(16) {
        let texts: Vec<String> = batch
            .iter()
            .map(|(_, title, heading, content)| format!("{title}\n{heading}\n{content}"))
            .collect();
        let mut vecs = embed_texts(&profile, &texts)?;
        for ((id, _, _, _), vec) in batch.iter().zip(vecs.iter_mut()) {
            normalize(vec);
            conn.execute(
                "INSERT OR REPLACE INTO chunk_vectors (chunk_id, vec, dim, model_name, updated_at)
                 VALUES (?1,?2,?3,?4,?5)",
                params![id, vec_to_blob(vec), vec.len() as i64, profile.model, now],
            )?;
            embedded += 1;
        }
    }
    let dim: i64 = conn
        .query_row(
            "SELECT dim FROM chunk_vectors v JOIN lore_chunks c ON c.id = v.chunk_id
             JOIN lore_docs d ON d.id = c.doc_id WHERE d.world_id = ?1 LIMIT 1",
            params![world_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    conn.execute(
        "INSERT OR REPLACE INTO embedding_meta (world_id, model_name, dim, built_at)
         VALUES (?1,?2,?3,?4)",
        params![world_id, profile.model, dim, now],
    )?;
    Ok((embedded, 0, profile.model))
}

// ---------------------------------------------------------------------------
// Hybrid retrieval (RRF)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct Hit {
    pub chunk_id: i64,
    pub title: String,
    pub heading: String,
    pub content: String,
    pub score: f64,
    /// bm25 | vec | rrf
    pub source: &'static str,
}

pub struct Pool {
    rows: HashMap<i64, (String, String, String)>,
    vectors: Vec<(i64, Vec<f32>)>,
}

/// Phase 1: everything that needs the DB lock (fast, no network).
pub fn prepare(conn: &Connection, world_id: &str) -> Pool {
    let mut rows = HashMap::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT c.id, d.title, c.heading_path, c.content FROM lore_chunks c
         JOIN lore_docs d ON d.id = c.doc_id WHERE d.world_id = ?1",
    ) {
        if let Ok(it) = stmt.query_map(params![world_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
        }) {
            for row in it.flatten() {
                rows.insert(row.0, (row.1, row.2, row.3));
            }
        }
    }
    let mut vectors = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT v.chunk_id, v.vec FROM chunk_vectors v
         JOIN lore_chunks c ON c.id = v.chunk_id
         JOIN lore_docs d ON d.id = c.doc_id WHERE d.world_id = ?1",
    ) {
        if let Ok(it) = stmt.query_map(params![world_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
        }) {
            for (id, blob) in it.flatten() {
                vectors.push((id, blob_to_vec(&blob)));
            }
        }
    }
    Pool { rows, vectors }
}

impl Pool {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn has_vectors(&self) -> bool {
        !self.vectors.is_empty()
    }
}

fn vec_search(pool: &Pool, query_vec: &[f32], top_k: usize) -> Vec<(i64, f64)> {
    let mut scored: Vec<(i64, f64)> = pool
        .vectors
        .iter()
        .filter(|(_, v)| v.len() == query_vec.len())
        .map(|(id, v)| {
            let dot: f32 = v.iter().zip(query_vec).map(|(a, b)| a * b).sum();
            (*id, dot as f64)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);
    scored
}

/// 加权求和的备选融合方式（`retrieval.fusion: weighted`）。分数先各自归一化，
/// 免得 bm25 的量级（几十）把余弦（0~1）完全压住。
fn weighted(bm25: &[(i64, f64)], vecs: &[(i64, f64)]) -> Vec<(i64, f64)> {
    let norm = |list: &[(i64, f64)], invert: bool| -> Vec<(i64, f64)> {
        let max = list
            .iter()
            .map(|(_, s)| if invert { -s } else { *s })
            .fold(0.0f64, f64::max)
            .max(1e-6);
        list.iter()
            .map(|(id, s)| (*id, (if invert { -s } else { *s }) / max))
            .collect()
    };
    let mut acc: HashMap<i64, f64> = HashMap::new();
    for (id, s) in norm(bm25, true) {
        *acc.entry(id).or_insert(0.0) += 0.55 * s;
    }
    for (id, s) in norm(vecs, false) {
        *acc.entry(id).or_insert(0.0) += 0.45 * s;
    }
    let mut out: Vec<(i64, f64)> = acc.into_iter().collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Reciprocal Rank Fusion over the two ranked lists.
fn rrf(lists: &[&[(i64, f64)]], k: usize) -> Vec<(i64, f64)> {
    let mut acc: HashMap<i64, f64> = HashMap::new();
    for list in lists {
        for (rank, (id, _)) in list.iter().enumerate() {
            *acc.entry(*id).or_insert(0.0) += 1.0 / (k as f64 + rank as f64 + 1.0);
        }
    }
    let mut out: Vec<(i64, f64)> = acc.into_iter().collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// A dead embedding endpoint must not be retried on every single turn.
static VEC_COOLDOWN: Mutex<Option<Instant>> = Mutex::new(None);

fn cooling_down() -> bool {
    VEC_COOLDOWN
        .lock()
        .ok()
        .and_then(|g| *g)
        .map(|t| t.elapsed() < Duration::from_secs(60))
        .unwrap_or(false)
}

fn trip_cooldown() {
    if let Ok(mut g) = VEC_COOLDOWN.lock() {
        *g = Some(Instant::now());
    }
}

/// Phase 2: optional query embedding (network) + fusion. Must run WITHOUT the
/// DB lock held, so a slow embedding server cannot freeze the app.
pub fn finish(
    state: &AppState,
    bm25: Vec<(i64, f64)>,
    pool: &Pool,
    query: &str,
    policy: &Policy,
) -> (Vec<Hit>, Option<String>) {
    let top_n = policy
        .modules
        .get("lore_chunks")
        .map(|m| if m.top_n > 0 { m.top_n } else { 3 })
        .unwrap_or(3);
    let bm25_k = policy.retrieval.bm25_topk.max(10);
    let vec_k = policy.retrieval.vec_topk.max(10);
    let rrf_k = policy.retrieval.rrf_k.max(1);

    let bm25 = bm25.into_iter().take(bm25_k).collect::<Vec<_>>();
    let mut warning = None;
    let mut vec_hits: Vec<(i64, f64)> = Vec::new();
    let vec_off = policy.retrieval.vec_topk == 0;
    match embedding_profile(state) {
        _ if vec_off => {}
        Some(_) if cooling_down() => {}
        Some(profile) if pool.has_vectors() => match embed_query(&profile, query) {
            Ok(qv) => vec_hits = vec_search(pool, &qv, vec_k),
            Err(e) => {
                trip_cooldown();
                warning = Some(format!("向量检索失败（已退回纯关键词，60 秒内不再重试）：{e}"));
            }
        },
        Some(_) => {
            warning = Some(
                "已配置 embedding 模型，但这个世界观还没有向量索引（设置 → 策略面板 → 建立索引）"
                    .into(),
            );
        }
        None => {}
    }

    let has_vec = !vec_hits.is_empty();
    let fused = if has_vec {
        if policy.retrieval.fusion.eq_ignore_ascii_case("weighted") {
            weighted(&bm25, &vec_hits)
        } else {
            rrf(&[&bm25, &vec_hits], rrf_k)
        }
    } else {
        bm25.clone()
    };
    let source = if has_vec { "rrf" } else { "bm25" };

    let hits = fused
        .into_iter()
        .take(top_n)
        .filter_map(|(id, score)| {
            pool.rows.get(&id).map(|(title, heading, content)| Hit {
                chunk_id: id,
                title: title.clone(),
                heading: heading.clone(),
                content: content.clone(),
                score,
                source,
            })
        })
        .collect();
    (hits, warning)
}

// ---------------------------------------------------------------------------
// Status / maintenance commands
// ---------------------------------------------------------------------------

/// 单独测一下 embedding 端点：填完在线 key 先点这个，比建整库索引快得多。
#[tauri::command]
pub async fn test_embedding(app: tauri::AppHandle) -> crate::error::CmdResult<Value> {
    use tauri::Manager;
    let profile = {
        let st = app.state::<AppState>();
        embedding_profile(&st).ok_or_else(|| {
            "没有可用的 embedding 配置：请填模型名（local 需勾本地服务，api 需填 key）".to_string()
        })?
    };
    let started = std::time::Instant::now();
    let model = profile.model.clone();
    let dims = tauri::async_runtime::spawn_blocking(move || -> Result<usize> {
        let v = embed_query(&profile, "连接测试 connection test")?;
        Ok(v.len())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(json!({
        "model": model,
        "dim": dims,
        "ms": started.elapsed().as_millis() as u64,
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub fn retrieval_status(
    state: tauri::State<'_, AppState>,
    world_id: String,
) -> crate::error::CmdResult<Value> {
    let st = &*state;
    let conn = st.db.lock().map_err(crate::error::cmd_err)?;
    let chunks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM lore_chunks c JOIN lore_docs d ON d.id = c.doc_id
             WHERE d.world_id = ?1",
            params![world_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let fts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM lore_fts f JOIN lore_chunks c ON c.id = f.chunk_id
             JOIN lore_docs d ON d.id = c.doc_id WHERE d.world_id = ?1",
            params![world_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let vectors: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM chunk_vectors v JOIN lore_chunks c ON c.id = v.chunk_id
             JOIN lore_docs d ON d.id = c.doc_id WHERE d.world_id = ?1",
            params![world_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    drop(conn);
    let profile = embedding_profile(st);
    Ok(json!({
        "chunks": chunks,
        "fts": fts,
        "vectors": vectors,
        "model": profile.as_ref().map(|p| p.model.clone()).unwrap_or_default(),
        "endpoint": profile.as_ref().map(|p| p.base_url.clone()).unwrap_or_default(),
        "vec_enabled": profile.is_some(),
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn rebuild_retrieval_index(
    app: tauri::AppHandle,
    world_id: String,
) -> crate::error::CmdResult<Value> {
    use tauri::Manager;
    // Embedding is blocking HTTP: keep it off the async runtime.
    let report = tauri::async_runtime::spawn_blocking(move || -> Result<Value> {
        let st = app.state::<AppState>();
        let indexed = {
            let conn = st.db.lock()?;
            sync_fts(&conn)?
        };
        let (embedded, _, model) = match build_vectors(&st, &world_id) {
            Ok(v) => (v.0, v.1, v.2),
            Err(e) => (0, 0, format!("向量未建立：{e}")),
        };
        Ok(json!({ "indexed": indexed, "embedded": embedded, "model": model }))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: anyhow::Error| e.to_string())?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_keeps_cjk_units_and_ascii_words() {
        let t = tokenize("唐三的昊天锤 Lantern 3F");
        assert!(t.contains(&"唐三".to_string()));
        assert!(t.contains(&"昊天".to_string()));
        assert!(t.contains(&"lantern".to_string()));
        assert!(!t.contains(&"的".to_string()), "stop chars are dropped");
    }

    #[test]
    fn rrf_prefers_documents_found_by_both_lists() {
        let bm25 = vec![(1, -1.0), (2, -2.0)];
        let vecs = vec![(2, 0.9), (3, 0.8)];
        let fused = rrf(&[&bm25, &vecs], 60);
        assert_eq!(fused[0].0, 2, "doc 2 appears in both lists, so it wins");
        assert!(fused.iter().any(|(id, _)| *id == 1));
        assert!(fused.iter().any(|(id, _)| *id == 3));
    }

    #[test]
    fn bm25_ranks_the_relevant_chunk_first() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("db/schema.sql")).unwrap();
        conn.execute_batch(
            "INSERT INTO worlds (id,name,description,created_at,updated_at,mirror_path)
               VALUES ('w','w','','t','t','w');
             INSERT INTO lore_docs (id,world_id,title,rel_path,sha256,chunk_count,origin,created_at)
               VALUES ('d1','w','武魂殿','a.md','',0,'import','t'),
                      ('d2','w','史莱克学院','b.md','',0,'import','t');
             INSERT INTO lore_chunks (id,doc_id,ordinal,heading_path,content,char_len,token_est,sha256)
               VALUES (1,'d1',0,'武魂殿','武魂殿是大陆最强的魂师组织，教皇比比东坐镇。',10,10,''),
                      (2,'d2',0,'史莱克学院','史莱克学院只收怪物，弗兰德当院长。',10,10,'');",
        )
        .unwrap();
        sync_fts(&conn).unwrap();
        let hits = bm25_search(&conn, "w", "教皇比比东在哪", 3);
        assert_eq!(hits.first().map(|h| h.0), Some(1));
    }
}
