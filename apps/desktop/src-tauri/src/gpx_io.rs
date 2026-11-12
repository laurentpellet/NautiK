//! Import et export GPX.
//!
//! Les fichiers sont choisis par l'utilisateur dans une boîte de dialogue
//! native ouverte côté Rust : le frontend ne transmet jamais de chemin,
//! ce qui évite d'exposer une commande d'accès arbitraire au disque.

use std::path::{Path, PathBuf};

use chrono::Utc;
use nav_routing::gpx;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tokio::sync::oneshot;

use crate::routing::{CommandResult, SIMULATED_TRACK_PREFIX};
use crate::runtime::AppState;

/// Taille maximale d'un fichier GPX importé.
pub const MAX_GPX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Bilan d'un import GPX.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    /// Waypoints isolés importés.
    pub waypoints: usize,
    /// Routes importées.
    pub routes: usize,
    /// Traces importées.
    pub tracks: usize,
    /// Éléments écartés et raisons.
    pub warnings: Vec<String>,
}

fn file_path(path: FilePath) -> CommandResult<PathBuf> {
    path.into_path()
        .map_err(|_| "chemin de fichier non pris en charge".to_owned())
}

/// Nom de fichier sûr dérivé d'un nom d'élément.
#[must_use]
pub fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        "nautik.gpx".to_owned()
    } else {
        format!("{cleaned}.gpx")
    }
}

/// Lit, analyse et enregistre un fichier GPX (en une transaction).
pub(crate) async fn import_file(state: &AppState, path: &Path) -> CommandResult<ImportReport> {
    let size = tokio::fs::metadata(path)
        .await
        .map_err(|e| format!("fichier illisible : {e}"))?
        .len();
    if size > MAX_GPX_FILE_BYTES {
        return Err(format!(
            "fichier trop volumineux ({} Mo, maximum {} Mo)",
            size / 1_048_576,
            MAX_GPX_FILE_BYTES / 1_048_576
        ));
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("fichier illisible : {e}"))?;
    let xml = String::from_utf8(bytes).map_err(|_| "fichier GPX non UTF-8".to_owned())?;
    let import = tauri::async_runtime::spawn_blocking(move || {
        gpx::parse(&xml).map(|doc| doc.into_models(Utc::now()))
    })
    .await
    .map_err(|e| format!("analyse interrompue : {e}"))?
    .map_err(|e| format!("GPX invalide : {e}"))?;

    state
        .db
        .save_all(&import.waypoints, &import.routes, &import.tracks)
        .await
        .map_err(|e| e.to_string())?;
    let report = ImportReport {
        waypoints: import.waypoints.len(),
        routes: import.routes.len(),
        tracks: import.tracks.len(),
        warnings: import.warnings,
    };
    tracing::info!(
        waypoints = report.waypoints,
        routes = report.routes,
        tracks = report.tracks,
        warnings = report.warnings.len(),
        "import GPX"
    );
    Ok(report)
}

