//! Commandes Tauri exposées au frontend.
//!
//! Chaque commande valide ses entrées et délègue au moteur. Les
//! structures retournées sont typées et versionnées (`nav-core`).

use std::path::PathBuf;

use nav_core::EngineInfo;
use nav_engine::CommandResult;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

use crate::charts::{
    default_charts_dir, ChartCatalogView, ChartState, ChartsDirSource, CHARTS_DIR_ENV,
    CHARTS_DIR_SETTING,
};
use crate::AppState;

/// Retourne les informations d'identification du moteur.
#[tauri::command]
pub fn get_engine_info() -> EngineInfo {
    EngineInfo::current()
}

/// Retourne le catalogue des cartes (chargé à la première demande).
#[tauri::command]
pub async fn get_chart_catalog(state: State<'_, ChartState>) -> Result<ChartCatalogView, ()> {
    Ok(state.view().await)
}

/// Refuse de changer un dossier imposé par l'environnement.
fn ensure_editable(charts: &ChartState) -> CommandResult<()> {
    if charts.source() == ChartsDirSource::Environment {
        return Err(format!(
            "dossier des cartes imposé par la variable {CHARTS_DIR_ENV}"
        ));
    }
    Ok(())
}

/// Choisit le dossier des cartes par le dialogue du système (aucun
/// chemin ne vient du frontend), l'enregistre et relit le catalogue.
/// `None` si l'utilisateur annule.
#[tauri::command]
pub async fn choose_charts_dir(
    app: AppHandle,
    service: State<'_, AppState>,
    charts: State<'_, ChartState>,
) -> CommandResult<Option<ChartCatalogView>> {
    ensure_editable(&charts)?;
    let (tx, rx) = oneshot::channel();
    let mut dialog = app.dialog().file().set_title("Dossier des cartes MBTiles");
    if let Ok(documents) = app.path().document_dir() {
        dialog = dialog.set_directory(documents);
    }
    dialog.pick_folder(move |path| {
        let _ = tx.send(path);
    });
    let Some(picked) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let dir: PathBuf = picked.into_path().map_err(|e| e.to_string())?;
    if !dir.is_dir() {
        return Err(format!("dossier introuvable : {}", dir.display()));
    }
    let text = dir
        .to_str()
        .ok_or_else(|| format!("chemin non pris en charge : {}", dir.display()))?
        .to_owned();
    service
        .db
        .set_setting(CHARTS_DIR_SETTING, &Some(text))
        .await
        .map_err(|e| e.to_string())?;
    charts.set_dir(dir, ChartsDirSource::Settings);
    Ok(Some(charts.view().await))
}

/// Relit le dossier des cartes (fichiers ajoutés ou retirés).
#[tauri::command]
pub async fn reload_charts(charts: State<'_, ChartState>) -> Result<ChartCatalogView, ()> {
    charts.reload();
    Ok(charts.view().await)
}

/// Revient au dossier des cartes par défaut de l'application.
#[tauri::command]
pub async fn reset_charts_dir(
    app: AppHandle,
    service: State<'_, AppState>,
    charts: State<'_, ChartState>,
) -> CommandResult<ChartCatalogView> {
    ensure_editable(&charts)?;
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dir = default_charts_dir(&data_dir).map_err(|e| e.to_string())?;
    service
        .db
        .set_setting(CHARTS_DIR_SETTING, &None::<String>)
        .await
        .map_err(|e| e.to_string())?;
    charts.set_dir(dir, ChartsDirSource::Default);
    Ok(charts.view().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_engine_info_delegates_to_core() {
        assert_eq!(get_engine_info(), EngineInfo::current());
    }
}
