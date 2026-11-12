
-- NautiK
-- SQLite schema v1
-- À utiliser comme base des migrations SQLx.

PRAGMA foreign_keys = ON;

CREATE TABLE vessels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    mmsi TEXT,
    length_m REAL,
    beam_m REAL,
    draft_m REAL,
    created_at TEXT NOT NULL
);

CREATE TABLE data_sources (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source_type TEXT NOT NULL,
    configuration_json TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 100,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE TABLE charts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    chart_format TEXT NOT NULL,
    min_zoom INTEGER,
    max_zoom INTEGER,
    bounds_json TEXT,
    attribution TEXT,
    priority INTEGER NOT NULL DEFAULT 100,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_verified_at TEXT
);

CREATE TABLE waypoints (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    latitude REAL NOT NULL
        CHECK(latitude BETWEEN -90 AND 90),
    longitude REAL NOT NULL
        CHECK(longitude BETWEEN -180 AND 180),
    description TEXT,
    icon TEXT,
    color TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE routes (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE route_points (
    route_id TEXT NOT NULL,
    waypoint_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    PRIMARY KEY(route_id, sequence),
    FOREIGN KEY(route_id)
        REFERENCES routes(id) ON DELETE CASCADE,
    FOREIGN KEY(waypoint_id)
        REFERENCES waypoints(id)
);

CREATE TABLE tracks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    is_recording INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE track_points (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id TEXT NOT NULL,
    timestamp_utc TEXT NOT NULL,
    latitude REAL NOT NULL,
    longitude REAL NOT NULL,
    sog_ms REAL,
    cog_rad REAL,
    depth_m REAL,
    FOREIGN KEY(track_id)
        REFERENCES tracks(id) ON DELETE CASCADE
);

CREATE INDEX idx_track_points_track_time
ON track_points(track_id, timestamp_utc);

CREATE TABLE ais_targets (
    mmsi TEXT PRIMARY KEY,
    name TEXT,
    vessel_type INTEGER,
    latitude REAL,
    longitude REAL,
    sog_ms REAL,
    cog_rad REAL,
    heading_rad REAL,
    navigation_status INTEGER,
    last_seen_at TEXT NOT NULL
);

CREATE TABLE alarms (
    id TEXT PRIMARY KEY,
    alarm_type TEXT NOT NULL,
    name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    configuration_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE alarm_events (
    id TEXT PRIMARY KEY,
    alarm_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    timestamp_utc TEXT NOT NULL,
    details_json TEXT,
    FOREIGN KEY(alarm_id)
        REFERENCES alarms(id)
);

CREATE INDEX idx_alarm_events_time
ON alarm_events(timestamp_utc);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);
