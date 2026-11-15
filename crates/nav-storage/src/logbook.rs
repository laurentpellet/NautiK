//! Repository du journal de bord.

use chrono::{DateTime, Utc};
use nav_routing::{LogEntry, LogKind};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use uuid::Uuid;

use crate::codec::{position_from_db, time_from_db, time_to_db, uuid_from_db};
use crate::{Db, StorageError};

/// Nombre maximal d'entrées renvoyées par une lecture.
pub const MAX_LOG_PAGE: u32 = 5_000;

fn entry_from_row(row: &SqliteRow) -> Result<LogEntry, StorageError> {
    let kind: String = row.try_get("kind")?;
    let lat: Option<f64> = row.try_get("latitude")?;
    let lon: Option<f64> = row.try_get("longitude")?;
    let position = match (lat, lon) {
        (Some(lat), Some(lon)) => Some(position_from_db(lat, lon)?),
        (None, None) => None,
        _ => return Err(StorageError::Corrupt("position incomplète".to_owned())),
    };
    Ok(LogEntry {
        id: uuid_from_db(row.try_get("id")?)?,
        at: time_from_db(row.try_get("timestamp_utc")?)?,
        kind: LogKind::from_code(&kind)
            .ok_or_else(|| StorageError::Corrupt(format!("type d'entrée « {kind} »")))?,
        text: row.try_get("text")?,
        position,
        position_stale: row.try_get::<i64, _>("position_stale")? != 0,
        sog_mps: row.try_get("sog_ms")?,
        cog_rad: row.try_get("cog_rad")?,
        heading_rad: row.try_get("heading_rad")?,
        tws_mps: row.try_get("tws_ms")?,
        twd_rad: row.try_get("twd_rad")?,
        pressure_pa: row.try_get("pressure_pa")?,
        depth_m: row.try_get("depth_m")?,
        simulated: row.try_get::<i64, _>("simulated")? != 0,
    })
}

/// Valeur finie ou NULL (SQLite refuse NaN).
fn finite(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite())
}

impl Db {
    /// Ajoute une entrée au journal.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn append_log_entry(&self, entry: &LogEntry) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO logbook_entries \
               (id, timestamp_utc, kind, text, latitude, longitude, position_stale, sog_ms, \
                cog_rad, heading_rad, tws_ms, twd_rad, pressure_pa, depth_m, simulated) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(entry.id.to_string())
        .bind(time_to_db(entry.at))
        .bind(entry.kind.code())
        .bind(&entry.text)
        .bind(entry.position.map(|p| p.latitude_deg()))
        .bind(entry.position.map(|p| p.longitude_deg()))
        .bind(i64::from(entry.position_stale))
        .bind(finite(entry.sog_mps))
        .bind(finite(entry.cog_rad))
        .bind(finite(entry.heading_rad))
        .bind(finite(entry.tws_mps))
        .bind(finite(entry.twd_rad))
        .bind(finite(entry.pressure_pa))
        .bind(finite(entry.depth_m))
        .bind(i64::from(entry.simulated))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Entrées du journal, de la plus récente à la plus ancienne, au plus
    /// `limit` (borné à [`MAX_LOG_PAGE`]), antérieures à `before` si donné.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée corrompue.
    pub async fn list_log_entries(
        &self,
        limit: u32,
        before: Option<DateTime<Utc>>,
    ) -> Result<Vec<LogEntry>, StorageError> {
        let limit = i64::from(limit.min(MAX_LOG_PAGE));
        let rows =
            match before {
                Some(before) => {
                    sqlx::query(
                        "SELECT * FROM logbook_entries WHERE timestamp_utc < ? \
                     ORDER BY timestamp_utc DESC, id DESC LIMIT ?",
                    )
                    .bind(time_to_db(before))
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await?
                }
                None => sqlx::query(
                    "SELECT * FROM logbook_entries ORDER BY timestamp_utc DESC, id DESC LIMIT ?",
                )
                .bind(limit)
                .fetch_all(&self.pool)
                .await?,
            };
        rows.iter().map(entry_from_row).collect()
    }

    /// Instant de la dernière entrée automatique.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée corrompue.
    pub async fn last_auto_log_at(&self) -> Result<Option<DateTime<Utc>>, StorageError> {
        let at: Option<String> = sqlx::query_scalar(
            "SELECT MAX(timestamp_utc) FROM logbook_entries WHERE kind = 'auto'",
        )
        .fetch_one(&self.pool)
        .await?;
        at.as_deref().map(time_from_db).transpose()
    }

    /// Supprime une entrée. Retourne `false` si elle n'existait pas.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn delete_log_entry(&self, id: Uuid) -> Result<bool, StorageError> {
        let deleted = sqlx::query("DELETE FROM logbook_entries WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(deleted > 0)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use nav_core::GeoPosition;

    fn t(s: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000 + s, 0).expect("t")
    }

    fn entry(s: i64, kind: LogKind) -> LogEntry {
        LogEntry {
            id: Uuid::new_v4(),
            at: t(s),
            kind,
            text: Some("Ris pris".to_owned()),
            position: Some(GeoPosition::new(-17.5, -149.6).expect("p")),
            position_stale: true,
            sog_mps: Some(3.0),
            cog_rad: Some(1.0),
            heading_rad: None,
            tws_mps: Some(f64::NAN),
            twd_rad: None,
            pressure_pa: Some(101_300.0),
            depth_m: None,
            simulated: true,
        }
    }

    #[tokio::test]
    async fn entries_round_trip_newest_first() {
        let db = Db::open_in_memory().await.expect("db");
        assert_eq!(db.last_auto_log_at().await.expect("aucune"), None);
        let a = entry(0, LogKind::Auto);
        let b = entry(60, LogKind::Manual);
        let c = entry(120, LogKind::Auto);
        for e in [&a, &b, &c] {
            db.append_log_entry(e).await.expect("ajout");
        }
        let list = db.list_log_entries(10, None).await.expect("liste");
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].id, c.id);
        assert_eq!(list[2].id, a.id);
        assert_eq!(list[1].text.as_deref(), Some("Ris pris"));
        assert!(list[1].position_stale && list[1].simulated);
        assert_eq!(list[1].tws_mps, None, "NaN jamais enregistré");
        assert_eq!(list[1].pressure_pa, Some(101_300.0));

        let page = db.list_log_entries(1, Some(t(120))).await.expect("page");
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, b.id);
        assert_eq!(db.last_auto_log_at().await.expect("auto"), Some(t(120)));

        assert!(db.delete_log_entry(b.id).await.expect("suppression"));
        assert!(!db.delete_log_entry(b.id).await.expect("déjà"));
        assert_eq!(db.list_log_entries(0, None).await.expect("vide").len(), 0);
    }

    #[tokio::test]
    async fn corrupt_entries_are_reported() {
        let db = Db::open_in_memory().await.expect("db");
        sqlx::query(
            "INSERT INTO logbook_entries (id, timestamp_utc, kind, latitude) \
             VALUES ('x', '2027-01-15T08:00:00.000000000Z', 'auto', 10)",
        )
        .execute(&db.pool)
        .await
        .expect("insertion brute");
        assert!(matches!(
            db.list_log_entries(10, None).await,
            Err(StorageError::Corrupt(_))
        ));
        let bad_kind = sqlx::query(
            "INSERT INTO logbook_entries (id, timestamp_utc, kind) VALUES ('y', 'a', 'robot')",
        )
        .execute(&db.pool)
        .await;
        assert!(bad_kind.is_err(), "type contrôlé par le schéma");
    }
}
