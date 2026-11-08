-- RAG over the trainer handbook (docs/engineering/upgrade-plan-1.1.md §4).
-- Rebuilt from data/rag/trainer_handbook.*.md and academy_info at start; embeddings are kept
-- while content_hash and embed_model stay the same, so a restart does not pay for them again.

CREATE TABLE handbook_chunks (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    -- handbook: a section of the handbook; academy_info: one topic of the short information table.
    doc          text NOT NULL CHECK (doc IN ('handbook', 'academy_info')),
    -- '§3.2' for the handbook, the topic code for academy_info.
    section      text NOT NULL,
    -- Position when one section is split into several chunks.
    part         smallint NOT NULL DEFAULT 0,
    lang         text NOT NULL CHECK (lang IN ('zh-TW', 'ja', 'en-GB')),
    chapter      text NOT NULL DEFAULT '',
    title        text NOT NULL,
    content      text NOT NULL,
    content_hash text NOT NULL,
    embedding    vector(768),
    embed_model  text,
    -- Who may read it (§6.2): public, trainer or staff. Unmarked means trainer.
    visibility   text NOT NULL DEFAULT 'trainer' CHECK (visibility IN ('public', 'trainer', 'staff')),
    source_type  text NOT NULL CHECK (source_type IN ('official', 'anime', 'community', 'reference', 'project')),
    source_ref   text NOT NULL DEFAULT '',
    sort_order   integer NOT NULL DEFAULT 0,
    UNIQUE (doc, section, part, lang)
);

-- Handbook search and, later, MCP queries are metered like the other model calls.
ALTER TABLE model_usage DROP CONSTRAINT model_usage_purpose_check;
ALTER TABLE model_usage ADD CONSTRAINT model_usage_purpose_check
    CHECK (purpose IN ('chat', 'memory', 'summary', 'embed', 'transcribe', 'rag', 'mcp_embed'));
