-- NautiK — marques personnelles et journal de bord.

CREATE TABLE marks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    latitude REAL NOT NULL CHECK (latitude BETWEEN -90 AND 90),
    longitude REAL NOT NULL CHECK (longitude BETWEEN -180 AND 180),
    icon TEXT NOT NULL,
    note TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_marks_name
ON marks(name COLLATE NOCASE);

-- Entrées du journal : automatiques (relevé périodique) ou manuelles.
-- Les valeurs absentes ou obsolètes au moment de l'entrée restent NULL.
CREATE TABLE logbook_entries (
    id TEXT PRIMARY KEY,
    timestamp_utc TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('auto', 'manual')),
    text TEXT,
    latitude REAL CHECK (latitude BETWEEN -90 AND 90),
    longitude REAL CHECK (longitude BETWEEN -180 AND 180),
    position_stale INTEGER NOT NULL DEFAULT 0,
    sog_ms REAL,
    cog_rad REAL,
    heading_rad REAL,
    tws_ms REAL,
    twd_rad REAL,
    pressure_pa REAL,
    depth_m REAL,
    simulated INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_logbook_time
ON logbook_entries(timestamp_utc);

CREATE INDEX idx_pois_name
ON pois(name COLLATE NOCASE);
