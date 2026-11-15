//! Journal de bord : entrées manuelles, relevés automatiques périodiques
//! (à heure ronde selon l'intervalle choisi) et export CSV.
//!
//! Un relevé automatique est écrit même sans position : l'absence de
//! données est alors visible dans le journal, jamais masquée.

use chrono::{DateTime, TimeZone, Utc};
use nav_routing::logbook::{self, AUTO_INTERVALS_MIN, DEFAULT_AUTO_INTERVAL_MIN};
use nav_routing::{LogEntry, LogKind};
use serde::{Deserialize, Serialize};

use crate::routing::parse_id;
use crate::service::Service;
use crate::CommandResult;

/// Clé du réglage du journal.
pub const LOGBOOK_SETTING: &str = "logbook";
/// Nombre maximal d'entrées exportées en CSV.
pub const MAX_EXPORT_ENTRIES: u32 = nav_storage::MAX_LOG_PAGE;

fn text<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

/// Réglages du journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogbookSettings {
    /// Intervalle des relevés automatiques (minutes ; 0 : désactivés).
    pub auto_interval_min: u32,
}

impl Default for LogbookSettings {
    fn default() -> Self {
        Self {
            auto_interval_min: DEFAULT_AUTO_INTERVAL_MIN,
        }
    }
}

/// État des relevés automatiques (en mémoire).
#[derive(Debug, Default)]
pub struct AutoLog {
    interval_min: u32,
    next_due: Option<DateTime<Utc>>,
}

/// Prochaine heure ronde strictement après `now` pour l'intervalle donné.
fn next_boundary(now: DateTime<Utc>, interval_min: u32) -> Option<DateTime<Utc>> {
    let period = i64::from(interval_min) * 60;
    if period <= 0 {
        return None;
    }
    let next = (now.timestamp().div_euclid(period) + 1) * period;
    Utc.timestamp_opt(next, 0).single()
}

impl AutoLog {
    /// Change l'intervalle ; le prochain relevé est à l'heure ronde
    /// suivante.
    pub fn configure(&mut self, interval_min: u32, now: DateTime<Utc>) {
        self.interval_min = interval_min;
        self.next_due = next_boundary(now, interval_min);
    }

    /// Un relevé est dû à `now` ; l'échéance suivante est alors fixée.
    pub fn due(&mut self, now: DateTime<Utc>) -> bool {
        match self.next_due {
            Some(due) if now >= due => {
                self.next_due = next_boundary(now, self.interval_min);
                true
            }
            _ => false,
        }
    }
}

fn validate(settings: LogbookSettings) -> CommandResult<LogbookSettings> {
    if AUTO_INTERVALS_MIN.contains(&settings.auto_interval_min) {
        Ok(settings)
    } else {
        Err(format!(
            "intervalle invalide : {} min (valeurs : {AUTO_INTERVALS_MIN:?})",
            settings.auto_interval_min
        ))
    }
}

/// Recharge le réglage enregistré (un réglage illisible ou invalide est
/// signalé et la valeur par défaut utilisée).
pub async fn restore(service: &Service) {
    let settings = match service
        .db
        .get_setting::<LogbookSettings>(LOGBOOK_SETTING)
        .await
    {
        Ok(Some(s)) => validate(s).unwrap_or_else(|error| {
            tracing::error!(%error, "réglage du journal invalide");
            LogbookSettings::default()
        }),
        Ok(None) => LogbookSettings::default(),
        Err(error) => {
            tracing::error!(%error, "réglage du journal illisible");
            LogbookSettings::default()
        }
    };
    service
        .auto_log()
        .configure(settings.auto_interval_min, Utc::now());
}

/// Réglages courants.
#[must_use]
pub fn settings(service: &Service) -> LogbookSettings {
    LogbookSettings {
        auto_interval_min: service.auto_log().interval_min,
    }
}

/// Modifie et enregistre l'intervalle des relevés automatiques.
///
/// # Errors
///
/// Message lisible si l'intervalle est invalide ou non enregistré.
pub async fn set_settings(
    service: &Service,
    settings: LogbookSettings,
) -> CommandResult<LogbookSettings> {
    let settings = validate(settings)?;
    service
        .db
        .set_setting(LOGBOOK_SETTING, &settings)
        .await
        .map_err(text)?;
    service
        .auto_log()
        .configure(settings.auto_interval_min, Utc::now());
    Ok(settings)
}

fn entry(
    service: &Service,
    kind: LogKind,
    text_value: Option<&str>,
    now: DateTime<Utc>,
) -> CommandResult<LogEntry> {
    let snapshot = service
        .latest_instruments()
        .unwrap_or_else(|| nav_core::InstrumentsSnapshot {
            computed_at: now,
            position: None,
            readings: Vec::new(),
        });
    let simulated = service.engine().is_simulated();
    LogEntry::from_snapshot(kind, text_value, &snapshot, simulated, now).map_err(text)
}

