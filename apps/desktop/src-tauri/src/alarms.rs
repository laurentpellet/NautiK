//! Commandes d'alarme (mouillage, perte de position), persistance de
//! leur configuration et historique.
//!
//! L'évaluation des alarmes est faite par le moteur (`nav-alarms`) à
//! chaque pas de la boucle, indépendamment de l'interface ; cette couche
//! ne fait que relayer les commandes de l'utilisateur.

use chrono::Utc;
use nav_alarms::{AlarmEvent, AlarmKind, AlarmsSnapshot, MAX_DELAY_S};
use tauri::{AppHandle, Emitter, State};

use crate::routing::CommandResult;
use crate::runtime::AppState;

/// Événement portant un `AlarmsSnapshot`.
pub const ALARMS_EVENT: &str = "alarms";

/// Diffuse l'état des alarmes.
pub fn emit_alarms(app: &AppHandle, alarms: &AlarmsSnapshot) {
    if let Err(error) = app.emit(ALARMS_EVENT, alarms) {
        tracing::warn!(error = %error, "émission des alarmes impossible");
    }
}

/// Journalise et historise des événements d'alarme.
pub async fn record(state: &AppState, events: &[AlarmEvent]) {
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
    if let Err(error) = state.db.append_alarm_events(events).await {
        tracing::error!(%error, "événements d'alarme non historisés");
    }
}

/// Restaure la configuration enregistrée (démarrage). Une configuration
/// illisible est signalée et remplacée par les valeurs par défaut
/// (ancre levée, perte de position surveillée).
pub async fn restore(state: &AppState) {
    match state.db.load_alarm_settings().await {
        Ok(settings) => {
            if let Some(anchor) = settings.anchor {
                tracing::info!(
                    radius_m = anchor.radius_m,
                    "surveillance du mouillage reprise"
                );
            }
            state.engine().restore_alarms(settings);
        }
        Err(error) => tracing::error!(%error, "configuration des alarmes ignorée"),
    }
}

/// Persiste la configuration et l'événement d'une commande, puis
/// retourne l'état à jour.
async fn commit(state: &AppState, event: Option<AlarmEvent>) -> CommandResult<AlarmsSnapshot> {
    let settings = state.engine().alarm_settings();
    state
        .db
        .save_alarm_settings(&settings)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(event) = event {
        record(state, &[event]).await;
    }
    Ok(state.engine().alarms(Utc::now()))
}

fn parse_kind(kind: &str) -> CommandResult<AlarmKind> {
    AlarmKind::from_code(kind).ok_or_else(|| format!("alarme inconnue « {kind} »"))
}

pub(crate) async fn drop_anchor_impl(
    state: &AppState,
    radius_m: f64,
    delay_s: u32,
) -> CommandResult<AlarmsSnapshot> {
    if delay_s > MAX_DELAY_S {
        return Err(format!("temporisation invalide : 0 à {MAX_DELAY_S} s"));
    }
    let event = state
        .engine()
        .drop_anchor(radius_m, delay_s, Utc::now())
        .map_err(|e| e.to_string())?;
    commit(state, Some(event)).await
}

pub(crate) async fn raise_anchor_impl(state: &AppState) -> CommandResult<AlarmsSnapshot> {
    let event = state.engine().raise_anchor(Utc::now());
    commit(state, event).await
}

pub(crate) async fn acknowledge_impl(
    state: &AppState,
    kind: &str,
) -> CommandResult<AlarmsSnapshot> {
    let kind = parse_kind(kind)?;
    let event = state.engine().acknowledge_alarm(kind, Utc::now());
    if let Some(event) = event {
        record(state, &[event]).await;
    }
    Ok(state.engine().alarms(Utc::now()))
}

pub(crate) async fn set_enabled_impl(
    state: &AppState,
    kind: &str,
    enabled: bool,
) -> CommandResult<AlarmsSnapshot> {
    match parse_kind(kind)? {
        AlarmKind::PositionLost => {
            let event = state.engine().set_position_alarm(enabled, Utc::now());
            commit(state, event).await
        }
        AlarmKind::Anchor => {
            Err("le mouillage se surveille en posant ou en levant l'ancre".to_owned())
        }
    }
}

/// État des alarmes.
#[tauri::command]
pub async fn get_alarms(state: State<'_, AppState>) -> CommandResult<AlarmsSnapshot> {
    Ok(state.engine().alarms(Utc::now()))
}

