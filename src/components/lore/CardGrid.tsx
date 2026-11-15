import type { LoreCard } from "../../types";

interface Props {
  cards: LoreCard[];
  onOpen: (c: LoreCard) => void;
  onSplit: (c: LoreCard) => void;
  onExternal: (c: LoreCard) => void;
  /** doc_id set of cards pinned into every turn of the current story. */
  pinned?: Set<string>;
  onPin?: (c: LoreCard) => void;
}

function shortDate(s: string) {
  if (!s) return "—";
  return s.slice(0, 10);
}

export default function CardGrid({ cards, onOpen, onSplit, onExternal, pinned, onPin }: Props) {
  if (cards.length === 0) {
    return <div className="empty">这里还没有设定卡片。可以在「世界观」页新建文档，或用右上角新建。</div>;
  }
  return (
    <>
      <div className="card-grid">
        {cards.map((c) => (
          <div key={c.doc_id} className="lore-card" onClick={() => onOpen(c)}>
            <div className="cat-label">设定 · {c.category_name || "未分类"}</div>
            <div className="title">{c.title}</div>
            <div className="summary">{c.summary || "（还没有简介）"}</div>
            <div className="row" style={{ gap: 4 }}>
              {c.tags.slice(0, 3).map((t) => <span className="chip" key={t}>{t}</span>)}
            </div>
            <div className="meta">
              <span>最近更新 {shortDate(c.updated_at)}</span>
              <span>{c.char_count} 字</span>
              <span>{c.chunk_count} 切片</span>
            </div>
            <div className="card-foot" onClick={(e) => e.stopPropagation()}>
              <button className="btn btn-sm" onClick={() => onOpen(c)}>编辑</button>
              <button className="btn btn-sm" onClick={() => onSplit(c)} title="按 ## 章节拆成多张卡片">拆分</button>
              <button className="btn btn-sm" onClick={() => onExternal(c)} title="用外部编辑器打开">外部</button>
              {onPin && (
                <button className={`btn btn-sm ${pinned?.has(c.doc_id) ? "btn-primary" : ""}`}
                  onClick={() => onPin(c)}
                  title="置顶后这张卡每回合都整张注入，不走检索">★</button>
              )}
            </div>
          </div>
        ))}
      </div>
      <div className="loaded-all"><button className="btn btn-sm">— 已加载全部设定 —</button></div>
    </>
  );
}
