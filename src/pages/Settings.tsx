import { useEffect, useState } from "react";
import EnginesForm from "../components/EnginesForm";
import PolicyPanel from "../components/PolicyPanel";
import { api } from "../ipc";
import type { AppInfo, ContextBundle, Nav } from "../types";

const CONFIG_FILES = [
  "engines.yaml", "app.yaml",
  "policies/api.yaml", "policies/local.yaml", "policies/paste.yaml",
  "templates/system.api.md", "templates/system.local.md", "templates/system.paste.md",
  "templates/system.omni.api.md", "templates/system.omni.local.md", "templates/system.omni.paste.md",
  "templates/output.api.md", "templates/output.local.md", "templates/output.paste.md",
];

type Tab = "engines" | "panel" | "policies" | "preview" | "about";

export default function SettingsPage({ nav }: { nav: Nav }) {
  const [tab, setTab] = useState<Tab>("engines");
  const [relPath, setRelPath] = useState("engines.yaml");
  const [text, setText] = useState("");
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [dirty, setDirty] = useState(false);
  const [action, setAction] = useState("主角决定先观察一下周围");
  const [bundle, setBundle] = useState<ContextBundle | null>(null);

  useEffect(() => { api.getAppInfo().then(setInfo).catch(() => {}); }, []);
  useEffect(() => {
    api.readConfigFile(relPath).then((v) => { setText(v); setDirty(false); })
      .catch((e) => nav.toast(String(e), "error"));
  }, [relPath]);

  const save = async () => {
    try {
      await api.writeConfigFile(relPath, text);
      setDirty(false);
      nav.toast(`${relPath} 已保存，YAML 校验通过`);
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const preview = async () => {
    if (!nav.storyId) { nav.toast("请先在「剧本」里打开一本剧本", "error"); return; }
    try {
      setBundle(await api.buildBundle(nav.storyId, action));
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const exportDiag = async () => {
    try {
      const path = await api.exportDiagnostics();
      nav.toast(`诊断包已生成：${path}`);
      await api.openInShell(path).catch(() => {});
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const restoreDefaults = async () => {
    if (!window.confirm(
      "恢复出厂配置？当前 config/ 会先完整备份到 backups/，然后所有配置文件被覆盖（含 engines.yaml 里的 API key，请从备份找回）。"
    )) return;
    try {
      const files = await api.resetConfigToDefaults();
      nav.toast(`已恢复 ${files.length} 个出厂文件，原配置已备份到 backups/`);
      setText(await api.readConfigFile(relPath));
      setDirty(false);
    } catch (e) { nav.toast(String(e), "error"); }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>设置</h1>
          <div className="sub">配置文件是唯一权威源：界面保存和用外部编辑器手改的是同一份文件。</div>
        </div>
      </div>

      <div className="tabs">
        {([["engines", "连接配置"], ["panel", "策略面板"], ["policies", "源码与模板"], ["preview", "上下文预览"], ["about", "关于"]] as [Tab, string][])
          .map(([k, label]) => (
            <button key={k} className={`tab ${tab === k ? "active" : ""}`} onClick={() => setTab(k)}>{label}</button>
          ))}
      </div>

      {tab === "panel" && <PolicyPanel nav={nav} />}

      {tab === "engines" && <EnginesForm nav={nav} />}

      {tab === "policies" && (
        <ConfigEditor relPath={relPath} setRelPath={setRelPath} files={CONFIG_FILES}
          text={text} setText={(v) => { setText(v); setDirty(true); }} dirty={dirty} save={save} />
      )}

      {tab === "preview" && (
        <>
          <div className="card">
            <div className="card-title">组装预览 <span className="muted">· 当前剧本：{nav.storyId ?? "未选择"}</span></div>
            <div className="row">
              <input className="input grow" value={action} onChange={(e) => setAction(e.target.value)}
                placeholder="模拟本回合主角的行动" />
              <button className="btn btn-primary" onClick={() => void preview()}>重新组装</button>
            </div>
            <div className="hint" style={{ marginTop: 8 }}>
              这一步不调用任何 AI，只在本地把上下文拼出来，用来检查预算裁剪与模块开关。
            </div>
          </div>
          {bundle && (
            <>
              <div className="card">
                <div className="card-title">
                  预算报告 <span className="muted">· 估算 {bundle.report.total_est} / 上限 {bundle.report.cap} tokens</span>
                </div>
                {!bundle.report.retrieval_enabled && (
                  <div className="hint" style={{ marginBottom: 10 }}>设定检索未启用：lore_chunks 关闭，额度已回流给最近原文与 buffer。</div>
                )}
                {bundle.report.items.map((it) => (
                  <div className="budget-item" key={it.name}>
                    <span className="name">{it.name}{it.trimmed ? " ✂" : ""}</span>
                    <span className="track">
                      <span className="fill" style={{
                        width: `${Math.min(100, it.budget ? (it.est / it.budget) * 100 : 0)}%`,
                        opacity: it.enabled ? 1 : 0.25,
                      }} />
                    </span>
                    <span className="num">{it.enabled ? `${it.est} / ${it.budget}` : "关闭"}</span>
                  </div>
                ))}
                {bundle.report.warnings.length > 0 && (
                  <ul className="warn-list">
                    {bundle.report.warnings.map((w, i) => <li key={i}>{w}</li>)}
                  </ul>
                )}
                <div className="budget-summary">
                  合计 {bundle.report.total_est} tokens；宁少勿超：估算偏高时自动少注入，避免本地 10K 窗口溢出。
                  事件背景与时间线全量注入、不参与裁剪。
                </div>
              </div>
              <div className="card">
                <div className="card-title">最终提示词（可整块复制）</div>
                <div className="code-block" style={{ maxHeight: 420 }}>{bundle.text}</div>
              </div>
            </>
          )}
        </>
      )}

      {tab === "about" && (
        <div className="card">
          <div className="card-title">数据与文件位置 <span className="muted">· 便携式</span></div>
          {info ? (
            <>
              <div className="code-block">
                运行方式：{info.portable ? "便携（数据与 exe 同目录）" : "自定义数据目录"}{"\n"}
                数据目录：{info.data_dir}{"\n"}
                配置目录：{info.config_dir}{"\n"}
                世界观目录：{info.worlds_dir}{"\n"}
                数据库：{info.db_path}{"\n"}
                出厂配置版本：v{info.defaults_version}{"\n"}
                旧数据迁移自：{info.migrated_from || "无"}{"\n\n"}
                整个程序文件夹可以拷到别的盘或 U 盘直接使用；{"\n"}
                首次启动会询问是否从 %APPDATA% 复制旧数据（含 WAL 文件），原目录不会被改动。{"\n"}
                设定文档位于 worlds/&lt;世界观&gt;/lore/*.md，可在应用内可视化/源码编辑，也可用外部编辑器打开。
              </div>
              {info.needs_merge.length > 0 && (
                <div className="card" style={{ marginTop: 12 }}>
                  <div className="card-title">需要手工合并的出厂文件</div>
                  <div className="hint">这些文件你改过，新版默认值已放在同目录的 <code>.new</code> 文件里，请对照合并：</div>
                  <ul className="warn-list">{info.needs_merge.map((f) => <li key={f}>{f}</li>)}</ul>
                </div>
              )}
              {info.config_warnings.length > 0 && (
                <div className="card" style={{ marginTop: 12 }}>
                  <div className="card-title">配置里有不会被读取的项</div>
                  <ul className="warn-list">
                    {info.config_warnings.map((w) => <li key={w}>{w}</li>)}
                  </ul>
                  <div className="hint">多半是拼写错误或旧版残留；删掉或改正即可，不影响运行。</div>
                </div>
              )}
              <div className="row" style={{ marginTop: 12 }}>
                <button className="btn btn-danger" onClick={() => void restoreDefaults()}>恢复出厂配置（先备份）</button>
                <button className="btn" onClick={() => void exportDiag()}>导出诊断包</button>
                <span className="hint">诊断包含日志、脱敏配置与库统计（不含正文）</span>
              </div>
            </>
          ) : <div className="hint">读取中…</div>}
          <div className="hint" style={{ marginTop: 12 }}>
            当前版本：小说式阅读器（暗场 / 阅读位置记忆 / 导出为小说）＋ 四段输出契约 ＋ 分层状态卡与角色两档
            ＋ 双视角 ＋ 导演模式 ＋ 自动存档 ＋ 便携数据目录 ＋ 设定三路注入（BM25 + 可选向量 RRF）。
          </div>
        </div>
      )}
    </div>
  );
}

function ConfigEditor({ relPath, setRelPath, files, text, setText, dirty, save }: {
  relPath: string;
  setRelPath: (v: string) => void;
  files: string[];
  text: string;
  setText: (v: string) => void;
  dirty: boolean;
  save: () => void;
}) {
  return (
    <div className="card">
      <div className="card-title">
        编辑 {relPath}
        <span className={`badge ${dirty ? "badge-dirty" : "badge-clean"}`} style={{ marginLeft: 8 }}>
          {dirty ? "未保存" : "已同步"}
        </span>
      </div>
      <div className="split">
        <div className="split-side">
          {files.map((f) => (
            <button key={f} className={`side-item ${relPath === f ? "active" : ""}`} onClick={() => setRelPath(f)}>
              <span className="mono" style={{ fontSize: 12 }}>{f}</span>
            </button>
          ))}
        </div>
        <div>
          <textarea className="textarea mono" style={{ minHeight: 420, fontSize: 12 }} value={text}
            onChange={(e) => setText(e.target.value)} />
          <div className="row" style={{ marginTop: 10 }}>
            <span className="hint">YAML 保存前会校验，解析失败不会覆盖原文件</span>
            <div className="grow" />
            <button className="btn btn-primary" disabled={!dirty} onClick={save}>保存</button>
          </div>
        </div>
      </div>
    </div>
  );
}
