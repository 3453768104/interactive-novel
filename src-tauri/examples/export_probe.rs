//! 冒烟用：拿真实数据目录里的第一本剧本跑一次导出，打印前几行与统计。
//! cargo run --example export_probe --no-default-features

use interactive_novel_lib::export::{build_doc, render_markdown, ExportOptions};
use rusqlite::Connection;

fn main() {
    let data = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target/debug/data"));
    let conn = Connection::open(data.join("novel.db")).expect("open db");
    let (story_id, story_name): (String, String) = conn
        .query_row("SELECT id, name FROM stories ORDER BY created_at LIMIT 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("at least one story");
    let protagonist: String = conn
        .query_row(
            "SELECT name FROM protagonists WHERE story_id = ?1",
            [&story_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let mut stmt = conn
        .prepare(
            "SELECT turn_index, trigger_action_md, content_md FROM passages
             WHERE story_id = ?1 AND parse_status != 'pending' ORDER BY turn_index",
        )
        .unwrap();
    let rows: Vec<(i64, String, String)> = stmt
        .query_map([&story_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    for (chapter_chars, label) in [(0usize, "不分章"), (300, "每 300 字分章")] {
        let opts = ExportOptions {
            title: story_name.clone(),
            path: String::new(),
            format: "md".into(),
            include_hidden: false,
            chapter_chars,
            include_action: false,
        };
        let (doc, turns) = build_doc(&story_name, &protagonist, &rows, &opts);
        let chapters = doc.chapters.len();
        let body = render_markdown(&doc);
        println!("==== {label} | {turns} 段 / {chapters} 章 / {} 字 ====", body.chars().count());
        for line in body.lines().take(14) {
            println!("{line}");
        }
        println!();
    }
    println!("story={story_id} name={story_name} protagonist={protagonist} rows={}", rows.len());
}
