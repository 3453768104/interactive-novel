import { useCallback, useEffect, useState } from "react";
import { api } from "../ipc";
import { patchInlineMap, patchYamlScalar, triggersToYaml } from "../reader/policyPatch";
import {
  STOP_TRIGGER_OPTIONS,
  type AppSettings, type ContextBundle, type EngineMode, type ModuleCfg,
  type Nav, type Policy, type RetrievalCfg, type RetrievalStatus,
} from "../types";

const MODES: { key: EngineMode; label: string }[] = [
  { key: "paste", label: "网页粘贴" },
  { key: "local", label: "本地推理" },
  { key: "api", label: "联网 API" },
];

const MODULE_LABELS: Record<string, string> = {
  system_contract: "输出契约",
  protagonist: "主角人设（不可裁剪）",
  opening_scene: "开场情境",
  state_card: "分层状态卡",
  rolling_summary: "滚动摘要",
  characters: "重要角色",
  lore_pins: "常驻设定卡（置顶）",
  lore_index: "设定集目录",
  lore_chunks: "设定检索片段",
  director_outline: "导演大纲",
  recent_passages: "最近正文",
  action_choices: "本回合行动",
  stop_policy: "停顿策略说明",
  buffer: "预留缓冲",
};

interface Props { nav: Nav }

