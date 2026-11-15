-- MVP schema. All lore/FTS/vector tables are created up front (mostly empty)
-- so enabling retrieval in phase 2 never requires a migration.
-- lore_chunks intentionally has NO source_* provenance columns:
-- lore lives at World level and is fully decoupled from story turns.

CREATE TABLE IF NOT EXISTS worlds (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    mirror_path TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS stories (
    id          TEXT PRIMARY KEY,
    world_id    TEXT NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    engine_mode TEXT NOT NULL DEFAULT 'paste',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    mirror_path TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_stories_world ON stories(world_id);

CREATE TABLE IF NOT EXISTS protagonists (
    story_id         TEXT PRIMARY KEY REFERENCES stories(id) ON DELETE CASCADE,
    name             TEXT NOT NULL DEFAULT '',
    persona_md       TEXT NOT NULL DEFAULT '',
    opening_scene_md TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS lore_docs (
    id          TEXT PRIMARY KEY,
    world_id    TEXT NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    rel_path    TEXT NOT NULL,
    sha256      TEXT NOT NULL DEFAULT '',
    chunk_count INTEGER NOT NULL DEFAULT 0,
    origin      TEXT NOT NULL DEFAULT 'empty',
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_lore_docs_world ON lore_docs(world_id);

CREATE TABLE IF NOT EXISTS lore_chunks (
    id           INTEGER PRIMARY KEY,
    doc_id       TEXT NOT NULL REFERENCES lore_docs(id) ON DELETE CASCADE,
    ordinal      INTEGER NOT NULL,
    heading_path TEXT NOT NULL DEFAULT '',
    content      TEXT NOT NULL,
    char_len     INTEGER NOT NULL DEFAULT 0,
    token_est    INTEGER NOT NULL DEFAULT 0,
    sha256       TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_chunks_doc ON lore_chunks(doc_id, ordinal);

-- Empty in MVP. Filled only when the user explicitly enables retrieval.
CREATE VIRTUAL TABLE IF NOT EXISTS lore_fts USING fts5(
    tokens,
    chunk_id UNINDEXED,
    tokenize = "unicode61 remove_diacritics 0"
);

CREATE TABLE IF NOT EXISTS chunk_vectors (
    chunk_id   INTEGER PRIMARY KEY,
    vec        BLOB NOT NULL,
    dim        INTEGER NOT NULL,
    model_name TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS embedding_meta (
    world_id   TEXT PRIMARY KEY,
    model_name TEXT NOT NULL,
    dim        INTEGER NOT NULL,
    built_at   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS passages (
    id                 TEXT PRIMARY KEY,
    story_id           TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
    turn_index         INTEGER NOT NULL,
    trigger_action_md  TEXT NOT NULL DEFAULT '',
    content_md         TEXT NOT NULL DEFAULT '',
    raw_output         TEXT NOT NULL DEFAULT '',
    parse_status       TEXT NOT NULL DEFAULT 'pending',
    engine_mode        TEXT NOT NULL DEFAULT '',
    bundle_snapshot    TEXT NOT NULL DEFAULT '',
    prompt_token_est   INTEGER NOT NULL DEFAULT 0,
    output_token_est   INTEGER NOT NULL DEFAULT 0,
    summary_delta_md   TEXT NOT NULL DEFAULT '',
    state_delta_md     TEXT NOT NULL DEFAULT '',
    created_at         TEXT NOT NULL,
    UNIQUE(story_id, turn_index)
);

CREATE TABLE IF NOT EXISTS choices (
    id            TEXT PRIMARY KEY,
    passage_id    TEXT NOT NULL REFERENCES passages(id) ON DELETE CASCADE,
    ordinal       INTEGER NOT NULL,
    label         TEXT NOT NULL,
    action_md     TEXT NOT NULL,
    is_free_input INTEGER NOT NULL DEFAULT 0,
    selected_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_choices_passage ON choices(passage_id, ordinal);

CREATE TABLE IF NOT EXISTS state_card (
    story_id   TEXT PRIMARY KEY REFERENCES stories(id) ON DELETE CASCADE,
    revision   INTEGER NOT NULL DEFAULT 0,
    data_json  TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL,
    updated_by TEXT NOT NULL DEFAULT 'manual'
);

CREATE TABLE IF NOT EXISTS state_card_history (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    story_id   TEXT NOT NULL,
    revision   INTEGER NOT NULL,
    data_json  TEXT NOT NULL,
    source     TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS summary_state (
    story_id            TEXT PRIMARY KEY REFERENCES stories(id) ON DELETE CASCADE,
    content_md          TEXT NOT NULL DEFAULT '',
    covered_through_turn INTEGER NOT NULL DEFAULT 0,
    updated_at          TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS summary_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    story_id    TEXT NOT NULL,
    content_md  TEXT NOT NULL,
    from_turn   INTEGER NOT NULL,
    to_turn     INTEGER NOT NULL,
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS context_build_log (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    story_id       TEXT NOT NULL,
    turn           INTEGER NOT NULL,
    query_text     TEXT NOT NULL DEFAULT '',
    budget_report  TEXT NOT NULL DEFAULT '{}',
    created_at     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS app_config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- ---------------------------------------------------------------------------
-- v1.5: side-car tables. No existing table gets a new column, so the whole
-- schema stays idempotent under execute_batch(). All of these hang off the
-- World (never off a passage/turn), keeping lore decoupled from story rewinds.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS lore_categories (
    id         TEXT PRIMARY KEY,
    world_id   TEXT NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    parent_id  TEXT REFERENCES lore_categories(id) ON DELETE SET NULL,
    name       TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    UNIQUE(world_id, parent_id, name)
);
CREATE INDEX IF NOT EXISTS idx_categories_world ON lore_categories(world_id, parent_id);

-- 1 card = 1 lore_docs row = 1 .md file. Metadata lives here, body stays in .md.
CREATE TABLE IF NOT EXISTS lore_cards (
    doc_id      TEXT PRIMARY KEY REFERENCES lore_docs(id) ON DELETE CASCADE,
    world_id    TEXT NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    category_id TEXT REFERENCES lore_categories(id) ON DELETE SET NULL,
    tags        TEXT NOT NULL DEFAULT '[]',
    summary     TEXT NOT NULL DEFAULT '',
    char_count  INTEGER NOT NULL DEFAULT 0,
    updated_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_cards_world ON lore_cards(world_id);
CREATE INDEX IF NOT EXISTS idx_cards_category ON lore_cards(category_id);

CREATE TABLE IF NOT EXISTS card_tags (
    card_id TEXT NOT NULL REFERENCES lore_cards(doc_id) ON DELETE CASCADE,
    tag     TEXT NOT NULL,
    PRIMARY KEY(card_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_card_tags_tag ON card_tags(tag);

CREATE TABLE IF NOT EXISTS timeline_events (
    id                  TEXT PRIMARY KEY,
    world_id            TEXT NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    era                 TEXT NOT NULL DEFAULT '',
    year_label          TEXT NOT NULL DEFAULT '',
    title               TEXT NOT NULL,
    body_md             TEXT NOT NULL DEFAULT '',
    ordinal             INTEGER NOT NULL DEFAULT 0,
    inject_into_context INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_timeline_world ON timeline_events(world_id, ordinal);

CREATE TABLE IF NOT EXISTS card_timeline (
    card_id  TEXT NOT NULL REFERENCES lore_cards(doc_id) ON DELETE CASCADE,
    event_id TEXT NOT NULL REFERENCES timeline_events(id) ON DELETE CASCADE,
    PRIMARY KEY(card_id, event_id)
);

CREATE TABLE IF NOT EXISTS world_event_background (
    world_id   TEXT PRIMARY KEY REFERENCES worlds(id) ON DELETE CASCADE,
    content_md TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL
);

-- enabled = 0 means "fall back to the world-level background".
CREATE TABLE IF NOT EXISTS story_event_background (
    story_id   TEXT PRIMARY KEY REFERENCES stories(id) ON DELETE CASCADE,
    content_md TEXT NOT NULL DEFAULT '',
    enabled    INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

-- ---------------------------------------------------------------------------
-- v2: reading experience. Everything is a side-car table: NO existing table
-- gets a new column, so execute_batch() stays fully idempotent.
-- ---------------------------------------------------------------------------

-- protagonist | omniscient. Absent row means the default (protagonist).
CREATE TABLE IF NOT EXISTS story_settings (
    story_id           TEXT PRIMARY KEY REFERENCES stories(id) ON DELETE CASCADE,
    pov                TEXT NOT NULL DEFAULT 'protagonist',
    show_scene_markers INTEGER NOT NULL DEFAULT 1,
    updated_at         TEXT NOT NULL
);

-- Per-turn output metadata produced by the STOP/ANALYSIS contract blocks.
CREATE TABLE IF NOT EXISTS passage_meta (
    passage_id    TEXT PRIMARY KEY REFERENCES passages(id) ON DELETE CASCADE,
    stop_decision INTEGER NOT NULL DEFAULT 1,
    stop_reason   TEXT NOT NULL DEFAULT '',
    stop_trigger  TEXT NOT NULL DEFAULT '',
    pov           TEXT NOT NULL DEFAULT 'protagonist',
    analysis_json TEXT NOT NULL DEFAULT '',
    created_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_passage_meta_stop ON passage_meta(stop_decision);

-- Characters: presets come from the lore library, emergent ones are nominated
-- by the analysis stage during play. importance controls long-term injection.
CREATE TABLE IF NOT EXISTS characters (
    id              TEXT PRIMARY KEY,
    world_id        TEXT NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    story_id        TEXT REFERENCES stories(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    kind            TEXT NOT NULL DEFAULT 'emergent',
    importance      TEXT NOT NULL DEFAULT 'passerby',
    preset_card_id  TEXT,
    current_state   TEXT NOT NULL DEFAULT '{}',
    first_turn      INTEGER,
    inject          INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_characters_world ON characters(world_id);
CREATE INDEX IF NOT EXISTS idx_characters_story ON characters(story_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_characters_unique
    ON characters(world_id, COALESCE(story_id, ''), name, kind);

-- ---------------------------------------------------------------------------
-- v3: context inspector / director / save points / pinned lore
-- Again: side-car tables only, so execute_batch() stays idempotent.
-- ---------------------------------------------------------------------------

-- The exact prompt sent for a turn, kept so the reader can audit what the
-- model actually saw. kind = narration | analysis.
CREATE TABLE IF NOT EXISTS turn_prompts (
    turn_id    TEXT NOT NULL,
    kind       TEXT NOT NULL,
    text       TEXT NOT NULL DEFAULT '',
    est        INTEGER NOT NULL DEFAULT 0,
    meta_json  TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    PRIMARY KEY(turn_id, kind)
);

-- Director mode: a long-range outline (soft guidance) plus a one-shot note
-- that the next turn MUST follow. next_note is cleared once applied.
CREATE TABLE IF NOT EXISTS story_director (
    story_id   TEXT PRIMARY KEY REFERENCES stories(id) ON DELETE CASCADE,
    enabled    INTEGER NOT NULL DEFAULT 0,
    outline_md TEXT NOT NULL DEFAULT '',
    next_note  TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL
);

-- Save points. A snapshot holds the memory layers (state card, summary,
-- characters) at the end of turn_index; restoring drops later turns and
-- puts these three back, so "后悔上一步" never needs a full DB copy.
CREATE TABLE IF NOT EXISTS story_saves (
    id              TEXT PRIMARY KEY,
    story_id        TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
    turn_index      INTEGER NOT NULL,
    kind            TEXT NOT NULL DEFAULT 'auto',
    label           TEXT NOT NULL DEFAULT '',
    state_json      TEXT NOT NULL DEFAULT '{}',
    summary_md      TEXT NOT NULL DEFAULT '',
    characters_json TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_saves_story ON story_saves(story_id, turn_index DESC);

-- Cards the user pinned into every turn of one story (bypasses retrieval).
CREATE TABLE IF NOT EXISTS story_lore_pins (
    story_id   TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
    doc_id     TEXT NOT NULL REFERENCES lore_docs(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY(story_id, doc_id)
);

-- ---------------------------------------------------------------------------
-- v5: 章节 / 全文检索 / 人工改名
-- ---------------------------------------------------------------------------

-- 自动切章，但允许用户改章名（只存覆盖值，不复制正文）
CREATE TABLE IF NOT EXISTS story_chapters (
    story_id   TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
    ordinal    INTEGER NOT NULL,
    title      TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL,
    PRIMARY KEY(story_id, ordinal)
);

-- 回合正文的全文索引（设定用 lore_fts，这里单独一份，互不影响）
CREATE VIRTUAL TABLE IF NOT EXISTS passage_fts USING fts5(
    tokens,
    passage_id UNINDEXED,
    tokenize = "unicode61 remove_diacritics 0"
);
