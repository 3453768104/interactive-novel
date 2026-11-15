import { useEffect, useState } from "react";
import { api } from "../../ipc";
import {
  EMPTY_TIMELINE_EVENT, type Nav, type TimelineEvent,
} from "../../types";

interface Props {
  worldId: string;
  nav: Nav;
  onChanged: () => void;
}

export default function TimelinePanel({ worldId, nav, onChanged }: Props) {
  const [events, setEvents] = useState<TimelineEvent[]>([]);
  const [editing, setEditing] = useState<TimelineEvent | null>(null);

  const load = async () => setEvents(await api.listTimelineEvents(worldId));
  useEffect(() => { void load().catch((e) => nav.toast(String(e), "error")); }, [worldId]);

  const openNew = () =>
    setEditing({ ...EMPTY_TIMELINE_EVENT, id: "", world_id: worldId, ordinal: events.length });

  const save = async () => {
    if (!editing || !editing.title.trim()) return;
    try {
      await api.upsertTimelineEvent(worldId, editing);
      setEditing(null);
      await load();
      onChanged();
      nav.toast("时间线事件已保存");
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const remove = async (ev: TimelineEvent) => {
    if (!window.confirm(`删除时间线事件「${ev.title}」？`)) return;
    try {
      await api.deleteTimelineEvent(ev.id);
      await load();
      onChanged();
    } catch (e) { nav.toast(String(e), "error"); }
  };

  return (
    <div>
      <div className="row" style={{ marginBottom: 14 }}>
        <button className="btn btn-primary" onClick={openNew}>＋ 新事件</button>
        <span className="hint">
          勾选「注入上下文」的事件会和时间线一起、跟随事件背景<b>全量注入</b>提示词（不裁剪）。
        </span>
        <div className="grow" />
        <span className="badge">{events.length} 条</span>
      </div>

      {events.length === 0 ? (
        <div className="empty">还没有历史年表。记一条「历 302 年：北境封印」试试。</div>
      ) : (
        <div className="card">
          {events.map((ev) => (
            <div className="timeline-item" key={ev.id}>
              <div className="when">
                {[ev.era, ev.year_label].filter(Boolean).join(" · ") || "未标注时间"}
                {ev.inject_into_context && <span className="badge badge-mode" style={{ marginLeft: 8 }}>注入上下文</span>}
              </div>
              <div className="t">{ev.title}</div>
              {ev.body_md && <div className="b">{ev.body_md}</div>}
              <div className="row">
                <button className="btn btn-sm" onClick={() => setEditing({ ...ev })}>编辑</button>
                <button className="btn btn-sm btn-danger" onClick={() => void remove(ev)}>删除</button>
              </div>
            </div>
          ))}
        </div>
      )}

      {editing && (
        <div className="dialog-backdrop">
          <div className="dialog">
            <h2>{editing.id ? "编辑事件" : "新事件"}</h2>
            <div className="form-grid">
              <div className="field">
                <span className="label">纪元 / 时代</span>
                <input className="input" value={editing.era}
                  onChange={(e) => setEditing({ ...editing, era: e.target.value })} placeholder="第二纪" />
              </div>
              <div className="field">
                <span className="label">年份标签</span>
                <input className="input" value={editing.year_label}
                  onChange={(e) => setEditing({ ...editing, year_label: e.target.value })} placeholder="历 302 年" />
              </div>
            </div>
            <div className="field">
              <span className="label">事件标题</span>
              <input className="input" value={editing.title}
                onChange={(e) => setEditing({ ...editing, title: e.target.value })} />
            </div>
            <div className="field">
              <span className="label">事件内容</span>
              <textarea className="textarea" value={editing.body_md}
                onChange={(e) => setEditing({ ...editing, body_md: e.target.value })} />
            </div>
            <div className="form-grid">
              <div className="field">
                <span className="label">排序（小的在前）</span>
                <input className="input" type="number" value={editing.ordinal}
                  onChange={(e) => setEditing({ ...editing, ordinal: Number(e.target.value) || 0 })} />
              </div>
              <div className="field">
                <span className="label">上下文</span>
                <label className="check-row" style={{ paddingTop: 8 }}>
                  <input type="checkbox" checked={editing.inject_into_context}
                    onChange={(e) => setEditing({ ...editing, inject_into_context: e.target.checked })} />
                  注入上下文
                </label>
              </div>
            </div>
            <div className="dialog-actions">
              <button className="btn" onClick={() => setEditing(null)}>取消</button>
              <button className="btn btn-primary" onClick={() => void save()}>保存</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