export default function PolicyPanel({ nav }: Props) {
  const [mode, setMode] = useState<EngineMode>("local");
  const [policy, setPolicy] = useState<Policy | null>(null);
  const [policyText, setPolicyText] = useState("");
  const [app, setApp] = useState<AppSettings | null>(null);
  const [appText, setAppText] = useState("");
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [action, setAction] = useState("主角决定先观察一下周围");
  const [bundle, setBundle] = useState<ContextBundle | null>(null);
  const [msg, setMsg] = useState("");
  const [retrieval, setRetrieval] = useState<RetrievalStatus | null>(null);
  const [idxBusy, setIdxBusy] = useState(false);

  const load = useCallback(async (m: EngineMode) => {
    try {
      const [p, pt, at, a] = await Promise.all([
        api.getPolicy(m),
        api.readConfigFile(`policies/${m}.yaml`),
        api.readConfigFile("app.yaml"),
        api.getAppSettings(),
      ]);
      setPolicy(p); setPolicyText(pt); setAppText(at); setApp(a);
      setDirty(false); setMsg("");
    } catch (e) { setMsg(String(e)); }
  }, []);

  useEffect(() => { void load(mode); }, [mode, load]);

  const loadRetrieval = useCallback(async () => {
    if (!nav.worldId) { setRetrieval(null); return; }
    try { setRetrieval(await api.retrievalStatus(nav.worldId)); }
    catch { setRetrieval(null); }
  }, [nav.worldId]);

  useEffect(() => { void loadRetrieval(); }, [loadRetrieval]);

  const rebuildIndex = async () => {
    if (!nav.worldId) { nav.toast("先选一个世界观", "error"); return; }
    setIdxBusy(true);
    try {
      const r = await api.rebuildRetrievalIndex(nav.worldId);
      nav.toast(`索引完成：新索引 ${r.indexed} 段，向量 ${r.embedded} 条（${r.model}）`);
      await loadRetrieval();
    } catch (e) { nav.toast(String(e), "error"); }
    finally { setIdxBusy(false); }
  };

  const updateRetrieval = (patch: Partial<RetrievalCfg>) => {
    if (!policy) return;
    setPolicy({ ...policy, retrieval: { ...policy.retrieval, ...patch } });
    setPolicyText((t) => patchInlineMap(t, "retrieval", patch as Record<string, string | number | boolean>));
    setDirty(true);
  };

  const updateModule = (name: string, patch: Partial<ModuleCfg>) => {
    if (!policy) return;
    const prev = policy.modules[name] ?? { on: true, budget: 0, n: 0, top_n: 0, trim: 0 };
    setPolicy({
      ...policy,
      modules: { ...policy.modules, [name]: { ...prev, ...patch } },
    });
    setPolicyText((t) => patchInlineMap(t, name, patch as Record<string, string | number | boolean>));
    setDirty(true);
  };

  const updatePolicyScalar = (key: "context_window" | "max_output_tokens", value: number) => {
    if (!policy) return;
    setPolicy({ ...policy, [key]: value });
    setPolicyText((t) => patchYamlScalar(t, key, value));
    setDirty(true);
  };

  const updateThrottle = (patch: { summary_every_n?: number; state_mode?: string }) => {
    if (!policy) return;
    setPolicy({ ...policy, throttle: { ...policy.throttle, ...patch } });
    setPolicyText((t) => patchInlineMap(t, "throttle", patch as Record<string, string | number | boolean>));
    setDirty(true);
  };

  const updateApp = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    if (!app) return;
    setApp({ ...app, [key]: value });
    const yamlValue = Array.isArray(value) ? triggersToYaml(value as string[]) : (value as string | number | boolean);
    setAppText((t) => patchYamlScalar(t, key, yamlValue));
    setDirty(true);
  };

  const toggleTrigger = (key: string) => {
    if (!app) return;
    const next = app.stop_triggers.includes(key)
      ? app.stop_triggers.filter((t) => t !== key)
      : [...app.stop_triggers, key];
    updateApp("stop_triggers", next);
  };

  const save = async () => {
    setBusy(true);
    try {
      await api.writeConfigFile(`policies/${mode}.yaml`, policyText);
      await api.writeConfigFile("app.yaml", appText);
      setDirty(false);
      setMsg("已保存：可视化面板与源码 tab 写的是同一份 YAML 文件");
      await load(mode);
    } catch (e) { setMsg(String(e)); }
    finally { setBusy(false); }
  };

  const preview = async () => {
    if (!nav.storyId) { setMsg("请先在「剧本」里打开一本剧本，才能组装预览"); return; }
    try { setBundle(await api.buildBundle(nav.storyId, action)); }
    catch (e) { setMsg(String(e)); }
  };

  return (
    <>
      <div className="card">
        <div className="card-title">
          可视化策略面板
          <span className={`badge ${dirty ? "badge-dirty" : "badge-clean"}`} style={{ marginLeft: 8 }}>
            {dirty ? "未保存" : "已同步"}
          </span>
        </div>
        <div className="row" style={{ marginBottom: 12 }}>
          <span className="hint">为哪个模式调整</span>
          {MODES.map((m) => (
            <button key={m.key} className={`btn btn-sm ${mode === m.key ? "btn-primary" : ""}`}
              onClick={() => setMode(m.key)}>{m.label}</button>
          ))}
          <div className="grow" />
          <button className="btn btn-sm btn-primary" disabled={!dirty || busy} onClick={() => void save()}>保存</button>
        </div>
        <div className="hint">
          改动直接写回 <code>policies/{mode}.yaml</code> 与 <code>app.yaml</code>；「策略与模板」源码 tab 看到的是同一份文件，注释会保留。
        </div>
      </div>

      <div className="policy-grid">
        <div>
          {policy && (
            <>
              <div className="card">
                <div className="card-title">上下文预算</div>
                <div className="policy-row">
                  <span className="pl">上下文窗口（tokens）</span>
                  <input type="number" min={1000} step={500} value={policy.context_window}
                    onChange={(e) => updatePolicyScalar("context_window", Number(e.target.value))} />
                  <span className="pl" style={{ width: "auto" }}>输出预留</span>
                  <input type="number" min={200} step={100} value={policy.max_output_tokens}
                    onChange={(e) => updatePolicyScalar("max_output_tokens", Number(e.target.value))} />
                </div>
                <div className="hint">主角人设、事件背景与时间线按约定 100% 注入、不占这里的裁剪额度。</div>
              </div>

              <div className="card">
                <div className="card-title">模块开关与 token 预算 <span className="muted">· 上限而非配额</span></div>
                {Object.entries(policy.modules).map(([name, m]) => (
                  <div className="policy-module" key={name}>
                    <label className="pm-name" style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
                      <input type="checkbox" checked={m.on}
                        onChange={(e) => updateModule(name, { on: e.target.checked })} />
                      {MODULE_LABELS[name] ?? name}
                    </label>
                    <input className="pm-num" type="number" min={0} step={100} value={m.budget}
                      disabled={!m.on}
                      onChange={(e) => updateModule(name, { budget: Number(e.target.value) })} />
                    <input type="range" min={0} max={Math.max(4000, m.budget)} step={50}
                      value={m.budget} disabled={!m.on}
                      onChange={(e) => updateModule(name, { budget: Number(e.target.value) })} />
                    <span className="muted" style={{ fontSize: 11 }}>
                      {name === "recent_passages" ? `N=${m.n}` : name === "lore_chunks" ? `top=${m.top_n}` : name === "protagonist" ? "不裁剪" : `trim=${m.trim}`}
                    </span>
                  </div>
                ))}
                <div className="hint" style={{ marginTop: 8 }}>
                  trim 数字越大，超预算时越先被压缩；主角人设与事件背景永远不参与裁剪。
                </div>
              </div>

              <div className="card">
                <div className="card-title">记忆节流（现在真的生效）</div>
                <div className="policy-row">
                  <span className="pl">每 N 回合写一次摘要</span>
                  <input type="number" min={1} max={20} value={policy.throttle.summary_every_n}
                    onChange={(e) => updateThrottle({ summary_every_n: Number(e.target.value) })} />
                  <span className="hint">本地 30 tok/s 下建议 3</span>
                </div>
                <div className="policy-row">
                  <span className="pl">状态卡更新方式</span>
                  <select value={policy.throttle.state_mode}
                    onChange={(e) => updateThrottle({ state_mode: e.target.value })}>
                    <option value="every_turn">每回合写</option>
                    <option value="every_n">按上面的 N</option>
                    <option value="manual">只手动写</option>
                  </select>
                </div>
              </div>
            </>
          )}

          {app && (
            <>
              <div className="card">
                <div className="card-title">AI 何时停下让我行动</div>
                <div className="policy-checks" style={{ marginBottom: 10 }}>
                  {STOP_TRIGGER_OPTIONS.map((t) => (
                    <label key={t.key} title={t.hint}>
                      <input type="checkbox" checked={app.stop_triggers.includes(t.key)}
                        onChange={() => toggleTrigger(t.key)} />
                      {t.label}
                    </label>
                  ))}
                </div>
                <div className="policy-row">
                  <span className="pl">最多连写几段（防跑飞）</span>
                  <input type="number" min={1} max={20} value={app.max_consecutive_turns}
                    onChange={(e) => updateApp("max_consecutive_turns", Number(e.target.value))} />
                </div>
              </div>

              <div className="card">
                <div className="card-title">设定集检索</div>
                <div className="policy-row">
                  <span className="pl">启用检索</span>
                  <label className="check-row" style={{ border: 0, padding: 0 }}>
                    <input type="checkbox" checked={app.retrieval_enabled}
                      onChange={(e) => updateApp("retrieval_enabled", e.target.checked)} />
                    用当前行动 + 最近正文去命中设定卡片（关键词打分，不调模型）
                  </label>
                </div>
                <div className="policy-row">
                  <span className="pl">本模式注入命中片段</span>
                  <label className="check-row" style={{ border: 0, padding: 0 }}>
                    <input type="checkbox" checked={policy?.modules.lore_chunks?.on ?? false}
                      onChange={(e) => updateModule("lore_chunks",
                        e.target.checked
                          ? { on: true, budget: 2000, top_n: 6 }
                          : { on: false, budget: 0 })} />
                    lore_chunks（当前 top_n = {policy?.modules.lore_chunks?.top_n ?? 0}）
                  </label>
                </div>
                <div className="policy-row">
                  <span className="pl">设定集目录</span>
                  <label className="check-row" style={{ border: 0, padding: 0 }}>
                    <input type="checkbox" checked={policy?.modules.lore_index?.on ?? false}
                      onChange={(e) => updateModule("lore_index",
                        e.target.checked
                          ? { on: true, budget: 300, trim: 9 }
                          : { on: false, budget: 0 })} />
                    每回合附一份「分类 + 卡片标题」清单，让模型知道有哪些设定
                  </label>
                </div>
                <div className="hint">
                  目录解决"不知道有这条设定"，检索负责把细节段落捞进来，置顶卡片则永远整张注入。
                  目录/置顶不受「启用检索」影响。
                </div>

                <div className="policy-row">
                  <span className="pl">索引状态</span>
                  {retrieval ? (
                    <span className="hint">
                      分片 {retrieval.chunks} · BM25 已索引 {retrieval.fts} · 向量 {retrieval.vectors}
                      {retrieval.vec_enabled
                        ? `（${retrieval.model} @ ${retrieval.endpoint || "默认地址"}）`
                        : "（未配置 embedding 模型，只用 BM25）"}
                    </span>
                  ) : <span className="hint">先选一个世界观</span>}
                  <button className="btn btn-sm" disabled={idxBusy || !nav.worldId}
                    onClick={() => void rebuildIndex()}>
                    {idxBusy ? "建索引中…" : "建立 / 更新索引"}
                  </button>
                </div>
                <div className="policy-row">
                  <span className="pl">向量召回条数</span>
                  <input type="number" min={0} max={60} value={policy?.retrieval.vec_topk ?? 30}
                    onChange={(e) => updateRetrieval({ vec_topk: Number(e.target.value) })} />
                  <span className="hint">0 = 关掉向量，只跑 BM25</span>
                </div>
                <div className="policy-row">
                  <span className="pl">BM25 / RRF 参数</span>
                  <input type="number" min={5} max={200} value={policy?.retrieval.bm25_topk ?? 30}
                    onChange={(e) => updateRetrieval({ bm25_topk: Number(e.target.value) })} />
                  <input type="number" min={1} max={200} value={policy?.retrieval.rrf_k ?? 60}
                    onChange={(e) => updateRetrieval({ rrf_k: Number(e.target.value) })} />
                  <span className="hint">前一个是 BM25 取几条，后一个是 RRF 平滑常数 k</span>
                </div>
              </div>

              <div className="card">
                <div className="card-title">分析、角色与阅读</div>
                <div className="policy-row">
                  <span className="pl">额外独立分析调用</span>
                  <label className="check-row" style={{ border: 0, padding: 0 }}>
                    <input type="checkbox" checked={app.analysis_second_pass}
                      onChange={(e) => updateApp("analysis_second_pass", e.target.checked)} />
                    api/local 再跑一次便宜小模型（默认关，速度优先）
                  </label>
                </div>
                <div className="policy-row">
                  <span className="pl">哪些角色进长期上下文</span>
                  <select value={app.character_inject}
                    onChange={(e) => updateApp("character_inject", e.target.value as AppSettings["character_inject"])}>
                    <option value="notable">重要角色（名字 + 当前状态）</option>
                    <option value="none">都不进</option>
                  </select>
                </div>
                <div className="policy-row">
                  <span className="pl">开场情境注入前 N 回合</span>
                  <input type="number" min={0} max={20} value={app.opening_scene_turns}
                    onChange={(e) => updateApp("opening_scene_turns", Number(e.target.value))} />
                </div>
                <div className="policy-row">
                  <span className="pl">主角人设软上限（只警告）</span>
                  <input type="number" min={100} step={100} value={app.persona_soft_cap}
                    onChange={(e) => updateApp("persona_soft_cap", Number(e.target.value))} />
                </div>
                <div className="policy-row">
                  <span className="pl">全知视角分隔标记</span>
                  <label className="check-row" style={{ border: 0, padding: 0 }}>
                    <input type="checkbox" checked={app.show_scene_markers}
                      onChange={(e) => updateApp("show_scene_markers", e.target.checked)} />
                    显示「◇ 与此同时」细分隔线
                  </label>
                </div>
              </div>
            </>
          )}
        </div>

        <div>
          <div className="card">
            <div className="card-title">实时预算预览</div>
            <div className="hint">不调用任何 AI，只在本地组装一次上下文。</div>
            <div className="row" style={{ margin: "10px 0" }}>
              <input className="input grow" value={action}
                onChange={(e) => setAction(e.target.value)} placeholder="模拟本回合主角行动" />
            </div>
            <button className="btn btn-sm btn-primary" onClick={() => void preview()}>重新组装</button>
            {bundle && (
              <div style={{ marginTop: 12 }}>
                <div className="budget-summary">
                  合计 {bundle.report.total_est} / 上限 {bundle.report.cap} tokens
                </div>
                {bundle.report.items.map((it) => (
                  <div className="budget-item" key={it.name}>
                    <span className="name">{it.name}{it.trimmed ? " ✂" : ""}</span>
                    <span className="track">
                      <span className="fill" style={{
                        width: `${Math.min(100, it.budget ? (it.est / it.budget) * 100 : 0)}%`,
                        opacity: it.enabled ? 1 : .25,
                      }} />
                    </span>
                    <span className="num">{it.enabled ? `${it.est}/${it.budget}` : "关"}</span>
                  </div>
                ))}
                {bundle.report.warnings.length > 0 && (
                  <ul className="warn-list">{bundle.report.warnings.map((w, i) => <li key={i}>{w}</li>)}</ul>
                )}
              </div>
            )}
          </div>
          {msg && <div className="card"><div className="hint">{msg}</div></div>}
        </div>
      </div>
    </>
  );
}