/// Importe un fichier GPX choisi par l'utilisateur. `None` si annulé.
#[tauri::command]
pub async fn import_gpx(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Option<ImportReport>> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .set_title("Importer un fichier GPX")
        .add_filter("GPX", &["gpx"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = rx.await.map_err(|_| "dialogue interrompu".to_owned())? else {
        return Ok(None);
    };
    import_file(&state, &file_path(path)?).await.map(Some)
}

async fn save_dialog(app: &AppHandle, title: &str, name: &str) -> CommandResult<Option<PathBuf>> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .set_title(title)
        .set_file_name(safe_file_name(name))
        .add_filter("GPX", &["gpx"])
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    match rx.await.map_err(|_| "dialogue interrompu".to_owned())? {
        Some(path) => file_path(path).map(Some),
        None => Ok(None),
    }
}

async fn write_gpx(path: &Path, xml: String) -> CommandResult<()> {
    tokio::fs::write(path, xml)
        .await
        .map_err(|e| format!("écriture impossible : {e}"))
}

/// Exporte une route en GPX. Retourne `false` si l'utilisateur annule.
#[tauri::command]
pub async fn export_route_gpx(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<bool> {
    let id = uuid::Uuid::parse_str(&id).map_err(|_| "identifiant invalide".to_owned())?;
    let route = state
        .db
        .get_route(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("route introuvable")?;
    let Some(path) = save_dialog(&app, "Exporter la route", &route.name).await? else {
        return Ok(false);
    };
    write_gpx(&path, gpx::write(&[], std::slice::from_ref(&route), &[])).await?;
    Ok(true)
}

/// Exporte une trace en GPX. Retourne `false` si l'utilisateur annule.
#[tauri::command]
pub async fn export_track_gpx(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<bool> {
    let id = uuid::Uuid::parse_str(&id).map_err(|_| "identifiant invalide".to_owned())?;
    let track = state
        .db
        .get_track(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("trace introuvable")?;
    let name = track
        .name
        .trim_start_matches(SIMULATED_TRACK_PREFIX)
        .to_owned();
    let Some(path) = save_dialog(&app, "Exporter la trace", &name).await? else {
        return Ok(false);
    };
    write_gpx(&path, gpx::write(&[], &[], std::slice::from_ref(&track))).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use nav_core::GeoPosition;
    use nav_storage::Db;

    async fn state() -> AppState {
        let db = Db::open_in_memory().await.expect("db");
        let start = GeoPosition::new(-14.968, -147.638).expect("pos");
        AppState::new(
            db,
            Engine::new(start, 4.0),
            std::sync::Arc::new(crate::secrets::MemoryStore::default()),
        )
    }

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<gpx version="1.1" creator="test">
  <wpt lat="-14.97" lon="-147.64"><name>Mouillage</name></wpt>
  <rte><name>Tour</name>
    <rtept lat="-14.968" lon="-147.638"/><rtept lat="-14.9" lon="-147.6"/>
  </rte>
  <rte><name>Courte</name><rtept lat="1" lon="1"/></rte>
  <trk><name>Sortie</name><trkseg>
    <trkpt lat="-14.968" lon="-147.638"><time>2026-11-12T10:00:00Z</time></trkpt>
  </trkseg></trk>
</gpx>"#;

    #[tokio::test]
    async fn imports_valid_file() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("in.gpx");
        tokio::fs::write(&path, SAMPLE).await.expect("write");
        let s = state().await;
        let report = import_file(&s, &path).await.expect("import");
        assert_eq!((report.waypoints, report.routes, report.tracks), (1, 1, 1));
        assert_eq!(report.warnings.len(), 1, "route trop courte signalée");
        assert_eq!(s.db.list_routes().await.expect("routes").len(), 1);
        assert_eq!(s.db.list_tracks().await.expect("tracks").len(), 1);
    }

    #[tokio::test]
    async fn rejects_invalid_files_without_writing() {
        let dir = tempfile::tempdir().expect("dir");
        let s = state().await;

        let bad_coord = dir.path().join("bad.gpx");
        tokio::fs::write(
            &bad_coord,
            r#"<gpx><wpt lat="1" lon="1"/><wpt lat="99" lon="1"/></gpx>"#,
        )
        .await
        .expect("write");
        let err = import_file(&s, &bad_coord).await.expect_err("invalide");
        assert!(err.contains("GPX invalide"), "{err}");

        let binary = dir.path().join("bin.gpx");
        tokio::fs::write(&binary, [0xff_u8, 0xfe, 0x00])
            .await
            .expect("write");
        assert!(import_file(&s, &binary).await.is_err());

        assert!(import_file(&s, &dir.path().join("absent.gpx"))
            .await
            .is_err());
        assert!(s.db.list_waypoints().await.expect("wps").is_empty());
    }

    #[test]
    fn file_names_are_sanitized() {
        assert_eq!(
            safe_file_name("Rangiroa → Tikehau"),
            "Rangiroa _ Tikehau.gpx"
        );
        assert_eq!(safe_file_name("../../etc/passwd"), "_.._etc_passwd.gpx");
        assert_eq!(safe_file_name("..."), "nautik.gpx");
        assert_eq!(safe_file_name(""), "nautik.gpx");
        assert!(safe_file_name(&"x".repeat(500)).len() <= 84);
    }
}
