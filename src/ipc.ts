import { invoke } from "@tauri-apps/api/core";
import type {
  AppInfo,
  AppSettings,
  Character,
  ContextBundle,
  GenerateResult,
  Policy,
  Pov,
  StorySettings,
  LoreCard,
  LoreCardDetail,
  LoreCardPatch,
  LoreCategory,
  LoreDoc,
  LoreDocContent,
  ImportReport,
  ParsedOutput,
  StoryEventBackground,
  TurnPrompt,
  PreviewBundle,
  ExportOptions,
  ExportResult,
  RetrievalStatus,
  RetrievalRebuild,
  EmbeddingTest,
  LoreAiRequest,
  LoreAiResult,
  EnginesConfig,
  Chapter,
  StoryHit,
  StateRevision,
  StoryDirector,
  StorySave,
  PinnedLore,
  TimelineEvent,
  WorldEventBackground,
  Passage,
  StateCard,
  Story,
  StoryDetail,
  SummaryState,
  World,
} from "./types";

// All commands use #[tauri::command(rename_all = "snake_case")],
// so JS argument names are snake_case too.
export const api = {
  getAppInfo: () => invoke<AppInfo>("get_app_info"),

  // ---- clipboard (paste mode round-trip) ----
  clipboardPeek: () => invoke<string>("clipboard_peek"),
  writeClipboard: (text: string) => invoke<void>("clipboard_write_text", { text }),

  listWorlds: () => invoke<World[]>("list_worlds"),
  createWorld: (name: string, description: string) =>
    invoke<World>("create_world", { name, description }),
  deleteWorld: (id: string) => invoke<void>("delete_world", { id }),

  listStories: (world_id: string) => invoke<Story[]>("list_stories", { world_id }),
  createStory: (args: {
    world_id: string;
    name: string;
    protagonist_name: string;
    persona_md: string;
    opening_scene_md: string;
    engine_mode: string;
  }) => invoke<Story>("create_story", args),
  getStory: (id: string) => invoke<StoryDetail>("get_story", { id }),
  setStoryMode: (id: string, engine_mode: string) =>
    invoke<void>("set_story_mode", { id, engine_mode }),
  deleteStory: (id: string) => invoke<void>("delete_story", { id }),

  listLoreDocs: (world_id: string) => invoke<LoreDoc[]>("list_lore_docs", { world_id }),
  readLoreDoc: (id: string) => invoke<LoreDocContent>("read_lore_doc", { id }),
  saveLoreDoc: (id: string, content: string, base_sha256: string, force: boolean) =>
    invoke<LoreDocContent>("save_lore_doc", { id, content, base_sha256, force }),
  createLoreDoc: (world_id: string, title: string) =>
    invoke<LoreDoc>("create_lore_doc", { world_id, title }),
  deleteLoreDoc: (id: string) => invoke<void>("delete_lore_doc", { id }),
  openLoreExternal: (id: string) => invoke<void>("open_lore_external", { id }),

  listPassages: (story_id: string) => invoke<Passage[]>("list_passages", { story_id }),
  getStateCard: (story_id: string) => invoke<StateCard>("get_state_card", { story_id }),
  saveStateCard: (story_id: string, data_json: string) =>
    invoke<StateCard>("save_state_card", { story_id, data_json }),
  getSummary: (story_id: string) => invoke<SummaryState>("get_summary", { story_id }),
  reopenTurn: (story_id: string, turn_index: number) =>
    invoke<void>("reopen_turn", { story_id, turn_index }),

  buildBundle: (story_id: string, action_md: string) =>
    invoke<ContextBundle>("build_bundle", { story_id, action_md }),
  generateStream: (story_id: string, action_md: string, auto_continue = false) =>
    invoke<GenerateResult>("generate_stream", { story_id, action_md, auto_continue }),
  cancelGenerate: (turn_id: string) => invoke<void>("cancel_generate", { turn_id }),
  submitPastedOutput: (turn_id: string, raw_output: string) =>
    invoke<ParsedOutput>("submit_pasted_output", { turn_id, raw_output }),
  applyOutput: (turn_id: string, parsed: ParsedOutput) =>
    invoke<Passage>("apply_output", { turn_id, parsed }),

  readConfigFile: (rel_path: string) =>
    invoke<string>("read_config_file", { rel_path }),
  writeConfigFile: (rel_path: string, content: string) =>
    invoke<void>("write_config_file", { rel_path, content }),
  testConnection: (mode: string) => invoke<string>("test_connection", { mode }),

  // ---- v1.5: config / event background / lore library / timeline ----
  resetConfigToDefaults: () => invoke<string[]>("reset_config_to_defaults"),

  getWorldEventBackground: (world_id: string) =>
    invoke<WorldEventBackground>("get_world_event_background", { world_id }),
  saveWorldEventBackground: (world_id: string, content_md: string) =>
    invoke<WorldEventBackground>("save_world_event_background", { world_id, content_md }),
  getStoryEventBackground: (story_id: string) =>
    invoke<StoryEventBackground>("get_story_event_background", { story_id }),
  saveStoryEventBackground: (story_id: string, enabled: boolean, content_md: string) =>
    invoke<void>("save_story_event_background", { story_id, enabled, content_md }),

  listLoreCategories: (world_id: string) =>
    invoke<LoreCategory[]>("list_lore_categories", { world_id }),
  createLoreCategory: (world_id: string, name: string, parent_id: string | null) =>
    invoke<LoreCategory>("create_lore_category", { world_id, name, parent_id }),
  renameLoreCategory: (id: string, name: string) =>
    invoke<void>("rename_lore_category", { id, name }),
  deleteLoreCategory: (id: string) => invoke<void>("delete_lore_category", { id }),

  listLoreCards: (world_id: string) => invoke<LoreCard[]>("list_lore_cards", { world_id }),
  getLoreCard: (doc_id: string) => invoke<LoreCardDetail>("get_lore_card", { doc_id }),
  updateLoreCard: (doc_id: string, patch: LoreCardPatch) =>
    invoke<LoreCard>("update_lore_card", { doc_id, patch }),
  splitLoreDocByHeadings: (doc_id: string, category_id: string | null) =>
    invoke<LoreCard[]>("split_lore_doc_by_headings", { doc_id, category_id }),
  importLoreMarkdown: (world_id: string, path: string) =>
    invoke<ImportReport>("import_lore_markdown", { world_id, path }),

  loreAiGenerate: (req: LoreAiRequest) => invoke<LoreAiResult>("lore_ai_generate", { req }),
  createLoreCard: (world_id: string, category_id: string | null, title: string, content: string) =>
    invoke<string>("create_lore_card", { world_id, category_id, title, content }),

  listTimelineEvents: (world_id: string) =>
    invoke<TimelineEvent[]>("list_timeline_events", { world_id }),
  upsertTimelineEvent: (world_id: string, event: TimelineEvent) =>
    invoke<TimelineEvent>("upsert_timeline_event", { world_id, event }),
  deleteTimelineEvent: (id: string) => invoke<void>("delete_timeline_event", { id }),
  setCardTimeline: (card_id: string, event_ids: string[]) =>
    invoke<void>("set_card_timeline", { card_id, event_ids }),

  // ---- v2: POV / characters / structured config ----
  getStorySettings: (story_id: string) =>
    invoke<StorySettings>("get_story_settings", { story_id }),
  saveStorySettings: (story_id: string, pov: Pov, show_scene_markers: boolean) =>
    invoke<StorySettings>("save_story_settings", { story_id, pov, show_scene_markers }),
  getAppSettings: () => invoke<AppSettings>("get_app_settings"),
  getEngines: () => invoke<EnginesConfig>("get_engines"),
  getPolicy: (mode: string) => invoke<Policy>("get_policy", { mode }),
  listCharacters: (world_id: string, story_id: string) =>
    invoke<Character[]>("list_characters", { world_id, story_id }),
  upsertCharacter: (args: {
    world_id: string;
    story_id: string;
    name: string;
    importance: string;
    current_state: string;
  }) => invoke<Character>("upsert_character", args),
  setCharacterImportance: (id: string, importance: string) =>
    invoke<Character>("set_character_importance", { id, importance }),
  deleteCharacter: (id: string) => invoke<void>("delete_character", { id }),

  // ---- v3: context inspector / director / saves / pinned lore ----
  listTurnPrompts: (turn_id: string) =>
    invoke<TurnPrompt[]>("list_turn_prompts", { turn_id }),
  previewBundle: (story_id: string, action_md: string) =>
    invoke<PreviewBundle>("preview_bundle", { story_id, action_md }),

  getDirector: (story_id: string) => invoke<StoryDirector>("get_director", { story_id }),
  saveDirector: (args: {
    story_id: string;
    enabled: boolean;
    outline_md: string;
    next_note: string;
  }) => invoke<StoryDirector>("save_director", args),

  listSaves: (story_id: string) => invoke<StorySave[]>("list_saves", { story_id }),
  createSave: (story_id: string, label: string) =>
    invoke<StorySave>("create_save", { story_id, label }),
  restoreSave: (save_id: string) => invoke<void>("restore_save", { save_id }),
  deleteSave: (save_id: string) => invoke<void>("delete_save", { save_id }),

  listLorePins: (story_id: string) => invoke<PinnedLore[]>("list_lore_pins", { story_id }),
  setLorePin: (story_id: string, doc_id: string, pinned: boolean) =>
    invoke<void>("set_lore_pin", { story_id, doc_id, pinned }),

  retrievalStatus: (world_id: string) =>
    invoke<RetrievalStatus>("retrieval_status", { world_id }),
  testEmbedding: () => invoke<EmbeddingTest>("test_embedding"),
  rebuildRetrievalIndex: (world_id: string) =>
    invoke<RetrievalRebuild>("rebuild_retrieval_index", { world_id }),

  // ---- v5: 原生对话框 / 章节 / 搜索 / 状态历史 / 诊断 ----
  pickFile: (title?: string) => invoke<string | null>("pick_file", { title }),
  pickSavePath: (default_name?: string, title?: string) =>
    invoke<string | null>("pick_save_path", { default_name, title }),
  pickFolder: (title?: string) => invoke<string | null>("pick_folder", { title }),

  listChapters: (story_id: string, target_chars?: number) =>
    invoke<Chapter[]>("list_chapters", { story_id, target_chars }),
  renameChapter: (story_id: string, ordinal: number, title: string) =>
    invoke<void>("rename_chapter", { story_id, ordinal, title }),
  searchStory: (story_id: string, query: string, limit?: number) =>
    invoke<StoryHit[]>("search_story", { story_id, query, limit }),

  listStateHistory: (story_id: string, limit?: number) =>
    invoke<StateRevision[]>("list_state_history", { story_id, limit }),
  undoState: (story_id: string) => invoke<StateCard>("undo_state", { story_id }),

  logEvent: (kind: string, data?: unknown) => invoke<void>("log_event", { kind, data }),
  logTail: (lines?: number) => invoke<string[]>("log_tail", { lines }),
  exportDiagnostics: () => invoke<string>("export_diagnostics"),

  exportNovel: (story_id: string, options: ExportOptions) =>
    invoke<ExportResult>("export_novel", { story_id, options }),
  openInShell: (path: string) => invoke<void>("open_in_shell", { path }),
};

export const CONFLICT_PREFIX = "LORE_CONFLICT:";
