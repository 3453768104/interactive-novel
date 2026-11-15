import { useCallback, useEffect, useState } from "react";
import { api } from "../../ipc";
import type { StorySave } from "../../types";

interface Props {
  storyId: string;
  onClose: () => void;
  onRestored: () => void;
  onToast: (msg: string, kind?: "ok" | "error") => void;
}

function stamp(s: string) {
  return s ? s.replace("T", " ").slice(0, 16) : "";
}

export default function SavesPanel({ storyId, onClose, onRestored, onToast }: Props) {
  const [saves, setSaves] = useState<StorySave[]>([]);
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try { setSaves(await api.listSaves(storyId)); }
    catch (e) { onToast(String(e), "error"); }
  }, [storyId, onToast]);

  useEffect(() => { void load(); }, [load]);

  const create = async () => {
    setBusy(true);
    try {
      await api.createSave(storyId, label.trim() || "手动存档");
      setLabel("");
      await load();
      onToast("已存一个档");
    } catch (e) { onToast(String(e), "error"); }
    finally { setBusy(false); }
  };

  const restore = async (s: StorySave) => {
    const ok = window.confirm(
      `读档到「回合 ${s.turn_index} 结束」？\n此后写的回合会被删除（设定文档不受影响）。\n建议先存一个新档再读。`,
    );
    if (!ok) return;
    setBusy(true);
    try {
      await api.restoreSave(s.id);
      await load();
      onRestored();
      onToast("已读档");
    } catch (e) { onToast(String(e), "error"); }
    finally { setBusy(false); }
  };

  const remove = async (s: StorySave) => {
    if (!window.confirm("删除这个存档？正文不会被删除。")) return;
    try { await api.deleteSave(s.id); await load(); }
    catch (e) { onToast(String(e), "error"); }
  };

  const manuals = saves.filter((s) => s.kind === "manual");
  const autos = saves.filter((s) => s.kind !== "manual");

  const row = (s: StorySave) => (
    <div className="save-row" key={s.id}>
      <div className="save-main">
        <span className="save-turn">回合 {s.turn_index < 0 ? "（开篇前）" : s.turn_index}</span>
        <span className="save-label">{s.label || (s.kind === "manual" ? "手动存档" : "自动存档")}</span>
        <span className="muted">{stamp(s.created_at)}</span>
      </div>
      <div className="row">
        <button className="btn btn-sm" disabled={busy} onClick={() => void restore(s)}>读档</button>
        <button className="btn btn-sm btn-ghost btn-danger" disabled={busy}
          onClick={() => void remove(s)}>×</button>
      </div>
    </div>
  );

  return (
    <aside className="archive-drawer saves-panel">
      <div className="archive-head">
        <strong>存档</strong>
        <span className="muted">{saves.length}</span>
        <div className="grow" />
        <button className="btn btn-sm btn-ghost" onClick={onClose}>收起</button>
      </div>

      <div className="archive-body">
        <div className="hint">
          每回合结束都会自动存一份（只保留最近 60 个）。读档会删除该回合之后的正文，并把状态卡、前情、角色一起还原。
        </div>

        <div className="row" style={{ margin: "10px 0" }}>
          <input className="input" value={label} placeholder="给这个存档起个名字（可留空）"
            onChange={(e) => setLabel(e.target.value)} />
          <button className="btn btn-sm btn-primary" disabled={busy} onClick={() => void create()}>
            存一个档
          </button>
        </div>

        {manuals.length > 0 && (
          <>
            <div className="field-title">手动存档</div>
            {manuals.map(row)}
          </>
        )}

        <div className="field-title" style={{ marginTop: 14 }}>自动存档</div>
        {autos.length === 0 && <div className="empty">还没有自动存档。</div>}
        {autos.map(row)}
      </div>
    </aside>
  );
}
