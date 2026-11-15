import { useCallback, useEffect, useState } from "react";
import { api } from "../../ipc";
import type { BudgetReport, PreviewBundle, TurnPrompt } from "../../types";

interface Props {
  storyId: string;
  /** The most recent applied turn (undefined when the story is still empty). */
  turnId: string | null;
  turnLabel: string;
  onClose: () => void;
  onToast: (msg: string, kind?: "ok" | "error") => void;
}

const MODULE_LABELS: Record<string, string> = {
  system_contract: "输出契约",
  protagonist: "主角人设（不裁剪）",
  persona_opening: "人设+开场",
  opening_scene: "开场情境",
  state_card: "状态卡",
  rolling_summary: "滚动摘要",
  characters: "重要角色",
  lore_pins: "常驻设定卡",
  lore_index: "设定集目录",
  lore_chunks: "设定检索",
  recent_passages: "最近正文",
  action_choices: "本次行动",
  action: "本次行动",
  stop_policy: "停下策略",
  director_outline: "导演大纲",
  director_note: "导演指令（不裁剪）",
  fixed_background: "固定背景",
  event_background: "事件背景+时间线（不裁剪）",
  buffer: "预留（给输出）",
};

function parseReport(p: PromptMeta | null): BudgetReport | null {
  if (!p || !p.report) return null;
  return p.report;
}

interface PromptMeta {
  mode?: string;
  pov?: string;
  template?: string;
  report?: BudgetReport;
}

function BudgetTable({ report }: { report: BudgetReport }) {
  return (
    <div className="budget-table">
      <div className="budget-head">
        <span>模块</span><span>额度</span><span>实际</span><span></span>
      </div>
      {report.items.map((it) => (
        <div className={`budget-row ${it.enabled ? "" : "is-off"}`} key={it.name}>
          <span>{MODULE_LABELS[it.name] ?? it.name}</span>
          <span className="num">{it.budget > 1_000_000 ? "—" : it.budget}</span>
          <span className="num">{it.est}</span>
          <span>{it.trimmed ? <em className="budget-cut">已压缩</em> : null}</span>
        </div>
      ))}
      <div className="budget-total">
        合计 {report.total_est} / 上限 {report.cap}
        {report.retrieval_enabled ? " · 检索已开" : " · 检索关闭"}
      </div>
      {report.warnings.map((w, i) => (
        <div className="budget-warn" key={i}>· {w}</div>
      ))}
    </div>
  );
}

export default function ContextInspector({ storyId, turnId, turnLabel, onClose, onToast }: Props) {
  const [prompts, setPrompts] = useState<TurnPrompt[]>([]);
  const [active, setActive] = useState<string>("narration");
  const [preview, setPreview] = useState<PreviewBundle | null>(null);
  const [previewAction, setPreviewAction] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    if (!turnId) { setPrompts([]); return; }
    try {
      const rows = await api.listTurnPrompts(turnId);
      setPrompts(rows);
      if (rows.length && !rows.some((r) => r.kind === active)) setActive(rows[0].kind);
    } catch (e) { onToast(String(e), "error"); }
  }, [turnId, active, onToast]);

  useEffect(() => { void load(); }, [load]);

  const current = prompts.find((p) => p.kind === active) ?? prompts[0] ?? null;
  let meta: PromptMeta | null = null;
  if (current?.meta_json) {
    try { meta = JSON.parse(current.meta_json) as PromptMeta; } catch { meta = null; }
  }

  const doPreview = async () => {
    setBusy(true);
    try {
      const b = await api.previewBundle(storyId, previewAction.trim() || "（读者尚未决定）");
      setPreview(b);
    } catch (e) { onToast(String(e), "error"); }
    finally { setBusy(false); }
  };

  const copy = async (text: string) => {
    try { await navigator.clipboard.writeText(text); onToast("已复制整段提示词"); }
    catch { onToast("复制失败，可手动选中", "error"); }
  };

  const shown = preview ? preview.text : current?.text ?? "";
  const report = preview ? preview.report : parseReport(meta);

  return (
    <aside className="archive-drawer context-inspector">
      <div className="archive-head">
        <strong>上下文</strong>
        <span className="muted">{turnLabel}</span>
        <div className="grow" />
        <button className="btn btn-sm btn-ghost" onClick={onClose}>收起</button>
      </div>

      <div className="tabs archive-tabs">
        {prompts.map((p) => (
          <button key={p.kind} className={`tab ${!preview && active === p.kind ? "active" : ""}`}
            onClick={() => { setPreview(null); setActive(p.kind); }}>
            {p.kind === "narration" ? "正文阶段" : "分析阶段"}
          </button>
        ))}
      </div>

      <div className="archive-body">
        {!turnId && <div className="empty">还没有回合。先写一段，注入内容会被完整记录下来。</div>}
        {turnId && !current && <div className="empty">这一回合没有留下记录（可能是旧版本写入的）。</div>}
        {prompts.length === 1 && (
          <div className="hint">
            没有独立的「分析阶段」记录：默认情况下 ANALYSIS 和正文在同一次调用里返回，
            模型看到的就是上面这一整套上下文。要让它成为独立调用，在设置里打开「额外独立分析调用」。
          </div>
        )}

        {current && !preview && (
          <div className="inspector-meta">
            {meta?.template && <span>模板 <code>{meta.template}</code></span>}
            {meta?.pov && <span>视角 {meta.pov === "omniscient" ? "全知读者" : "主角"}</span>}
            {meta?.mode && <span>模式 {meta.mode}</span>}
            <span>约 {current.est} tokens</span>
          </div>
        )}
        {preview && (
          <div className="inspector-meta">
            <span>预览 · 模板 <code>{preview.template}</code></span>
            <span>视角 {preview.pov === "omniscient" ? "全知读者" : "主角"}</span>
            <span>约 {preview.report.total_est} tokens</span>
          </div>
        )}

        {report && <BudgetTable report={report} />}

        {shown ? (
          <>
            <div className="row" style={{ margin: "8px 0" }}>
              <button className="btn btn-sm" onClick={() => void copy(shown)}>复制全文</button>
              {preview && (
                <button className="btn btn-sm btn-ghost" onClick={() => setPreview(null)}>回到本回合</button>
              )}
            </div>
            <pre className="prompt-view">{shown}</pre>
          </>
        ) : null}

        <div className="preview-box">
          <div className="hint">预览下一回合：填一个假设的行动，不调用模型，只看会注入什么。</div>
          <div className="row" style={{ marginTop: 6 }}>
            <input className="input" value={previewAction}
              placeholder="例如：唐三相助小舞，问她的来历"
              onChange={(e) => setPreviewAction(e.target.value)} />
            <button className="btn btn-sm btn-primary" disabled={!storyId || busy}
              onClick={() => void doPreview()}>{busy ? "生成中…" : "预览"}</button>
          </div>
        </div>
      </div>
    </aside>
  );
}
