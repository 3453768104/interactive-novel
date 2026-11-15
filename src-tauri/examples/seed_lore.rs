//! Seed a world + lore library from a single Markdown setting book.
//!
//! Run from `src-tauri/`:
//!
//! ```text
//! cargo run --example seed_lore --no-default-features -- \
//!     --file "C:\path\to\设定集.md" \
//!     --world "斗罗大陆（斗1）" \
//!     --event-bg "C:\path\to\事件背景.md"
//! ```
//!
//! It uses the same `AppState` + `lore` code path as the app, so chunking, sha
//! hashes and mirror files are identical to what the UI would produce. Close
//! the app first, or accept that concurrent writes may wait on the DB lock
//! (a 10s busy timeout is set below).

use interactive_novel_lib::{importer, lore, state::AppState};
use rusqlite::params;
use std::path::PathBuf;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let mut file: Option<PathBuf> = None;
    let mut world_name = "斗罗大陆（斗1）".to_string();
    let mut description = "斗罗大陆第一部同人写作与游玩用的世界。".to_string();
    let mut data_dir: Option<PathBuf> = None;
    let mut event_bg: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--file" => file = args.next().map(PathBuf::from),
            "--world" => world_name = args.next().unwrap_or(world_name),
            "--desc" => description = args.next().unwrap_or(description),
            "--data-dir" => data_dir = args.next().map(PathBuf::from),
            "--event-bg" => event_bg = args.next().map(PathBuf::from),
            other => eprintln!("[warn] 忽略未知参数 {other}"),
        }
    }
    let file = file.unwrap_or_else(|| {
        eprintln!("用法: cargo run --example seed_lore -- --file <设定集.md> [--world 名称] [--event-bg 文件] [--data-dir 目录]");
        std::process::exit(2);
    });

    // Default to the portable layout next to the compiled exe, which for
    // `cargo run` is exactly the same folder the dev app uses. Examples land in
    // target/debug/examples/, one level deeper than target/debug/.
    let data_dir = data_dir.unwrap_or_else(|| {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let mut dir = exe.parent().map(|d| d.to_path_buf()).unwrap_or_default();
        if matches!(
            dir.file_name().and_then(|n| n.to_str()),
            Some("examples") | Some("deps")
        ) {
            if let Some(parent) = dir.parent() {
                dir = parent.to_path_buf();
            }
        }
        dir.join("data")
    });
    println!("数据目录: {}", data_dir.display());

    let state = AppState::new(data_dir, None)?;
    state.db.lock()?.busy_timeout(Duration::from_secs(10))?;
    let world_id = ensure_world(&state, &world_name, &description)?;
    println!("世界观: {world_name}  ({world_id})");

    let md = std::fs::read_to_string(&file)?;
    let report = importer::import_bundle(&state, &world_id, &md)?;
    println!(
        "导入完成：新建分类 {} 个，新建卡片 {} 张，已存在跳过 {} 张",
        report.categories_created, report.cards_created, report.cards_skipped
    );
    if !report.samples.is_empty() {
        println!("示例卡片: {}", report.samples.join(" / "));
    }

    if let Some(bg) = event_bg {
        let text = std::fs::read_to_string(&bg)?;
        let conn = state.db.lock()?;
        conn.execute(
            "INSERT INTO world_event_background (world_id, content_md, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(world_id) DO UPDATE SET content_md = excluded.content_md, updated_at = excluded.updated_at",
            params![world_id, text, AppState::now()],
        )?;
        println!("事件背景已写入（{} 字）", text.chars().count());
    }

    Ok(())
}

/// Find a world by name, or create one (folder + `lore/` + mirror path).
fn ensure_world(state: &AppState, name: &str, description: &str) -> anyhow::Result<String> {
    let conn = state.db.lock()?;
    if let Ok(id) = conn.query_row("SELECT id FROM worlds WHERE name = ?1", params![name], |r| {
        r.get::<_, String>(0)
    }) {
        return Ok(id);
    }
    let id = AppState::new_id();
    let now = AppState::now();
    std::fs::create_dir_all(lore::lore_dir(state, &id))?;
    // Mirror path stays relative so the whole data folder can be moved.
    conn.execute(
        "INSERT INTO worlds (id, name, description, created_at, updated_at, mirror_path)
         VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
        params![id, name, description, now, id],
    )?;
    Ok(id)
}
