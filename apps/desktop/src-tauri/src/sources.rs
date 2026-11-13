//! Commandes de choix de la source de données : délèguent à
//! `nav_engine::sources`. Le dialogue de choix d'un enregistrement NMEA
//! reste propre au desktop.

use nav_engine::sources::{self, SourcesRequest, SourcesView};
use nav_engine::CommandResult;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

use crate::AppState;

/// Réglages et état de la source.
#[tauri::command]
pub async fn get_sources(state: State<'_, AppState>) -> CommandResult<SourcesView> {
    sources::get(&state).await
}

/// Modifie la source.
#[tauri::command]
pub async fn set_sources(
    state: State<'_, AppState>,
    request: SourcesRequest,
) -> CommandResult<SourcesView> {
    sources::set(&state, request).await
}

/// Ports série présents sur la machine.
#[tauri::command]
pub async fn list_serial_ports() -> CommandResult<Vec<String>> {
    sources::serial_ports().await
}

/// Choix d'un enregistrement NMEA à rejouer (dialogue du système). Le
/// chemin vient toujours du dialogue, jamais d'une saisie libre.
#[tauri::command]
pub async fn choose_nmea_recording(app: AppHandle) -> CommandResult<Option<String>> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .set_title("Rejouer un enregistrement NMEA 0183")
        .add_filter("NMEA 0183", &["nmea", "log", "txt"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}
