-- Trainer profile picture and pinned conversations (2026-10-25 user request).

-- One picture per account, already cropped and resized by the browser; the size cap keeps the
-- table small (the site shows it at most 96 px wide).
CREATE TABLE user_avatars (
    user_id      uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    content_type text NOT NULL CHECK (content_type IN ('image/png', 'image/jpeg', 'image/webp')),
    image        bytea NOT NULL CHECK (octet_length(image) BETWEEN 1 AND 524288),
    updated_at   timestamptz NOT NULL DEFAULT now()
);

-- Pinned conversations stay at the top of the sidebar, most recently pinned first.
ALTER TABLE conversations ADD COLUMN pinned_at timestamptz;
CREATE INDEX conversations_user_pinned_idx ON conversations (user_id, pinned_at DESC NULLS LAST, updated_at DESC);
