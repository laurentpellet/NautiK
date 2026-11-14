//! Dossier des cartes ENC S-57 (Paramètres → Cartes).
//!
//! Le dossier est choisi par le dialogue du système (le frontend ne donne
//! jamais de chemin absolu) et mémorisé dans la base. Son contenu est
//! inventorié par `nav-charts`. Une cellule de l'inventaire peut être
//! décodée pour en voir le résumé : le frontend ne transmet que son chemin
//! relatif, vérifié ici (il ne peut pas sortir du dossier). Les cellules
//! sont aussi servies pour la carte : index des emprises, puis entités
//! d'affichage d'une cellule (cache `EncLibrary`). Un dossier disparu (clé
//! USB retirée) est signalé, jamais oublié en silence.

use std::path::{Path, PathBuf};

use nav_charts::{
    decode_cell_file, resolve_cell_path, scan_enc_dir, Cm93Library, Cm93View, EncInventory,
    EncLibrary, EncMapIndex, S57CellSummary,
};
use nav_engine::CommandResult;
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

use crate::AppState;

/// Réglage (base) du dossier ENC choisi par l'utilisateur.
pub const ENC_DIR_SETTING: &str = "enc_dir";

/// État du dossier ENC transmis au frontend.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncFolderView {
    /// Dossier choisi ; `None` tant qu'aucun n'a été choisi.
    pub dir: Option<String>,
    /// Inventaire du dossier, s'il est lisible.
    pub inventory: Option<EncInventory>,
    /// Dossier illisible ou absent, le cas échéant.
    pub error: Option<String>,
}

/// Inventorie le dossier choisi.
async fn folder_view(dir: Option<String>) -> EncFolderView {
    let Some(text) = dir else {
        return EncFolderView {
            dir: None,
            inventory: None,
            error: None,
        };
    };
    match scan_enc_dir(PathBuf::from(&text)).await {
        Ok(inventory) => EncFolderView {
            dir: Some(text),
            inventory: Some(inventory),
            error: None,
        },
        Err(error) => {
            tracing::warn!(dir = %text, %error, "dossier ENC illisible");
            EncFolderView {
                dir: Some(text),
                inventory: None,
                error: Some(error.to_string()),
            }
        }
    }
}

async fn stored_dir(service: &AppState) -> CommandResult<Option<String>> {
    service
        .db
        .get_setting::<Option<String>>(ENC_DIR_SETTING)
        .await
        .map(Option::flatten)
        .map_err(|e| e.to_string())
}

/// Dossier ENC mémorisé et son inventaire (relu à chaque appel).
#[tauri::command]
pub async fn get_enc_folder(service: State<'_, AppState>) -> CommandResult<EncFolderView> {
    Ok(folder_view(stored_dir(&service).await?).await)
}

/// Choisit le dossier ENC par le dialogue du système et l'enregistre.
/// `None` si l'utilisateur annule.
#[tauri::command]
pub async fn choose_enc_dir(
    app: AppHandle,
    service: State<'_, AppState>,
) -> CommandResult<Option<EncFolderView>> {
    let (tx, rx) = oneshot::channel();
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Dossier des cartes ENC (S-57)");
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
        .set_setting(ENC_DIR_SETTING, &Some(text.clone()))
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!(dir = %text, "dossier ENC");
    Ok(Some(folder_view(Some(text)).await))
}

/// Oublie le dossier ENC (les fichiers ne sont pas touchés).
#[tauri::command]
pub async fn clear_enc_dir(service: State<'_, AppState>) -> CommandResult<EncFolderView> {
    service
        .db
        .set_setting(ENC_DIR_SETTING, &None::<String>)
        .await
        .map_err(|e| e.to_string())?;
    Ok(folder_view(None).await)
}

/// Lit une cellule de l'inventaire et la résume.
async fn inspect(dir: Option<String>, relative: &str) -> Result<S57CellSummary, String> {
    let dir = dir.ok_or_else(|| "aucun dossier ENC choisi".to_owned())?;
    let path = resolve_cell_path(Path::new(&dir), relative).map_err(|e| e.to_string())?;
    match decode_cell_file(path).await {
        Ok(cell) => Ok(cell.summary()),
        Err(error) => {
            tracing::warn!(cell = %relative, %error, "cellule ENC illisible");
            Err(error.to_string())
        }
    }
}

