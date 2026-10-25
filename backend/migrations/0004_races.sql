-- In-game race schedule shown on the academy calendar (2026-10-26 user request).

-- Game data: each race runs every year in one half of a month; the calendar places it on that
-- half (1st–15th or 16th–end). Rebuilt from data/seed/race_schedule.csv like the other catalog tables.
CREATE TABLE races (
    id          text PRIMARY KEY,
    name        text NOT NULL,
    name_ja     text NOT NULL,
    name_en     text NOT NULL,
    grade       text NOT NULL CHECK (grade IN ('G1', 'G2', 'G3')),
    racecourse  text NOT NULL,
    surface     text NOT NULL CHECK (surface IN ('turf', 'dirt')),
    distance_m  integer NOT NULL CHECK (distance_m > 0),
    direction   text NOT NULL CHECK (direction IN ('right', 'left', 'straight')),
    month       smallint NOT NULL CHECK (month BETWEEN 1 AND 12),
    half        smallint NOT NULL CHECK (half IN (1, 2)),
    -- Which career years may enter: junior, classic, senior.
    classes     text[] NOT NULL CHECK (classes <@ ARRAY['junior', 'classic', 'senior'] AND cardinality(classes) > 0),
    source_type text NOT NULL CHECK (source_type IN ('official', 'anime', 'community', 'reference', 'project')),
    source_ref  text NOT NULL DEFAULT '',
    sort_order  integer NOT NULL DEFAULT 0
);
CREATE INDEX races_month_idx ON races (month, half);
