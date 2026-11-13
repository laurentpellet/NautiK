//! Opérations d'alarme (mouillage, perte de position), persistance de
//! leur configuration et historique.
//!
//! L'évaluation des alarmes est faite par le moteur (`nav-alarms`) à
//! chaque pas de la boucle, indépendamment de toute interface ; ces
//! opérations ne font que relayer les commandes de l'utilisateur. Chaque
//! changement est diffusé ([`EngineEvent::Alarms`]).

use chrono::Utc;
use nav_alarms::{AlarmEvent, AlarmKind, AlarmsSnapshot, MAX_DELAY_S};

use crate::service::{EngineEvent, Service};
use crate::CommandResult;

/// Journalise et historise des événements d'alarme.
pub async fn record(service: &Service, events: &[AlarmEvent]) {
    for e in events {
        match e.kind {
            nav_alarms::EventKind::Activated => tracing::warn!(
                component = nav_alarms::COMPONENT,
                alarm = e.alarm.code(),
                simulated = e.details.simulated,
                message = %e.details.message,
                "alarme déclenchée"
            ),
            kind => tracing::info!(
                component = nav_alarms::COMPONENT,
                alarm = e.alarm.code(),
                event = kind.code(),
                message = %e.details.message,
                "événement d'alarme"
            ),
        }
    }
    if let Err(error) = service.db.append_alarm_events(events).await {
        tracing::error!(%error, "événements d'alarme non historisés");
    }
}

/// Restaure la configuration enregistrée (démarrage). Une configuration
/// illisible est signalée et remplacée par les valeurs par défaut
/// (ancre levée, perte de position surveillée).
pub async fn restore(service: &Service) {
    match service.db.load_alarm_settings().await {
        Ok(settings) => {
            if let Some(anchor) = settings.anchor {
                tracing::info!(
                    radius_m = anchor.radius_m,
                    "surveillance du mouillage reprise"
                );
            }
            service.engine().restore_alarms(settings);
        }
        Err(error) => tracing::error!(%error, "configuration des alarmes ignorée"),
    }
}

/// État des alarmes.
#[must_use]
pub fn current(service: &Service) -> AlarmsSnapshot {
    service.engine().alarms(Utc::now())
}

/// Diffuse et retourne l'état à jour.
fn publish(service: &Service) -> AlarmsSnapshot {
    let snapshot = current(service);
    service.publish(EngineEvent::Alarms(snapshot.clone()));
    snapshot
}

/// Persiste la configuration et l'événement d'une commande, puis
/// diffuse et retourne l'état à jour.
async fn commit(service: &Service, event: Option<AlarmEvent>) -> CommandResult<AlarmsSnapshot> {
    let settings = service.engine().alarm_settings();
    service
        .db
        .save_alarm_settings(&settings)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(event) = event {
        record(service, &[event]).await;
    }
    Ok(publish(service))
}

fn parse_kind(kind: &str) -> CommandResult<AlarmKind> {
    AlarmKind::from_code(kind).ok_or_else(|| format!("alarme inconnue « {kind} »"))
}

/// Pose l'ancre à la position courante (qui doit être récente).
///
/// # Errors
///
/// Message lisible si la position n'est pas récente ou si le rayon ou la
/// temporisation est invalide.
pub async fn drop_anchor(
    service: &Service,
    radius_m: f64,
    delay_s: u32,
) -> CommandResult<AlarmsSnapshot> {
    if delay_s > MAX_DELAY_S {
        return Err(format!("temporisation invalide : 0 à {MAX_DELAY_S} s"));
    }
    let event = service
        .engine()
        .drop_anchor(radius_m, delay_s, Utc::now())
        .map_err(|e| e.to_string())?;
    commit(service, Some(event)).await
}

/// Lève l'ancre (fin de la surveillance du mouillage).
///
/// # Errors
///
/// Message lisible si la configuration ne peut pas être enregistrée.
pub async fn raise_anchor(service: &Service) -> CommandResult<AlarmsSnapshot> {
    let event = service.engine().raise_anchor(Utc::now());
    commit(service, event).await
}

