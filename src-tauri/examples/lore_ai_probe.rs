//! 冒烟：用真实库拼一次设定卡提示词，检查上下文与 SQL 都没问题。
//! cargo run --example lore_ai_probe --no-default-features

use interactive_novel_lib::commands_ai::{build_prompt, LoreAiRequest};
use interactive_novel_lib::state::AppState;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target/debug/data"));
    let st = AppState::new(dir, None).expect("data dir");
    let world: String = {
        let conn = st.db.lock().unwrap();
        conn.query_row("SELECT id FROM worlds ORDER BY created_at LIMIT 1", [], |r| r.get(0))
            .expect("world")
    };
    let req = LoreAiRequest {
        world_id: world,
        title: "武魂殿的等级制度".into(),
        brief: "写清选拔、职级、与两大帝国的关系".into(),
        category_id: None,
        expand_doc_id: None,
        length: Some("medium".into()),
        tone: Some("canon".into()),
        mode: Some("paste".into()),
    };
    let prompt = build_prompt(&st, &req).expect("prompt");
    println!("--- prompt ({} chars) ---", prompt.chars().count());
    println!("{}", prompt.chars().take(1200).collect::<String>());
}
