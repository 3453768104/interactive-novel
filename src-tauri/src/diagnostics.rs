//! 轻量日志与诊断包。
//!
//! 不引 tracing：单机应用只需要"出了事能回看当时发生了什么"，JSONL 足够，
//! 也方便直接 grep。诊断包会把日志 + 脱敏配置 + 库统计打成一个 zip。

use crate::error::{cmd_err, CmdResult};
use crate::state::AppState;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use tauri::State;

const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

fn log_path(st: &AppState) -> std::path::PathBuf {
    let day = chrono::Local::now().format("%Y-%m-%d");
    st.data_dir.join("logs").join(format!("app-{day}.jsonl"))
}

/// 追加一行结构化日志。失败一律忽略——日志永远不该影响主流程。
pub fn log(st: &AppState, kind: &str, data: Value) {
    let path = log_path(st);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = fs::rename(&path, path.with_extension("jsonl.old"));
        }
    }
    let line = json!({
        "ts": chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        "kind": kind,
        "data": data,
    });
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{line}");
    }
}

/// 把 api_key 之类的东西抹掉再打包。
fn redact(text: &str) -> String {
    let re = regex::Regex::new(r#"(?m)^(\s*api_key\s*:\s*).*$"#).unwrap();
    re.replace_all(text, |c: &regex::Captures| format!("{}\"***\"", &c[1]))
        .to_string()
}

#[tauri::command(rename_all = "snake_case")]
pub fn log_event(
    state: State<'_, AppState>,
    kind: String,
    data: Option<Value>,
) -> CmdResult<()> {
    log(&*state, &kind, data.unwrap_or(Value::Null));
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn log_tail(state: State<'_, AppState>, lines: Option<usize>) -> CmdResult<Vec<String>> {
    let st = &*state;
    let path = log_path(st);
    let text = fs::read_to_string(&path).unwrap_or_default();
    let take = lines.unwrap_or(200).clamp(1, 2000);
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(take);
    Ok(all[start..].iter().map(|s| s.to_string()).collect())
}

#[tauri::command(rename_all = "snake_case")]
pub fn export_diagnostics(state: State<'_, AppState>) -> CmdResult<String> {
    let st = &*state;
    let dir = st.data_dir.join("exports");
    fs::create_dir_all(&dir).map_err(cmd_err)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("diagnostics-{stamp}.zip"));
    let file = fs::File::create(&path).map_err(cmd_err)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // 1) 日志（最近 5 个文件）
    let log_dir = st.data_dir.join("logs");
    if let Ok(entries) = fs::read_dir(&log_dir) {
        let mut names: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        names.sort();
        for p in names.iter().rev().take(5) {
            if let Ok(body) = fs::read_to_string(p) {
                let name = format!("logs/{}", p.file_name().unwrap_or_default().to_string_lossy());
                let _ = zip.start_file(name, opts);
                let _ = zip.write_all(body.as_bytes());
            }
        }
    }

    // 2) 配置（脱敏）
    let cfg_dir = st.config_dir.clone();
    let mut stack = vec![cfg_dir.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let rel = p.strip_prefix(&cfg_dir).unwrap_or(&p).to_string_lossy().to_string();
            let Ok(body) = fs::read_to_string(&p) else { continue };
            let _ = zip.start_file(format!("config/{}", rel.replace('\\', "/")), opts);
            let _ = zip.write_all(redact(&body).as_bytes());
        }
    }

    // 3) 库统计（不含正文，避免诊断包过大）
    let stats = {
        let conn = st.db.lock().map_err(cmd_err)?;
        let count = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
        json!({
            "worlds": count("SELECT COUNT(*) FROM worlds"),
            "stories": count("SELECT COUNT(*) FROM stories"),
            "passages": count("SELECT COUNT(*) FROM passages"),
            "lore_docs": count("SELECT COUNT(*) FROM lore_docs"),
            "lore_chunks": count("SELECT COUNT(*) FROM lore_chunks"),
            "characters": count("SELECT COUNT(*) FROM characters"),
        })
    };
    let meta = json!({
        "app": "interactive-novel",
        "data_dir": st.data_dir.to_string_lossy(),
        "portable": st.portable,
        "defaults_version": crate::config_migrate::defaults_version(st),
        "db": stats,
        "generated_at": AppState::now(),
    });
    let _ = zip.start_file("diagnostics.json", opts);
    let _ = zip.write_all(
        serde_json::to_string_pretty(&meta)
            .unwrap_or_default()
            .as_bytes(),
    );

    zip.finish().map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into())
}