/// Ajoute une entrée manuelle avec l'état du bord.
///
/// # Errors
///
/// Message lisible si le texte est invalide ou l'entrée non enregistrée.
pub async fn add(
    service: &Service,
    text_value: &str,
    now: DateTime<Utc>,
) -> CommandResult<LogEntry> {
    if text_value.trim().is_empty() {
        return Err("texte de l'entrée vide".to_owned());
    }
    let e = entry(service, LogKind::Manual, Some(text_value), now)?;
    service.db.append_log_entry(&e).await.map_err(text)?;
    Ok(e)
}

/// Écrit un relevé automatique s'il est dû (appelé à chaque pas).
pub async fn auto_entry(service: &Service, now: DateTime<Utc>) {
    if !service.auto_log().due(now) {
        return;
    }
    match entry(service, LogKind::Auto, None, now) {
        Ok(e) => {
            if let Err(error) = service.db.append_log_entry(&e).await {
                tracing::error!(%error, "relevé du journal non enregistré");
            }
        }
        Err(error) => tracing::error!(%error, "relevé du journal impossible"),
    }
}

/// Entrées, de la plus récente à la plus ancienne.
///
/// # Errors
///
/// Message lisible en cas d'erreur de la base.
pub async fn list(
    service: &Service,
    limit: u32,
    before: Option<DateTime<Utc>>,
) -> CommandResult<Vec<LogEntry>> {
    service
        .db
        .list_log_entries(limit, before)
        .await
        .map_err(text)
}

/// Supprime une entrée.
///
/// # Errors
///
/// Message lisible si l'identifiant est invalide ou l'entrée introuvable.
pub async fn delete(service: &Service, id: &str) -> CommandResult<()> {
    if !service
        .db
        .delete_log_entry(parse_id(id)?)
        .await
        .map_err(text)?
    {
        return Err("entrée introuvable".to_owned());
    }
    Ok(())
}

/// Export CSV des dernières entrées (au plus [`MAX_EXPORT_ENTRIES`]), du
/// plus ancien au plus récent.
///
/// # Errors
///
/// Message lisible en cas d'erreur de la base.
pub async fn export_csv(service: &Service) -> CommandResult<String> {
    let mut entries = list(service, MAX_EXPORT_ENTRIES, None).await?;
    entries.reverse();
    Ok(logbook::to_csv(&entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_service;

    fn t(s: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_800_000_000 + s, 0).single().expect("t")
    }

    #[test]
    fn auto_entries_fall_on_round_times() {
        let mut auto = AutoLog::default();
        assert!(!auto.due(t(0)), "non configuré");
        // 1 800 000 000 s = …08:00:00 ; intervalle 15 min.
        auto.configure(15, t(10));
        assert!(!auto.due(t(899)));
        assert!(auto.due(t(900)));
        assert!(!auto.due(t(901)), "un seul relevé par échéance");
        // Après une longue pause : un seul relevé, puis l'heure ronde
        // suivante.
        assert!(auto.due(t(5_000)));
        assert!(!auto.due(t(5_300)));
        assert!(auto.due(t(5_400)));
        auto.configure(0, t(6_000));
        assert!(!auto.due(t(100_000)), "désactivé");
    }

    #[tokio::test]
    async fn settings_are_validated_and_persisted() {
        let s = test_service().await;
        restore(&s).await;
        assert_eq!(settings(&s), LogbookSettings::default());
        assert!(set_settings(
            &s,
            LogbookSettings {
                auto_interval_min: 7
            }
        )
        .await
        .is_err());
        let set = set_settings(
            &s,
            LogbookSettings {
                auto_interval_min: 15,
            },
        )
        .await
        .expect("réglage");
        assert_eq!(settings(&s), set);
        s.auto_log().configure(0, Utc::now());
        restore(&s).await;
        assert_eq!(settings(&s).auto_interval_min, 15, "rechargé");
    }

    #[tokio::test]
    async fn manual_and_auto_entries_use_the_boat_state() {
        let s = test_service().await;
        assert!(add(&s, "  ", Utc::now()).await.is_err());
        // Avant tout pas : aucune position, l'entrée l'indique.
        let first = add(&s, "Départ", Utc::now()).await.expect("entrée");
        assert_eq!(first.position, None);
        s.step(Utc::now()).await;
        let e = add(&s, "Ris pris", Utc::now()).await.expect("entrée");
        assert!(e.position.is_some());
        assert!(e.simulated, "simulateur signalé");

        s.auto_log().configure(15, t(0));
        auto_entry(&s, t(10)).await;
        auto_entry(&s, t(900)).await;
        auto_entry(&s, t(901)).await;
        let all = list(&s, 100, None).await.expect("liste");
        assert_eq!(all.len(), 3);
        assert_eq!(all.iter().filter(|e| e.kind == LogKind::Auto).count(), 1);

        let csv = export_csv(&s).await.expect("csv");
        assert_eq!(csv.lines().count(), 4);
        assert!(
            csv.lines().nth(1).expect("ligne").contains("Départ"),
            "ordre chronologique"
        );

        delete(&s, &e.id.to_string()).await.expect("suppression");
        assert!(delete(&s, &e.id.to_string()).await.is_err());
        assert!(delete(&s, "x").await.is_err());
    }
}
