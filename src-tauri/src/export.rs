//! 导出为小说：Markdown / EPUB / DOCX。
//!
//! 导出的是"读者看到的那本"：没有回合号、没有行动回显、没有 JSON，
//! 默认也不含〔暗场〕（那是作者用的）。三种格式共用同一份章节模型。

use crate::error::{cmd_err, CmdResult};
use crate::state::AppState;
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::State;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExportOptions {
    /// 书名。留空则用剧本名。
    #[serde(default)]
    pub title: String,
    /// 完整文件路径。留空则写到 `data/exports/<书名>-<时间>.<ext>`。
    #[serde(default)]
    pub path: String,
    /// md | epub | docx
    #[serde(default)]
    pub format: String,
    /// 是否把〔暗场〕段落也导出（作者版）。
    #[serde(default)]
    pub include_hidden: bool,
    /// 每章大致字数；0 = 不分章。
    #[serde(default)]
    pub chapter_chars: usize,
    /// 每段前是否附上"主角行动"。
    #[serde(default)]
    pub include_action: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub path: String,
    pub turns: usize,
    pub chapters: usize,
    pub chars: usize,
    pub format: String,
}

pub struct NovelChapter {
    pub title: String,
    pub paragraphs: Vec<String>,
}

pub struct NovelDoc {
    pub title: String,
    pub author: String,
    pub chapters: Vec<NovelChapter>,
}

fn hidden_line() -> Regex {
    Regex::new(r"^\s*(?:〔\s*暗场\s*〕|【\s*暗场\s*】|\[\s*暗场\s*\]|（\s*暗场\s*）|<<<\s*暗场\s*>>>)\s*")
        .unwrap()
}

fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\n' | '\r' | '\t' => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "novel".into()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn passage_to_paragraphs(content: &str, include_hidden: bool, hidden: &Regex) -> Vec<String> {
    let mut out = Vec::new();
    for raw in content.replace("\r\n", "\n").split('\n') {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("```") {
            continue;
        }
        let is_hidden = hidden.is_match(line);
        if is_hidden && !include_hidden {
            continue;
        }
        let text = if is_hidden {
            hidden.replace(line, "").to_string()
        } else {
            line.to_string()
        };
        if let Some(rest) = text.strip_prefix('◇').or_else(|| text.strip_prefix('◆')) {
            out.push("＊　＊　＊".to_string());
            let rest = rest.trim();
            if !rest.is_empty() {
                out.push(rest.to_string());
            }
            continue;
        }
        if !text.trim().is_empty() {
            out.push(text.trim().to_string());
        }
    }
    out
}

fn clean_action(action: &str) -> String {
    action
        .trim()
        .trim_start_matches(|c: char| {
            matches!(c, 'A'..='D' | 'a'..='d' | '一'..='四' | '①'..='④')
        })
        .trim_start_matches(['.', '、', '．', ':', '：'])
        .trim()
        .to_string()
}

/// 把回合拼成章节模型。返回 (doc, 实际写入的回合数)。
pub fn build_doc(
    title: &str,
    protagonist: &str,
    rows: &[(i64, String, String)],
    options: &ExportOptions,
) -> (NovelDoc, usize) {
    let hidden = hidden_line();
    let mut doc = NovelDoc {
        title: title.to_string(),
        author: protagonist.trim().to_string(),
        chapters: Vec::new(),
    };
    let mut pending: Vec<String> = Vec::new();
    let mut chapter_len = 0usize;
    let mut turns = 0usize;

    let flush = |doc: &mut NovelDoc, pending: &mut Vec<String>| {
        if pending.is_empty() {
            return;
        }
        let title = if options.chapter_chars > 0 {
            format!("第 {} 章", doc.chapters.len() + 1)
        } else {
            String::new()
        };
        doc.chapters.push(NovelChapter {
            title,
            paragraphs: std::mem::take(pending),
        });
    };

    for (_idx, action, content) in rows {
        let paras = passage_to_paragraphs(content, options.include_hidden, &hidden);
        if paras.is_empty() {
            continue;
        }
        turns += 1;
        if options.include_action && !action.trim().is_empty() {
            pending.push(format!("（行动：{}）", clean_action(action)));
        }
        let len: usize = paras.iter().map(|p| p.chars().count()).sum();
        pending.extend(paras);
        chapter_len += len;
        if options.chapter_chars > 0 && chapter_len >= options.chapter_chars {
            flush(&mut doc, &mut pending);
            chapter_len = 0;
        }
    }
    flush(&mut doc, &mut pending);
    if doc.chapters.is_empty() {
        doc.chapters.push(NovelChapter {
            title: String::new(),
            paragraphs: vec![String::new()],
        });
    }
    (doc, turns)
}

