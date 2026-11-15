import { useMemo, useState } from "react";
import { displayValue, fieldLabel, readableState } from "../../reader/prefs";
import type { Character, Protagonist } from "../../types";

type Tab = "world" | "player" | "characters" | "summary" | "persona";

const TABS: { key: Tab; label: string }[] = [
  { key: "world", label: "世界状态" },
  { key: "player", label: "玩家状态" },
  { key: "characters", label: "角色" },
  { key: "summary", label: "前情" },
  { key: "persona", label: "人设" },
];

const IMPORTANCE: { key: Character["importance"]; label: string }[] = [
  { key: "passerby", label: "路人" },
  { key: "notable", label: "重要" },
];

interface Props {
  stateJson: string;
  summary: string;
  persona: Protagonist | null;
  characters: Character[];
  revision: number;
  onSetImportance: (id: string, importance: Character["importance"]) => void;
  onDeleteCharacter: (id: string) => void;
  onSaveState: (json: string) => Promise<void>;
  onUndoState: () => Promise<void>;
  onClose: () => void;
}

type Layer = "world" | "player";

/** 数组按「、」编辑，其余按文本编辑；保存时再拼回 JSON。 */
function toDraft(obj: Record<string, unknown>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(obj)) {
    out[k] = Array.isArray(v) ? v.map((x) => displayValue(x)).join("、") : displayValue(v);
  }
  return out;
}

function fromDraft(
  original: Record<string, unknown>,
  draft: Record<string, string>,
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(draft)) {
    out[k] = Array.isArray(original[k])
      ? v.split(/[、,，]/).map((s) => s.trim()).filter(Boolean)
      : v;
  }
  return out;
}

export default function ArchiveDrawer({
  stateJson, summary, persona, characters, revision,
  onSetImportance, onDeleteCharacter, onSaveState, onUndoState, onClose,
}: Props) {
  const [tab, setTab] = useState<Tab>("world");
  const [editing, setEditing] = useState<Layer | null>(null);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const layers = useMemo(() => {
    try {
      const v = JSON.parse(stateJson) as Record<string, unknown>;
      const hasLayers = v && (v.world != null || v.player != null);
      const obj = (x: unknown) =>
        x && typeof x === "object" && !Array.isArray(x)
          ? (x as Record<string, unknown>)
          : {};
      return {
        world: hasLayers ? obj(v.world) : obj(v),
        player: hasLayers ? obj(v.player) : {},
      };
    } catch {
      return { world: {}, player: {} };
    }
  }, [stateJson]);

  const startEdit = (layer: Layer) => {
    setEditing(layer);
    setDraft(toDraft(layers[layer]));
  };

  const saveEdit = async () => {
    if (!editing) return;
    const merged = {
      ...(JSON.parse(stateJson || "{}") as Record<string, unknown>),
      [editing]: fromDraft(layers[editing], draft),
    };
    setBusy(true);
    try {
      await onSaveState(JSON.stringify(merged));
      setEditing(null);
    } finally { setBusy(false); }
  };

  const renderFields = (layer: Layer) => {
    const obj = layers[layer];
    if (editing === layer) {
      return (
        <div className="archive-fields">
          {Object.entries(draft).map(([k, v]) => (
            <div className="archive-field" key={k}>
              <span className="field-key">{fieldLabel(k)}</span>
              <input className="input" value={v}
                onChange={(e) => setDraft((d) => ({ ...d, [k]: e.target.value }))} />
            </div>
          ))}
          <div className="row" style={{ marginTop: 8 }}>
            <button className="btn btn-sm btn-primary" disabled={busy} onClick={() => void saveEdit()}>
              {busy ? "保存中…" : "保存"}
            </button>
            <button className="btn btn-sm btn-ghost" onClick={() => setEditing(null)}>取消</button>
            <span className="hint">数组用「、」分隔</span>
          </div>
        </div>
      );
    }
    const entries = Object.entries(obj).filter(
      ([k, v]) => k !== "delta_log" && displayValue(v).trim() !== "",
    );
    if (entries.length === 0) {
      return (
        <div className="empty">
          （暂无记录，分析段写入后会出现在这里）
          <div className="row" style={{ marginTop: 8 }}>
            <button className="btn btn-sm" onClick={() => startEdit(layer)}>手动填写</button>
          </div>
        </div>
      );
    }
    return (
      <>
        <div className="archive-fields">
          {entries.map(([k, v]) => (
            <div className="archive-field" key={k}>
              <span className="field-key">{fieldLabel(k)}</span>
              <span className="field-value">{displayValue(v)}</span>
            </div>
          ))}
        </div>
        <button className="btn btn-sm" style={{ marginTop: 8 }} onClick={() => startEdit(layer)}>编辑</button>
      </>
    );
  };

  return (
    <aside className="archive-drawer">
      <div className="archive-head">
        <strong>档案</strong>
        <span className="muted">r{revision}</span>
        <div className="grow" />
        <button className="btn btn-sm btn-ghost" title="撤销上一次状态修改（Ctrl+Z）"
          onClick={() => void onUndoState()}>撤销</button>
        <button className="btn btn-sm btn-ghost" onClick={onClose}>收起</button>
      </div>
      <div className="tabs archive-tabs">
        {TABS.map((t) => (
          <button key={t.key} className={`tab ${tab === t.key ? "active" : ""}`}
            onClick={() => { setTab(t.key); setEditing(null); }}>{t.label}</button>
        ))}
      </div>

      <div className="archive-body">
        {tab === "world" && renderFields("world")}
        {tab === "player" && renderFields("player")}
        {tab === "characters" && (
          <div className="archive-characters">
            {characters.length === 0 && (
              <div className="empty">还没有角色。正文里出现的角色会由分析阶段提名，也可以手动新建。</div>
            )}
            {characters.map((c) => (
              <div className="archive-character" key={c.id}>
                <div className="row">
                  <span className="char-name">{c.name}</span>
                  <span className={`badge badge-${c.importance}`}>
                    {IMPORTANCE.find((i) => i.key === c.importance)?.label}
                  </span>
                  <div className="grow" />
                  <button className="btn btn-sm btn-ghost btn-danger"
                    onClick={() => onDeleteCharacter(c.id)} title="删除这个角色">×</button>
                </div>
                <div className="hint">
                  {c.kind === "preset" ? "设定集预设" : `游玩中出现 · 首次第 ${(c.first_turn ?? 0) + 1} 回合`}
                  {c.inject ? " · 进上下文" : " · 不进上下文"}
                </div>
                {readableState(c.current_state) && (
                  <div className="char-state">{readableState(c.current_state)}</div>
                )}
                <div className="char-importance">
                  {IMPORTANCE.map((i) => (
                    <button key={i.key}
                      className={`btn btn-sm ${c.importance === i.key ? "btn-primary" : ""}`}
                      onClick={() => onSetImportance(c.id, i.key)}>{i.label}</button>
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
        {tab === "summary" && (
          <div className="archive-prose">{summary || "（暂无前情摘要）"}</div>
        )}
        {tab === "persona" && (
          <div className="archive-prose">
            <div className="hint">主角人设每回合 100% 注入，永不裁剪。</div>
            <p><b>{persona?.name || "（未填写）"}</b></p>
            <p>{persona?.persona_md || "（未填写人设）"}</p>
            <div className="hint" style={{ marginTop: 12 }}>开场情境</div>
            <p>{persona?.opening_scene_md || "（未填写）"}</p>
          </div>
        )}
      </div>
    </aside>
  );
}
