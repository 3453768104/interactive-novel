use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub mirror_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    pub id: String,
    pub world_id: String,
    pub name: String,
    pub engine_mode: String,
    pub created_at: String,
    pub updated_at: String,
    pub mirror_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Protagonist {
    pub story_id: String,
    pub name: String,
    pub persona_md: String,
    pub opening_scene_md: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryDetail {
    pub story: Story,
    pub protagonist: Protagonist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoreDoc {
    pub id: String,
    pub world_id: String,
    pub title: String,
    pub rel_path: String,
    pub sha256: String,
    pub chunk_count: i64,
    pub origin: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoreDocContent {
    pub id: String,
    pub title: String,
    pub rel_path: String,
    pub content: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub id: String,
    pub passage_id: String,
    pub ordinal: i64,
    pub label: String,
    pub action_md: String,
    pub is_free_input: bool,
    pub selected_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passage {
    pub id: String,
    pub story_id: String,
    pub turn_index: i64,
    pub trigger_action_md: String,
    pub content_md: String,
    pub raw_output: String,
    pub parse_status: String,
    pub engine_mode: String,
    pub bundle_snapshot: String,
    pub created_at: String,
    pub choices: Vec<Choice>,
    /// v2: side-car metadata (passage_meta). true = wait for the reader's action.
    #[serde(default = "default_true")]
    pub stop_decision: bool,
    #[serde(default)]
    pub stop_reason: String,
    #[serde(default)]
    pub stop_trigger: String,
    #[serde(default)]
    pub pov: String,
    #[serde(default)]
    pub analysis_json: String,
}

fn default_true() -> bool { true }
fn default_pov() -> String { "protagonist".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCard {
    pub story_id: String,
    pub revision: i64,
    pub data_json: String,
    pub updated_at: String,
    pub updated_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryState {
    pub story_id: String,
    pub content_md: String,
    pub covered_through_turn: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub data_dir: String,
    pub config_dir: String,
    pub worlds_dir: String,
    pub db_path: String,
    pub portable: bool,
    pub migrated_from: Option<String>,
    pub needs_merge: Vec<String>,
    pub defaults_version: i64,
    /// 配置里写了但不会被读取的键
    #[serde(default)]
    pub config_warnings: Vec<String>,
}

// ---------- Parsed model output ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ParsedOutput {
    pub narration: String,
    pub choices: Vec<String>,
    /// Legacy contract fields. Kept so old pasted replies and the manual-fix
    /// dialog keep working; the ANALYSIS block supersedes them when present.
    #[serde(default)]
    pub state_delta: String,
    #[serde(default)]
    pub summary_delta: String,
    /// Raw `<<<ANALYSIS>>>` payload (JSON). Empty when the model omitted it.
    #[serde(default)]
    pub analysis_json: String,
    /// true = stop and offer protagonist actions; false = keep writing.
    #[serde(default = "default_true")]
    pub stop: bool,
    #[serde(default)]
    pub stop_reason: String,
    #[serde(default)]
    pub stop_trigger: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub raw_output: String,
}

/// Warning prefix used when several options look glued into one line.
/// The UI treats this as "show the manual-fix dialog".
pub const CHOICES_SUSPECT: &str = "CHOICES_SUSPECT";

impl ParsedOutput {
    pub fn needs_fix(&self) -> bool {
        self.narration.trim().is_empty()
            || self.warnings.iter().any(|w| w.starts_with(CHOICES_SUSPECT))
    }
}

// ---------- v2: layered analysis, characters, POV, stop policy ----------

/// The ANALYSIS block. world/player are open maps (small models invent keys),
/// so the reader can render any field instead of silently dropping it.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Analysis {
    #[serde(default)]
    pub world: BTreeMap<String, Value>,
    #[serde(default)]
    pub player: BTreeMap<String, Value>,
    #[serde(default)]
    pub characters: Vec<CharacterDelta>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub new_character_candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharacterDelta {
    #[serde(default)]
    pub name: String,
    #[serde(default, alias = "state")]
    pub current_state: String,
    /// passerby | notable
    #[serde(default)]
    pub importance: String,
    #[serde(default)]
    pub is_new: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    pub id: String,
    pub world_id: String,
    pub story_id: Option<String>,
    pub name: String,
    /// preset = from the lore library "角色" category; emergent = appeared in play.
    pub kind: String,
    /// passerby | notable
    pub importance: String,
    pub preset_card_id: Option<String>,
    pub current_state: String,
    pub first_turn: Option<i64>,
    pub inject: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorySettings {
    #[serde(default = "default_pov")]
    pub pov: String,
    #[serde(default = "default_true")]
    pub show_scene_markers: bool,
}

impl Default for StorySettings {
    fn default() -> Self {
        Self { pov: default_pov(), show_scene_markers: true }
    }
}

/// Structured view of app.yaml for the GUI policy panel. The YAML file itself
/// remains the single source of truth; this is only a read model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub retrieval_enabled: bool,
    pub event_background_soft_cap: usize,
    pub persona_soft_cap: usize,
    pub opening_scene_turns: usize,
    pub stop_triggers: Vec<String>,
    pub max_consecutive_turns: usize,
    pub analysis_second_pass: bool,
    pub character_inject: String,
    pub show_scene_markers: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            retrieval_enabled: false,
            event_background_soft_cap: 2000,
            persona_soft_cap: 800,
            opening_scene_turns: 3,
            stop_triggers: vec!["dialogue".into(), "major_decision".into(), "new_character".into()],
            max_consecutive_turns: 3,
            analysis_second_pass: false,
            character_inject: "notable".into(),
            show_scene_markers: true,
        }
    }
}

// ---------- Context policy (parsed subset of policies/*.yaml) ----------

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModuleCfg {
    #[serde(default)]
    pub on: bool,
    #[serde(default)]
    pub budget: usize,
    #[serde(default)]
    pub n: usize,
    #[serde(default)]
    pub top_n: usize,
    #[serde(default)]
    pub trim: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimatorCfg {
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default = "default_zh")]
    pub zh_ratio: f64,
    #[serde(default = "default_ascii")]
    pub ascii_ratio: f64,
    #[serde(default = "default_other")]
    pub other_ratio: f64,
    #[serde(default = "default_margin")]
    pub safety_margin: f64,
}

fn default_strategy() -> String { "hybrid".into() }
fn default_zh() -> f64 { 1.0 }
fn default_ascii() -> f64 { 0.28 }
fn default_other() -> f64 { 0.5 }
fn default_margin() -> f64 { 1.15 }

impl Default for EstimatorCfg {
    fn default() -> Self {
        Self {
            strategy: default_strategy(),
            zh_ratio: default_zh(),
            ascii_ratio: default_ascii(),
            other_ratio: default_other(),
            safety_margin: default_margin(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetrievalCfg {
    #[serde(default)]
    pub bm25_topk: usize,
    #[serde(default)]
    pub vec_topk: usize,
    #[serde(default)]
    pub fusion: String,
    #[serde(default)]
    pub rrf_k: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThrottleCfg {
    #[serde(default)]
    pub summary_every_n: usize,
    #[serde(default)]
    pub state_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Policy {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub estimator: EstimatorCfg,
    #[serde(default = "default_window")]
    pub context_window: usize,
    #[serde(default = "default_output")]
    pub max_output_tokens: usize,
    #[serde(default)]
    pub modules: BTreeMap<String, ModuleCfg>,
    #[serde(default)]
    pub retrieval: RetrievalCfg,
    #[serde(default)]
    pub throttle: ThrottleCfg,
    #[serde(default)]
    pub templates: BTreeMap<String, String>,
    #[serde(default)]
    pub output_template: String,
}

fn default_window() -> usize { 10000 }
fn default_output() -> usize { 1200 }

// ---------- Context bundle ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetItem {
    pub name: String,
    pub enabled: bool,
    pub budget: usize,
    pub est: usize,
    pub trimmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetReport {
    pub items: Vec<BudgetItem>,
    pub total_est: usize,
    pub cap: usize,
    pub retrieval_enabled: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBundle {
    pub text: String,
    pub report: BudgetReport,
    pub mode: String,
    /// Which system template produced this prompt (shown in the inspector).
    #[serde(default)]
    pub template: String,
    /// The POV the prompt was built for.
    #[serde(default = "default_pov")]
    pub pov: String,
}

// ---------- v3: context inspector / director / saves ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnPrompt {
    pub turn_id: String,
    /// narration | analysis
    pub kind: String,
    pub text: String,
    pub est: i64,
    pub meta_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryDirector {
    pub story_id: String,
    pub enabled: bool,
    pub outline_md: String,
    /// One-shot instruction that the NEXT turn must follow (cleared after use).
    pub next_note: String,
    pub updated_at: String,
}

impl Default for StoryDirector {
    fn default() -> Self {
        Self {
            story_id: String::new(),
            enabled: false,
            outline_md: String::new(),
            next_note: String::new(),
            updated_at: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorySave {
    pub id: String,
    pub story_id: String,
    pub turn_index: i64,
    /// auto | manual
    pub kind: String,
    pub label: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedLore {
    pub doc_id: String,
    pub title: String,
    pub rel_path: String,
}

// ---------- Engine config (engines.yaml) ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleParams {
    #[serde(default = "default_temp")]
    pub temperature: f64,
    #[serde(default = "default_tokens")]
    pub max_tokens: usize,
    #[serde(default)]
    pub top_p: Option<f64>,
}
fn default_temp() -> f64 { 0.8 }
fn default_tokens() -> usize { 1200 }

impl Default for SampleParams {
    fn default() -> Self {
        Self { temperature: default_temp(), max_tokens: default_tokens(), top_p: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineProfile {
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub context_window: Option<usize>,
    #[serde(default)]
    pub params: SampleParams,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub retries: u8,
}
fn default_timeout() -> u64 { 300000 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasteProfile {
    #[serde(default)]
    pub target_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineProfiles {
    #[serde(default)]
    pub api: Option<EngineProfile>,
    #[serde(default)]
    pub local: Option<EngineProfile>,
    #[serde(default)]
    pub paste: Option<PasteProfile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// local | api | custom —— 决定 base_url / api_key 缺省时向谁借。
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub base_url: String,
    /// 留空则按 provider 复用 local / api 的 key（在线服务基本都要填）。
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub fallback: String,
    /// 部分模型（OpenAI text-embedding-3-*）支持降维，0 = 用默认维度。
    #[serde(default)]
    pub dimensions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnginesConfig {
    #[serde(default)]
    pub engines: Option<EngineProfiles>,
    #[serde(default)]
    pub embedding: Option<EmbeddingConfig>,
}

// ---------- v1.5: lore library, timeline, event background ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoreCategory {
    pub id: String,
    pub world_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub card_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoreCard {
    pub doc_id: String,
    pub world_id: String,
    pub title: String,
    pub rel_path: String,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub tags: Vec<String>,
    pub summary: String,
    pub char_count: i64,
    pub chunk_count: i64,
    pub sha256: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoreCardDetail {
    pub card: LoreCard,
    pub content: String,
    pub timeline_event_ids: Vec<String>,
}

/// The visual form always submits every field, so no nested Option semantics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoreCardPatch {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub timeline_event_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub world_id: String,
    pub era: String,
    pub year_label: String,
    pub title: String,
    pub body_md: String,
    pub ordinal: i64,
    pub inject_into_context: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEventBackground {
    pub content_md: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryEventBackground {
    pub enabled: bool,
    pub content_md: String,
    pub inherited_content_md: String,
    pub updated_at: String,
}
