//! Repository des traces.

use chrono::{DateTime, Utc};
use nav_routing::{Track, TrackPoint};
use serde::Serialize;
use sqlx::{Row, SqliteConnection};
use uuid::Uuid;

use crate::codec::{position_from_db, time_from_db, time_to_db, uuid_from_db};
use crate::{Db, StorageError};

/// Résumé d'une trace (sans ses points).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSummary {
    /// Identifiant.
    pub id: Uuid,
    /// Nom.
    pub name: String,
    /// Début (UTC).
    pub started_at: DateTime<Utc>,
    /// Fin (UTC), absente pendant l'enregistrement.
    pub ended_at: Option<DateTime<Utc>>,
    /// Enregistrement en cours.
    pub is_recording: bool,
    /// Nombre de points.
    pub point_count: u64,
}

async fn insert_points(
    conn: &mut SqliteConnection,
    track_id: Uuid,
    points: &[TrackPoint],
) -> Result<(), StorageError> {
    for p in points {
        sqlx::query(
            "INSERT INTO track_points \
               (track_id, timestamp_utc, latitude, longitude, sog_ms, cog_rad, depth_m) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(track_id.to_string())
        .bind(time_to_db(p.timestamp))
        .bind(p.position.latitude_deg())
        .bind(p.position.longitude_deg())
        .bind(p.sog_mps.filter(|v| v.is_finite()))
        .bind(p.cog_rad.filter(|v| v.is_finite()))
        .bind(p.depth_m.filter(|v| v.is_finite()))
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub(crate) async fn create_track_in(
    conn: &mut SqliteConnection,
    track: &Track,
    recording: bool,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO tracks (id, name, started_at, ended_at, is_recording) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(track.id.to_string())
    .bind(&track.name)
    .bind(time_to_db(track.started_at))
    .bind(track.ended_at.map(time_to_db))
    .bind(recording)
    .execute(&mut *conn)
    .await?;
    insert_points(conn, track.id, &track.points).await
}

impl Db {
    /// Crée une trace avec ses points éventuels. `recording` indique
    /// qu'un enregistrement est en cours (des points seront ajoutés).
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite (rien n'est écrit).
    pub async fn create_track(&self, track: &Track, recording: bool) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        create_track_in(&mut tx, track, recording).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Ajoute des points à une trace en cours d'enregistrement.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] ou [`StorageError::TrackNotRecording`]
    /// selon l'état de la trace.
    pub async fn append_track_points(
        &self,
        track_id: Uuid,
        points: &[TrackPoint],
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        let recording: Option<bool> =
            sqlx::query_scalar("SELECT is_recording FROM tracks WHERE id = ?")
                .bind(track_id.to_string())
                .fetch_optional(&mut *tx)
                .await?;
        match recording {
            None => return Err(StorageError::NotFound(track_id)),
            Some(false) => return Err(StorageError::TrackNotRecording(track_id)),
            Some(true) => {}
        }
        insert_points(&mut tx, track_id, points).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Termine l'enregistrement d'une trace.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si la trace n'existe pas.
    pub async fn finish_track(
        &self,
        track_id: Uuid,
        ended_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let updated = sqlx::query("UPDATE tracks SET ended_at = ?, is_recording = 0 WHERE id = ?")
            .bind(time_to_db(ended_at))
            .bind(track_id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        if updated == 0 {
            return Err(StorageError::NotFound(track_id));
        }
        Ok(())
    }

    /// Clôt les enregistrements interrompus (arrêt brutal). La fin retenue
    /// est l'horodatage du dernier point enregistré, ou le début de la
    /// trace si elle est vide : aucune heure n'est inventée. Retourne le
    /// nombre de traces clôturées.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn close_interrupted_tracks(&self) -> Result<u64, StorageError> {
        let closed = sqlx::query(
            "UPDATE tracks SET is_recording = 0, ended_at = COALESCE( \
               (SELECT MAX(timestamp_utc) FROM track_points WHERE track_id = tracks.id), \
               started_at) \
             WHERE is_recording = 1",
        )
        .execute(&self.pool)
        .await?
        .rows_affected();
        if closed > 0 {
            tracing::warn!(
                component = crate::COMPONENT,
                closed,
                "enregistrements de trace interrompus clôturés"
            );
        }
        Ok(closed)
    }

    /// Liste les traces, de la plus récente à la plus ancienne.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée corrompue.
    pub async fn list_tracks(&self) -> Result<Vec<TrackSummary>, StorageError> {
        let rows = sqlx::query(
            "SELECT t.id, t.name, t.started_at, t.ended_at, t.is_recording, \
               (SELECT COUNT(*) FROM track_points p WHERE p.track_id = t.id) AS point_count \
             FROM tracks t ORDER BY t.started_at DESC, t.id",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                let ended: Option<String> = row.try_get("ended_at")?;
                let count: i64 = row.try_get("point_count")?;
                Ok(TrackSummary {
                    id: uuid_from_db(row.try_get("id")?)?,
                    name: row.try_get("name")?,
                    started_at: time_from_db(row.try_get("started_at")?)?,
                    ended_at: ended.as_deref().map(time_from_db).transpose()?,
                    is_recording: row.try_get("is_recording")?,
                    point_count: u64::try_from(count).unwrap_or(0),
                })
            })
            .collect()
    }

    /// Charge une trace avec ses points, dans l'ordre chronologique.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée corrompue.
    pub async fn get_track(&self, id: Uuid) -> Result<Option<Track>, StorageError> {
        let mut conn = self.pool.acquire().await?;
        let Some(row) =
            sqlx::query("SELECT id, name, started_at, ended_at FROM tracks WHERE id = ?")
                .bind(id.to_string())
                .fetch_optional(&mut *conn)
                .await?
        else {
            return Ok(None);
        };
        let points = sqlx::query(
            "SELECT timestamp_utc, latitude, longitude, sog_ms, cog_rad, depth_m \
             FROM track_points WHERE track_id = ? ORDER BY timestamp_utc, id",
        )
        .bind(id.to_string())
        .fetch_all(&mut *conn)
        .await?
        .iter()
        .map(|p| {
            Ok(TrackPoint {
                timestamp: time_from_db(p.try_get("timestamp_utc")?)?,
                position: position_from_db(p.try_get("latitude")?, p.try_get("longitude")?)?,
                sog_mps: p.try_get("sog_ms")?,
                cog_rad: p.try_get("cog_rad")?,
                depth_m: p.try_get("depth_m")?,
            })
        })
        .collect::<Result<Vec<_>, StorageError>>()?;
        let ended: Option<String> = row.try_get("ended_at")?;
        Ok(Some(Track {
            id: uuid_from_db(row.try_get("id")?)?,
            name: row.try_get("name")?,
            started_at: time_from_db(row.try_get("started_at")?)?,
            ended_at: ended.as_deref().map(time_from_db).transpose()?,
            points,
        }))
    }

    /// Supprime une trace et ses points. Retourne `false` si elle
    /// n'existait pas.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn delete_track(&self, id: Uuid) -> Result<bool, StorageError> {
        let deleted = sqlx::query("DELETE FROM tracks WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(deleted > 0)
    }
}
