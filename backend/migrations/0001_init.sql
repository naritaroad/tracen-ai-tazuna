-- Tracen Academy administrative information system: initial schema (PostgreSQL 18).
-- Data owners: see docs/data-model.md.

-- ---------------------------------------------------------------------------
-- Accounts and sessions
-- ---------------------------------------------------------------------------
CREATE TABLE users (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    username        text NOT NULL,
    display_name    text NOT NULL,
    role            text NOT NULL CHECK (role IN ('trainer', 'staff', 'admin')),
    password_hash   text NOT NULL,
    preferred_locale text CHECK (preferred_locale IN ('zh-TW', 'ja', 'en-GB')),
    created_at      timestamptz NOT NULL DEFAULT now(),
    disabled_at     timestamptz
);
CREATE UNIQUE INDEX users_username_key ON users (lower(username));

-- Only a SHA-256 of the session token is stored; the token itself lives in the cookie.
CREATE TABLE sessions (
    token_hash      bytea PRIMARY KEY,
    user_id         uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at      timestamptz NOT NULL DEFAULT now(),
    expires_at      timestamptz NOT NULL,
    last_seen_at    timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX sessions_user_idx ON sessions (user_id);

-- ---------------------------------------------------------------------------
-- Academy catalog (demo data, rebuilt from data/seed/*.csv)
-- ---------------------------------------------------------------------------
CREATE TABLE academy_info (
    topic       text PRIMARY KEY,
    title       text NOT NULL,
    title_ja    text NOT NULL,
    title_en    text NOT NULL,
    content     text NOT NULL,
    content_ja  text NOT NULL,
    content_en  text NOT NULL,
    source_type text NOT NULL CHECK (source_type IN ('official', 'reference', 'project')),
    source_ref  text NOT NULL,
    updated_on  date NOT NULL,
    sort_order  integer NOT NULL DEFAULT 0
);

CREATE TABLE training_services (
    id           text PRIMARY KEY,
    name         text NOT NULL,
    name_ja      text NOT NULL,
    name_en      text NOT NULL,
    category     text NOT NULL CHECK (category IN ('track', 'equipment', 'analysis', 'room', 'workshop', 'support')),
    unit_minutes integer NOT NULL CHECK (unit_minutes > 0),
    capacity     integer NOT NULL CHECK (capacity > 0),
    fee_jpy      integer NOT NULL CHECK (fee_jpy >= 0),
    available    boolean NOT NULL,
    location     text NOT NULL,
    aliases      text[] NOT NULL DEFAULT '{}',
    notes        text NOT NULL DEFAULT '',
    source_type  text NOT NULL CHECK (source_type IN ('official', 'reference', 'project')),
    sort_order   integer NOT NULL DEFAULT 0
);

CREATE TABLE academy_events (
    id          text PRIMARY KEY,
    title       text NOT NULL,
    title_ja    text NOT NULL,
    title_en    text NOT NULL,
    kind        text NOT NULL CHECK (kind IN ('class', 'exam', 'event', 'maintenance', 'workshop')),
    starts_at   timestamptz NOT NULL,
    ends_at     timestamptz NOT NULL,
    -- Service ids this event blocks; {'all'} blocks every service.
    affects     text[] NOT NULL DEFAULT '{}',
    source_type text NOT NULL CHECK (source_type IN ('official', 'reference', 'project')),
    CHECK (ends_at > starts_at)
);
CREATE INDEX academy_events_time_idx ON academy_events (starts_at, ends_at);

-- ---------------------------------------------------------------------------
-- Trainer memory (editable by the trainer; never a source of academy facts)
-- ---------------------------------------------------------------------------
CREATE TABLE trainer_profiles (
    user_id         uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    preferred_name  text,
    reply_locale    text CHECK (reply_locale IN ('zh-TW', 'ja', 'en-GB')),
    updated_at      timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE memory_items (
    id                 uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id            uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind               text NOT NULL CHECK (kind IN ('preference', 'fact', 'summary')),
    content            text NOT NULL CHECK (length(content) BETWEEN 1 AND 500),
    source             text NOT NULL CHECK (source IN ('trainer', 'agent', 'staff')),
    source_message_id  uuid,
    status             text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'deleted')),
    created_at         timestamptz NOT NULL DEFAULT now(),
    updated_at         timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX memory_items_user_idx ON memory_items (user_id, status);

-- ---------------------------------------------------------------------------
-- Conversations, agent runs and tool traces
-- ---------------------------------------------------------------------------
CREATE TABLE conversations (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title       text NOT NULL DEFAULT '',
    status      text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'handoff', 'closed')),
    summary     text,            -- reserved for long-conversation summaries
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX conversations_user_idx ON conversations (user_id, updated_at DESC);

CREATE TABLE agent_runs (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    conversation_id uuid NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    status          text NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'completed', 'failed')),
    model           text NOT NULL,
    locale          text NOT NULL,
    steps           integer NOT NULL DEFAULT 0,
    error_code      text,
    usage           jsonb NOT NULL DEFAULT '{}',
    started_at      timestamptz NOT NULL DEFAULT now(),
    finished_at     timestamptz
);
CREATE INDEX agent_runs_conversation_idx ON agent_runs (conversation_id, started_at);

CREATE TABLE messages (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    conversation_id uuid NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    role            text NOT NULL CHECK (role IN ('user', 'assistant', 'director', 'notice')),
    content         text NOT NULL,
    author_user_id  uuid REFERENCES users (id) ON DELETE SET NULL,
    run_id          uuid REFERENCES agent_runs (id) ON DELETE SET NULL,
    created_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX messages_conversation_idx ON messages (conversation_id, created_at);

CREATE TABLE tool_calls (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    run_id      uuid NOT NULL REFERENCES agent_runs (id) ON DELETE CASCADE,
    step        integer NOT NULL,
    call_id     text NOT NULL,
    tool_name   text NOT NULL,
    arguments   jsonb NOT NULL,
    result      jsonb NOT NULL,
    status      text NOT NULL CHECK (status IN ('ok', 'rejected', 'error')),
    duration_ms integer NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX tool_calls_run_idx ON tool_calls (run_id, created_at);

-- ---------------------------------------------------------------------------
-- Director's Office cases (human hand-over)
-- ---------------------------------------------------------------------------
CREATE SEQUENCE director_ticket_no_seq START 1001;

CREATE TABLE director_tickets (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    ticket_no        text NOT NULL UNIQUE DEFAULT ('DIR-' || nextval('director_ticket_no_seq')),
    requester_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    conversation_id  uuid REFERENCES conversations (id) ON DELETE SET NULL,
    reason           text NOT NULL CHECK (reason IN ('not_in_data', 'exception_request', 'complaint', 'asked_for_director', 'other')),
    summary          text NOT NULL CHECK (length(summary) BETWEEN 1 AND 2000),
    origin           text NOT NULL CHECK (origin IN ('agent', 'form')),
    status           text NOT NULL DEFAULT 'waiting' CHECK (status IN ('waiting', 'in_progress', 'resolved', 'returned')),
    assignee_id      uuid REFERENCES users (id) ON DELETE SET NULL,
    -- Retries of the same request (same run, or the same form submit) must not create a second case.
    idempotency_key  text UNIQUE,
    created_at       timestamptz NOT NULL DEFAULT now(),
    accepted_at      timestamptz,
    closed_at        timestamptz
);
CREATE INDEX director_tickets_status_idx ON director_tickets (status, created_at);
CREATE INDEX director_tickets_requester_idx ON director_tickets (requester_id, created_at DESC);

-- Audit trail and replies. actor_user_id is the real operator, whatever persona the UI shows.
CREATE TABLE ticket_events (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),
    ticket_id      uuid NOT NULL REFERENCES director_tickets (id) ON DELETE CASCADE,
    actor_user_id  uuid REFERENCES users (id) ON DELETE SET NULL,
    kind           text NOT NULL CHECK (kind IN ('created', 'accepted', 'reply', 'trainer_message', 'resolved', 'returned', 'note')),
    body           text NOT NULL DEFAULT '',
    created_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX ticket_events_ticket_idx ON ticket_events (ticket_id, created_at);
