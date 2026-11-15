//! Commandes du journal de bord : délèguent à `nav_engine::logbook`.

use chrono::{DateTime, Utc};
use nav_engine::logbook::{self, LogbookSettings};
use nav_engine::CommandResult;
use nav_routing::LogEntry;
use tauri::{AppHandle, State};

use crate::gpx_io::save_file;
use crate::AppState;

/// Dernières entrées (de la plus récente à la plus ancienne).
#[tauri::command]
pub async fn list_log_entries(
    state: State<'_, AppState>,
    limit: u32,
    before: Option<DateTime<Utc>>,
) -> CommandResult<Vec<LogEntry>> {
    logbook::list(&state, limit, before).await
}

/// Ajoute une entrée manuelle avec l'état du bord.
#[tauri::command]
pub async fn add_log_entry(state: State<'_, AppState>, text: String) -> CommandResult<LogEntry> {
    logbook::add(&state, &text, Utc::now()).await
}

/// Supprime une entrée.
#[tauri::command]
pub async fn delete_log_entry(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    logbook::delete(&state, &id).await
}

/// Réglages du journal.
#[tauri::command]
pub fn get_logbook_settings(state: State<'_, AppState>) -> LogbookSettings {
    logbook::settings(&state)
}

/// Modifie l'intervalle des relevés automatiques.
#[tauri::command]
pub async fn set_logbook_settings(
    state: State<'_, AppState>,
    settings: LogbookSettings,
) -> CommandResult<LogbookSettings> {
    logbook::set_settings(&state, settings).await
}

/// Exporte le journal en CSV. `false` si l'utilisateur annule.
#[tauri::command]
pub async fn export_logbook_csv(app: AppHandle, state: State<'_, AppState>) -> CommandResult<bool> {
    let csv = logbook::export_csv(&state).await?;
    let Some(path) = save_file(
        &app,
        "Exporter le journal de bord",
        "journal-nautik.csv",
        "CSV",
        "csv",
    )
    .await?
    else {
        return Ok(false);
    };
    // BOM UTF-8 : accents lus correctement par les tableurs.
    tokio::fs::write(&path, format!("\u{feff}{csv}"))
        .await
        .map_err(|e| format!("écriture impossible : {e}"))?;
    Ok(true)
}
