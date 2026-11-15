import { useCallback, useEffect, useMemo, useState } from "react";
import CardEditorPanel from "../components/lore/CardEditorPanel";
import CardGrid from "../components/lore/CardGrid";
import CategoryTree from "../components/lore/CategoryTree";
import EventBackgroundEditor from "../components/lore/EventBackgroundEditor";
import LoreAiDialog from "../components/lore/LoreAiDialog";
import SearchBar from "../components/lore/SearchBar";
import TimelinePanel from "../components/lore/TimelinePanel";
import { api } from "../ipc";
import type {
  LoreCard, LoreCategory, Nav, TimelineEvent,
} from "../types";

type Tab = "cards" | "timeline" | "background";

export default function LoreLibrary({ nav }: { nav: Nav }) {
  const worldId = nav.worldId;
  const [tab, setTab] = useState<Tab>("cards");
  const [categories, setCategories] = useState<LoreCategory[]>([]);
  const [cards, setCards] = useState<LoreCard[]>([]);
  const [events, setEvents] = useState<TimelineEvent[]>([]);
  const [selected, setSelected] = useState<string | null | "all">("all");
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<LoreCard | null>(null);
  const [newCardTitle, setNewCardTitle] = useState("");
  const [addingCategory, setAddingCategory] = useState(false);
  const [newCategory, setNewCategory] = useState("");
  const [importPath, setImportPath] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [importing, setImporting] = useState(false);
  const [pins, setPins] = useState<Set<string>>(new Set());
  const [aiOpen, setAiOpen] = useState(false);

  const load = useCallback(async () => {
    if (!worldId) return;
    try {
      const [c, k, e] = await Promise.all([
        api.listLoreCategories(worldId),
        api.listLoreCards(worldId),
        api.listTimelineEvents(worldId),
      ]);
      setCategories(c);
      setCards(k);
      setEvents(e);
    } catch (err) { nav.toast(String(err), "error"); }
  }, [worldId]);

  useEffect(() => { void load(); }, [load]);

  useEffect(() => {
    if (!nav.storyId) { setPins(new Set()); return; }
    api.listLorePins(nav.storyId)
      .then((rows) => setPins(new Set(rows.map((r) => r.doc_id))))
      .catch(() => setPins(new Set()));
  }, [nav.storyId]);

  const togglePin = async (c: LoreCard) => {
    if (!nav.storyId) { nav.toast("先在「剧本」里打开一本剧本，置顶是剧本级设置", "error"); return; }
    const next = !pins.has(c.doc_id);
    try {
      await api.setLorePin(nav.storyId, c.doc_id, next);
      setPins((cur) => {
        const s = new Set(cur);
        if (next) s.add(c.doc_id); else s.delete(c.doc_id);
        return s;
      });
      nav.toast(next ? `「${c.title}」已置顶，每回合整张注入` : `「${c.title}」已取消置顶`);
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return cards.filter((c) => {
      const catOk =
        selected === "all" ||
        (selected === null ? !c.category_id : c.category_id === selected);
      if (!catOk) return false;
      if (!q) return true;
      return `${c.title} ${c.summary} ${c.tags.join(" ")}`.toLowerCase().includes(q);
    });
  }, [cards, selected, query]);

  const categoryName =
    selected === "all" ? "全部设定"
    : selected === null ? "未分类"
    : (categories.find((c) => c.id === selected)?.name ?? "分类");
  const uncategorized = cards.filter((c) => !c.category_id).length;

  if (!worldId) {
    return <div className="page"><div className="empty">请先在「世界观」里选中一个世界观。</div></div>;
  }

  const createCard = async () => {
    if (!newCardTitle.trim()) return;
    try {
      const doc = await api.createLoreDoc(worldId, newCardTitle.trim());
      setNewCardTitle("");
      await load();
      const fresh = (await api.listLoreCards(worldId)).find((c) => c.doc_id === doc.id);
      if (fresh) setEditing(fresh);
      nav.toast("新设定卡片已创建");
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const addCategory = async () => {
    if (!newCategory.trim()) return;
    try {
      await api.createLoreCategory(worldId, newCategory.trim(), null);
      setNewCategory("");
      setAddingCategory(false);
      await load();
      nav.toast("分类已创建");
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const renameCategory = async (c: LoreCategory) => {
    const name = window.prompt("新的分类名称", c.name);
    if (!name || name === c.name) return;
    try {
      await api.renameLoreCategory(c.id, name);
      await load();
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const deleteCategory = async (c: LoreCategory) => {
    if (!window.confirm(`删除分类「${c.name}」？卡片不会被删除，只会变为未分类。`)) return;
    try {
      await api.deleteLoreCategory(c.id);
      await load();
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const runImport = async () => {
    if (!importPath.trim() || importing) return;
    setImporting(true);
    try {
      const r = await api.importLoreMarkdown(worldId, importPath.trim());
      await load();
      setImportOpen(false);
      setImportPath("");
      nav.toast(
        `已导入：新建 ${r.categories_created} 个分类、${r.cards_created} 张卡片` +
        (r.cards_skipped ? `（${r.cards_skipped} 张同名已跳过）` : ""),
      );
    } catch (e) { nav.toast(String(e), "error"); }
    finally { setImporting(false); }
  };

  const splitCard = async (c: LoreCard) => {    if (!window.confirm(`把「${c.title}」按 ## 章节拆成多张卡片？原文档会先备份再删除。`)) return;
    try {
      const made = await api.splitLoreDocByHeadings(c.doc_id, c.category_id);
      await load();
      nav.toast(`已拆成 ${made.length} 张卡片`);
    } catch (e) { nav.toast(String(e), "error"); }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>设定集</h1>
          <div className="sub">
            分类树 + 卡片 + 时间线 + 事件背景。正文以 Markdown 为权威源，元数据存在数据库；
            可视化表单、源码编辑、外部编辑器三路并存但不互相覆盖。
          </div>
        </div>
        <div className="row">
          <button className="btn btn-primary" onClick={() => setAiOpen(true)}>AI 生成</button>
          <button className="btn" onClick={() => setImportOpen((v) => !v)}>导入 Markdown</button>
          <button className="btn" onClick={() => void load()}>刷新</button>
        </div>
      </div>

      {aiOpen && worldId && (
        <LoreAiDialog
          worldId={worldId}
          categories={categories}
          cards={cards}
          mode={nav.mode}
          onClose={() => setAiOpen(false)}
          onSaved={() => void load()}
          onToast={nav.toast}
        />
      )}

      {importOpen && (
        <div className="card" style={{ marginBottom: 14 }}>
          <div className="card-title">从 Markdown 批量导入</div>
          <div className="row">
            <input className="input grow" placeholder="设定集 .md 的完整路径，例如 D:\设定\斗罗大陆.md"
              value={importPath} onChange={(e) => setImportPath(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") void runImport(); }} />
            <button className="btn btn-primary" disabled={!importPath.trim() || importing}
              onClick={() => void runImport()}>{importing ? "导入中…" : "开始导入"}</button>
            <button className="btn btn-ghost" onClick={() => setImportOpen(false)}>取消</button>
          </div>
          <div className="hint">
            「# 一级标题」变成分类，「## 二级标题」变成一张卡片（一个 .md 文件）；同名卡片会跳过，可重复导入。
          </div>
        </div>
      )}

      <div className="tabs">
        {([["cards", "设定卡片"], ["timeline", "时间线"], ["background", "事件背景"]] as [Tab, string][])
          .map(([k, label]) => (
            <button key={k} className={`tab ${tab === k ? "active" : ""}`} onClick={() => setTab(k)}>
              {label}
            </button>
          ))}
      </div>

      {tab === "cards" && (
        <>
          <div className="card" style={{ marginBottom: 14 }}>
            <div className="row">
              <input className="input" style={{ maxWidth: 260 }} placeholder="新设定卡片标题"
                value={newCardTitle} onChange={(e) => setNewCardTitle(e.target.value)} />
              <button className="btn btn-primary" disabled={!newCardTitle.trim()}
                onClick={() => void createCard()}>＋ 新建卡片</button>
              <span className="hint">一张卡片 = 一个 .md 文件；长文档可用卡片上的「拆分」按 ## 章节拆开</span>
            </div>
          </div>

          <div className="lore-layout">
            <CategoryTree
              categories={categories}
              selected={selected}
              totalCount={cards.length}
              uncategorizedCount={uncategorized}
              onSelect={setSelected}
              onAdd={() => setAddingCategory(true)}
              onRename={(c) => void renameCategory(c)}
              onDelete={(c) => void deleteCategory(c)}
            />
            <div>
              {addingCategory && (
                <div className="card" style={{ marginBottom: 12 }}>
                  <div className="row">
                    <input className="input" style={{ maxWidth: 240 }} placeholder="新分类名称"
                      value={newCategory} onChange={(e) => setNewCategory(e.target.value)} />
                    <button className="btn btn-primary btn-sm" onClick={() => void addCategory()}>创建</button>
                    <button className="btn btn-sm" onClick={() => setAddingCategory(false)}>取消</button>
                  </div>
                </div>
              )}
              <SearchBar query={query} onQuery={setQuery} categoryName={categoryName}
                shown={filtered.length} total={cards.length} />
              <CardGrid
                cards={filtered}
                onOpen={setEditing}
                onSplit={(c) => void splitCard(c)}
                onExternal={(c) => void api.openLoreExternal(c.doc_id).catch((e) => nav.toast(String(e), "error"))}
                pinned={pins}
                onPin={(c) => void togglePin(c)}
              />
            </div>
          </div>
        </>
      )}

      {tab === "timeline" && <TimelinePanel worldId={worldId} nav={nav} onChanged={() => void load()} />}
      {tab === "background" && <EventBackgroundEditor worldId={worldId} nav={nav} />}

      {editing && (
        <CardEditorPanel
          card={editing}
          categories={categories}
          events={events}
          nav={nav}
          onChanged={() => void load()}
          onClose={() => setEditing(null)}
        />
      )}
    </div>
  );
}
