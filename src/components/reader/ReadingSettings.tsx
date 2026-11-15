import { DEFAULT_READING_PREFS, type ReadingPrefs } from "../../types";

interface Props {
  prefs: ReadingPrefs;
  onChange: (p: ReadingPrefs) => void;
  onClose: () => void;
}

const THEMES: { key: ReadingPrefs["theme"]; label: string; hint: string }[] = [
  { key: "parchment", label: "羊皮纸", hint: "最接近读纸质小说，默认" },
  { key: "light", label: "纯白", hint: "干净极简" },
  { key: "dark", label: "深色", hint: "夜间阅读" },
];

const FONTS: { key: ReadingPrefs["fontFamily"]; label: string }[] = [
  { key: "serif", label: "宋体 / 衬线" },
  { key: "sans", label: "黑体 / 无衬线" },
  { key: "mono", label: "等宽" },
];

export default function ReadingSettings({ prefs, onChange, onClose }: Props) {
  const set = <K extends keyof ReadingPrefs>(key: K, value: ReadingPrefs[K]) =>
    onChange({ ...prefs, [key]: value });

  return (
    <div className="reading-settings" onClick={(e) => e.stopPropagation()}>
      <div className="rs-head">
        <strong>阅读设置</strong>
        <button className="btn btn-sm btn-ghost" onClick={onClose}>收起</button>
      </div>

      <div className="rs-row">
        <span className="rs-label">主题</span>
        <div className="rs-controls">
          {THEMES.map((t) => (
            <button key={t.key} title={t.hint}
              className={`btn btn-sm ${prefs.theme === t.key ? "btn-primary" : ""}`}
              onClick={() => set("theme", t.key)}>{t.label}</button>
          ))}
        </div>
      </div>

      <div className="rs-row">
        <span className="rs-label">字体</span>
        <div className="rs-controls">
          {FONTS.map((f) => (
            <button key={f.key}
              className={`btn btn-sm ${prefs.fontFamily === f.key ? "btn-primary" : ""}`}
              onClick={() => set("fontFamily", f.key)}>{f.label}</button>
          ))}
        </div>
      </div>

      <div className="rs-row">
        <span className="rs-label">字号 <b>{prefs.fontSize}px</b></span>
        <input type="range" min={14} max={26} step={1} value={prefs.fontSize}
          onChange={(e) => set("fontSize", Number(e.target.value))} />
      </div>
      <div className="rs-row">
        <span className="rs-label">行距 <b>{prefs.lineHeight.toFixed(2)}</b></span>
        <input type="range" min={1.5} max={2.6} step={0.05} value={prefs.lineHeight}
          onChange={(e) => set("lineHeight", Number(e.target.value))} />
      </div>
      <div className="rs-row">
        <span className="rs-label">页宽 <b>{prefs.pageWidth}px</b></span>
        <input type="range" min={540} max={960} step={20} value={prefs.pageWidth}
          onChange={(e) => set("pageWidth", Number(e.target.value))} />
      </div>

      <div className="rs-row rs-checks">
        <label className="check-row">
          <input type="checkbox" checked={prefs.indent}
            onChange={(e) => set("indent", e.target.checked)} />
          首行缩进
        </label>
        <label className="check-row">
          <input type="checkbox" checked={prefs.immersive}
            onChange={(e) => set("immersive", e.target.checked)} />
          一键沉浸（隐藏侧栏与工具栏）
        </label>
        <label className="check-row" title="主角视角下也显示〔暗场〕段落，含剧透">
          <input type="checkbox" checked={prefs.showHidden}
            onChange={(e) => set("showHidden", e.target.checked)} />
          显示暗场（作者视图）
        </label>
        <button className="btn btn-sm" onClick={() => onChange({ ...DEFAULT_READING_PREFS })}>
          恢复默认
        </button>
      </div>
      <div className="hint">设置只影响本机阅读界面，保存在浏览器本地，不改任何配置文件。</div>
    </div>
  );
}
