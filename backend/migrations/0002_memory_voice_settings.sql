-- Questionnaire follow-up (docs/decisions.md):
--   Q04  anime and community sources
--   Q08  semantic memory with pgvector, automatic curation, conversation summaries
--   Q13  reserved storage for spoken replies
--   AI settings and monthly budget adjustable on the management site

-- ---------------------------------------------------------------------------
-- Q04: source types, in priority order official > anime > community; reference/project are this system's rules
-- ---------------------------------------------------------------------------
ALTER TABLE academy_info DROP CONSTRAINT academy_info_source_type_check;
ALTER TABLE academy_info ADD CONSTRAINT academy_info_source_type_check
    CHECK (source_type IN ('official', 'anime', 'community', 'reference', 'project'));
ALTER TABLE training_services DROP CONSTRAINT training_services_source_type_check;
ALTER TABLE training_services ADD CONSTRAINT training_services_source_type_check
    CHECK (source_type IN ('official', 'anime', 'community', 'reference', 'project'));
ALTER TABLE academy_events DROP CONSTRAINT academy_events_source_type_check;
ALTER TABLE academy_events ADD CONSTRAINT academy_events_source_type_check
    CHECK (source_type IN ('official', 'anime', 'community', 'reference', 'project'));

-- ---------------------------------------------------------------------------
-- Q08: semantic memory (768-dimension embeddings)
-- ---------------------------------------------------------------------------
CREATE EXTENSION IF NOT EXISTS vector;

ALTER TABLE memory_items DROP CONSTRAINT memory_items_status_check;
ALTER TABLE memory_items ADD CONSTRAINT memory_items_status_check CHECK (status IN ('active', 'superseded', 'deleted'));
ALTER TABLE memory_items ADD COLUMN embedding vector(768);
-- Vectors from different embedding models are not comparable; search only within one model.
ALTER TABLE memory_items ADD COLUMN embed_model text;
ALTER TABLE memory_items ADD COLUMN superseded_by uuid REFERENCES memory_items (id) ON DELETE SET NULL;

-- One short note per past exchange, found again by meaning, not by keyword.
CREATE TABLE memory_events (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id          uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    conversation_id  uuid REFERENCES conversations (id) ON DELETE SET NULL,
    run_id           uuid REFERENCES agent_runs (id) ON DELETE SET NULL,
    content          text NOT NULL CHECK (length(content) BETWEEN 1 AND 500),
    embedding        vector(768),
    embed_model      text,
    created_at       timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX memory_events_user_idx ON memory_events (user_id, created_at DESC);
CREATE INDEX memory_events_embedding_idx ON memory_events USING hnsw (embedding vector_cosine_ops);
CREATE INDEX memory_items_embedding_idx ON memory_items USING hnsw (embedding vector_cosine_ops);

-- How many messages the stored summary already covers.
ALTER TABLE conversations ADD COLUMN summary_message_count integer NOT NULL DEFAULT 0;

-- ---------------------------------------------------------------------------
-- AI settings and usage (management site)
-- ---------------------------------------------------------------------------
CREATE TABLE app_settings (
    key         text PRIMARY KEY,
    value       jsonb NOT NULL,
    updated_at  timestamptz NOT NULL DEFAULT now(),
    updated_by  uuid REFERENCES users (id) ON DELETE SET NULL
);

-- Every paid model call; cost is computed from the prices entered on the management site.
CREATE TABLE model_usage (
    id                 uuid PRIMARY KEY DEFAULT uuidv7(),
    occurred_at        timestamptz NOT NULL DEFAULT now(),
    purpose            text NOT NULL CHECK (purpose IN ('chat', 'memory', 'summary', 'embed', 'transcribe')),
    model              text NOT NULL,
    prompt_tokens      integer NOT NULL DEFAULT 0,
    completion_tokens  integer NOT NULL DEFAULT 0,
    cost_usd           numeric(12, 6) NOT NULL DEFAULT 0,
    priced             boolean NOT NULL DEFAULT false,
    user_id            uuid REFERENCES users (id) ON DELETE SET NULL,
    run_id             uuid REFERENCES agent_runs (id) ON DELETE SET NULL
);
CREATE INDEX model_usage_time_idx ON model_usage (occurred_at);

-- ---------------------------------------------------------------------------
-- Q13: spoken replies (reserved, empty for now; docs/voice-contract.md)
-- ---------------------------------------------------------------------------
CREATE TABLE message_audio (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    message_id   uuid NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    voice        text NOT NULL,
    engine       text NOT NULL,
    storage_key  text NOT NULL,
    duration_ms  integer,
    visemes      jsonb,
    created_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX message_audio_message_idx ON message_audio (message_id);
