import { useCallback, useEffect, useState } from "react";
import { api } from "../../ipc";
import type { Chapter } from "../../types";

interface Props {
  storyId: string;
  totalTurns: number;
  onJump: (turn: number) => void;
  onClose: () => void;
  onToast: (msg: string, kind?: "ok" | "error") => void;
  onChanged: () => void;
}

export default function ChaptersDrawer({
  storyId, totalTurns, onJump, onClose, onToast, onChanged,
}: Props) {
  const [chapters, setChapters] = useState<Chapter[]>([]);
  const [target, setTarget] = useState(3000);
  const [editing, setEditing] = useState<number | null>(null);
  const [draft, setDraft] = useState("");

  const load = useCallback(async () => {
    try { setChapters(await api.listChapters(storyId, target)); }
    catch (e) { onToast(String(e), "error"); }
  }, [storyId, target, onToast]);

  useEffect(() => { void load(); }, [load]);

  const rename = async (c: Chapter) => {
    try {
      await api.renameChapter(storyId, c.ordinal, draft);
      setEditing(null);
      await load();
      onChanged();
      onToast("章名已改");
    } catch (e) { onToast(String(e), "error"); }
  };

  const totalChars = chapters.reduce((n, c) => n + c.chars, 0);

  return (
    <aside className="archive-drawer chapters-drawer">
      <div className="archive-head">
        <strong>目录</strong>
        <span className="muted">{chapters.length} 章 · {totalChars} 字</span>
        <div className="grow" />
        <button className="btn btn-sm btn-ghost" onClick={onClose}>收起</button>
      </div>

      <div className="archive-body">
        <div className="row" style={{ marginBottom: 8 }}>
          <span className="hint">每章字数</span>
          <input className="input" type="number" min={500} step={500} value={target}
            style={{ width: 90 }}
            onChange={(e) => setTarget(Number(e.target.value) || 3000)} />
          <button className="btn btn-sm" onClick={() => void load()}>重算</button>
        </div>

        {chapters.length === 0 && <div className="empty">还没有正文，写几回合再来。</div>}

        {chapters.map((c) => (
          <div className="chapter-row" key={c.ordinal}>
            <div className="chapter-main">
              {editing === c.ordinal ? (
                <div className="row">
                  <input className="input" autoFocus value={draft}
                    onChange={(e) => setDraft(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") void rename(c);
                      if (e.key === "Escape") setEditing(null);
                    }} />
                  <button className="btn btn-sm btn-primary" onClick={() => void rename(c)}>存</button>
                </div>
              ) : (
                <button className="chapter-name" onClick={() => onJump(c.start_turn)}>
                  {c.title}
                </button>
              )}
              <div className="chapter-meta">
                回合 {c.start_turn}–{c.end_turn} · {c.chars} 字
                {c.lead && <span className="chapter-lead">「{c.lead}」</span>}
              </div>
            </div>
            <div className="row">
              <button className="btn btn-sm btn-ghost" title="改章名"
                onClick={() => { setEditing(c.ordinal); setDraft(c.title); }}>✎</button>
              <button className="btn btn-sm btn-ghost" title="跳到这一章"
                onClick={() => onJump(c.start_turn)}>→</button>
            </div>
          </div>
        ))}

        {chapters.length > 0 && totalTurns > 0 && (
          <div className="hint" style={{ marginTop: 10 }}>
            当前共 {totalTurns} 回合；章名只覆盖显示，不会改写正文。
          </div>
        )}
      </div>
    </aside>
  );
}