pub fn render_markdown(doc: &NovelDoc) -> String {
    let mut out = format!("# {}\n\n", doc.title);
    if !doc.author.is_empty() {
        out.push_str(&format!("> 主角：{}\n\n", doc.author));
    }
    for ch in &doc.chapters {
        if !ch.title.is_empty() {
            out.push_str(&format!("## {}\n\n", ch.title));
        }
        for p in &ch.paragraphs {
            out.push_str(p);
            out.push_str("\n\n");
        }
    }
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn zip_file(
    w: &mut zip::ZipWriter<fs::File>,
    name: &str,
    body: &[u8],
    stored: bool,
) -> anyhow::Result<()> {
    use zip::write::SimpleFileOptions;
    let method = if stored {
        zip::CompressionMethod::Stored
    } else {
        zip::CompressionMethod::Deflated
    };
    w.start_file(name, SimpleFileOptions::default().compression_method(method))?;
    w.write_all(body)?;
    Ok(())
}

fn write_epub(doc: &NovelDoc, path: &Path) -> anyhow::Result<()> {
    let file = fs::File::create(path)?;
    let mut z = zip::ZipWriter::new(file);

    // mimetype 必须是第一个、且不压缩
    zip_file(&mut z, "mimetype", b"application/epub+zip", true)?;
    zip_file(
        &mut z,
        "META-INF/container.xml",
        br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#,
        false,
    )?;

    let mut manifest = String::new();
    let mut spine = String::new();
    let mut nav_items = String::new();
    for (i, ch) in doc.chapters.iter().enumerate() {
        let id = format!("chap{}", i + 1);
        let name = format!("{id}.xhtml");
        manifest.push_str(&format!(
            "    <item id=\"{id}\" href=\"{name}\" media-type=\"application/xhtml+xml\"/>\n"
        ));
        spine.push_str(&format!("    <itemref idref=\"{id}\"/>\n"));
        let label = if ch.title.is_empty() {
            format!("第 {} 节", i + 1)
        } else {
            ch.title.clone()
        };
        nav_items.push_str(&format!(
            "      <li><a href=\"{name}\">{}</a></li>\n",
            xml_escape(&label)
        ));

        let mut body = String::new();
        if !ch.title.is_empty() {
            body.push_str(&format!("<h2>{}</h2>\n", xml_escape(&ch.title)));
        }
        for p in &ch.paragraphs {
            if p.trim().is_empty() {
                continue;
            }
            body.push_str(&format!("<p>{}</p>\n", xml_escape(p)));
        }
        let xhtml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"zh-CN\">\n\
             <head><title>{}</title><link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/></head>\n\
             <body>\n{body}\n</body>\n</html>",
            xml_escape(&label)
        );
        zip_file(&mut z, &format!("OEBPS/{name}"), xhtml.as_bytes(), false)?;
    }

    let css = "body{font-family:serif;line-height:1.8;margin:1em}\nh2{font-size:1.2em;margin:1.2em 0 .8em}\np{text-indent:2em;margin:0 0 .6em}";
    zip_file(&mut z, "OEBPS/style.css", css.as_bytes(), false)?;

    let nav = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"zh-CN\">\n\
         <head><title>目录</title></head>\n<body>\n\
         <nav epub:type=\"toc\" id=\"toc\"><h1>目录</h1>\n<ol>\n{nav_items}</ol>\n</nav>\n\
         </body>\n</html>"
    );
    zip_file(&mut z, "OEBPS/nav.xhtml", nav.as_bytes(), false)?;
    manifest.push_str(
        "    <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n\
         \x20   <item id=\"css\" href=\"style.css\" media-type=\"text/css\"/>\n",
    );

    let opf = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"bookid\">\n\
         <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n\
         <dc:identifier id=\"bookid\">urn:uuid:{uuid}</dc:identifier>\n\
         <dc:title>{title}</dc:title>\n<dc:language>zh-CN</dc:language>\n\
         <dc:creator>{author}</dc:creator>\n</metadata>\n\
         <manifest>\n{manifest}</manifest>\n\
         <spine>\n{spine}</spine>\n</package>",
        uuid = uuid::Uuid::new_v4(),
        title = xml_escape(&doc.title),
        author = xml_escape(&doc.author),
    );
    zip_file(&mut z, "OEBPS/content.opf", opf.as_bytes(), false)?;
    z.finish()?;
    Ok(())
}

