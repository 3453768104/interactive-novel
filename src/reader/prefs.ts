import { DEFAULT_READING_PREFS, type ReadingPrefs } from "../types";

const KEY = "interactive-novel.reading-prefs.v1";

export function loadReadingPrefs(): ReadingPrefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...DEFAULT_READING_PREFS };
    const parsed = JSON.parse(raw) as Partial<ReadingPrefs>;
    return { ...DEFAULT_READING_PREFS, ...parsed };
  } catch {
    return { ...DEFAULT_READING_PREFS };
  }
}

export function saveReadingPrefs(prefs: ReadingPrefs) {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs));
  } catch {
    /* private mode / storage disabled: preferences simply don't persist */
  }
}

export const FONT_STACKS: Record<ReadingPrefs["fontFamily"], string> = {
  serif: '"Noto Serif SC", "Source Han Serif SC", "Songti SC", "SimSun", Georgia, serif',
  sans: '"Noto Sans SC", "Microsoft YaHei", "PingFang SC", system-ui, sans-serif',
  mono: '"Cascadia Mono", Consolas, "Noto Sans Mono CJK SC", monospace',
};

/** Internal parser keeps A./B./C. labels (anti-glue); readers see bare prose. */
export function stripChoiceLabel(label: string): string {
  return label.replace(/^[A-DＡ-Ｄa-d一二三四①②③④][\.、．:：]\s*/, "").trim();
}

/** Turn a JSON-ish state value into readable prose for the archive drawer. */
export function displayValue(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "string") return value;
  if (Array.isArray(value)) return value.map(displayValue).filter(Boolean).join("、");
  if (typeof value === "object") {
    return Object.entries(value as Record<string, unknown>)
      .map(([k, v]) => `${k}: ${displayValue(v)}`)
      .filter(Boolean)
      .join("；");
  }
  return String(value);
}

/** Character state arrives as a JSON string; show it as prose, not raw braces. */
export function readableState(raw: string): string {
  const t = (raw ?? "").trim();
  if (!t.startsWith("{")) return t;
  try {
    const v = JSON.parse(t) as Record<string, unknown>;
    const empty = Object.values(v).every((x) => displayValue(x).trim() === "");
    return empty ? "" : displayValue(v);
  } catch {
    return t;
  }
}

const FIELD_LABELS: Record<string, string> = {
  time: "时间",
  place: "地点",
  situation: "局势",
  threads: "未回收伏笔",
  factions: "势力",
  body: "身体状态",
  mind: "心思",
  items: "持有物",
  goal: "当前目标",
  mood: "情绪",
  abilities: "能力/功法",
  conditions: "持续状态",
  relations: "关系",
  characters: "人物",
  locations: "地点",
  open_threads: "伏笔",
  delta_log: "变更记录",
};

export function fieldLabel(key: string): string {
  return FIELD_LABELS[key] ?? key;
}

export function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
