//! Repository des îles et amers téléchargés (OpenStreetMap), par région.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use nav_poi::{Poi, PoiKind};
use serde::Serialize;
use sqlx::Row;

use crate::codec::{position_from_db, time_from_db, time_to_db};
use crate::{Db, StorageError};

/// État d'une région téléchargée.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoiRegionStatus {
    /// Identifiant de la région.
    pub id: String,
    /// Date du téléchargement (UTC).
    pub fetched_at: DateTime<Utc>,
    /// Service d'origine.
    pub source: String,
    /// Nombre de points enregistrés.
    pub poi_count: u64,
}

impl Db {
    /// Remplace les points d'une région, en une transaction : en cas
    /// d'échec, les points précédents restent intacts.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn replace_region_pois(
        &self,
        region_id: &str,
        fetched_at: DateTime<Utc>,
        source: &str,
        pois: &[Poi],
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM poi_regions WHERE id = ?")
            .bind(region_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO poi_regions (id, fetched_at, source, poi_count) VALUES (?, ?, ?, ?)",
        )
        .bind(region_id)
        .bind(time_to_db(fetched_at))
        .bind(source)
        .bind(i64::try_from(pois.len()).unwrap_or(i64::MAX))
        .execute(&mut *tx)
        .await?;
        for poi in pois {
            sqlx::query(
                "INSERT INTO pois \
                   (region_id, osm_ref, kind, name, latitude, longitude, category, details) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(region_id)
            .bind(&poi.osm_ref)
            .bind(poi.kind.code())
            .bind(&poi.name)
            .bind(poi.position.latitude_deg())
            .bind(poi.position.longitude_deg())
            .bind(&poi.category)
            .bind(&poi.details)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Régions téléchargées.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée corrompue.
    pub async fn list_poi_regions(&self) -> Result<Vec<PoiRegionStatus>, StorageError> {
        sqlx::query("SELECT id, fetched_at, source, poi_count FROM poi_regions ORDER BY id")
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|row| {
                let count: i64 = row.try_get("poi_count")?;
                Ok(PoiRegionStatus {
                    id: row.try_get("id")?,
                    fetched_at: time_from_db(row.try_get("fetched_at")?)?,
                    source: row.try_get("source")?,
                    poi_count: u64::try_from(count).unwrap_or(0),
                })
            })
            .collect()
    }

    /// Tous les points enregistrés, sans doublon entre régions qui se
    /// recouvrent.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée corrompue.
    pub async fn list_pois(&self) -> Result<Vec<Poi>, StorageError> {
        let rows = sqlx::query(
            "SELECT osm_ref, kind, name, latitude, longitude, category, details \
             FROM pois ORDER BY region_id, osm_ref",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut seen = HashSet::new();
        let mut pois = Vec::with_capacity(rows.len());
        for row in &rows {
            let osm_ref: String = row.try_get("osm_ref")?;
            if !seen.insert(osm_ref.clone()) {
                continue;
            }
            let kind: String = row.try_get("kind")?;
            pois.push(Poi {
                osm_ref,
                kind: kind
                    .parse::<PoiKind>()
                    .map_err(|e| StorageError::Corrupt(e.to_string()))?,
                name: row.try_get("name")?,
                position: position_from_db(row.try_get("latitude")?, row.try_get("longitude")?)?,
                category: row.try_get("category")?,
                details: row.try_get("details")?,
            });
        }
        Ok(pois)
    }

    /// Supprime une région et ses points. Retourne `false` si elle
    /// n'existait pas.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn delete_poi_region(&self, region_id: &str) -> Result<bool, StorageError> {
        let deleted = sqlx::query("DELETE FROM poi_regions WHERE id = ?")
            .bind(region_id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(deleted > 0)
    }
}
