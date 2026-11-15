import { useState } from "react";
import { api } from "../../ipc";
import type { LoreCard, LoreCategory } from "../../types";

interface Props {
  worldId: string;
  categories: LoreCategory[];
  cards: LoreCard[];
  /** 从某张卡发起"扩写"时预填 */
  initialDocId?: string | null;
  mode: "local" | "api" | "paste";
  onClose: () => void;
  onSaved: () => void;
  onToast: (msg: string, kind?: "ok" | "error") => void;
}

type AiMode = "new" | "expand";

export default function LoreAiDialog({
  worldId, categories, cards, initialDocId, mode, onClose, onSaved, onToast,
}: Props) {
  const [aiMode, setAiMode] = useState<AiMode>(initialDocId ? "expand" : "new");
  const [title, setTitle] = useState("");
  const [brief, setBrief] = useState("");
  const [categoryId, setCategoryId] = useState<string>("");
  const [expandId, setExpandId] = useState<string>(initialDocId ?? "");
  const [length, setLength] = useState("medium");
  const [tone, setTone] = useState("canon");
  const [busy, setBusy] = useState(false);
  const [prompt, setPrompt] = useState("");
  const [text, setText] = useState("");
  const [pasteReply, setPasteReply] = useState("");
  const [promptOpen, setPromptOpen] = useState(false);

  const selectedCard = cards.find((c) => c.doc_id === expandId) ?? null;

  const generate = async () => {
    if (aiMode === "new" && !title.trim()) { onToast("先给这张卡起个标题", "error"); return; }
    if (aiMode === "expand" && !expandId) { onToast("选一张要扩写的卡", "error"); return; }
    setBusy(true);
    try {
      const r = await api.loreAiGenerate({
        world_id: worldId,
        title: aiMode === "new" ? title.trim() : (selectedCard?.title ?? ""),
        brief: brief.trim(),
        category_id: categoryId || null,
        expand_doc_id: aiMode === "expand" ? expandId : null,
        length,
        tone,
        mode,
      });
      setPrompt(r.prompt);
      if (r.mode === "paste") {
        setText("");
        onToast("提示词已生成：复制到网页 AI，再把回复贴回下方");
      } else {
        setText(r.text);
        onToast("已生成，可直接改再保存");
      }
    } catch (e) { onToast(String(e), "error"); }
    finally { setBusy(false); }
  };

  const save = async (asNew = false) => {
    if (!text.trim()) { onToast("还没有内容可以保存", "error"); return; }
    setBusy(true);
    try {
      if (aiMode === "new" || asNew) {
        if (asNew && aiMode === "expand") {
          const t = `${selectedCard?.title ?? "扩写"}·扩写`;
          await api.createLoreCard(worldId, selectedCard?.category_id ?? (categoryId || null), t, text);
          onToast(`已另存为新卡片《${t}》（原卡未动）`);
          onSaved();
          onClose();
          return;
        }
        const newTitle = title.trim();
        await api.createLoreCard(worldId, categoryId || null, newTitle, text);
        onToast(`已保存为新卡片《${newTitle}》`);
      } else {
        const detail = await api.getLoreCard(expandId);
        const full = `# ${detail.card.title}\n\n${text.trim()}\n`;
        await api.saveLoreDoc(expandId, full, detail.card.sha256, true);
        await api.updateLoreCard(expandId, {
          title: detail.card.title,
          category_id: detail.card.category_id,
          tags: detail.card.tags,
          summary: text.trim().slice(0, 80).replace(/\n/g, " "),
          timeline_event_ids: detail.timeline_event_ids,
        });
        onToast("已写回原卡片");
      }
      onSaved();
      onClose();
    } catch (e) { onToast(String(e), "error"); }
    finally { setBusy(false); }
  };

  const copyPrompt = async () => {
    try { await api.writeClipboard(prompt); onToast("提示词已复制"); }
    catch (e) { onToast(String(e), "error"); }
  };

  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div className="dialog" style={{ width: "min(780px, 100%)" }} onClick={(e) => e.stopPropagation()}>
        <h2>AI 辅助创作</h2>
        <div className="hint" style={{ marginBottom: 12 }}>
          生成的是**草稿**：先落在下面的编辑框里，你改完再保存。不会直接动你已有的设定。
        </div>

        <div className="row" style={{ marginBottom: 12 }}>
          <div className="segmented">
            <button className={aiMode === "new" ? "active" : ""} onClick={() => setAiMode("new")}>新建卡片</button>
            <button className={aiMode === "expand" ? "active" : ""} onClick={() => setAiMode("expand")}>扩写已有</button>
          </div>
          <span className="badge badge-mode">{mode === "paste" ? "粘贴模式" : mode}</span>
        </div>

        <div className="grid-2">
          <div className="field">
            <span className="label">{aiMode === "new" ? "标题" : "要扩写的卡片"}</span>
            {aiMode === "new" ? (
              <input className="input" value={title} placeholder="例如：武魂殿的等级制度"
                onChange={(e) => setTitle(e.target.value)} />
            ) : (
              <select className="select" value={expandId} onChange={(e) => setExpandId(e.target.value)}>
                <option value="">选择一张卡…</option>
                {cards.map((c) => <option key={c.doc_id} value={c.doc_id}>{c.title}</option>)}
              </select>
            )}
          </div>
          <div className="field">
            <span className="label">分类</span>
            <select className="select" value={categoryId} onChange={(e) => setCategoryId(e.target.value)}
              disabled={aiMode === "expand"}>
              <option value="">未分类</option>
              {categories.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </div>
        </div>

        <div className="field">
          <span className="label">写作要求（可留空）</span>
          <textarea className="textarea" rows={3} value={brief}
            placeholder="例如：写清武魂殿如何选拔、有哪些职级、和两大帝国的关系；不要涉及主角。"
            onChange={(e) => setBrief(e.target.value)} />
        </div>

        <div className="row" style={{ marginBottom: 12 }}>
          <label className="inline-num">篇幅
            <select className="select" value={length} onChange={(e) => setLength(e.target.value)}>
              <option value="short">短（300–500 字）</option>
              <option value="medium">中（600–900 字）</option>
              <option value="long">长（1000–1500 字）</option>
            </select>
          </label>
          <label className="inline-num">语气
            <select className="select" value={tone} onChange={(e) => setTone(e.target.value)}>
              <option value="canon">考据严谨</option>
              <option value="plain">通俗易读</option>
              <option value="epic">热血有画面</option>
            </select>
          </label>
          <div className="grow" />
          <button className="btn btn-primary" disabled={busy} onClick={() => void generate()}>
            {busy ? "生成中…" : "生成草稿"}
          </button>
        </div>

        {prompt && (
          <div className="field">
            <span className="label">
              提示词
              <button className="btn btn-sm btn-ghost" style={{ marginLeft: 8 }}
                onClick={() => setPromptOpen((v) => !v)}>{promptOpen ? "收起" : "展开"}</button>
              <button className="btn btn-sm" style={{ marginLeft: 6 }} onClick={() => void copyPrompt()}>复制</button>
            </span>
            {promptOpen && <div className="code-block" style={{ maxHeight: 220 }}>{prompt}</div>}
          </div>
        )}

        {mode === "paste" && prompt && (
          <div className="field">
            <span className="label">把网页 AI 的回复贴回这里</span>
            <textarea className="textarea" rows={4} value={pasteReply}
              onChange={(e) => { setPasteReply(e.target.value); setText(e.target.value); }} />
          </div>
        )}

        <div className="field">
          <span className="label">草稿（可直接编辑）</span>
          <textarea className="textarea mono" rows={12} value={text}
            placeholder="生成后出现在这里；也可以自己先写，再让 AI 续。"
            onChange={(e) => setText(e.target.value)} />
        </div>

        <div className="dialog-actions">
          <span className="hint" style={{ marginRight: "auto" }}>
            {text.trim() ? `${text.trim().length} 字` : "还没有内容"}
            {aiMode === "expand" && " · 保存会覆盖原卡正文"}
          </span>
          <button className="btn btn-ghost" onClick={onClose}>取消</button>
          {aiMode === "expand" && (
            <button className="btn" disabled={busy || !text.trim()} onClick={() => void save(true)}>
              另存为新卡
            </button>
          )}
          <button className="btn btn-primary" disabled={busy || !text.trim()} onClick={() => void save(false)}>
            {aiMode === "new" ? "保存为新卡片" : "写回原卡片"}
          </button>
        </div>
      </div>
    </div>
  );
}
