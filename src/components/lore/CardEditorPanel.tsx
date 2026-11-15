import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { api, CONFLICT_PREFIX } from "../../ipc";
import type {
  LoreCard, LoreCardDetail, LoreCategory, Nav, TimelineEvent,
} from "../../types";

interface Props {
  card: LoreCard;
  categories: LoreCategory[];
  events: TimelineEvent[];
  nav: Nav;
  onChanged: () => void;
  onClose: () => void;
}

interface Conflict {
  sha256: string;
  content: string;
}

/**
 * Holds the ONE editing buffer for a card. The visual form and the Markdown
 * source tab are two views of the same `value`, so the existing two-way
 * conflict protocol (app buffer vs external file) still applies unchanged.
 * Metadata lives only in SQLite and never touches the .md / its sha.
 */
export default function CardEditorPanel({
  card,
  categories,
  events,
  nav,
  onChanged,
  onClose,
}: Props) {
  const [detail, setDetail] = useState<LoreCardDetail | null>(null);
  const [value, setValue] = useState("");
  const [baseSha, setBaseSha] = useState(card.sha256);
  const [dirty, setDirty] = useState(false);
  const [tab, setTab] = useState<"form" | "source">("form");
  const [title, setTitle] = useState(card.title);
  const [categoryId, setCategoryId] = useState<string | null>(card.category_id);
  const [tags, setTags] = useState(card.tags.join(", "));
  const [summary, setSummary] = useState(card.summary);
  const [eventIds, setEventIds] = useState<string[]>([]);
  const [conflict, setConflict] = useState<Conflict | null>(null);
  const [saving, setSaving] = useState(false);

  const load = async () => {
    const d = await api.getLoreCard(card.doc_id);
    setDetail(d);
    setValue(d.content);
    setBaseSha(d.card.sha256);
    setDirty(false);
    setTitle(d.card.title);
    setCategoryId(d.card.category_id);
    setTags(d.card.tags.join(", "));
    setSummary(d.card.summary);
    setEventIds(d.timeline_event_ids);
  };

  useEffect(() => { void load().catch((e) => nav.toast(String(e), "error")); }, [card.doc_id]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<{ world_id: string; rel_path: string }>("lore://changed", async (event) => {
      if (event.payload.rel_path !== card.rel_path) return;
      try {
        const fresh = await api.readLoreDoc(card.doc_id);
        if (dirty) {
          setConflict({ sha256: fresh.sha256, content: fresh.content });
        } else {
          await load();
          nav.toast("已同步外部编辑器的修改");
        }
      } catch (e) {
        nav.toast(String(e), "error");
      }
    }).then((f) => (unlisten = f));
    return () => unlisten?.();
  }, [card.doc_id, card.rel_path, dirty]);

  const patch = () => ({
    title,
    category_id: categoryId,
    tags: tags.split(/[,，]/).map((s) => s.trim()).filter(Boolean),
    summary,
    timeline_event_ids: eventIds,
  });

  const save = async (forceSha?: string, force = false) => {
    if (saving) return;
    setSaving(true);
    try {
      // Metadata first: pure DB write, sha unchanged, no watcher event.
      await api.updateLoreCard(card.doc_id, patch());
      // Body only when the shared buffer actually changed.
      if (dirty) {
        const result = await api.saveLoreDoc(
          card.doc_id, value, forceSha ?? baseSha, force,
        );
        setBaseSha(result.sha256);
        setDirty(false);
      }
      setConflict(null);
      onChanged();
      nav.toast("卡片已保存");
    } catch (e) {
      const msg = String(e);
      if (msg.startsWith(CONFLICT_PREFIX)) {
        try {
          setConflict(JSON.parse(msg.slice(CONFLICT_PREFIX.length)) as Conflict);
          nav.toast("检测到外部编辑器改动，请选择处理方式", "error");
          return;
        } catch { /* fall through */ }
      }
      nav.toast(msg, "error");
    } finally {
      setSaving(false);
    }
  };

  const takeExternal = () => {
    if (!conflict) return;
    setValue(conflict.content);
    setBaseSha(conflict.sha256);
    setDirty(false);
    setConflict(null);
    nav.toast("已载入外部版本");
  };

  const keepBoth = async () => {
    if (!conflict) return;
    try {
      const created = await api.createLoreDoc(card.world_id, `${title}-我的改动`);
      await api.saveLoreDoc(created.id, value, created.sha256, true);
      onChanged();
      nav.toast(`未保存内容已另存为「${created.title}」`);
      takeExternal();
    } catch (e) {
      nav.toast(String(e), "error");
    }
  };

  const toggleEvent = (id: string) =>
    setEventIds((prev) => prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]);

  return (
    <div className="dialog-backdrop">
      <div className="dialog lore-dialog">
        <div className="page-header" style={{ marginBottom: 12 }}>
          <div>
            <h1 style={{ fontSize: 18 }}>编辑设定卡片</h1>
            <div className="sub mono">{card.rel_path}</div>
          </div>
          <div className="row">
            <span className={`badge ${dirty ? "badge-dirty" : "badge-clean"}`}>
              {dirty ? "正文未保存" : "正文已同步"}
            </span>
            <button className="btn btn-sm" onClick={onClose}>关闭</button>
          </div>
        </div>

        <div className="tabs">
          <button className={`tab ${tab === "form" ? "active" : ""}`} onClick={() => setTab("form")}>
            可视化表单
          </button>
          <button className={`tab ${tab === "source" ? "active" : ""}`} onClick={() => setTab("source")}>
            Markdown 源码
          </button>
        </div>

        {conflict && (
          <div className="card" style={{ borderColor: "#f0cdd5", marginBottom: 14 }}>
            <div className="card-title">应用内（表单或源码）还有未保存的修改，外部编辑器也保存过这份文件</div>
            <div className="hint">不会自动合并、也不会静默覆盖，请选择处理方式：</div>
            <div className="grid-2" style={{ marginTop: 10 }}>
              <div>
                <div className="hint">外部版本（前 300 字）</div>
                <div className="code-block" style={{ maxHeight: 120 }}>{conflict.content.slice(0, 300)}</div>
              </div>
              <div>
                <div className="hint">应用内未保存版本（前 300 字）</div>
                <div className="code-block" style={{ maxHeight: 120 }}>{value.slice(0, 300)}</div>
              </div>
            </div>
            <div className="row" style={{ marginTop: 10 }}>
              <button className="btn btn-sm" onClick={() => void keepBoth()}>两边都留</button>
              <button className="btn btn-sm" onClick={takeExternal}>载入外部</button>
              <button className="btn btn-sm btn-primary" onClick={() => void save(conflict.sha256, true)}>
                保留我的
              </button>
            </div>
          </div>
        )}

        {tab === "form" ? (
          <>
            <div className="form-grid">
              <div className="field">
                <span className="label">标题</span>
                <input className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
              </div>
              <div className="field">
                <span className="label">分类</span>
                <select className="select" value={categoryId ?? ""} onChange={(e) => setCategoryId(e.target.value || null)}>
                  <option value="">未分类</option>
                  {categories.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
                </select>
              </div>
            </div>
            <div className="field">
              <span className="label">标签（逗号分隔）</span>
              <input className="input" value={tags} onChange={(e) => setTags(e.target.value)} placeholder="魔法, 北境, 禁忌" />
            </div>
            <div className="field">
              <span className="label">简介</span>
              <textarea className="textarea" style={{ minHeight: 60 }} value={summary}
                onChange={(e) => setSummary(e.target.value)} placeholder="一两句说明这张卡片讲什么" />
            </div>
            {events.length > 0 && (
              <div className="field">
                <span className="label">关联时间线事件</span>
                <div className="check-row">
                  {events.map((ev) => (
                    <label key={ev.id}>
                      <input type="checkbox" checked={eventIds.includes(ev.id)} onChange={() => toggleEvent(ev.id)} />
                      {ev.year_label ? `${ev.year_label} ` : ""}{ev.title}
                    </label>
                  ))}
                </div>
              </div>
            )}
            <div className="field">
              <span className="label">正文（就是 .md 原文，改这里与改「源码」是同一份内容）</span>
              <textarea className="textarea" style={{ minHeight: 220, fontFamily: "Consolas, monospace", fontSize: 12 }}
                value={value} onChange={(e) => { setValue(e.target.value); setDirty(true); }} />
            </div>
          </>
        ) : (
          <div className="editor-wrap">
            <CodeMirror value={value} height="360px" theme="light" extensions={[markdown()]}
              onChange={(v) => { setValue(v); setDirty(true); }} />
          </div>
        )}

        <div className="hint" style={{ marginTop: 10 }}>
          标题 / 分类 / 标签 / 简介 / 时间线只写入数据库，不改变 .md 文件；正文改动才会写盘并触发 watcher 回灌。
        </div>
        <div className="dialog-actions">
          <button className="btn" onClick={onClose}>取消</button>
          <button className="btn btn-primary" disabled={saving || !title.trim()}
            onClick={() => void save()}>
            {saving ? <span className="spinner" /> : "保存"}
          </button>
        </div>
        {detail === null && <div className="hint">读取中…</div>}
      </div>
    </div>
  );
}
