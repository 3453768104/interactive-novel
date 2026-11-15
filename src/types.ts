export interface World {
  id: string;
  name: string;
  description: string;
  created_at: string;
  updated_at: string;
}

export interface Story {
  id: string;
  world_id: string;
  name: string;
  engine_mode: EngineMode;
  created_at: string;
  updated_at: string;
}

export interface Protagonist {
  story_id: string;
  name: string;
  persona_md: string;
  opening_scene_md: string;
}

export interface StoryDetail {
  story: Story;
  protagonist: Protagonist;
}

export type EngineMode = "api" | "local" | "paste";

export interface LoreDoc {
  id: string;
  world_id: string;
  title: string;
  rel_path: string;
  sha256: string;
  chunk_count: number;
  origin: string;
  created_at: string;
}

export interface LoreDocContent {
  id: string;
  title: string;
  rel_path: string;
  content: string;
  sha256: string;
}

export interface Choice {
  id: string;
  passage_id: string;
  ordinal: number;
  label: string;
  action_md: string;
  is_free_input: boolean;
  selected_at: string | null;
}

export interface Passage {
  id: string;
  story_id: string;
  turn_index: number;
  trigger_action_md: string;
  content_md: string;
  raw_output: string;
  parse_status: string;
  engine_mode: string;
  bundle_snapshot: string;
  created_at: string;
  choices: Choice[];
  /** true = stop and offer protagonist actions; false = keep reading. */
  stop_decision: boolean;
  stop_reason: string;
  stop_trigger: string;
  pov: Pov;
  analysis_json: string;
}

export interface StateCard {
  story_id: string;
  revision: number;
  data_json: string;
  updated_at: string;
  updated_by: string;
}

export interface SummaryState {
  story_id: string;
  content_md: string;
  covered_through_turn: number;
  updated_at: string;
}

export interface ParsedOutput {
  narration: string;
  choices: string[];
  /** Legacy contract blocks, still accepted from pasted replies. */
  state_delta: string;
  summary_delta: string;
  /** Raw ANALYSIS block (JSON). Empty when the model omitted it. */
  analysis_json: string;
  /** true = stop and offer protagonist actions; false = keep writing. */
  stop: boolean;
  stop_reason: string;
  stop_trigger: string;
  warnings: string[];
  raw_output: string;
}

export interface BudgetItem {
  name: string;
  enabled: boolean;
  budget: number;
  est: number;
  trimmed: boolean;
}

export interface BudgetReport {
  items: BudgetItem[];
  total_est: number;
  cap: number;
  retrieval_enabled: boolean;
  warnings: string[];
}

export interface ContextBundle {
  text: string;
  report: BudgetReport;
  mode: EngineMode;
  /** Which system template built this prompt (shown in the inspector). */
  template?: string;
  pov?: Pov;
}

export interface GenerateResult {
  turn_id: string;
  turn_index: number;
  bundle: ContextBundle;
}

export interface AppInfo {
  data_dir: string;
  config_dir: string;
  worlds_dir: string;
  db_path: string;
  portable: boolean;
  migrated_from: string | null;
  needs_merge: string[];
  defaults_version: number;
  /** 配置里写了但不会被读取的键 */
  config_warnings: string[];
}

export interface GenDelta {
  turn_id: string;
  delta: string;
}

export interface GenDone {
  turn_id: string;
  raw_output: string;
  parsed: ParsedOutput;
  cancelled: boolean;
}

export interface GenError {
  turn_id: string;
  message: string;
}

export type View = "worlds" | "stories" | "reader" | "paste" | "settings" | "lore";

export interface Nav {
  worldId: string | null;
  storyId: string | null;
  mode: EngineMode;
  setWorldId: (id: string | null) => void;
  setStoryId: (id: string | null) => void;
  setMode: (m: EngineMode) => void;
  go: (v: View) => void;
  openPaste: (turnId: string, bundle: ContextBundle) => void;
  toast: (msg: string, kind?: "ok" | "error") => void;
}

// ---------- v1.5: lore library / timeline / event background ----------

export interface LoreCategory {
  id: string;
  world_id: string;
  parent_id: string | null;
  name: string;
  sort_order: number;
  card_count: number;
}

