import { useEffect, useRef, useState } from "react";
import { api } from "../../ipc";
import type { StoryHit } from "../../types";

interface Props {
  storyId: string;
  onJump: (turn: number) => void;
  onClose: () => void;
  onToast: (msg: string, kind?: "ok" | "error") => void;
}

/** 正文全文检索：查得到、点得过去。 */
export default function SearchPanel({ storyId, onJump, onClose, onToast }: Props) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<StoryHit[]>([]);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => { inputRef.current?.focus(); }, []);

  const run = async () => {
    const q = query.trim();
    if (!q) { setHits([]); return; }
    setBusy(true);
    try { setHits(await api.searchStory(storyId, q, 40)); }
    catch (e) { onToast(String(e), "error"); }
    finally { setBusy(false); }
  };

  return (
    <aside className="archive-drawer search-panel">
      <div className="archive-head">
        <strong>搜索</strong>
        <span className="muted">正文</span>
        <div className="grow" />
        <button className="btn btn-sm btn-ghost" onClick={onClose}>收起</button>
      </div>
      <div className="archive-body">
        <div className="row">
          <input ref={inputRef} className="input" value={query}
            placeholder="找人名、地名、一句话…"
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") void run(); }} />
          <button className="btn btn-sm btn-primary" disabled={busy} onClick={() => void run()}>
            {busy ? "…" : "搜"}
          </button>
        </div>
        <div className="hint" style={{ margin: "6px 0 10px" }}>
          中文按字与二字组匹配，两字以上更准；结果按相关度排序。
        </div>

        {hits.length === 0 && query.trim() && !busy && (
          <div className="empty">没有命中。换个词，或确认这段是在当前剧本里写的。</div>
        )}

        {hits.map((h) => (
          <button key={h.passage_id} className="search-hit"
            onClick={() => onJump(h.turn_index)}>
            <span className="search-turn">回合 {h.turn_index}</span>
            <span className="search-snippet">{h.snippet}</span>
          </button>
        ))}
      </div>
    </aside>
  );
}