fn docx_paragraph(text: &str, heading: bool) -> String {
    let run_props = if heading {
        "<w:rPr><w:b/><w:sz w:val=\"32\"/></w:rPr>"
    } else {
        ""
    };
    let spacing = if heading {
        "<w:spacing w:before=\"240\" w:after=\"120\"/>"
    } else {
        "<w:spacing w:line=\"360\" w:lineRule=\"auto\"/><w:ind w:firstLine=\"480\"/>"
    };
    format!(
        "<w:p><w:pPr>{spacing}</w:pPr><w:r>{run_props}<w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        xml_escape(text)
    )
}

fn write_docx(doc: &NovelDoc, path: &Path) -> anyhow::Result<()> {
    let file = fs::File::create(path)?;
    let mut z = zip::ZipWriter::new(file);
    zip_file(
        &mut z,
        "[Content_Types].xml",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
          <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n\
          <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n\
          <Default Extension=\"xml\" ContentType=\"application/xml\"/>\n\
          <Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\n\
          </Types>",
        false,
    )?;
    zip_file(
        &mut z,
        "_rels/.rels",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
          <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n\
          <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\n\
          </Relationships>",
        false,
    )?;

    let mut body = String::new();
    body.push_str(&format!(
        "<w:p><w:pPr><w:jc w:val=\"center\"/><w:spacing w:after=\"240\"/></w:pPr><w:r><w:rPr><w:b/><w:sz w:val=\"48\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        xml_escape(&doc.title)
    ));
    if !doc.author.is_empty() {
        body.push_str(&format!(
            "<w:p><w:pPr><w:jc w:val=\"center\"/><w:spacing w:after=\"360\"/></w:pPr><w:r><w:rPr><w:color w:val=\"666666\"/></w:rPr><w:t xml:space=\"preserve\">主角：{}</w:t></w:r></w:p>",
            xml_escape(&doc.author)
        ));
    }
    for ch in &doc.chapters {
        if !ch.title.is_empty() {
            body.push_str(&docx_paragraph(&ch.title, true));
        }
        for p in &ch.paragraphs {
            if p.trim().is_empty() {
                continue;
            }
            body.push_str(&docx_paragraph(p, false));
        }
    }

    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n\
         <w:body>\n{body}\n\
         <w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\"/></w:sectPr>\n\
         </w:body></w:document>"
    );
    zip_file(&mut z, "word/document.xml", document.as_bytes(), false)?;
    z.finish()?;
    Ok(())
}