export interface LoreCard {
  doc_id: string;  world_id: string;
  title: string;
  rel_path: string;
  category_id: string | null;
  category_name: string | null;
  tags: string[];
  summary: string;
  char_count: number;
  chunk_count: number;
  sha256: string;
  updated_at: string;
}

/** Result of bulk-importing one Markdown setting book. */
export interface ImportReport {
  doc_title: string;
  categories_created: number;
  cards_created: number;
  cards_skipped: number;
  samples: string[];
}

export interface LoreCardDetail {
  card: LoreCard;
  content: string;
  timeline_event_ids: string[];
}

export interface LoreCardPatch {
  title: string;
  category_id: string | null;
  tags: string[];
  summary: string;
  timeline_event_ids: string[];
}

export interface TimelineEvent {
  id: string;
  world_id: string;
  era: string;
  year_label: string;
  title: string;
  body_md: string;
  ordinal: number;
  inject_into_context: boolean;
  created_at: string;
  updated_at: string;
}

export interface WorldEventBackground {
  content_md: string;
  updated_at: string;
}

export interface StoryEventBackground {
  enabled: boolean;
  content_md: string;
  inherited_content_md: string;
  updated_at: string;
}

/** Warning prefix emitted when several options were glued into one line. */
export const CHOICES_SUSPECT = "CHOICES_SUSPECT";

export const EMPTY_TIMELINE_EVENT: TimelineEvent = {
  id: "",
  world_id: "",
  era: "",
  year_label: "",
  title: "",
  body_md: "",
  ordinal: 0,
  inject_into_context: false,
  created_at: "",
  updated_at: "",
};

export const EMPTY_PARSED: ParsedOutput = {
  narration: "",
  choices: [],
  state_delta: "",
  summary_delta: "",
  analysis_json: "",
  stop: true,
  stop_reason: "",
  stop_trigger: "",
  warnings: [],
  raw_output: "",
};

// ---------- v2: POV, layered state, characters, structured policy ----------

export type Pov = "protagonist" | "omniscient";

export interface StorySettings {
  pov: Pov;
  show_scene_markers: boolean;
}

export interface Character {
  id: string;
  world_id: string;
  story_id: string | null;
  name: string;
  /** preset = from the lore library; emergent = nominated during play. */
  kind: "preset" | "emergent";
  importance: "passerby" | "notable";
  preset_card_id: string | null;
  current_state: string;
  first_turn: number | null;
  inject: boolean;
  created_at: string;
  updated_at: string;
}

export interface AppSettings {
  retrieval_enabled: boolean;
  event_background_soft_cap: number;
  persona_soft_cap: number;
  opening_scene_turns: number;
  stop_triggers: string[];
  max_consecutive_turns: number;
  analysis_second_pass: boolean;
  /** notable | none（路人永不注入） */
  character_inject: "notable" | "none";
  show_scene_markers: boolean;
}

export interface ModuleCfg {
  on: boolean;
  budget: number;
  n: number;
  top_n: number;
  trim: number;
}

export interface EstimatorCfg {
  strategy: string;
  zh_ratio: number;
  ascii_ratio: number;
  other_ratio: number;
  safety_margin: number;
}

export interface RetrievalCfg {
  bm25_topk: number;
  vec_topk: number;
  fusion: string;
  rrf_k: number;
}

export interface ThrottleCfg {
  summary_every_n: number;
  state_mode: string;
}

export interface Policy {
  mode: string;
  estimator: EstimatorCfg;
  context_window: number;
  max_output_tokens: number;
  modules: Record<string, ModuleCfg>;
  retrieval: RetrievalCfg;
  throttle: ThrottleCfg;
  templates: Record<string, string>;
  output_template: string;
}

export interface ReadingPrefs {
  theme: "parchment" | "light" | "dark";
  fontSize: number;
  lineHeight: number;
  pageWidth: number;
  fontFamily: "serif" | "sans" | "mono";
  indent: boolean;
  immersive: boolean;
  /** 作者视图：主角视角下也显示「〔暗场〕」段落（剧透）。 */
  showHidden: boolean;
}

export const DEFAULT_READING_PREFS: ReadingPrefs = {
  theme: "parchment",
  fontSize: 17,
  lineHeight: 1.95,
  pageWidth: 720,
  fontFamily: "serif",
  indent: true,
  immersive: false,
  showHidden: false,
};

