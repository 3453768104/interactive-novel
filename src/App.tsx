import { useCallback, useEffect, useState } from "react";
import { api } from "./ipc";
import LoreLibrary from "./pages/LoreLibrary";
import StoriesPage from "./pages/Stories";
import WorldsPage from "./pages/Worlds";
import ReaderPage from "./pages/Reader";
import PasteWorkspace from "./pages/PasteWorkspace";
import SettingsPage from "./pages/Settings";
import type { AppInfo, ContextBundle, EngineMode, Nav, View } from "./types";

const NAV_ITEMS: { key: View; label: string; hint: string }[] = [
  { key: "worlds", label: "世界观", hint: "创建世界观、编辑设定文档" },
  { key: "stories", label: "剧本", hint: "每本一个主角" },
  { key: "lore", label: "设定集", hint: "分类树、卡片、时间线、事件背景" },
  { key: "reader", label: "阅读", hint: "小说式连续正文 · 双视角 · 档案抽屉" },
  { key: "paste", label: "粘贴工作区", hint: "网页端 AI 往返" },
  { key: "settings", label: "设置", hint: "策略面板、连接、源码与预算预览" },
];

export default function App() {
  const [view, setView] = useState<View>("worlds");
  const [worldId, setWorldId] = useState<string | null>(null);
  const [storyId, setStoryId] = useState<string | null>(null);
  const [mode, setMode] = useState<EngineMode>("paste");
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [toastState, setToastState] = useState<{ msg: string; kind: "ok" | "error" } | null>(null);
  const [pasteTurn, setPasteTurn] = useState<{ turnId: string; bundle: ContextBundle } | null>(null);

  useEffect(() => {
    api.getAppInfo().then(setInfo).catch((e) => setToastState({ msg: String(e), kind: "error" }));
  }, []);

  // Stable identity: panels put `nav.toast` in useCallback/useEffect deps, and a
  // fresh function on every render would make their loaders loop forever.
  const toast = useCallback((msg: string, kind: "ok" | "error" = "ok") => {
    setToastState({ msg, kind });
    window.setTimeout(() => setToastState(null), 4200);
  }, []);

  const go = (v: View) => setView(v);
  const openPaste = (turnId: string, bundle: ContextBundle) => {
    setPasteTurn({ turnId, bundle });
    setView("paste");
  };

  const nav: Nav = {
    worldId,
    storyId,
    mode,
    setWorldId,
    setStoryId,
    setMode,
    go,
    openPaste,
    toast,
  };

  const disabled = (v: View) =>
    (v === "stories" && !worldId) ||
    (v === "lore" && !worldId) ||
    (v === "reader" && !storyId) ||
    (v === "paste" && !pasteTurn);

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          互动小说
          <small>无大纲 · 你就是主角的大脑</small>
        </div>
        {NAV_ITEMS.map((item) => (
          <button
            key={item.key}
            className={`nav-item ${view === item.key ? "active" : ""}`}
            onClick={() => !disabled(item.key) && setView(item.key)}
            title={item.hint}
            disabled={disabled(item.key)}
          >
            {item.label}
          </button>
        ))}
        <div className="sidebar-spacer" />
        <div className="sidebar-foot">
          <div>当前模式：<span className={`badge badge-mode`}>{mode}</span></div>
          {info && (
            <div style={{ marginTop: 8 }}>
              数据目录<br />
              <code>{info.data_dir}</code>
            </div>
          )}
        </div>
      </aside>

      <main className="main">
        {view === "worlds" && <WorldsPage nav={nav} />}
        {view === "stories" && <StoriesPage nav={nav} />}
        {view === "lore" && <LoreLibrary nav={nav} />}
        {view === "reader" && <ReaderPage nav={nav} />}
        {view === "paste" && pasteTurn && (
          <PasteWorkspace nav={nav} turnId={pasteTurn.turnId} bundle={pasteTurn.bundle} />
        )}
        {view === "settings" && <SettingsPage nav={nav} />}
      </main>

      {toastState && <div className={`toast ${toastState.kind}`}>{toastState.msg}</div>}
    </div>
  );
}