fn normalize_format(raw: &str) -> &'static str {
    match raw.trim().to_lowercase().as_str() {
        "epub" => "epub",
        "docx" | "doc" | "word" => "docx",
        _ => "md",
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn export_novel(
    state: State<'_, AppState>,
    story_id: String,
    options: ExportOptions,
) -> CmdResult<ExportResult> {
    let st = &*state;
    let conn = st.db.lock().map_err(cmd_err)?;
    let story_name: String = conn
        .query_row(
            "SELECT name FROM stories WHERE id = ?1",
            params![story_id],
            |r| r.get(0),
        )
        .map_err(|_| "剧本不存在".to_string())?;
    let protagonist: String = conn
        .query_row(
            "SELECT name FROM protagonists WHERE story_id = ?1",
            params![story_id],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_default();
    let mut stmt = conn
        .prepare(
            "SELECT turn_index, trigger_action_md, content_md FROM passages
             WHERE story_id = ?1 AND parse_status != 'pending' ORDER BY turn_index",
        )
        .map_err(cmd_err)?;
    let rows: Vec<(i64, String, String)> = stmt
        .query_map(params![story_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(cmd_err)?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    drop(conn);

    let format = normalize_format(&options.format);
    let title = if options.title.trim().is_empty() {
        story_name
    } else {
        options.title.trim().to_string()
    };
    let (doc, turns) = build_doc(&title, &protagonist, &rows, &options);
    let body = render_markdown(&doc);
    let chars = body.chars().count();

    let path = if options.path.trim().is_empty() {
        let dir = st.data_dir.join("exports");
        fs::create_dir_all(&dir).map_err(cmd_err)?;
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M");
        dir.join(format!("{}-{stamp}.{format}", sanitize(&title)))
    } else {
        let p = PathBuf::from(options.path.trim());
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(cmd_err)?;
        }
        p
    };

    match format {
        "epub" => write_epub(&doc, &path).map_err(|e| e.to_string())?,
        "docx" => write_docx(&doc, &path).map_err(|e| e.to_string())?,
        _ => fs::write(&path, body).map_err(cmd_err)?,
    }
    crate::diagnostics::log(
        st,
        "export",
        serde_json::json!({ "format": format, "turns": turns, "path": path.to_string_lossy() }),
    );

    Ok(ExportResult {
        path: path.to_string_lossy().into(),
        turns,
        chapters: doc.chapters.len(),
        chars,
        format: format.to_string(),
    })
}

/// Reveal a file (or folder) in the OS shell.
#[tauri::command(rename_all = "snake_case")]
pub fn open_in_shell(path: String) -> CmdResult<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(chapter_chars: usize) -> ExportOptions {
        ExportOptions {
            title: "测试书".into(),
            path: String::new(),
            format: "md".into(),
            include_hidden: false,
            chapter_chars,
            include_action: false,
        }
    }

    fn rows() -> Vec<(i64, String, String)> {
        vec![
            (
                0,
                "A. 走".into(),
                "他推门进去。\n〔暗场〕小舞心里一沉。\n◇\n另一处。".into(),
            ),
            (1, "B. 停".into(), "他停下了。".into()),
        ]
    }

    #[test]
    fn markdown_drops_hidden_and_keeps_scene_breaks() {
        let (doc, turns) = build_doc("书", "秋", &rows(), &opts(0));
        let md = render_markdown(&doc);
        assert_eq!(turns, 2);
        assert!(!md.contains("暗场"));
        assert!(md.contains("＊　＊　＊"));
        assert!(md.starts_with("# 书"));
    }

    #[test]
    fn author_mode_keeps_hidden_beats() {
        let mut o = opts(0);
        o.include_hidden = true;
        let (doc, _) = build_doc("书", "秋", &rows(), &o);
        assert!(render_markdown(&doc).contains("小舞心里一沉"));
    }

    #[test]
    fn filenames_are_sanitized() {
        assert_eq!(sanitize("斗罗/同人:卷一"), "斗罗_同人_卷一");
        assert_eq!(sanitize("   "), "novel");
    }

    #[test]
    fn epub_and_docx_are_valid_zips() {
        let (doc, _) = build_doc("书", "秋", &rows(), &opts(300));
        let dir = std::env::temp_dir().join(format!("novel-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let epub = dir.join("a.epub");
        let docx = dir.join("a.docx");
        write_epub(&doc, &epub).unwrap();
        write_docx(&doc, &docx).unwrap();
        for p in [&epub, &docx] {
            let f = fs::File::open(p).unwrap();
            let z = zip::ZipArchive::new(f).unwrap();
            assert!(z.len() >= 3, "{} 至少要有几个部件", p.display());
        }
        let f = fs::File::open(&epub).unwrap();
        let mut z = zip::ZipArchive::new(f).unwrap();
        assert!(z.by_name("mimetype").is_ok());
        assert!(z.by_name("OEBPS/content.opf").is_ok());
        let _ = fs::remove_dir_all(&dir);
    }
}