export const STOP_TRIGGER_OPTIONS: { key: string; label: string; hint: string }[] = [
  { key: "dialogue", label: "对话开始前", hint: "对方等着主角回应时停下" },
  { key: "major_decision", label: "重大抉择", hint: "需要读者替主角做决定" },
  { key: "new_character", label: "新角色登场", hint: "出现值得留意的人物" },
  { key: "scene_change", label: "场景转换", hint: "地点或时间明显跳变" },
  { key: "word_limit", label: "写得够长", hint: "本段已充分展开" },
  { key: "every_n_turns", label: "连写多段", hint: "按最大连写段数强制停下" },
];

// ---------- v3: context inspector / director / saves / pinned lore ----------

export interface TurnPrompt {
  turn_id: string;
  /** narration = 正文阶段，analysis = 二次分析阶段 */
  kind: "narration" | "analysis" | string;
  text: string;
  est: number;
  meta_json: string;
  created_at: string;
}

/** What the next turn WOULD be sent, without calling the model. */
export interface PreviewBundle {
  text: string;
  report: BudgetReport;
  mode: EngineMode;
  template: string;
  pov: Pov;
}

export interface StoryDirector {
  story_id: string;
  enabled: boolean;
  outline_md: string;
  next_note: string;
  updated_at: string;
}

export interface StorySave {
  id: string;
  story_id: string;
  turn_index: number;
  kind: "auto" | "manual" | string;
  label: string;
  created_at: string;
}

export interface PinnedLore {
  doc_id: string;
  title: string;
  rel_path: string;
}

// ---------- v3: 导出为小说 ----------

export interface ExportOptions {
  /** 书名，留空用剧本名 */
  title: string;
  /** 完整文件路径，留空写到 data/exports/ */
  path: string;
  /** md | epub | docx */
  format: string;
  /** 作者版：把〔暗场〕也导出 */
  include_hidden: boolean;
  /** 每章大致字数，0 = 不分章 */
  chapter_chars: number;
  /** 每段前附上「主角行动」 */
  include_action: boolean;
}

export interface ExportResult {
  path: string;
  turns: number;
  chapters: number;
  chars: number;
  format: string;
}

// ---------- v5: 章节 / 搜索 / 状态历史 ----------

export interface Chapter {
  ordinal: number;
  index: number;
  title: string;
  start_turn: number;
  end_turn: number;
  chars: number;
  turns: number;
  lead: string;
}

export interface StoryHit {
  passage_id: string;
  turn_index: number;
  snippet: string;
  score: number;
}

export interface StateRevision {
  revision: number;
  source: string;
  created_at: string;
}

// ---------- v4: 检索索引状态 ----------

export interface RetrievalStatus {
  chunks: number;
  fts: number;
  vectors: number;
  model: string;
  endpoint: string;
  vec_enabled: boolean;
}

export interface EmbeddingTest {
  model: string;
  dim: number;
  ms: number;
}

export interface RetrievalRebuild {
  indexed: number;
  embedded: number;
  model: string;
}

// ---------- v4: engines.yaml 结构化读模型（文件仍是权威源） ----------

export interface SampleParams {
  temperature: number;
  max_tokens: number;
  top_p?: number | null;
}

export interface EngineProfile {
  base_url: string;
  api_key?: string | null;
  model: string;
  context_window?: number | null;
  params: SampleParams;
  timeout_ms: number;
  retries: number;
}

export interface PasteProfile {
  target_hint: string;
}

export interface EngineProfiles {
  api?: EngineProfile | null;
  local?: EngineProfile | null;
  paste?: PasteProfile | null;
}

export interface EmbeddingConfig {
  /** local | api | custom —— 决定 base_url / api_key 缺省时向谁借 */
  provider: string;
  base_url: string;
  api_key: string;
  model: string;
  fallback: string;
  /** 0 = 用模型默认维度 */
  dimensions: number;
}

export interface EnginesConfig {
  engines?: EngineProfiles | null;
  embedding?: EmbeddingConfig | null;
}

// ---------- v6: 设定 AI 辅助创作 ----------

export interface LoreAiRequest {
  world_id: string;
  title: string;
  brief: string;
  category_id: string | null;
  expand_doc_id: string | null;
  /** short | medium | long */
  length: string;
  /** canon | plain | epic */
  tone: string;
  /** local | api | paste */
  mode: string;
}

export interface LoreAiResult {
  prompt: string;
  text: string;
  mode: string;
}
