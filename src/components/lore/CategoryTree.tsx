import type { LoreCategory } from "../../types";

interface Props {
  categories: LoreCategory[];
  selected: string | null | "all";
  totalCount: number;
  uncategorizedCount: number;
  onSelect: (v: string | null | "all") => void;
  onAdd: () => void;
  onRename: (c: LoreCategory) => void;
  onDelete: (c: LoreCategory) => void;
}

export default function CategoryTree({
  categories,
  selected,
  totalCount,
  uncategorizedCount,
  onSelect,
  onAdd,
  onRename,
  onDelete,
}: Props) {
  const roots = categories.filter((c) => !c.parent_id);
  const children = (id: string) => categories.filter((c) => c.parent_id === id);

  const node = (c: LoreCategory, child: boolean) => (
    <div key={c.id} style={{ display: "flex", alignItems: "center" }}>
      <button
        className={`category-node ${child ? "child" : ""} ${selected === c.id ? "active" : ""}`}
        onClick={() => onSelect(c.id)}
      >
        {child ? "└ " : "▦ "}{c.name}
        <span className="count">{c.card_count}</span>
      </button>
      <button className="btn btn-sm btn-ghost" title="重命名" onClick={() => onRename(c)}>✎</button>
      <button className="btn btn-sm btn-ghost" title="删除分类" onClick={() => onDelete(c)}>×</button>
    </div>
  );

  return (
    <aside className="lore-side">
      <h3>分类</h3>
      <button
        className={`category-node ${selected === "all" ? "active" : ""}`}
        onClick={() => onSelect("all")}
      >
        ▤ 全部设定 <span className="count">{totalCount}</span>
      </button>
      <button
        className={`category-node ${selected === null ? "active" : ""}`}
        onClick={() => onSelect(null)}
      >
        ◌ 未分类 <span className="count">{uncategorizedCount}</span>
      </button>
      <div style={{ height: 8 }} />
      {roots.map((c) => (
        <div key={c.id}>
          {node(c, false)}
          {children(c.id).map((ch) => node(ch, true))}
        </div>
      ))}
      <div className="category-tree-actions">
        <button className="btn btn-sm" onClick={onAdd}>＋ 新分类</button>
      </div>
    </aside>
  );
}