/// Pose l'ancre à la position courante (qui doit être récente).
#[tauri::command]
pub async fn drop_anchor(
    app: AppHandle,
    state: State<'_, AppState>,
    radius_m: f64,
    delay_s: u32,
) -> CommandResult<AlarmsSnapshot> {
    let snapshot = drop_anchor_impl(&state, radius_m, delay_s).await?;
    emit_alarms(&app, &snapshot);
    Ok(snapshot)
}

/// Lève l'ancre (fin de la surveillance du mouillage).
#[tauri::command]
pub async fn raise_anchor(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<AlarmsSnapshot> {
    let snapshot = raise_anchor_impl(&state).await?;
    emit_alarms(&app, &snapshot);
    Ok(snapshot)
}

/// Acquitte une alarme (coupe le son, l'alarme reste affichée).
#[tauri::command]
pub async fn acknowledge_alarm(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
) -> CommandResult<AlarmsSnapshot> {
    let snapshot = acknowledge_impl(&state, &kind).await?;
    emit_alarms(&app, &snapshot);
    Ok(snapshot)
}

/// Active ou désactive une alarme (perte de position).
#[tauri::command]
pub async fn set_alarm_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    enabled: bool,
) -> CommandResult<AlarmsSnapshot> {
    let snapshot = set_enabled_impl(&state, &kind, enabled).await?;
    emit_alarms(&app, &snapshot);
    Ok(snapshot)
}

/// Derniers événements d'alarme (du plus récent au plus ancien).
#[tauri::command]
pub async fn list_alarm_events(
    state: State<'_, AppState>,
    limit: u32,
) -> CommandResult<Vec<AlarmEvent>> {
    state
        .db
        .list_alarm_events(limit.min(nav_storage::MAX_EVENTS_PAGE))
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::engine::Engine;
    use crate::secrets::MemoryStore;
    use nav_alarms::{AlarmState, EventKind};
    use nav_core::GeoPosition;
    use nav_storage::Db;

    async fn state() -> AppState {
        let db = Db::open_in_memory().await.expect("db");
        let start = GeoPosition::new(-14.968, -147.638).expect("pos");
        AppState::new(
            db,
            Engine::new(start, 4.0),
            Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn anchor_is_persisted_logged_and_restored() {
        let state = state().await;
        // Sans position : refus, rien d'enregistré.
        assert!(drop_anchor_impl(&state, 50.0, 10).await.is_err());
        state.engine().tick(Utc::now());
        assert!(drop_anchor_impl(&state, 5.0, 10).await.is_err());
        assert!(drop_anchor_impl(&state, 50.0, 301).await.is_err());
        let snap = drop_anchor_impl(&state, 60.0, 10).await.expect("ancre");
        assert_eq!(snap.anchor.map(|a| a.config.radius_m), Some(60.0));

        // Redémarrage : la surveillance est reprise depuis la base.
        let settings = state.db.load_alarm_settings().await.expect("lu");
        assert_eq!(settings.anchor.map(|a| a.radius_m), Some(60.0));
        state.engine().restore_alarms(Default::default());
        restore(&state).await;
        assert!(state.engine().alarm_settings().anchor.is_some());

        let snap = raise_anchor_impl(&state).await.expect("levée");
        assert!(snap.anchor.is_none());
        assert_eq!(snap.anchor_radius_m, 60.0);
        let events = state.db.list_alarm_events(10).await.expect("historique");
        let kinds: Vec<_> = events.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [EventKind::Disabled, EventKind::Enabled]);
    }

    #[tokio::test]
    async fn position_alarm_can_be_disabled_explicitly_only() {
        let state = state().await;
        assert!(set_enabled_impl(&state, "anchor", false).await.is_err());
        assert!(set_enabled_impl(&state, "inconnue", false).await.is_err());
        assert!(acknowledge_impl(&state, "x").await.is_err());
        let snap = set_enabled_impl(&state, "positionLost", false)
            .await
            .expect("désactivée");
        assert_eq!(snap.alarms[1].state, AlarmState::Off);
        assert!(
            !state
                .db
                .load_alarm_settings()
                .await
                .expect("lu")
                .position_lost_enabled
        );
        let events = state.db.list_alarm_events(10).await.expect("historique");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::Disabled);
        // Rien à acquitter : sans effet, sans événement.
        acknowledge_impl(&state, "positionLost").await.expect("ok");
        assert_eq!(state.db.list_alarm_events(10).await.expect("h").len(), 1);
    }
}
