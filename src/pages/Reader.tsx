import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import ParseFixDialog from "../components/ParseFixDialog";
import ArchiveDrawer from "../components/reader/ArchiveDrawer";
import ChaptersDrawer from "../components/reader/ChaptersDrawer";
import SearchPanel from "../components/reader/SearchPanel";
import ContextInspector from "../components/reader/ContextInspector";
import DirectorPanel from "../components/reader/DirectorPanel";
import ExportDialog from "../components/reader/ExportDialog";
import Narration from "../components/reader/Narration";
import SavesPanel from "../components/reader/SavesPanel";
import ReadingSettings from "../components/reader/ReadingSettings";
import { api } from "../ipc";
import { FONT_STACKS, loadReadingPrefs, saveReadingPrefs, stripChoiceLabel } from "../reader/prefs";
import { streamingNarration } from "../reader/streamText";
import {
  CHOICES_SUSPECT,
  type AppSettings, type Character, type EngineMode, type GenDelta, type GenDone,
  type GenError, type Nav, type ParsedOutput, type Passage, type Pov,
  type ReadingPrefs, type StateCard, type StoryDetail, type StoryEventBackground,
  type StorySettings, type SummaryState,
} from "../types";

const MODE_LABEL: Record<EngineMode, string> = {
  api: "联网", local: "本地", paste: "粘贴",
};
const POV_LABEL: Record<Pov, string> = {
  protagonist: "主角视角", omniscient: "全知读者",
};
/** 「看小说」模式自动续写时发给模型的那句话。 */
const AUTO_CONTINUE = "（继续阅读，接着往下写，暂不让读者介入）";
const AUTO_KEY = (id: string) => `interactive-novel.auto.${id}`;

