import { useState } from "react";
import { api } from "../../ipc";
import type { ExportResult } from "../../types";

interface Props {
  storyId: string;
  defaultTitle: string;
  onClose: () => void;
  onToast: (msg: string, kind?: "ok" | "error") => void;
}

export default function ExportDialog({ storyId, defaultTitle, onClose, onToast }: Props) {
  const [title, setTitle] = useState(defaultTitle);
  const [path, setPath] = useState("");
  const [format, setFormat] = useState<"md" | "epub" | "docx">("md");
  const [chapterChars, setChapterChars] = useState(3000);
  const [includeHidden, setIncludeHidden] = useState(false);
  const [includeAction, setIncludeAction] = useState(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ExportResult | null>(null);

  const run = async () => {
    setBusy(true);
    try {
      const r = await api.exportNovel(storyId, {
        title: title.trim(),
        path: path.trim(),
        format,
        include_hidden: includeHidden,
        chapter_chars: Math.max(0, Math.round(chapterChars) || 0),
        include_action: includeAction,
      });
      setResult(r);
      onToast(`已导出 ${r.turns} 段 / ${r.chars} 字`);
    } catch (e) { onToast(String(e), "error"); }
    finally { setBusy(false); }
  };

  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div className="dialog" style={{ width: "min(560px, 100%)" }}
        onClick={(e) => e.stopPropagation()}>
        <h2>导出为小说</h2>

        <div className="export-body">
          <div className="hint">
            导出的是读者看到的那本小说：没有回合号、没有行动回显、也没有 JSON。
            输出 Markdown（`#` 标题 + 空行分段），Word / EPUB / 排版工具都能直接吃。
          </div>

          <div className="field">
            <span className="label">书名</span>
            <input className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
          </div>
          <div className="field">
            <span className="label">格式</span>
            <div className="row">
              {([["md", "Markdown"], ["epub", "EPUB 电子书"], ["docx", "Word 文档"]] as const).map(([k, label]) => (
                <button key={k} className={`btn btn-sm ${format === k ? "btn-primary" : ""}`}
                  onClick={() => { setFormat(k); setResult(null); }}>{label}</button>
              ))}
            </div>
            <div className="hint">
              EPUB 带目录，可直接导入阅读器；Word 是标准 docx（宋正式排版，可再改）。
            </div>
          </div>
          <div className="field">
            <span className="label">每章字数（0 = 不分章，正文连排）</span>
            <input className="input" type="number" min={0} step={500} value={chapterChars}
              onChange={(e) => setChapterChars(Number(e.target.value))} />
          </div>
          <div className="field">
            <span className="label">保存到（留空 = 程序目录 data/exports/）</span>
            <div className="row">
              <input className="input grow" value={path}
                placeholder={`data/exports/${defaultTitle || "书名"}-时间.${format}`}
                onChange={(e) => setPath(e.target.value)} />
              <button className="btn btn-sm" onClick={() => {
                void api.pickSavePath(`${title || "小说"}.${format}`, "导出为小说")
                  .then((p) => { if (p) setPath(p); })
                  .catch((e) => onToast(String(e), "error"));
              }}>选择…</button>
            </div>
          </div>

          <label className="check-row" style={{ marginTop: 6 }}>
            <input type="checkbox" checked={includeHidden}
              onChange={(e) => setIncludeHidden(e.target.checked)} />
            包含〔暗场〕（作者版；主角看不到的那些段落）
          </label>
          <label className="check-row">
            <input type="checkbox" checked={includeAction}
              onChange={(e) => setIncludeAction(e.target.checked)} />
            每段前附上「主角行动」（更像工作稿，不像小说）
          </label>

          {result && (
            <div className="export-result">
              <div className="hint">
                {result.format.toUpperCase()} · {result.turns} 段 · {result.chapters > 0 ? `${result.chapters} 章 · ` : ""}{result.chars} 字
              </div>
              <div className="code-block" style={{ maxHeight: 80 }}>{result.path}</div>
              <div className="row" style={{ marginTop: 8 }}>
                <button className="btn btn-sm btn-primary"
                  onClick={() => void api.openInShell(result.path).catch((e) => onToast(String(e), "error"))}>
                  打开文件
                </button>
                <button className="btn btn-sm"
                  onClick={() => void navigator.clipboard.writeText(result.path)
                    .then(() => onToast("已复制路径")).catch(() => onToast("复制失败", "error"))}>
                  复制路径
                </button>
              </div>
            </div>
          )}
        </div>

        <div className="dialog-actions">
          <button className="btn btn-ghost" onClick={onClose}>取消</button>
          <button className="btn btn-primary" disabled={busy} onClick={() => void run()}>
            {busy ? "导出中…" : "导出"}
          </button>
        </div>
      </div>
    </div>
  );
}
