import { useCallback, useEffect, useState } from "react";
import { api } from "../ipc";
import type { EngineMode, Nav, Pov, Story, StoryEventBackground } from "../types";

const MODES: { key: EngineMode; label: string; note: string }[] = [
  { key: "paste", label: "网页粘贴", note: "免费、零依赖，应用生成整块提示词，你复制粘贴往返" },
  { key: "local", label: "本地推理", note: "qwen 9B 等，约 10K 上下文，30 tokens/s" },
  { key: "api", label: "联网 API", note: "大上下文、按量付费" },
];

export default function StoriesPage({ nav }: { nav: Nav }) {
  const [stories, setStories] = useState<Story[]>([]);
  const [name, setName] = useState("");
  const [protagonistName, setProtagonistName] = useState("");
  const [persona, setPersona] = useState("");
  const [opening, setOpening] = useState("");
  const [mode, setMode] = useState<EngineMode>("paste");
  const [pov, setPov] = useState<Pov>("protagonist");
  const [busy, setBusy] = useState(false);
  const [bgStory, setBgStory] = useState<Story | null>(null);
  const [bgData, setBgData] = useState<StoryEventBackground | null>(null);
  const [bgEnabled, setBgEnabled] = useState(false);
  const [bgText, setBgText] = useState("");

  const load = useCallback(async () => {
    if (!nav.worldId) return;
    try { setStories(await api.listStories(nav.worldId)); } catch (e) { nav.toast(String(e), "error"); }
  }, [nav.worldId]);

  useEffect(() => { void load(); }, [load]);

  const create = async () => {
    if (!nav.worldId || !name.trim()) return;
    setBusy(true);
    try {
      const s = await api.createStory({
        world_id: nav.worldId,
        name: name.trim(),
        protagonist_name: protagonistName.trim(),
        persona_md: persona,
        opening_scene_md: opening,
        engine_mode: mode,
      });
      await api.saveStorySettings(s.id, pov, true);
      nav.setStoryId(s.id);
      nav.setMode(mode);
      setName(""); setProtagonistName(""); setPersona(""); setOpening("");
      await load();
      nav.toast("剧本已创建，可以开始第一段了");
      nav.go("reader");
    } catch (e) { nav.toast(String(e), "error"); }
    finally { setBusy(false); }
  };

  const openStory = (s: Story) => {
    nav.setStoryId(s.id);
    nav.setMode(s.engine_mode);
    nav.go("reader");
  };

  const remove = async (s: Story) => {
    if (!window.confirm(`删除剧本「${s.name}」？该剧本的全部回合也会被删除。`)) return;
    try {
      await api.deleteStory(s.id);
      if (nav.storyId === s.id) nav.setStoryId(null);
      await load();
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const openBackground = async (s: Story) => {
    try {
      const data = await api.getStoryEventBackground(s.id);
      setBgData(data);
      setBgEnabled(data.enabled);
      setBgText(data.content_md);
      setBgStory(s);
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const saveBackground = async () => {
    if (!bgStory) return;
    try {
      await api.saveStoryEventBackground(bgStory.id, bgEnabled, bgText);
      nav.toast(bgEnabled ? "已保存本剧本的背景覆盖" : "已恢复继承世界观背景");
      setBgStory(null);
    } catch (e) { nav.toast(String(e), "error"); }
  };

  if (!nav.worldId) {
    return <div className="page"><div className="empty">请先在「世界观」里选中一个世界观。</div></div>;
  }

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>剧本</h1>
          <div className="sub">每个剧本有独立的主角、人设与开场情境；同一个世界观下可以开很多本。</div>
        </div>
      </div>

      <div className="card">
        <div className="card-title">新建剧本</div>
        <div className="grid-2">
          <div className="field">
            <span className="label">剧本名称</span>
            <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="例如：北境旧宅" />
          </div>
          <div className="field">
            <span className="label">主角姓名</span>
            <input className="input" value={protagonistName} onChange={(e) => setProtagonistName(e.target.value)} placeholder="例如：沈砚" />
          </div>
        </div>
        <div className="field">
          <span className="label">主角人设</span>
          <textarea className="textarea" value={persona} onChange={(e) => setPersona(e.target.value)}
            placeholder="性格、身份、能力、说话方式……会作为静态锚点注入每回合" />
        </div>
        <div className="field">
          <span className="label">开场情境</span>
          <textarea className="textarea" value={opening} onChange={(e) => setOpening(e.target.value)}
            placeholder="故事从哪里开始？第一段的起点" />
        </div>
        <div className="field">
          <span className="label">调用模式（之后可随时切换，历史与状态卡共用）</span>
          <div className="row">
            {MODES.map((m) => (
              <button key={m.key} className={`btn ${mode === m.key ? "btn-primary" : ""}`}
                onClick={() => setMode(m.key)} title={m.note}>
                {m.label}
              </button>
            ))}
          </div>
          <span className="hint">{MODES.find((m) => m.key === mode)?.note}</span>
        </div>
        <div className="field">
          <span className="label">叙事视角（之后可在阅读页随时切换，历史不受影响）</span>
          <div className="row">
            <button className={`btn ${pov === "protagonist" ? "btn-primary" : ""}`}
              onClick={() => setPov("protagonist")}>主角视角</button>
            <button className={`btn ${pov === "omniscient" ? "btn-primary" : ""}`}
              onClick={() => setPov("omniscient")}>全知读者视角</button>
          </div>
          <span className="hint">
            {pov === "protagonist"
              ? "只写主角能感知到的事物；你完全代入主角。"
              : "可以写配角、幕后与并行事件；停下时给出的仍是主角行动，你依旧代入主角。"}
          </span>
        </div>
        <button className="btn btn-primary" disabled={busy || !name.trim()} onClick={() => void create()}>
          创建并开始
        </button>
      </div>

      <div className="card">
        <div className="card-title">已有剧本 <span className="muted">· {stories.length} 本</span></div>
        {stories.length === 0 ? <div className="empty">还没有剧本</div> : (
          <div className="list">
            {stories.map((s) => (
              <div key={s.id} className="list-item">
                <div>
                  <div className="title">{s.name}</div>
                  <div className="desc">模式：{MODES.find((m) => m.key === s.engine_mode)?.label ?? s.engine_mode}</div>
                </div>
                <div className="actions">
                  <button className="btn btn-sm" onClick={() => void openBackground(s)}>事件背景</button>
                  <button className="btn btn-sm btn-primary" onClick={() => openStory(s)}>打开</button>
                  <button className="btn btn-sm btn-danger" onClick={() => void remove(s)}>删除</button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {bgStory && (
        <div className="dialog-backdrop">
          <div className="dialog">
            <h2>「{bgStory.name}」的事件背景覆盖</h2>
            <div className="hint">
              默认继承世界观的事件背景；关掉开关即恢复继承。覆盖内容全量注入上下文、不裁剪，不影响其它剧本。
            </div>
            <div className="field" style={{ marginTop: 12 }}>
              <span className="label">世界观默认背景（只读）</span>
              <div className="code-block" style={{ maxHeight: 120 }}>
                {bgData?.inherited_content_md || "（这个世界观还没写事件背景，去「设定集 → 事件背景」写）"}
              </div>
            </div>
            <div className="field">
              <label className="check-row" style={{ border: 0, padding: 0 }}>
                <input type="checkbox" checked={bgEnabled}
                  onChange={(e) => setBgEnabled(e.target.checked)} />
                用本剧本自己的背景覆盖
              </label>
            </div>
            <div className="field">
              <span className="label">本剧本背景</span>
              <textarea className="textarea" style={{ minHeight: 160 }} value={bgText}
                disabled={!bgEnabled} onChange={(e) => setBgText(e.target.value)} />
            </div>
            <div className="dialog-actions">
              <button className="btn" onClick={() => setBgStory(null)}>取消</button>
              <button className="btn btn-primary" onClick={() => void saveBackground()}>保存</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
