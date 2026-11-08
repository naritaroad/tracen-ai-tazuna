-- MCP server for outside agents (docs/engineering/upgrade-plan-1.1.md §6).
-- Outside agents only ever read the public rows, through the mcp_* views below, and may only add
-- feedback to a review queue. The tracen_mcp role can use exactly that; see docs/engineering/development.md
-- for giving it a password and pointing MCP_DATABASE_URL at it.

-- Client keys for higher limits (§6.6). Only the SHA-256 of a key is stored; the key is shown once.
CREATE TABLE mcp_clients (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    name         text NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    key_hash     text NOT NULL UNIQUE,
    scopes       text[] NOT NULL DEFAULT ARRAY['public:read', 'feedback:write']
                 CHECK (scopes <@ ARRAY['public:read', 'feedback:write']),
    created_by   uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    disabled_at  timestamptz,
    last_used_at timestamptz
);

-- Feedback from outside agents (§6.5). No contact fields; never read by the in-app agent.
CREATE SEQUENCE external_feedback_no START 1001;
CREATE TABLE external_feedback (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    feedback_no     text NOT NULL UNIQUE DEFAULT 'FB-' || nextval('external_feedback_no'),
    client_id       uuid REFERENCES mcp_clients (id) ON DELETE SET NULL,
    -- Scope of the idempotency key: the client id, or 'anonymous'. No IP address is stored.
    client_key      text NOT NULL,
    category        text NOT NULL CHECK (category IN ('error_report', 'missing_info', 'suggestion', 'other')),
    -- Emptied 30 days after a rejection; the row stays for the counts.
    text            text NOT NULL,
    language        text NOT NULL CHECK (language IN ('zh-TW', 'ja', 'en-GB')),
    related_ref     text,
    status          text NOT NULL DEFAULT 'pending_review' CHECK (status IN ('pending_review', 'accepted', 'rejected', 'merged')),
    flags           text[] NOT NULL DEFAULT '{}',
    idempotency_key text NOT NULL,
    -- SHA-256 of the normalised text: the same feedback is accepted once.
    text_hash       text NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    reviewed_by     uuid REFERENCES users (id) ON DELETE SET NULL,
    reviewed_at     timestamptz,
    review_note     text,
    purged_at       timestamptz,
    UNIQUE (client_key, idempotency_key)
);
CREATE UNIQUE INDEX external_feedback_text_once ON external_feedback (text_hash);
CREATE INDEX external_feedback_status_idx ON external_feedback (status, created_at DESC);

-- One row per MCP tool call: no query text, no IP address.
CREATE TABLE mcp_calls (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),
    occurred_at    timestamptz NOT NULL DEFAULT now(),
    client_id      uuid REFERENCES mcp_clients (id) ON DELETE SET NULL,
    transport      text NOT NULL CHECK (transport IN ('stdio', 'http')),
    tool           text NOT NULL,
    status         text NOT NULL CHECK (status IN ('ok', 'rejected', 'error', 'rate_limited', 'disabled')),
    duration_ms    integer NOT NULL DEFAULT 0,
    argument_chars integer NOT NULL DEFAULT 0
);
CREATE INDEX mcp_calls_time_idx ON mcp_calls (occurred_at DESC);

-- Public views. Each one filters visibility itself and lists its columns, so a new private
-- column never leaks by accident.
CREATE VIEW mcp_info AS
    SELECT topic, title, title_ja, title_en, content, content_ja, content_en, source_type, source_ref, updated_on, sort_order, visibility
    FROM academy_info WHERE visibility = 'public';
CREATE VIEW mcp_services AS
    SELECT id, name, name_ja, name_en, category, unit_minutes, capacity, fee_jpy, available, location, aliases,
           ''::text AS notes, source_type, sort_order, visibility
    FROM training_services WHERE visibility = 'public';
CREATE VIEW mcp_events AS
    SELECT id, title, title_ja, title_en, kind, starts_at, ends_at, affects, source_type, visibility
    FROM academy_events WHERE visibility = 'public';
CREATE VIEW mcp_races AS
    SELECT id, name, name_ja, name_en, grade, racecourse, surface, distance_m, direction, month, half, classes, source_type, source_ref, sort_order, visibility
    FROM races WHERE visibility = 'public';
CREATE VIEW mcp_handbook AS
    SELECT id, doc, section, part, lang, chapter, title, content, embedding, embed_model, visibility, source_type, source_ref, sort_order
    FROM handbook_chunks WHERE visibility = 'public';
-- Model names, prices, the budget and the MCP limits; no secrets live in app_settings.
CREATE VIEW mcp_settings AS
    SELECT key, value FROM app_settings WHERE key IN ('ai', 'mcp', 'skills');
-- Spending without user or run ids, for the budget checks.
CREATE VIEW mcp_usage AS
    SELECT occurred_at, purpose, cost_usd FROM model_usage;
CREATE VIEW mcp_client_keys AS
    SELECT id, key_hash, scopes, disabled_at FROM mcp_clients;
-- What duplicate and idempotency checks need; never the feedback text.
CREATE VIEW mcp_feedback_keys AS
    SELECT client_key, idempotency_key, text_hash, feedback_no, status FROM external_feedback;

-- The read-only role. Created without a password; skipped where the migrating user may not create roles.
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'tracen_mcp') THEN
        CREATE ROLE tracen_mcp NOLOGIN;
    END IF;
    GRANT SELECT ON mcp_info, mcp_services, mcp_events, mcp_races, mcp_handbook, mcp_settings, mcp_usage,
        mcp_client_keys, mcp_feedback_keys TO tracen_mcp;
    GRANT INSERT ON model_usage, mcp_calls, external_feedback TO tracen_mcp;
    GRANT SELECT (id, feedback_no, status) ON external_feedback TO tracen_mcp;
    GRANT SELECT (id), UPDATE (last_used_at) ON mcp_clients TO tracen_mcp;
    GRANT USAGE ON SEQUENCE external_feedback_no TO tracen_mcp;
EXCEPTION WHEN insufficient_privilege THEN
    RAISE NOTICE 'tracen_mcp role not created (insufficient privilege); MCP will use the main role';
END
$$;
