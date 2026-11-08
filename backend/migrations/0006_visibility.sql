-- Who may read each catalog row (docs/engineering/upgrade-plan-1.1.md §6.2).
-- source_type says where a row comes from; visibility says who may read it. Unmarked rows are for
-- signed-in trainers only; only 'public' rows ever reach the MCP server.

ALTER TABLE academy_info ADD COLUMN visibility text NOT NULL DEFAULT 'trainer'
    CHECK (visibility IN ('public', 'trainer', 'staff'));
ALTER TABLE training_services ADD COLUMN visibility text NOT NULL DEFAULT 'trainer'
    CHECK (visibility IN ('public', 'trainer', 'staff'));
ALTER TABLE academy_events ADD COLUMN visibility text NOT NULL DEFAULT 'trainer'
    CHECK (visibility IN ('public', 'trainer', 'staff'));
ALTER TABLE races ADD COLUMN visibility text NOT NULL DEFAULT 'trainer'
    CHECK (visibility IN ('public', 'trainer', 'staff'));
