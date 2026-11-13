//! Commandes d'alarme : délèguent à `nav_engine::alarms`, qui persiste,
//! historise et diffuse chaque changement.

use nav_alarms::{AlarmEvent, AlarmsSnapshot};
use nav_engine::{alarms, CommandResult};
use tauri::State;

use crate::AppState;

/// État des alarmes.
#[tauri::command]
pub async fn get_alarms(state: State<'_, AppState>) -> CommandResult<AlarmsSnapshot> {
    Ok(alarms::current(&state))
}

/// Pose l'ancre à la position courante (qui doit être récente).
#[tauri::command]
pub async fn drop_anchor(
    state: State<'_, AppState>,
    radius_m: f64,
    delay_s: u32,
) -> CommandResult<AlarmsSnapshot> {
    alarms::drop_anchor(&state, radius_m, delay_s).await
}

/// Lève l'ancre (fin de la surveillance du mouillage).
#[tauri::command]
pub async fn raise_anchor(state: State<'_, AppState>) -> CommandResult<AlarmsSnapshot> {
    alarms::raise_anchor(&state).await
}

/// Acquitte une alarme (coupe le son, l'alarme reste affichée).
#[tauri::command]
pub async fn acknowledge_alarm(
    state: State<'_, AppState>,
    kind: String,
) -> CommandResult<AlarmsSnapshot> {
    alarms::acknowledge(&state, &kind).await
}

/// Active ou désactive une alarme (perte de position).
#[tauri::command]
pub async fn set_alarm_enabled(
    state: State<'_, AppState>,
    kind: String,
    enabled: bool,
) -> CommandResult<AlarmsSnapshot> {
    alarms::set_enabled(&state, &kind, enabled).await
}

/// Derniers événements d'alarme (du plus récent au plus ancien).
#[tauri::command]
pub async fn list_alarm_events(
    state: State<'_, AppState>,
    limit: u32,
) -> CommandResult<Vec<AlarmEvent>> {
    alarms::events(&state, limit).await
}
