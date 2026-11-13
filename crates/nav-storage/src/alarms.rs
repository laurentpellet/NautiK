//! Configuration des alarmes (table `alarms`) et historique de leurs
//! événements (table `alarm_events`).

use chrono::Utc;
use nav_alarms::{
    AlarmEvent, AlarmKind, AlarmSettings, AnchorConfig, EventDetails, EventKind, PositionLostConfig,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::codec::{time_from_db, time_to_db, uuid_from_db};
use crate::{Db, StorageError};

/// Nombre maximal d'événements lus en une fois.
pub const MAX_EVENTS_PAGE: u32 = 1_000;

/// Configuration du mouillage (colonne `configuration_json`). Le dernier
/// mouillage est conservé après la levée de l'ancre (`enabled = 0`) pour
/// proposer le même rayon.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnchorRow {
    anchor: Option<AnchorConfig>,
    radius_m: f64,
}

fn corrupt(what: &str, e: impl std::fmt::Display) -> StorageError {
    StorageError::Corrupt(format!("{what} : {e}"))
}

impl Db {
    async fn upsert_alarm(
        &self,
        kind: AlarmKind,
        enabled: bool,
        json: String,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO alarms (id, alarm_type, name, enabled, configuration_json, created_at) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET enabled = excluded.enabled, \
             configuration_json = excluded.configuration_json",
        )
        .bind(kind.id().to_string())
        .bind(kind.code())
        .bind(kind.label())
        .bind(enabled)
        .bind(json)
        .bind(time_to_db(Utc::now()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Enregistre la configuration des alarmes.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite.
    pub async fn save_alarm_settings(&self, settings: &AlarmSettings) -> Result<(), StorageError> {
        let anchor = AnchorRow {
            anchor: settings.anchor,
            radius_m: settings.anchor_radius_m,
        };
        let anchor_json = serde_json::to_string(&anchor).map_err(|e| corrupt("mouillage", e))?;
        let position_json = serde_json::to_string(&settings.position_lost)
            .map_err(|e| corrupt("perte de position", e))?;
        self.upsert_alarm(AlarmKind::Anchor, settings.anchor.is_some(), anchor_json)
            .await?;
        self.upsert_alarm(
            AlarmKind::PositionLost,
            settings.position_lost_enabled,
            position_json,
        )
        .await
    }

    /// Lit la configuration des alarmes (valeurs par défaut pour une
    /// alarme jamais configurée).
    ///
    /// # Errors
    ///
    /// [`StorageError::Corrupt`] si une configuration est illisible ou
    /// hors bornes : l'appelant décide du repli, qui doit être signalé.
    pub async fn load_alarm_settings(&self) -> Result<AlarmSettings, StorageError> {
        let rows = sqlx::query("SELECT id, enabled, configuration_json FROM alarms")
            .fetch_all(&self.pool)
            .await?;
        let mut settings = AlarmSettings::default();
        for row in rows {
            let id = uuid_from_db(row.try_get::<&str, _>("id")?)?;
            let enabled: bool = row.try_get("enabled")?;
            let json: &str = row.try_get("configuration_json")?;
            if id == AlarmKind::Anchor.id() {
                let r: AnchorRow =
                    serde_json::from_str(json).map_err(|e| corrupt("mouillage", e))?;
                if let Some(a) = &r.anchor {
                    a.validate().map_err(|e| corrupt("mouillage", e))?;
                }
                settings.anchor = if enabled { r.anchor } else { None };
                if r.radius_m.is_finite() && r.radius_m > 0.0 {
                    settings.anchor_radius_m = r.radius_m;
                }
            } else if id == AlarmKind::PositionLost.id() {
                let c: PositionLostConfig =
                    serde_json::from_str(json).map_err(|e| corrupt("perte de position", e))?;
                c.validate().map_err(|e| corrupt("perte de position", e))?;
                settings.position_lost_enabled = enabled;
                settings.position_lost = c;
            }
        }
        Ok(settings)
    }

    /// Historise des événements d'alarme (une transaction).
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite.
    pub async fn append_alarm_events(&self, events: &[AlarmEvent]) -> Result<(), StorageError> {
        if events.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for event in events {
            // L'alarme doit exister (clé étrangère), même jamais configurée.
            sqlx::query(
                "INSERT OR IGNORE INTO alarms \
                 (id, alarm_type, name, enabled, configuration_json, created_at) \
                 VALUES (?, ?, ?, 1, '{}', ?)",
            )
            .bind(event.alarm.id().to_string())
            .bind(event.alarm.code())
            .bind(event.alarm.label())
            .bind(time_to_db(event.at))
            .execute(&mut *tx)
            .await?;
            let details =
                serde_json::to_string(&event.details).map_err(|e| corrupt("événement", e))?;
            sqlx::query(
                "INSERT INTO alarm_events (id, alarm_id, event_type, timestamp_utc, details_json) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(event.id.to_string())
            .bind(event.alarm.id().to_string())
            .bind(event.kind.code())
            .bind(time_to_db(event.at))
            .bind(details)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Derniers événements d'alarme, du plus récent au plus ancien.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée illisible.
    pub async fn list_alarm_events(&self, limit: u32) -> Result<Vec<AlarmEvent>, StorageError> {
        let rows = sqlx::query(
            "SELECT e.id, a.alarm_type, e.event_type, e.timestamp_utc, e.details_json \
             FROM alarm_events e JOIN alarms a ON a.id = e.alarm_id \
             ORDER BY e.timestamp_utc DESC, e.rowid DESC LIMIT ?",
        )
        .bind(limit.min(MAX_EVENTS_PAGE))
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                let alarm_type: &str = row.try_get("alarm_type")?;
                let event_type: &str = row.try_get("event_type")?;
                let details: Option<&str> = row.try_get("details_json")?;
                Ok(AlarmEvent {
                    id: uuid_from_db(row.try_get("id")?)?,
                    alarm: AlarmKind::from_code(alarm_type)
                        .ok_or_else(|| corrupt("type d'alarme", alarm_type))?,
                    kind: EventKind::from_code(event_type)
                        .ok_or_else(|| corrupt("type d'événement", event_type))?,
                    at: time_from_db(row.try_get("timestamp_utc")?)?,
                    details: match details {
                        Some(json) => {
                            serde_json::from_str(json).map_err(|e| corrupt("événement", e))?
                        }
                        None => EventDetails::default(),
                    },
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration};
    use nav_alarms::AlarmManager;
    use nav_core::GeoPosition;

    fn t(s: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t") + Duration::seconds(s)
    }

    fn anchor() -> AnchorConfig {
        AnchorConfig::new(GeoPosition::new(-17.5, -149.5).expect("pos"), 60.0, 15).expect("cfg")
    }

    #[tokio::test]
    async fn default_settings_when_never_saved() {
        let db = Db::open_in_memory().await.expect("db");
        assert_eq!(
            db.load_alarm_settings().await.expect("lu"),
            AlarmSettings::default()
        );
    }

    #[tokio::test]
    async fn settings_round_trip_and_raised_anchor_keeps_radius() {
        let db = Db::open_in_memory().await.expect("db");
        let mut m = AlarmManager::new(AlarmSettings::default());
        m.drop_anchor(anchor(), t(0));
        m.set_position_lost_enabled(false, t(0));
        db.save_alarm_settings(&m.settings()).await.expect("save");
        assert_eq!(db.load_alarm_settings().await.expect("lu"), m.settings());

        m.raise_anchor(t(1));
        db.save_alarm_settings(&m.settings()).await.expect("save");
        let loaded = db.load_alarm_settings().await.expect("lu");
        assert_eq!(loaded.anchor, None);
        assert_eq!(loaded.anchor_radius_m, 60.0);
        assert!(!loaded.position_lost_enabled);
    }

    #[tokio::test]
    async fn corrupt_or_out_of_range_settings_are_reported() {
        let db = Db::open_in_memory().await.expect("db");
        db.upsert_alarm(AlarmKind::Anchor, true, "pas du json".to_owned())
            .await
            .expect("upsert");
        assert!(matches!(
            db.load_alarm_settings().await,
            Err(StorageError::Corrupt(_))
        ));
        let mut bad = anchor();
        bad.radius_m = 5_000.0;
        let json = serde_json::to_string(&AnchorRow {
            anchor: Some(bad),
            radius_m: 5_000.0,
        })
        .expect("json");
        db.upsert_alarm(AlarmKind::Anchor, true, json)
            .await
            .expect("upsert");
        assert!(matches!(
            db.load_alarm_settings().await,
            Err(StorageError::Corrupt(_))
        ));
        db.upsert_alarm(
            AlarmKind::Anchor,
            false,
            r#"{"anchor":null,"radiusM":50}"#.to_owned(),
        )
        .await
        .expect("upsert");
        db.upsert_alarm(
            AlarmKind::PositionLost,
            true,
            r#"{"delayS":9999}"#.to_owned(),
        )
        .await
        .expect("upsert");
        assert!(matches!(
            db.load_alarm_settings().await,
            Err(StorageError::Corrupt(_))
        ));
    }

    #[tokio::test]
    async fn events_are_appended_and_listed_newest_first() {
        let db = Db::open_in_memory().await.expect("db");
        db.append_alarm_events(&[]).await.expect("vide");
        let mut m = AlarmManager::new(AlarmSettings::default());
        let dropped = m.drop_anchor(anchor(), t(0));
        let lost = m.set_position_lost_enabled(false, t(5)).expect("événement");
        // Alarme jamais configurée : la ligne `alarms` est créée.
        db.append_alarm_events(&[dropped.clone(), lost.clone()])
            .await
            .expect("append");
        let raised = m.raise_anchor(t(9)).expect("événement");
        db.append_alarm_events(std::slice::from_ref(&raised))
            .await
            .expect("append");

        let events = db.list_alarm_events(10).await.expect("liste");
        assert_eq!(events, vec![raised, lost, dropped.clone()]);
        assert_eq!(db.list_alarm_events(1).await.expect("liste").len(), 1);
        assert!(db.list_alarm_events(0).await.expect("liste").is_empty());

        // Un identifiant dupliqué annule toute la transaction.
        let fresh = AlarmEvent {
            id: uuid::Uuid::new_v4(),
            ..dropped.clone()
        };
        assert!(db.append_alarm_events(&[fresh, dropped]).await.is_err());
        assert_eq!(db.list_alarm_events(10).await.expect("liste").len(), 3);
    }
}
