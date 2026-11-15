import { useCallback, useEffect, useState } from "react";
import { api } from "../ipc";
import { patchNestedScalar } from "../reader/policyPatch";
import type { EnginesConfig, Nav } from "../types";

type Kind = "local" | "api" | "paste";

interface Props { nav: Nav }

const MODES: { key: Kind; title: string; hint: string }[] = [
  {
    key: "local",
    title: "本地推理",
    hint: "LM Studio / Ollama / llama.cpp 的 OpenAI-compatible 服务，不花钱、不联网。",
  },
  {
    key: "api",
    title: "联网 API",
    hint: "任何 OpenAI-compatible 云服务（DeepSeek、Moonshot、OpenAI…）。key 只存在本机这个文件里。",
  },
  {
    key: "paste",
    title: "网页粘贴",
    hint: "不需要任何服务：应用把提示词放进剪贴板，你贴到网页 AI，回复自动读回。",
  },
];

/** Immutable set at a path, creating objects as needed. */
function setIn(obj: unknown, path: (string | number)[], value: unknown): unknown {
  if (path.length === 0) return value;
  const [head, ...rest] = path;
  const base: Record<string, unknown> =
    obj && typeof obj === "object" ? { ...(obj as Record<string, unknown>) } : {};
  base[head as string] = setIn(base[head as string], rest, value);
  return base;
}