/// Décode une cellule du dossier ENC (chemin relatif donné par
/// l'inventaire, vérifié côté moteur) et en renvoie le résumé.
#[tauri::command]
pub async fn inspect_enc_cell(
    service: State<'_, AppState>,
    path: String,
) -> CommandResult<S57CellSummary> {
    inspect(stored_dir(&service).await?, &path).await
}

/// Index cartographique du dossier ENC.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncMapIndexView {
    /// Dossier choisi ; `None` tant qu'aucun n'a été choisi.
    pub dir: Option<String>,
    /// Cellules affichables (emprise, usage, état des mises à jour).
    pub index: Option<EncMapIndex>,
    /// Dossier illisible ou absent.
    pub error: Option<String>,
    /// Base CM93 trouvée dans le dossier (chemin relatif, `.` : le
    /// dossier lui-même).
    pub cm93: Option<String>,
    /// Base CM93 illisible.
    pub cm93_error: Option<String>,
}

async fn map_index(
    dir: Option<String>,
    library: &EncLibrary,
    cm93: &Cm93Library,
) -> EncMapIndexView {
    let Some(text) = dir else {
        return EncMapIndexView {
            dir: None,
            index: None,
            error: None,
            cm93: None,
            cm93_error: None,
        };
    };
    let root = PathBuf::from(&text);
    match scan_enc_dir(root.clone()).await {
        Ok(inventory) => {
            // Base CM93 relue à chaque relecture du dossier.
            let (cm93, cm93_error) = match cm93.refresh(&root).await {
                Ok(base) => (base, None),
                Err(error) => {
                    tracing::warn!(dir = %text, %error, "base CM93 illisible");
                    (None, Some(error.to_string()))
                }
            };
            EncMapIndexView {
                index: Some(library.index(&root, &inventory).await),
                dir: Some(text),
                error: None,
                cm93,
                cm93_error,
            }
        }
        Err(error) => EncMapIndexView {
            dir: Some(text),
            index: None,
            error: Some(error.to_string()),
            cm93: None,
            cm93_error: None,
        },
    }
}

/// Index cartographique des cellules du dossier ENC (décodées une fois,
/// puis servies depuis le cache tant que leurs fichiers ne changent pas).
/// Relit aussi l'index de la base CM93 du dossier, s'il y en a une.
#[tauri::command]
pub async fn get_enc_map_index(
    service: State<'_, AppState>,
    library: State<'_, EncLibrary>,
    cm93: State<'_, Cm93Library>,
) -> CommandResult<EncMapIndexView> {
    Ok(map_index(stored_dir(&service).await?, &library, &cm93).await)
}

/// Cellules CM93 à dessiner pour la vue `[ouest, sud, est, nord]`
/// (vérifiée côté moteur) ; vide sans dossier ou sans base CM93.
#[tauri::command]
pub async fn get_cm93_view(
    service: State<'_, AppState>,
    cm93: State<'_, Cm93Library>,
    view: [f64; 4],
) -> CommandResult<Cm93View> {
    let Some(dir) = stored_dir(&service).await? else {
        return Ok(Cm93View::default());
    };
    cm93.view(Path::new(&dir), view)
        .await
        .map_err(|e| e.to_string())
}

/// Entités d'une cellule CM93 de l'index (chemin vérifié côté moteur) :
/// objet JSON `{ features, dataDate, declaredOffsetM }`.
#[tauri::command]
pub async fn get_cm93_cell_features(
    service: State<'_, AppState>,
    cm93: State<'_, Cm93Library>,
    path: String,
) -> CommandResult<Response> {
    let dir = stored_dir(&service)
        .await?
        .ok_or_else(|| "aucun dossier ENC choisi".to_owned())?;
    let json = cm93
        .features(Path::new(&dir), &path)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Response::new(json.as_str().to_owned()))
}

