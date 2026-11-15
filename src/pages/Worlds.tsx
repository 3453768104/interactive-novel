import { useCallback, useEffect, useState } from "react";
import LoreEditor from "../components/LoreEditor";
import { api } from "../ipc";
import type { LoreDoc, LoreDocContent, Nav, World } from "../types";

export default function WorldsPage({ nav }: { nav: Nav }) {
  const [worlds, setWorlds] = useState<World[]>([]);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [creating, setCreating] = useState(false);
  const [docs, setDocs] = useState<LoreDoc[]>([]);
  const [activeDocId, setActiveDocId] = useState<string | null>(null);
  const [docContent, setDocContent] = useState<LoreDocContent | null>(null);
  const [newDocTitle, setNewDocTitle] = useState("");

  const loadWorlds = useCallback(async () => {
    try { setWorlds(await api.listWorlds()); } catch (e) { nav.toast(String(e), "error"); }
  }, []);

  const loadDocs = useCallback(async (worldId: string) => {
    try { setDocs(await api.listLoreDocs(worldId)); } catch (e) { nav.toast(String(e), "error"); }
  }, []);

  useEffect(() => { void loadWorlds(); }, [loadWorlds]);

  useEffect(() => {
    if (nav.worldId) void loadDocs(nav.worldId);
    else { setDocs([]); setActiveDocId(null); setDocContent(null); }
  }, [nav.worldId, loadDocs]);

  useEffect(() => {
    if (!activeDocId) { setDocContent(null); return; }
    api.readLoreDoc(activeDocId).then(setDocContent).catch((e) => nav.toast(String(e), "error"));
  }, [activeDocId]);

  const createWorld = async () => {
    if (!name.trim()) return;
    setCreating(true);
    try {
      const w = await api.createWorld(name.trim(), description.trim());
      nav.setWorldId(w.id);
      setName(""); setDescription("");
      await loadWorlds();
      nav.toast("世界观已创建，并生成了一个可直接编辑的空设定文档");
    } catch (e) { nav.toast(String(e), "error"); }
    finally { setCreating(false); }
  };

  const deleteWorld = async (w: World) => {
    if (!window.confirm(`删除世界观「${w.name}」？其目录会被移动到 backups/ 而不是直接销毁。`)) return;
    try {
      await api.deleteWorld(w.id);
      if (nav.worldId === w.id) { nav.setWorldId(null); nav.setStoryId(null); }
      await loadWorlds();
      nav.toast("已删除（目录已备份）");
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const createDoc = async () => {
    if (!nav.worldId || !newDocTitle.trim()) return;
    try {
      const d = await api.createLoreDoc(nav.worldId, newDocTitle.trim());
      setNewDocTitle("");
      await loadDocs(nav.worldId);
      setActiveDocId(d.id);
      nav.toast("设定文档已创建");
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const deleteDoc = async (d: LoreDoc) => {
    if (!nav.worldId || !window.confirm(`删除设定文档「${d.title}」？删除前会备份到 backups/。`)) return;
    try {
      await api.deleteLoreDoc(d.id);
      if (activeDocId === d.id) setActiveDocId(null);
      await loadDocs(nav.worldId);
      nav.toast("已删除（已备份）");
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const selectedWorld = worlds.find((w) => w.id === nav.worldId);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>世界观</h1>
          <div className="sub">一个世界观 = 一套设定集 + 若干剧本。设定随时可编辑，剧情重开不影响设定。</div>
        </div>
        <button className="btn btn-primary" disabled={!nav.worldId} onClick={() => nav.go("lore")}>
          打开设定集 →
        </button>
      </div>

      <div className="card">
        <div className="card-title">新建世界观</div>
        <div className="row">
          <input className="input" style={{ width: 240 }} placeholder="世界观名称" value={name}
            onChange={(e) => setName(e.target.value)} />
          <input className="input grow" placeholder="一句话描述（可选）" value={description}
            onChange={(e) => setDescription(e.target.value)} />
          <button className="btn btn-primary" disabled={creating || !name.trim()} onClick={() => void createWorld()}>
            创建
          </button>
        </div>
      </div>

      <div className="card">
        <div className="card-title">已有的世界观 <span className="muted">· {worlds.length} 个</span></div>
        {worlds.length === 0 ? (
          <div className="empty">还没有世界观。上面填个名字就能开始。</div>
        ) : (
          <div className="list">
            {worlds.map((w) => (
              <div key={w.id} className="list-item">
                <div>
                  <div className="title">{w.name}</div>
                  <div className="desc">{w.description || "（无描述）"}</div>
                </div>
                <div className="actions">
                  <button className={`btn btn-sm ${nav.worldId === w.id ? "btn-primary" : ""}`}
                    onClick={() => nav.setWorldId(w.id)}>
                    {nav.worldId === w.id ? "已选中" : "选中"}
                  </button>
                  <button className="btn btn-sm" onClick={() => { nav.setWorldId(w.id); nav.go("stories"); }}>
                    剧本
                  </button>
                  <button className="btn btn-sm btn-danger" onClick={() => void deleteWorld(w)}>删除</button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {selectedWorld && (
        <div className="card">
          <div className="card-title">
            「{selectedWorld.name}」的设定文档
            <span className="muted"> · 检索默认关闭，编辑只写入 lore_chunks，不建索引</span>
          </div>
          <div className="row" style={{ marginBottom: 14 }}>
            <input className="input" style={{ width: 240 }} placeholder="新设定文档标题" value={newDocTitle}
              onChange={(e) => setNewDocTitle(e.target.value)} />
            <button className="btn" disabled={!newDocTitle.trim()} onClick={() => void createDoc()}>新建文档</button>
          </div>

          <div className="split">
            <div className="split-side">
              {docs.length === 0 && <div className="hint">暂无设定文档</div>}
              {docs.map((d) => (
                <button key={d.id} className={`side-item ${activeDocId === d.id ? "active" : ""}`}
                  onClick={() => setActiveDocId(d.id)}>
                  {d.title}
                  <span className="sub">{d.chunk_count} 个切片 · {d.origin}</span>
                </button>
              ))}
              {activeDocId && (
                <button className="btn btn-sm btn-danger" style={{ marginTop: 10 }}
                  onClick={() => { const d = docs.find((x) => x.id === activeDocId); if (d) void deleteDoc(d); }}>
                  删除当前文档
                </button>
              )}
            </div>
            <div>
              {docContent && nav.worldId ? (
                <LoreEditor
                  doc={docContent}
                  worldId={nav.worldId}
                  nav={nav}
                  onChanged={() => nav.worldId && void loadDocs(nav.worldId)}
                />
              ) : (
                <div className="empty">从左侧选一个设定文档开始编辑</div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
