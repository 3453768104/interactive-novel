import { useCallback, useEffect, useRef, useState } from "react";
import ParseFixDialog from "../components/ParseFixDialog";
import { api } from "../ipc";
import { CHOICES_SUSPECT, type ContextBundle, type Nav, type ParsedOutput } from "../types";

interface Props {
  nav: Nav;
  turnId: string;
  bundle: ContextBundle;
}

const AUTO_READ_KEY = "interactive-novel.paste.autoRead";
const AUTO_APPLY_KEY = "interactive-novel.paste.autoApply";

function loadFlag(key: string, fallback: boolean): boolean {
  const v = localStorage.getItem(key);
  return v == null ? fallback : v === "1";
}

/**
 * Does this look like a model reply rather than some random thing the user
 * copied? Paste mode watches the clipboard, so false positives are the danger:
 * a copied URL or a single sentence must not overwrite the workspace.
 */
function looksLikeReply(text: string, promptText: string): boolean {
  const t = text.trim();
  if (t.length < 300) return false;
  if (t === promptText.trim()) return false;
  return (
    t.includes("<<<") ||
    t.includes("【正文】") ||
    t.includes('"narration"') ||
    t.length >= 800
  );
}

export default function PasteWorkspace({ nav, turnId, bundle }: Props) {
  const [raw, setRaw] = useState("");
  const [parsed, setParsed] = useState<ParsedOutput | null>(null);
  const [fixOpen, setFixOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [autoRead, setAutoRead] = useState(() => loadFlag(AUTO_READ_KEY, true));
  const [autoApply, setAutoApply] = useState(() => loadFlag(AUTO_APPLY_KEY, true));
  const [status, setStatus] = useState("准备中…");
  const [copied, setCopied] = useState(false);

  /** The clipboard content we already consumed (or that was there on open). */
  const seenRef = useRef<string>("");
  const busyRef = useRef(false);
  const done = useRef(false);

  useEffect(() => { localStorage.setItem(AUTO_READ_KEY, autoRead ? "1" : "0"); }, [autoRead]);
  useEffect(() => { localStorage.setItem(AUTO_APPLY_KEY, autoApply ? "1" : "0"); }, [autoApply]);
  useEffect(() => { busyRef.current = busy; }, [busy]);

  const copyPrompt = useCallback(async (announce: boolean) => {
    try {
      await api.writeClipboard(bundle.text);
      setCopied(true);
      seenRef.current = bundle.text;
      if (announce) nav.toast("提示词已复制，去网页 AI 里 Ctrl+V");
    } catch (e) {
      nav.toast(`复制失败：${String(e)}`, "error");
    }
  }, [bundle.text, nav]);

  // Coming from a choice click, the only remaining manual step should be
  // Ctrl+V in the browser — so copy before the user has to think about it.
  useEffect(() => {
    void copyPrompt(false);
    setStatus("提示词已复制 · 正在等网页 AI 的回复");
  }, [copyPrompt]);

  const parseAndMaybeApply = useCallback(async (text: string) => {
    if (done.current || busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    try {
      const p = await api.submitPastedOutput(turnId, text);
      setParsed(p);
      const suspect = p.warnings.some((w) => w.startsWith(CHOICES_SUSPECT));
      const clean = Boolean(p.narration.trim()) && !suspect;
      if (!clean) {
        setFixOpen(true);
        setStatus("已读取，但解析需要你确认（点人工修正）");
        return;
      }
      if (!autoApply) {
        setStatus("已读取并解析 · 检查后点「应用到剧本」");
        return;
      }
      done.current = true;
      await api.applyOutput(turnId, p);
      nav.toast("已自动写入这一回合");
      nav.go("reader");
    } catch (e) {
      nav.toast(String(e), "error");
      setStatus("写入失败，请手动重试");
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }, [autoApply, nav, turnId]);

  // Clipboard watcher. It must keep running while the app is in the background
  // (that is exactly when the user copies the reply), so no visibility check.
  useEffect(() => {
    if (!autoRead) return;
    let stop = false;
    const tick = async () => {
      if (stop || done.current || busyRef.current) return;
      try {
        const text = await api.clipboardPeek();
        if (!text || text === seenRef.current) return;
        if (!looksLikeReply(text, bundle.text)) { seenRef.current = text; return; }
        seenRef.current = text;
        setRaw(text);
        setStatus(`已从剪贴板读取 ${text.length} 字 · 解析中…`);
        await parseAndMaybeApply(text);
      } catch {
        /* clipboard busy / empty: try again next tick */
      }
    };
    const id = window.setInterval(() => void tick(), 900);
    void tick();
    return () => { stop = true; window.clearInterval(id); };
  }, [autoRead, bundle.text, parseAndMaybeApply]);

  const copy = async () => {
    await copyPrompt(true);
  };

  const parse = async () => {
    if (!raw.trim()) return;
    await parseAndMaybeApply(raw);
  };

  const apply = async (p: ParsedOutput) => {
    if (done.current) return;
    done.current = true;
    setBusy(true);
    try {
      await api.applyOutput(turnId, p);
      nav.toast("已写入剧本");
      nav.go("reader");
    } catch (e) {
      done.current = false;
      nav.toast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>粘贴工作区</h1>
          <div className="sub">
            网页端 AI 没有应用侧记忆，所以左边这份提示词是<b>自包含</b>的。
            开着「自动读取」时，你在网页里选中回复按 Ctrl+C，这里会自己接住并写入。
          </div>
        </div>
        <button className="btn" onClick={() => nav.go("reader")}>返回阅读</button>
      </div>

      <div className="card paste-auto">
        <div className="row" style={{ gap: 18, flexWrap: "wrap" }}>
          <label className="check-row" style={{ border: 0, padding: 0 }}>
            <input type="checkbox" checked={autoRead} onChange={(e) => setAutoRead(e.target.checked)} />
            自动读取剪贴板
          </label>
          <label className="check-row" style={{ border: 0, padding: 0 }}>
            <input type="checkbox" checked={autoApply} onChange={(e) => setAutoApply(e.target.checked)} />
            解析无误时自动写入剧本
          </label>
          <span className={`paste-status ${copied ? "is-ok" : ""}`}>{status}</span>
        </div>
        <div className="hint">
          用的是系统剪贴板，不碰浏览器、不碰登录态。复制别的东西（短句、链接）不会被误收；
          只有像模型回复的长文本才会被接住。
        </div>
      </div>

      <div className="grid-2">
        <div className="card">
          <div className="card-title">
            ① 整块提示词
            <span className="muted"> · 约 {bundle.report.total_est} / {bundle.report.cap} tokens</span>
          </div>
          <div className="row" style={{ marginBottom: 10 }}>
            <button className="btn btn-primary btn-sm" onClick={() => void copy()}>再复制一次</button>
            <span className="hint">{copied ? "已在剪贴板" : "尚未复制"}</span>
          </div>
          <div className="code-block" style={{ maxHeight: 460 }}>{bundle.text}</div>
        </div>

        <div className="card">
          <div className="card-title">
            ② 网页 AI 的回复
            <span className="muted"> · 自动读取，也可手动粘贴</span>
          </div>
          <textarea className="textarea" style={{ minHeight: 300 }}
            placeholder="把网页版 AI 的整块回复原样粘贴到这里（带客套话、Markdown 围栏也没关系，解析器会处理）"
            value={raw} onChange={(e) => setRaw(e.target.value)} />
          <div className="row" style={{ marginTop: 10 }}>
            <button className="btn btn-primary" disabled={busy || !raw.trim()} onClick={() => void parse()}>
              解析并预览
            </button>
            <span className="hint">解析失败会打开人工修正</span>
          </div>
        </div>
      </div>

      {parsed && (
        <div className="card">
          <div className="card-title">
            解析预览
            <span className={`badge ${parsed.stop ? "badge-clean" : "badge-dirty"}`} style={{ marginLeft: 8 }}>
              {parsed.stop ? "停下等你行动" : "继续写（不给选项）"}
            </span>
            {parsed.stop_trigger && <span className="muted" style={{ marginLeft: 8 }}>{parsed.stop_trigger}</span>}
          </div>
          {parsed.warnings.length > 0 && (
            <ul className="warn-list">{parsed.warnings.map((w, i) => <li key={i}>{w}</li>)}</ul>
          )}
          <div className="field">
            <span className="label">正文</span>
            <div className="passage-body" style={{ whiteSpace: "pre-wrap" }}>{parsed.narration || "（空）"}</div>
          </div>
          <div className="field">
            <span className="label">选项</span>
            <div className="code-block">{parsed.choices.join("\n") || "（已补默认项 / 可自由输入）"}</div>
          </div>
          <div className="grid-2">
            <div className="field">
              <span className="label">分析段 ANALYSIS（分层状态 / 角色 / 摘要）</span>
              <div className="code-block" style={{ maxHeight: 160 }}>{parsed.analysis_json || "（无，本次不更新分层状态卡）"}</div>
            </div>
            <div className="field">
              <span className="label">停顿决定 & 旧格式增量</span>
              <div className="code-block" style={{ maxHeight: 160 }}>
                {`STOP: ${parsed.stop ? "true" : "false"}${parsed.stop_reason ? ` · ${parsed.stop_reason}` : ""}`}{"\n"}
                {`STATE_DELTA: ${parsed.state_delta || "（无）"}`}{"\n"}
                {`SUMMARY_DELTA: ${parsed.summary_delta || "（无）"}`}
              </div>
            </div>
          </div>
          <div className="row">
            <button className="btn" onClick={() => setFixOpen(true)}>人工修正</button>
            <div className="grow" />
            <button className="btn btn-primary" disabled={busy || !parsed.narration.trim()}
              onClick={() => void apply(parsed)}>
              应用到剧本
            </button>
          </div>
        </div>
      )}

      {fixOpen && parsed && (
        <ParseFixDialog parsed={parsed} onCancel={() => setFixOpen(false)} onConfirm={(p) => void apply(p)} />
      )}
    </div>
  );
}