/// Acquitte une alarme (coupe le son, l'alarme reste affichée).
///
/// # Errors
///
/// Message lisible si l'alarme est inconnue.
pub async fn acknowledge(service: &Service, kind: &str) -> CommandResult<AlarmsSnapshot> {
    let kind = parse_kind(kind)?;
    let event = service.engine().acknowledge_alarm(kind, Utc::now());
    if let Some(event) = event {
        record(service, &[event]).await;
    }
    Ok(publish(service))
}

/// Active ou désactive une alarme (perte de position). Le mouillage se
/// surveille en posant ou en levant l'ancre.
///
/// # Errors
///
/// Message lisible si l'alarme est inconnue ou non désactivable.
pub async fn set_enabled(
    service: &Service,
    kind: &str,
    enabled: bool,
) -> CommandResult<AlarmsSnapshot> {
    match parse_kind(kind)? {
        AlarmKind::PositionLost => {
            let event = service.engine().set_position_alarm(enabled, Utc::now());
            commit(service, event).await
        }
        AlarmKind::Anchor => {
            Err("le mouillage se surveille en posant ou en levant l'ancre".to_owned())
        }
    }
}

/// Derniers événements d'alarme (du plus récent au plus ancien), au plus
/// [`nav_storage::MAX_EVENTS_PAGE`].
///
/// # Errors
///
/// Message lisible en cas d'erreur de la base.
pub async fn events(service: &Service, limit: u32) -> CommandResult<Vec<AlarmEvent>> {
    service
        .db
        .list_alarm_events(limit.min(nav_storage::MAX_EVENTS_PAGE))
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_service;
    use nav_alarms::{AlarmState, EventKind};

    #[tokio::test]
    async fn anchor_is_persisted_logged_and_restored() {
        let service = test_service().await;
        // Sans position : refus, rien d'enregistré.
        assert!(drop_anchor(&service, 50.0, 10).await.is_err());
        service.engine().tick(Utc::now());
        assert!(drop_anchor(&service, 5.0, 10).await.is_err());
        assert!(drop_anchor(&service, 50.0, 301).await.is_err());
        let mut rx = service.subscribe();
        let snap = drop_anchor(&service, 60.0, 10).await.expect("ancre");
        assert_eq!(snap.anchor.map(|a| a.config.radius_m), Some(60.0));
        assert!(
            matches!(rx.try_recv(), Ok(EngineEvent::Alarms(_))),
            "changement diffusé"
        );

        // Redémarrage : la surveillance est reprise depuis la base.
        let settings = service.db.load_alarm_settings().await.expect("lu");
        assert_eq!(settings.anchor.map(|a| a.radius_m), Some(60.0));
        service.engine().restore_alarms(Default::default());
        restore(&service).await;
        assert!(service.engine().alarm_settings().anchor.is_some());

        let snap = raise_anchor(&service).await.expect("levée");
        assert!(snap.anchor.is_none());
        assert_eq!(snap.anchor_radius_m, 60.0);
        let history = events(&service, 10).await.expect("historique");
        let kinds: Vec<_> = history.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [EventKind::Disabled, EventKind::Enabled]);
    }

    #[tokio::test]
    async fn position_alarm_can_be_disabled_explicitly_only() {
        let service = test_service().await;
        assert!(set_enabled(&service, "anchor", false).await.is_err());
        assert!(set_enabled(&service, "inconnue", false).await.is_err());
        assert!(acknowledge(&service, "x").await.is_err());
        let snap = set_enabled(&service, "positionLost", false)
            .await
            .expect("désactivée");
        assert_eq!(snap.alarms[1].state, AlarmState::Off);
        assert!(
            !service
                .db
                .load_alarm_settings()
                .await
                .expect("lu")
                .position_lost_enabled
        );
        let history = events(&service, 10).await.expect("historique");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].kind, EventKind::Disabled);
        // Rien à acquitter : sans effet, sans événement.
        acknowledge(&service, "positionLost").await.expect("ok");
        assert_eq!(events(&service, 10).await.expect("h").len(), 1);
    }
}
