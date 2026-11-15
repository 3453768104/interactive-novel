//! 冒烟：对真实设定库建索引并打印几条查询的 BM25 排名。
//! cargo run --example retrieval_probe --no-default-features

use interactive_novel_lib::retrieval::{bm25_search, sync_fts};
use rusqlite::Connection;

fn main() {
    let data = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target/debug/data"));
    let conn = Connection::open(data.join("novel.db")).expect("open db");
    let world: String = conn
        .query_row("SELECT id FROM worlds ORDER BY created_at LIMIT 1", [], |r| r.get(0))
        .expect("world");
    let t0 = std::time::Instant::now();
    let inserted = sync_fts(&conn).unwrap();
    println!("sync_fts inserted {inserted} rows in {:?}", t0.elapsed());

    let queries = [
        "唐三用昊天锤砸门",
        "十万年魂环怎么吸收",
        "海神九考",
        "七宝琉璃宗为什么有钱",
        "小舞的兔耳被发现",
    ];
    for q in queries {
        let t = std::time::Instant::now();
        let hits = bm25_search(&conn, &world, q, 3);
        println!("\nQ: {q}  ({} hits, {:?})", hits.len(), t.elapsed());
        for (i, (id, score)) in hits.iter().enumerate() {
            let (title, heading): (String, String) = conn
                .query_row(
                    "SELECT d.title, c.heading_path FROM lore_chunks c JOIN lore_docs d ON d.id=c.doc_id WHERE c.id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            let content: String = conn
                .query_row("SELECT content FROM lore_chunks WHERE id=?1", [id], |r| r.get(0))
                .unwrap();
            println!("  {}. [{:.4}] {title} / {heading}", i + 1, score);
            println!("     {}", content.chars().take(70).collect::<String>());
        }
    }
}