export default function EnginesForm({ nav }: Props) {
  const [cfg, setCfg] = useState<EnginesConfig | null>(null);
  const [text, setText] = useState("");
  const [dirty, setDirty] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [sourceOpen, setSourceOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const [c, t] = await Promise.all([api.getEngines(), api.readConfigFile("engines.yaml")]);
      setCfg(c);
      setText(t);
      setDirty(false);
    } catch (e) {
      // Broken YAML: still let the user see and fix the source.
      nav.toast(String(e), "error");
      setCfg({});
      try { setText(await api.readConfigFile("engines.yaml")); } catch { setText(""); }
    }
  }, [nav]);

  useEffect(() => { void load(); }, [load]);

  const set = (path: (string | number)[], value: string | number | boolean | null) => {
    setCfg((c) => setIn(c ?? {}, path, value) as EnginesConfig);
    setText((t) => patchNestedScalar(t, path.map(String), value));
    setDirty(true);
  };

  const save = async () => {
    setBusy(true);
    try {
      await api.writeConfigFile("engines.yaml", text);
      setDirty(false);
      nav.toast("连接配置已保存（下一次调用即生效）");
      await load();
    } catch (e) { nav.toast(String(e), "error"); }
    finally { setBusy(false); }
  };

  const testEmbedding = async () => {
    nav.toast("正在测试向量服务…");
    try {
      const r = await api.testEmbedding();
      nav.toast(`向量服务可用：${r.model} · ${r.dim} 维 · ${r.ms} ms`);
    } catch (e) { nav.toast(`向量服务失败：${String(e)}`, "error"); }
  };

  const test = async (mode: Kind) => {
    nav.toast(`正在测试 ${mode}…`);
    try { nav.toast(await api.testConnection(mode)); }
    catch (e) { nav.toast(`失败：${String(e)}`, "error"); }
  };

  if (!cfg) return <div className="card"><div className="hint">读取 engines.yaml…</div></div>;

  const profile = (k: Kind) => (cfg.engines?.[k] ?? null) as
    | { base_url?: string; api_key?: string | null; model?: string; context_window?: number | null; target_hint?: string;
        params?: { temperature?: number; max_tokens?: number; top_p?: number | null };
        timeout_ms?: number; retries?: number }
    | null;

  return (
    <>
      <div className="card">
        <div className="card-title">
          连接配置 <span className="muted">· 表单改的就是 engines.yaml，注释会保留</span>
          <span className={`badge ${dirty ? "badge-dirty" : "badge-clean"}`} style={{ marginLeft: 8 }}>
            {dirty ? "未保存" : "已同步"}
          </span>
        </div>

        {MODES.map(({ key, title, hint }) => {
          const p = profile(key);
          if (!p) return null;
          return (
            <div className="engine-block" key={key}>
              <div className="engine-head">
                <strong>{title}</strong>
                <span className="mono muted">{key}</span>
                <div className="grow" />
                <button className="btn btn-sm" onClick={() => void test(key)}>测试连接</button>
              </div>
              <div className="hint">{hint}</div>

              {key === "paste" ? (
                <div className="policy-row">
                  <span className="pl">提示文案</span>
                  <input className="input" value={p.target_hint ?? ""}
                    placeholder="ChatGPT 网页版"
                    onChange={(e) => set(["engines", key, "target_hint"], e.target.value)} />
                  <span className="hint">只影响界面提示，不影响任何请求</span>
                </div>
              ) : (
                <>
                  <div className="policy-row">
                    <span className="pl">base_url</span>
                    <input className="input grow mono" value={p.base_url ?? ""}
                      placeholder={key === "local" ? "http://127.0.0.1:1234/v1" : "https://api.deepseek.com/v1"}
                      onChange={(e) => set(["engines", key, "base_url"], e.target.value)} />
                  </div>
                  <div className="policy-row">
                    <span className="pl">API Key</span>
                    <input className="input grow mono" type={showKey ? "text" : "password"}
                      value={p.api_key ?? ""} placeholder={key === "local" ? "本地服务通常留空" : "sk-…"}
                      onChange={(e) => set(["engines", key, "api_key"], e.target.value)} />
                    <label className="check-row" style={{ border: 0, padding: 0 }}>
                      <input type="checkbox" checked={showKey} onChange={(e) => setShowKey(e.target.checked)} />
                      显示
                    </label>
                  </div>
                  <div className="policy-row">
                    <span className="pl">模型名</span>
                    <input className="input grow mono" value={p.model ?? ""}
                      placeholder={key === "local" ? "qwen2.5-9b-instruct-q4_k_m" : "deepseek-chat"}
                      onChange={(e) => set(["engines", key, "model"], e.target.value)} />
                  </div>
                  <div className="policy-row">
                    <span className="pl">上下文窗口</span>
                    <input className="input" type="number" min={1024} step={1024}
                      value={p.context_window ?? ""} placeholder={key === "local" ? "10000" : "64000"}
                      onChange={(e) => set(["engines", key, "context_window"], Number(e.target.value) || 0)} />
                    <span className="hint">用于粗校验；真正决定裁剪的是策略面板的 context_window</span>
                  </div>
                  <div className="policy-row">
                    <span className="pl">采样参数</span>
                    <label className="inline-num">temperature
                      <input className="input" type="number" step={0.05} min={0} max={2}
                        value={p.params?.temperature ?? 0.8}
                        onChange={(e) => set(["engines", key, "params", "temperature"], Number(e.target.value))} />
                    </label>
                    <label className="inline-num">max_tokens
                      <input className="input" type="number" min={64} step={64}
                        value={p.params?.max_tokens ?? 1200}
                        onChange={(e) => set(["engines", key, "params", "max_tokens"], Number(e.target.value))} />
                    </label>
                    <label className="inline-num">top_p
                      <input className="input" type="number" step={0.05} min={0} max={1}
                        value={p.params?.top_p ?? ""} placeholder="留空"
                        onChange={(e) => {
                          const v = e.target.value.trim();
                          set(["engines", key, "params", "top_p"], v === "" ? null : Number(v));
                        }} />
                    </label>
                  </div>
                  <div className="policy-row">
                    <span className="pl">超时 / 重试</span>
                    <label className="inline-num">timeout_ms
                      <input className="input" type="number" min={5000} step={5000}
                        value={p.timeout_ms ?? 120000}
                        onChange={(e) => set(["engines", key, "timeout_ms"], Number(e.target.value))} />
                    </label>
                    <label className="inline-num">retries
                      <input className="input" type="number" min={0} max={5}
                        value={p.retries ?? 1}
                        onChange={(e) => set(["engines", key, "retries"], Number(e.target.value))} />
                    </label>
                  </div>
                </>
              )}
            </div>
          );
        })}

        <div className="engine-block">
          <div className="engine-head">
            <strong>向量检索（embedding）</strong>
            <span className="mono muted">embedding</span>
            <div className="grow" />
            <button className="btn btn-sm" onClick={() => void testEmbedding()}>测试向量服务</button>
          </div>
          <div className="hint">
            填了模型名才会启用向量那一路；留空就只用 BM25。改完先点「测试向量服务」，再到「策略面板 → 设定集检索」建索引。
          </div>
          <div className="policy-row">
            <span className="pl">来源</span>
            <select className="select" value={cfg.embedding?.provider || "local"}
              onChange={(e) => set(["embedding", "provider"], e.target.value)}>
              <option value="local">本地服务（借用 local 的地址与 key）</option>
              <option value="api">联网 API（借用 api 的地址与 key）</option>
              <option value="custom">自定义（只用手写地址与 key）</option>
            </select>
            <span className="hint">聊天用一个服务商、向量用另一个时选「自定义」</span>
          </div>
          <div className="policy-row">
            <span className="pl">base_url</span>
            <input className="input grow mono" value={cfg.embedding?.base_url ?? ""}
              placeholder={cfg.embedding?.provider === "api" ? "https://api.siliconflow.cn/v1（留空 = 用 api 的）" : "http://127.0.0.1:1234/v1（留空 = 用 local 的）"}
              onChange={(e) => set(["embedding", "base_url"], e.target.value)} />
          </div>
          <div className="policy-row">
            <span className="pl">API Key</span>
            <input className="input grow mono" type={showKey ? "text" : "password"}
              value={cfg.embedding?.api_key ?? ""}
              placeholder="留空 = 沿用所选来源的 key"
              onChange={(e) => set(["embedding", "api_key"], e.target.value)} />
            <span className="hint">在线向量服务通常必填</span>
          </div>
          <div className="policy-row">
            <span className="pl">模型名</span>
            <input className="input grow mono" value={cfg.embedding?.model ?? ""}
              placeholder="BAAI/bge-m3 / text-embedding-3-small / nomic-embed-text"
              onChange={(e) => set(["embedding", "model"], e.target.value)} />
          </div>
          <div className="policy-row">
            <span className="pl">维度</span>
            <input className="input" type="number" min={0} step={64}
              value={cfg.embedding?.dimensions ?? 0}
              onChange={(e) => set(["embedding", "dimensions"], Number(e.target.value) || 0)} />
            <span className="hint">0 = 用模型默认；只有 text-embedding-3-* 这类支持降维的模型才需要改</span>
          </div>
        </div>

        <div className="row" style={{ marginTop: 10 }}>
          <button className="btn btn-primary" disabled={!dirty || busy} onClick={() => void save()}>
            {busy ? "保存中…" : "保存连接配置"}
          </button>
          <button className="btn btn-ghost" onClick={() => void load()}>重新读取</button>
          <div className="grow" />
          <button className="btn btn-sm btn-ghost" onClick={() => setSourceOpen((v) => !v)}>
            {sourceOpen ? "隐藏源码" : "查看源码"}
          </button>
        </div>
        {dirty && <div className="hint warn-text">有未保存的改动；保存后下一次调用即生效。</div>}
      </div>

      {sourceOpen && (
        <div className="card">
          <div className="card-title">engines.yaml 源码 <span className="muted">· 和上面的表单是同一份文件</span></div>
          <textarea className="textarea mono" style={{ minHeight: 360, fontSize: 12 }} value={text}
            onChange={(e) => { setText(e.target.value); setDirty(true); }} />
          <div className="hint">保存前会做 YAML 校验；解析失败不会覆盖原文件（上面表单也会一起失效，请修正后再存）。</div>
        </div>
      )}
    </>
  );
}