export default function ReaderPage({ nav }: { nav: Nav }) {
  const [detail, setDetail] = useState<StoryDetail | null>(null);
  const [passages, setPassages] = useState<Passage[]>([]);
  const [stateCard, setStateCard] = useState<StateCard | null>(null);
  const [summary, setSummary] = useState<SummaryState | null>(null);
  const [bg, setBg] = useState<StoryEventBackground | null>(null);
  const [storySettings, setStorySettings] = useState<StorySettings | null>(null);
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  const [characters, setCharacters] = useState<Character[]>([]);
  const [prefs, setPrefs] = useState<ReadingPrefs>(() => loadReadingPrefs());
  const [panel, setPanel] = useState<"archive" | "context" | "director" | "saves" | "chapters" | "search" | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  /** 0 = 关；-1 = 一直连写到手动停；n>0 = 还剩 n 回合 */
  const [autoLeft, setAutoLeft] = useState<number>(() => {
    if (!nav.storyId) return 0;
    const v = sessionStorage.getItem(AUTO_KEY(nav.storyId));
    return v ? Number(v) : 0;
  });
  const [autoMenu, setAutoMenu] = useState(false);
  const autoRef = useRef(0);
  const startRef = useRef<((action: string, auto?: boolean) => Promise<void>) | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [insertOpen, setInsertOpen] = useState(false);
  const [markName, setMarkName] = useState<string | null>(null);
  const [freeInput, setFreeInput] = useState("");
  const [draft, setDraft] = useState("");
  const [generating, setGenerating] = useState(false);
  const [fix, setFix] = useState<{ turnId: string; parsed: ParsedOutput } | null>(null);
  const activeTurn = useRef<string | null>(null);
  /** Turns already written to the DB — a duplicated event must not apply twice. */
  const appliedTurns = useRef(new Set<string>());
  /** Deltas can arrive before `generateStream` resolves with the turn id. */
  const earlyDeltas = useRef(new Map<string, string>());
  /** Token deltas batched to one render per animation frame. */
  const deltaBuffer = useRef("");
  const flushPending = useRef(false);
  const flowRef = useRef<HTMLElement | null>(null);
  const stickToBottom = useRef(true);
  /** Per-story scroll position, so leaving the reader doesn't lose your place. */
  const restoredScroll = useRef(false);
  const seenTurns = useRef(0);
  const lastScrollSave = useRef(0);

  const queueDelta = useCallback((delta: string) => {
    deltaBuffer.current += delta;
    if (flushPending.current) return;
    flushPending.current = true;
    requestAnimationFrame(() => {
      flushPending.current = false;
      const batch = deltaBuffer.current;
      deltaBuffer.current = "";
      if (batch) setDraft((d) => d + batch);
    });
  }, []);

  const reload = useCallback(async () => {
    if (!nav.storyId) return;
    try {
      const d = await api.getStory(nav.storyId);
      const [p, sc, su, background, ss, app] = await Promise.all([
        api.listPassages(nav.storyId),
        api.getStateCard(nav.storyId),
        api.getSummary(nav.storyId),
        api.getStoryEventBackground(nav.storyId),
        api.getStorySettings(nav.storyId),
        api.getAppSettings(),
      ]);
      let chars: Character[] = [];
      try { chars = await api.listCharacters(d.story.world_id, nav.storyId); } catch { /* ignore */ }
      setDetail(d);
      setPassages(p.filter((x) => x.parse_status !== "pending"));
      setStateCard(sc);
      setSummary(su);
      setBg(background);
      setStorySettings(ss);
      setAppSettings(app);
      setCharacters(chars);
      nav.setMode(d.story.engine_mode);
    } catch (e) { nav.toast(String(e), "error"); }
  }, [nav.storyId]);

  useEffect(() => { void reload(); }, [reload]);

  useEffect(() => {
    autoRef.current = autoLeft;
    if (!nav.storyId) return;
    try {
      if (autoLeft === 0) sessionStorage.removeItem(AUTO_KEY(nav.storyId));
      else sessionStorage.setItem(AUTO_KEY(nav.storyId), String(autoLeft));
    } catch { /* private mode */ }
  }, [autoLeft, nav.storyId]);

  const scrollKey = (id: string) => `interactive-novel.scroll.${id}`;

  // Restore the reading position once the passages are on screen. Without this
  // every re-entry dumped the reader back at the very first paragraph.
  useEffect(() => {
    if (!nav.storyId || passages.length === 0 || restoredScroll.current) return;
    restoredScroll.current = true;
    const el = flowRef.current;
    if (!el) return;
    const saved = Number(localStorage.getItem(scrollKey(nav.storyId)) ?? "");
    requestAnimationFrame(() => {
      el.scrollTop = Number.isFinite(saved) && saved > 0 ? saved : el.scrollHeight;
      stickToBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 180;
    });
    seenTurns.current = passages.length;
  }, [passages.length, nav.storyId]);

  // A newly written turn should be followed, but re-reading old text should not
  // be interrupted (that is what `stickToBottom` tracks).
  useEffect(() => {
    // Skip the initial mount (restore handles it) and only follow real growth.
    if (passages.length <= seenTurns.current) return;
    seenTurns.current = passages.length;
    if (!stickToBottom.current) return;
    const el = flowRef.current;
    if (!el) return;
    requestAnimationFrame(() => { el.scrollTop = el.scrollHeight; });
  }, [passages.length]);

  useEffect(() => () => {
    const el = flowRef.current;
    if (el && nav.storyId) {
      try { localStorage.setItem(scrollKey(nav.storyId), String(el.scrollTop)); } catch { /* ignore */ }
    }
  }, [nav.storyId]);

  useEffect(() => {
    saveReadingPrefs(prefs);
    document.body.classList.toggle("immersive-mode", prefs.immersive);
    return () => document.body.classList.remove("immersive-mode");
  }, [prefs]);

  useEffect(() => {
    // StrictMode mounts effects twice in dev. `listen` is async, so the first
    // cleanup can run before its unlisten resolves — without this flag the old
    // listener leaks and every `gen://done` fires twice.
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    const reg = (p: Promise<() => void>) =>
      void p.then((f) => { if (disposed) f(); else unlisteners.push(f); });

    reg(listen<GenDelta>("gen://delta", (e) => {
      const { turn_id, delta } = e.payload;
      if (!delta) return;
      if (turn_id === activeTurn.current) queueDelta(delta);
      else {
        if (earlyDeltas.current.size > 8) earlyDeltas.current.clear();
        earlyDeltas.current.set(turn_id, (earlyDeltas.current.get(turn_id) ?? "") + delta);
      }
    }));

    reg(listen<GenDone>("gen://done", async (e) => {
      if (e.payload.turn_id !== activeTurn.current) return;
      deltaBuffer.current = "";
      setGenerating(false);
      setDraft("");
      earlyDeltas.current.delete(e.payload.turn_id);
      if (e.payload.cancelled) {
        nav.toast("已停止，这一段没有写入");
        return;
      }
      let parsed = e.payload.parsed;
      const suspect = parsed.warnings.some((w) => w.startsWith(CHOICES_SUSPECT));
      if (!parsed.narration.trim() || suspect) {
        parsed = {
          ...parsed,
          narration: parsed.narration.trim() || (parsed.raw_output || e.payload.raw_output).trim(),
        };
        setFix({ turnId: e.payload.turn_id, parsed });
        return;
      }
      if (appliedTurns.current.has(e.payload.turn_id)) return;
      appliedTurns.current.add(e.payload.turn_id);
      try {
        await api.applyOutput(e.payload.turn_id, parsed);
        await reload();
        // 「看小说」：读者没在操作，把下一段也自动推下去。
        const left = autoRef.current;
        if (left !== 0) {
          if (left > 0) setAutoLeft(left - 1);
          window.setTimeout(() => { void startRef.current?.(AUTO_CONTINUE, true); }, 900);
        }
      } catch (err) {
        appliedTurns.current.delete(e.payload.turn_id);
        nav.toast(String(err), "error");
      }
    }));

    reg(listen<GenError>("gen://error", (e) => {
      if (e.payload.turn_id !== activeTurn.current) return;
      deltaBuffer.current = "";
      setGenerating(false);
      setDraft("");
      earlyDeltas.current.delete(e.payload.turn_id);
      nav.toast(`生成失败：${e.payload.message}`, "error");
    }));

    return () => {
      disposed = true;
      unlisteners.forEach((f) => f());
    };
  }, [reload, queueDelta]);

  const start = async (action: string, auto = false) => {
    if (!nav.storyId || generating || !action.trim()) return;
    setGenerating(true);
    deltaBuffer.current = "";
    setDraft("");
    setInsertOpen(false);
    setMarkName(null);
    try {
      const res = await api.generateStream(nav.storyId, action.trim(), auto);
      activeTurn.current = res.turn_id;
      stickToBottom.current = true;
      const early = earlyDeltas.current.get(res.turn_id);
      if (early) {
        setDraft(early);
        earlyDeltas.current.delete(res.turn_id);
      }
      if (res.bundle.mode === "paste") {
        setGenerating(false);
        nav.openPaste(res.turn_id, res.bundle);
        return;
      }
      setFreeInput("");
    } catch (e) {
      setGenerating(false);
      nav.toast(String(e), "error");
    }
  };

  useEffect(() => { startRef.current = start; });

  /** 粘贴模式来回切页会把组件卸载，回来时按 sessionStorage 续上。 */
  const autoAtMount = useRef(autoRef.current);
  const resumeDone = useRef(false);
  useEffect(() => {
    if (resumeDone.current) return;
    if (autoAtMount.current === 0) { resumeDone.current = true; return; }
    if (passages.length === 0) return;
    resumeDone.current = true;
    const t = window.setTimeout(() => { void startRef.current?.(AUTO_CONTINUE, true); }, 700);
    return () => window.clearTimeout(t);
  }, [passages.length]);

  const stopAuto = () => {
    setAutoLeft(0);
    if (generating && activeTurn.current) void api.cancelGenerate(activeTurn.current);
    nav.toast("已停止自动连写");
  };

  const changeMode = async (m: EngineMode) => {
    if (!nav.storyId) return;
    try {
      await api.setStoryMode(nav.storyId, m);
      nav.setMode(m);
      await reload();
      nav.toast(`已切换到「${MODE_LABEL[m]}」`);
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const changePov = async (pov: Pov) => {
    if (!nav.storyId) return;
    try {
      const ss = await api.saveStorySettings(
        nav.storyId, pov, storySettings?.show_scene_markers ?? true,
      );
      setStorySettings(ss);
      nav.toast(`叙事视角：${POV_LABEL[pov]}，下一回合起生效（停下时仍是主角行动）`);
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const reopen = async (turnIndex: number) => {
    if (!nav.storyId) return;
    if (!window.confirm(`从回合 ${turnIndex} 重开？该回合及之后的正文会被删除；设定文档不受影响。`)) return;
    try {
      await api.reopenTurn(nav.storyId, turnIndex);
      await reload();
      nav.toast("已重开（设定文档未改动）");
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const confirmFix = async (p: ParsedOutput) => {
    if (!fix) return;
    try {
      await api.applyOutput(fix.turnId, p);
      setFix(null);
      await reload();
      nav.toast("人工修正结果已写入");
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const markAs = async (importance: Character["importance"]) => {
    if (!markName || !detail || !nav.storyId) return;
    try {
      await api.upsertCharacter({
        world_id: detail.story.world_id,
        story_id: nav.storyId,
        name: markName,
        importance,
        current_state: "",
      });
      nav.toast(`「${markName}」已标为${importance === "notable" ? "重要角色（会注入上下文）" : "路人（不注入）"}`);
      setMarkName(null);
      await reload();
    } catch (e) { nav.toast(String(e), "error"); }
  };

  const setImportance = async (id: string, importance: Character["importance"]) => {
    try { await api.setCharacterImportance(id, importance); await reload(); }
    catch (e) { nav.toast(String(e), "error"); }
  };

  const removeCharacter = async (id: string) => {
    try { await api.deleteCharacter(id); await reload(); }
    catch (e) { nav.toast(String(e), "error"); }
  };

  const candidates = useMemo(() => {
    const names = characters
      .filter((c) => c.kind === "emergent" && c.importance === "passerby")
      .map((c) => c.name);
    const latest = passages[passages.length - 1];
    if (latest?.analysis_json) {
      try {
        const a = JSON.parse(latest.analysis_json) as { new_character_candidates?: string[] };
        if (Array.isArray(a.new_character_candidates)) names.push(...a.new_character_candidates);
      } catch { /* tolerant */ }
    }
    return [...new Set(names.map((n) => n.trim()).filter(Boolean))].slice(0, 10);
  }, [characters, passages]);

  /** Only the prose that has streamed in — never the markers or the JSON blocks. */
  const streamText = useMemo(() => streamingNarration(draft), [draft]);

  // Follow the writing as it grows, but never yank the page away from a reader
  // who scrolled up to re-read something.
  useEffect(() => {
    if (!generating) return;
    const el = flowRef.current;
    if (!el || !stickToBottom.current) return;
    el.scrollTop = el.scrollHeight;
  }, [streamText, generating]);

  const latest = passages[passages.length - 1];
  const choices = latest?.choices ?? [];
  // 自动连写时模型可能既不给选项又被护栏强制停下——那就还是"接着读"。
  const keepWriting = latest != null && (!latest.stop_decision || latest.choices.length === 0);
  const pov: Pov = storySettings?.pov ?? "protagonist";
  const showMarkers = appSettings?.show_scene_markers ?? true;

  /** 跳到某一回合（目录 / 搜索结果用）。 */
  const jumpToTurn = useCallback((turn: number) => {
    const el = document.querySelector(`[data-turn="${turn}"]`);
    el?.scrollIntoView({ block: "center", behavior: "smooth" });
    stickToBottom.current = false;
    setPanel(null);
  }, []);

  // 快捷键：读小说时手不用离开键盘。输入框里不拦截。
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      const typing = t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        setPanel("search");
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z" && !typing) {
        e.preventDefault();
        if (nav.storyId) {
          void api.undoState(nav.storyId)
            .then(() => { nav.toast("已撤销上一步状态修改"); void reload(); })
            .catch((err) => nav.toast(String(err), "error"));
        }
        return;
      }
      if (typing) return;
      if (e.key === "Escape") {
        if (generating && activeTurn.current) void api.cancelGenerate(activeTurn.current);
        setPanel(null);
        return;
      }
      if (generating) return;
      if (e.key === " " || e.key === "ArrowRight") {
        if (keepWriting) { e.preventDefault(); void start(AUTO_CONTINUE); }
        return;
      }
      if (["1", "2", "3", "4"].includes(e.key)) {
        const c = choices[Number(e.key) - 1];
        if (c) { e.preventDefault(); void start(stripChoiceLabel(c.label)); }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [generating, keepWriting, choices, nav, reload, start]);

  if (!nav.storyId) {
    return <div className="page"><div className="empty">请先在「剧本」里打开一本剧本。</div></div>;
  }

  const novelStyle = {
    "--novel-font-size": `${prefs.fontSize}px`,
    "--novel-line-height": String(prefs.lineHeight),
    "--novel-page-width": `${prefs.pageWidth}px`,
    "--novel-font": FONT_STACKS[prefs.fontFamily],
  } as CSSProperties;

  return (
    <div className={`page reader-page ${prefs.immersive ? "is-immersive" : ""}`}
      data-theme={prefs.theme} style={novelStyle}>
      <div className="reader-toolbar">
        <div className="reader-heading">
          <span className="reader-title">{detail?.story.name ?? "阅读"}</span>
          <span className="reader-sub">
            {detail?.protagonist.name || "（未命名主角）"} · {passages.length} 回合
            {bg?.enabled && <span className="badge">本剧本背景覆盖</span>}
          </span>
        </div>
        <div className="row reader-tools">
          <div className="segmented">
            {(["protagonist", "omniscient"] as Pov[]).map((p) => (
              <button key={p} className={pov === p ? "active" : ""}
                onClick={() => void changePov(p)} title={p === "omniscient" ? "可写配角与幕后，选项仍是主角行动" : "只写主角能感知的事物"}>
                {POV_LABEL[p]}
              </button>
            ))}
          </div>
          <div className="segmented">
            {(["paste", "local", "api"] as EngineMode[]).map((m) => (
              <button key={m} className={detail?.story.engine_mode === m ? "active" : ""}
                onClick={() => void changeMode(m)}>{MODE_LABEL[m]}</button>
            ))}
          </div>
          {([
            ["archive", "档案"],
            ["chapters", "目录"],
            ["search", "搜索"],
            ["context", "上下文"],
            ["director", "导演"],
            ["saves", "存档"],
          ] as const).map(([key, label]) => (
            <button key={key} className={`btn btn-sm ${panel === key ? "btn-primary" : ""}`}
              onClick={() => setPanel((cur) => (cur === key ? null : key))}>{label}</button>
          ))}
          <div className="auto-read">
            {autoLeft === 0 ? (
              <button className="btn btn-sm" onClick={() => setAutoMenu((v) => !v)}>看小说</button>
            ) : (
              <button className="btn btn-sm btn-primary" onClick={stopAuto}
                title="停止自动连写">
                自动中 {autoLeft < 0 ? "∞" : autoLeft} · 停
              </button>
            )}
            {autoMenu && autoLeft === 0 && (
              <div className="auto-menu" onMouseLeave={() => setAutoMenu(false)}>
                <div className="hint">不打断地往下写，你只管读：</div>
                {[3, 5, 10].map((n) => (
                  <button key={n} className="btn btn-sm"
                    onClick={() => { setAutoLeft(n); setAutoMenu(false); nav.toast(`自动连写 ${n} 回合，随时可停`); }}>
                    {n} 回合
                  </button>
                ))}
                <button className="btn btn-sm"
                  onClick={() => { setAutoLeft(-1); setAutoMenu(false); nav.toast("自动连写直到你点停止"); }}>
                  一直写下去
                </button>
                <div className="hint">这些回合里模型不会给选项，主角的行动由它按人设代劳。</div>
              </div>
            )}
          </div>
          <button className="btn btn-sm" onClick={() => setExportOpen(true)}>导出</button>
          <button className={`btn btn-sm ${settingsOpen ? "btn-primary" : ""}`}
            onClick={() => setSettingsOpen((v) => !v)}>阅读设置</button>
          <button className="btn btn-sm btn-ghost" onClick={() => void reload()}>刷新</button>
          {generating && (
            <button className="btn btn-sm btn-ghost"
              onClick={() => activeTurn.current && void api.cancelGenerate(activeTurn.current)}>停止</button>
          )}
        </div>
      </div>

      {settingsOpen && (
        <ReadingSettings prefs={prefs} onChange={setPrefs} onClose={() => setSettingsOpen(false)} />
      )}

      <div className="novel-shell">
        <article className={`novel-flow ${prefs.indent ? "indent" : ""}`}
          ref={(el) => { flowRef.current = el; }}
          onScroll={(e) => {
            const el = e.currentTarget;
            stickToBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 180;
            const now = Date.now();
            if (nav.storyId && now - lastScrollSave.current > 400) {
              lastScrollSave.current = now;
              try { localStorage.setItem(scrollKey(nav.storyId), String(el.scrollTop)); } catch { /* ignore */ }
            }
          }}>
          {passages.length === 0 && !generating && (
            <Narration text={detail?.protagonist.opening_scene_md || "（还没写开场情境，可以直接开始）"}
              pov={pov} showSceneMarkers={showMarkers} candidates={candidates}
              onMarkCharacter={setMarkName} />
          )}

          {passages.map((p) => (
            <section className="novel-turn" key={p.id} data-turn={p.turn_index}>
              <div className="turn-mark">
                <span className="turn-dot" />
                <div className="turn-popover">
                  <span>回合 {p.turn_index} · {MODE_LABEL[p.engine_mode as EngineMode] ?? p.engine_mode}</span>
                  {p.parse_status !== "ok" && <span className="badge badge-dirty">{p.parse_status}</span>}
                  <button className="btn btn-sm btn-ghost" onClick={() => void reopen(p.turn_index)}>从这里重开</button>
                  {p.trigger_action_md && (
                    <div className="turn-action">· {stripChoiceLabel(p.trigger_action_md)} ·</div>
                  )}
                </div>
              </div>
              <Narration text={p.content_md} pov={pov}
                showSceneMarkers={showMarkers} candidates={candidates}
                showHidden={prefs.showHidden}
                onRevealHidden={() => {
                  setPrefs((cur) => ({ ...cur, showHidden: true }));
                  nav.toast("已打开作者视图，可在阅读设置里关闭");
                }}
                onMarkCharacter={setMarkName} />
            </section>
          ))}

          {generating && (
            <section className="novel-turn">
              <div className="generating-hint"><span className="spinner" /> 正在写…</div>
              <Narration text={streamText} pov={pov} showSceneMarkers={showMarkers}
                candidates={[]} onMarkCharacter={() => {}} showHidden={prefs.showHidden} draft />
            </section>
          )}

          {markName && (
            <div className="mark-bar">
              <span>把「<b>{markName}</b>」记为：</span>
              {(["passerby", "notable"] as Character["importance"][]).map((i) => (
                <button key={i} className="btn btn-sm" onClick={() => void markAs(i)}>
                  {i === "passerby" ? "路人（不进上下文）" : "重要（每回合注入）"}
                </button>
              ))}
              <button className="btn btn-sm btn-ghost" onClick={() => setMarkName(null)}>取消</button>
            </div>
          )}

          {!generating && (passages.length > 0 || detail) && (
            <div className="novel-actions">
              {keepWriting ? (
                <div className="row">
                  <button className="continue-link" onClick={() => void start("（继续阅读，接着往下写，暂不让读者介入）")}>
                    继续阅读 →
                  </button>
                  <span className="hint">模型判断这里还不到你做决定的时候</span>
                  <button className="quiet-link" onClick={() => setInsertOpen((v) => !v)}>插入行动</button>
                </div>
              ) : (
                <>
                  {choices.length > 0 && (
                    <div className="choice-lines">
                      {choices.map((c) => (
                        <button key={c.id} className="choice-line"
                          onClick={() => void start(stripChoiceLabel(c.label))}>
                          {stripChoiceLabel(c.label)}
                        </button>
                      ))}
                    </div>
                  )}
                  <button className="quiet-link" onClick={() => setInsertOpen((v) => !v)}>
                    都不合意？自己写一句…
                  </button>
                </>
              )}
              {insertOpen && (
                <div className="free-line">
                  <input className="input" autoFocus value={freeInput}
                    placeholder="写主角要做的事、说的话或心里冒出的念头…"
                    onChange={(e) => setFreeInput(e.target.value)}
                    onKeyDown={(e) => { if (e.key === "Enter") void start(freeInput); }} />
                  <button className="btn btn-sm btn-primary" disabled={!freeInput.trim()}
                    onClick={() => void start(freeInput)}>写入</button>
                </div>
              )}
            </div>
          )}
        </article>

        {panel === "chapters" && (
          <ChaptersDrawer storyId={nav.storyId} totalTurns={passages.length}
            onJump={jumpToTurn} onClose={() => setPanel(null)}
            onToast={nav.toast} onChanged={() => void reload()} />
        )}
        {panel === "search" && (
          <SearchPanel storyId={nav.storyId} onJump={jumpToTurn}
            onClose={() => setPanel(null)} onToast={nav.toast} />
        )}
        {panel === "context" && (
          <ContextInspector
            storyId={nav.storyId}
            turnId={latest?.id ?? null}
            turnLabel={latest ? `回合 ${latest.turn_index}` : "尚无回合"}
            onClose={() => setPanel(null)}
            onToast={nav.toast} />
        )}
        {panel === "director" && (
          <DirectorPanel storyId={nav.storyId} onClose={() => setPanel(null)}
            onToast={nav.toast} onSaved={() => void reload()} />
        )}
        {panel === "saves" && (
          <SavesPanel storyId={nav.storyId} onClose={() => setPanel(null)}
            onToast={nav.toast} onRestored={() => void reload()} />
        )}
        {panel === "archive" && (
          <ArchiveDrawer
            stateJson={stateCard?.data_json ?? "{}"}
            summary={summary?.content_md ?? ""}
            persona={detail?.protagonist ?? null}
            characters={characters}
            revision={stateCard?.revision ?? 0}
            onSetImportance={(id, imp) => void setImportance(id, imp)}
            onDeleteCharacter={(id) => void removeCharacter(id)}
            onSaveState={async (json) => {
              try {
                await api.saveStateCard(nav.storyId!, json);
                await reload();
                nav.toast("状态卡已保存（下一回合生效）");
              } catch (e) { nav.toast(String(e), "error"); }
            }}
            onUndoState={async () => {
              try {
                await api.undoState(nav.storyId!);
                await reload();
                nav.toast("已撤销上一步状态修改");
              } catch (e) { nav.toast(String(e), "error"); }
            }}
            onClose={() => setPanel(null)}
          />
        )}
      </div>

      {fix && (
        <ParseFixDialog parsed={fix.parsed} onCancel={() => setFix(null)}
          onConfirm={(p) => void confirmFix(p)} />
      )}

      {exportOpen && (
        <ExportDialog storyId={nav.storyId} defaultTitle={detail?.story.name ?? "小说"}
          onClose={() => setExportOpen(false)} onToast={nav.toast} />
      )}
    </div>
  );
}
