-- NautiK — îles et amers nautiques téléchargés (OpenStreetMap), par région.
-- Une région est remplacée en bloc à chaque mise à jour (transaction).

CREATE TABLE poi_regions (
    id TEXT PRIMARY KEY,
    fetched_at TEXT NOT NULL,
    source TEXT NOT NULL,
    poi_count INTEGER NOT NULL
);

CREATE TABLE pois (
    region_id TEXT NOT NULL,
    osm_ref TEXT NOT NULL,
    kind TEXT NOT NULL,
    name TEXT,
    latitude REAL NOT NULL CHECK (latitude BETWEEN -90 AND 90),
    longitude REAL NOT NULL CHECK (longitude BETWEEN -180 AND 180),
    category TEXT,
    details TEXT,
    PRIMARY KEY(region_id, osm_ref),
    FOREIGN KEY(region_id)
        REFERENCES poi_regions(id) ON DELETE CASCADE
);

CREATE INDEX idx_pois_position
ON pois(latitude, longitude);
