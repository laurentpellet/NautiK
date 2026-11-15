//! Commandes des marques personnelles : délèguent à `nav_engine::marks`.
//! Les fichiers KML sont choisis dans une boîte de dialogue native ouverte
//! côté Rust : le frontend ne transmet jamais de chemin.

use std::path::Path;

use chrono::Utc;
use nav_core::GeoPosition;
use nav_engine::marks::{self, MarksImportReport};
use nav_engine::CommandResult;
use nav_routing::Mark;
use tauri::{AppHandle, State};

use crate::gpx_io::{pick_file, save_file};
use crate::AppState;

/// Taille maximale d'un fichier KML importé.
pub const MAX_KML_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// Liste les marques.
#[tauri::command]
pub async fn list_marks(state: State<'_, AppState>) -> CommandResult<Vec<Mark>> {
    marks::list(&state).await
}

/// Crée (`id` absent) ou modifie une marque.
#[tauri::command]
pub async fn save_mark(
    state: State<'_, AppState>,
    id: Option<String>,
    name: String,
    position: GeoPosition,
    icon: String,
    note: Option<String>,
) -> CommandResult<Mark> {
    marks::save(
        &state,
        id.as_deref(),
        &name,
        position,
        &icon,
        note.as_deref(),
        Utc::now(),
    )
    .await
}

/// Supprime une marque.
#[tauri::command]
pub async fn delete_mark(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    marks::delete(&state, &id).await
}

/// Lit et importe un fichier KML (en une transaction).
pub(crate) async fn import_file(state: &AppState, path: &Path) -> CommandResult<MarksImportReport> {
    let size = tokio::fs::metadata(path)
        .await
        .map_err(|e| format!("fichier illisible : {e}"))?
        .len();
    if size > MAX_KML_FILE_BYTES {
        return Err(format!(
            "fichier trop volumineux ({} Mo, maximum {} Mo)",
            size / 1_048_576,
            MAX_KML_FILE_BYTES / 1_048_576
        ));
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("fichier illisible : {e}"))?;
    let xml = String::from_utf8(bytes).map_err(|_| "fichier KML non UTF-8".to_owned())?;
    marks::import_kml(state, &xml, Utc::now()).await
}

/// Importe les marques d'un fichier KML choisi par l'utilisateur. `None`
/// si annulé.
#[tauri::command]
pub async fn import_kml(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Option<MarksImportReport>> {
    let Some(path) = pick_file(&app, "Importer des marques (KML)", "KML", "kml").await? else {
        return Ok(None);
    };
    import_file(&state, &path).await.map(Some)
}

/// Exporte toutes les marques en KML. `false` si l'utilisateur annule.
#[tauri::command]
pub async fn export_marks_kml(app: AppHandle, state: State<'_, AppState>) -> CommandResult<bool> {
    let xml = marks::export_kml(&state).await?;
    let Some(path) = save_file(
        &app,
        "Exporter les marques",
        "marques-nautik.kml",
        "KML",
        "kml",
    )
    .await?
    else {
        return Ok(false);
    };
    tokio::fs::write(&path, xml)
        .await
        .map_err(|e| format!("écriture impossible : {e}"))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nav_engine::{Engine, MemoryStore, Service};
    use nav_storage::Db;

    async fn state() -> AppState {
        let db = Db::open_in_memory().await.expect("db");
        let start = GeoPosition::new(-14.968, -147.638).expect("pos");
        Service::new(
            db,
            Engine::new(start, 4.0),
            std::sync::Arc::new(MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn imports_valid_file_and_rejects_bad_ones() {
        let dir = tempfile::tempdir().expect("dir");
        let s = state().await;
        let good = dir.path().join("in.kml");
        tokio::fs::write(
            &good,
            r#"<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
<Placemark><name>Roche</name><Point><coordinates>-149.6,-17.5,0</coordinates></Point></Placemark>
</Document></kml>"#,
        )
        .await
        .expect("write");
        let report = import_file(&s, &good).await.expect("import");
        assert_eq!(report.imported, 1);

        let binary = dir.path().join("bin.kml");
        tokio::fs::write(&binary, [0xff_u8, 0xfe])
            .await
            .expect("write");
        assert!(import_file(&s, &binary).await.is_err());
        assert!(import_file(&s, &dir.path().join("absent.kml"))
            .await
            .is_err());
        assert_eq!(marks::list(&s).await.expect("liste").len(), 1);
    }
}
