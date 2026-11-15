import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { api, CONFLICT_PREFIX } from "../ipc";
import type { LoreDocContent, Nav } from "../types";

interface Props {
  doc: LoreDocContent;
  worldId: string;
  nav: Nav;
  onChanged: () => void;
}

interface Conflict {
  sha256: string;
  content: string;
}

export default function LoreEditor({ doc, worldId, nav, onChanged }: Props) {
  const [value, setValue] = useState(doc.content);
  const [baseSha, setBaseSha] = useState(doc.sha256);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [conflict, setConflict] = useState<Conflict | null>(null);

  // Switching documents resets the buffer. We deliberately do NOT depend on
  // doc.sha256 here: external changes are handled explicitly below.
  useEffect(() => {
    setValue(doc.content);
    setBaseSha(doc.sha256);
    setDirty(false);
    setConflict(null);
  }, [doc.id]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<{ world_id: string; rel_path: string }>("lore://changed", async (event) => {
      if (event.payload.rel_path !== doc.rel_path) return;
      onChanged();
      try {
        const fresh = await api.readLoreDoc(doc.id);
        if (dirty) {
          // Never silently overwrite unsaved in-app edits.
          setConflict({ sha256: fresh.sha256, content: fresh.content });
        } else {
          setValue(fresh.content);
          setBaseSha(fresh.sha256);
          setDirty(false);
          nav.toast("已同步外部编辑器的修改");
        }
      } catch (e) {
        nav.toast(String(e), "error");
      }
    }).then((f) => (unlisten = f));
    return () => unlisten?.();
  }, [doc.id, doc.rel_path, dirty]);

  const save = async (forceSha?: string, force = false) => {
    if (saving) return;
    setSaving(true);
    try {
      const sha = forceSha ?? baseSha;
      const result = await api.saveLoreDoc(doc.id, value, sha, force);
      setBaseSha(result.sha256);
      setDirty(false);
      setConflict(null);
      onChanged();
      nav.toast("设定文档已保存并重新切片");
    } catch (e) {
      const msg = String(e);
      if (msg.startsWith(CONFLICT_PREFIX)) {
        try {
          const payload = JSON.parse(msg.slice(CONFLICT_PREFIX.length)) as Conflict;
          setConflict(payload);
          nav.toast("检测到外部编辑器改动，请选择处理方式", "error");
          return;
        } catch { /* fall through */ }
      }
      nav.toast(msg, "error");
    } finally {
      setSaving(false);
    }
  };

  const keepMine = async () => {
    if (!conflict) return;
    await save(conflict.sha256, true);
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
      const created = await api.createLoreDoc(worldId, `${doc.title}-我的改动`);
      await api.saveLoreDoc(created.id, value, created.sha256, true);
      onChanged();
      nav.toast(`已把未保存内容另存为「${created.title}」`);
      takeExternal();
    } catch (e) {
      nav.toast(String(e), "error");
    }
  };

  return (
    <div onKeyDown={(e) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        void save();
      }
    }}>
      <div className="editor-toolbar">
        <strong>{doc.title}</strong>
        <span className="mono muted">{doc.rel_path}</span>
        <span className={`badge ${dirty ? "badge-dirty" : "badge-clean"}`}>
          {dirty ? "未保存" : "已同步"}
        </span>
        <div className="grow" />
        <button className="btn btn-sm" onClick={() => void api.openLoreExternal(doc.id)}>
          用默认编辑器打开
        </button>
        <button className="btn btn-sm btn-primary" disabled={!dirty || saving} onClick={() => void save()}>
          {saving ? <span className="spinner" /> : "保存"} <span className="muted">Ctrl+S</span>
        </button>
      </div>
      <div className="hint" style={{ marginBottom: 8 }}>
        设定文档是唯一权威源：应用内编辑和用外部编辑器改的是同一份文件，保存后会自动重新切片。
      </div>
      <div className="editor-wrap">
        <CodeMirror
          value={value}
          height="440px"
          theme="light"
          extensions={[markdown()]}
          onChange={(v) => { setValue(v); setDirty(true); }}
        />
      </div>

      {conflict && (
        <div className="dialog-backdrop">
          <div className="dialog">
            <h2>设定文档在别处被改动了</h2>
            <p className="hint">
              应用内（设定集的表单或源码）还有未保存的修改，外部编辑器也保存过这份文件。不会自动合并、也不会静默覆盖，请选择：
            </p>
            <div className="grid-2">
              <div className="card">
                <div className="card-title">外部版本（前 400 字）</div>
                <div className="code-block" style={{ maxHeight: 180 }}>
                  {conflict.content.slice(0, 400)}
                </div>
              </div>
              <div className="card">
                <div className="card-title">应用内未保存的版本（前 400 字）</div>
                <div className="code-block" style={{ maxHeight: 180 }}>{value.slice(0, 400)}</div>
              </div>
            </div>
            <div className="dialog-actions">
              <button className="btn" onClick={keepBoth}>两边都留（另存为新文档）</button>
              <button className="btn" onClick={takeExternal}>放弃我的，载入外部</button>
              <button className="btn btn-primary" onClick={keepMine}>保留我的（覆盖外部）</button>
            </div>
            <div className="hint">选择「保留我的」时，外部版本会被自动备份到 backups/ 目录。</div>
          </div>
        </div>
      )}
    </div>
  );
}