/// Entités GeoJSON d'une cellule (chemin relatif de l'index, vérifié
/// ici) ; une cellule annulée est refusée.
#[tauri::command]
pub async fn get_enc_cell_features(
    service: State<'_, AppState>,
    library: State<'_, EncLibrary>,
    path: String,
) -> CommandResult<Response> {
    let dir = stored_dir(&service)
        .await?
        .ok_or_else(|| "aucun dossier ENC choisi".to_owned())?;
    let json = library
        .features(Path::new(&dir), &path)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Response::new(json.as_str().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn no_folder_chosen_yet() {
        let view = folder_view(None).await;
        assert!(view.dir.is_none() && view.inventory.is_none() && view.error.is_none());
    }

    #[tokio::test]
    async fn chosen_folder_is_inventoried() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        std::fs::write(dir.path().join("FR501130.000"), b"x").expect("cellule");
        let text = dir.path().to_str().expect("chemin").to_owned();
        let view = folder_view(Some(text.clone())).await;
        assert_eq!(view.dir.as_deref(), Some(text.as_str()));
        assert_eq!(view.inventory.expect("inventaire").cells.len(), 1);
        assert!(view.error.is_none());
    }

    #[tokio::test]
    async fn missing_folder_is_reported_not_forgotten() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let gone = dir.path().join("cle-usb-retiree");
        let text = gone.to_str().expect("chemin").to_owned();
        let view = folder_view(Some(text.clone())).await;
        assert_eq!(view.dir.as_deref(), Some(text.as_str()));
        assert!(view.inventory.is_none());
        assert!(view.error.is_some());
    }

    #[test]
    fn view_uses_camel_case() {
        let json = serde_json::to_value(EncFolderView {
            dir: None,
            inventory: Some(EncInventory::default()),
            error: None,
        })
        .expect("JSON");
        assert_eq!(json["inventory"]["unreadableDirs"], 0);
        assert_eq!(json["inventory"]["truncated"], false);
    }
    #[tokio::test]
    async fn inspect_requires_a_folder_and_a_safe_path() {
        assert!(inspect(None, "A.000").await.is_err());
        let dir = tempfile::tempdir().expect("dossier temporaire");
        std::fs::write(dir.path().join("BAD.000"), b"pas une cellule").expect("cellule");
        std::fs::write(dir.path().join("BAD.001"), b"x").expect("mise a jour");
        let text = Some(dir.path().to_str().expect("chemin").to_owned());
        let error = inspect(text.clone(), "BAD.000").await.unwrap_err();
        assert!(error.contains("S-57"), "{error}");
        for refused in ["../BAD.000", "BAD.001", "ABSENT.000"] {
            let error = inspect(text.clone(), refused).await.unwrap_err();
            assert!(error.contains("refusée"), "{error}");
        }
    }
    #[tokio::test]
    async fn map_index_without_folder_or_with_missing_folder() {
        let library = EncLibrary::new();
        let cm93 = Cm93Library::new();
        let view = map_index(None, &library, &cm93).await;
        assert!(view.dir.is_none() && view.index.is_none() && view.error.is_none());
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let gone = dir
            .path()
            .join("absent")
            .to_str()
            .expect("chemin")
            .to_owned();
        let view = map_index(Some(gone), &library, &cm93).await;
        assert!(view.index.is_none());
        assert!(view.error.is_some());
        let text = dir.path().to_str().expect("chemin").to_owned();
        std::fs::write(dir.path().join("BAD.000"), b"x").expect("cellule");
        let view = map_index(Some(text), &library, &cm93).await;
        assert!(view.cm93.is_none() && view.cm93_error.is_none());
        let index = view.index.expect("index");
        assert_eq!(index.cells.len(), 1);
        assert!(index.cells[0].error.is_some());
        let json = serde_json::to_value(&index).expect("JSON");
        assert_eq!(json["cells"][0]["name"], "BAD");
        assert!(json["cells"][0].get("updateError").is_some());
    }
}
