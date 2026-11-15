//! Îles et amers nautiques (OpenStreetMap) : régions proposées,
//! téléchargement à la demande, enregistrement dans la base et lecture
//! hors ligne.
//!
//! Aucun accès réseau sans action de l'utilisateur ; le frontend ne
//! transmet qu'un identifiant de région, validé ici.

use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use nav_engine::CommandResult;
use nav_poi::{BBox, Poi};
use serde::Serialize;
use tauri::State;

use crate::AppState;

/// Un seul téléchargement à la fois (service public partagé).
static DOWNLOADING: AtomicBool = AtomicBool::new(false);

/// Région proposée et état de son téléchargement.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoiRegionView {
    /// Identifiant.
    pub id: &'static str,
    /// Nom.
    pub name: &'static str,
    /// Emprise.
    pub boxes: &'static [BBox],
    /// Date du dernier téléchargement.
    pub fetched_at: Option<DateTime<Utc>>,
    /// Nombre de points enregistrés.
    pub poi_count: u64,
}

/// Points d'intérêt enregistrés et attribution à afficher.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoiCollectionView {
    /// Points.
    pub pois: Vec<Poi>,
    /// Attribution (licence des données).
    pub attribution: &'static str,
}

async fn regions_view(state: &AppState) -> CommandResult<Vec<PoiRegionView>> {
    let stored = state
        .db
        .list_poi_regions()
        .await
        .map_err(|e| e.to_string())?;
    Ok(nav_poi::REGIONS
        .iter()
        .map(|r| {
            let status = stored.iter().find(|s| s.id == r.id);
            PoiRegionView {
                id: r.id,
                name: r.name,
                boxes: r.boxes,
                fetched_at: status.map(|s| s.fetched_at),
                poi_count: status.map_or(0, |s| s.poi_count),
            }
        })
        .collect())
}

/// Régions proposées et état de leur téléchargement.
#[tauri::command]
pub async fn list_poi_regions(state: State<'_, AppState>) -> CommandResult<Vec<PoiRegionView>> {
    regions_view(&state).await
}

/// Libère le verrou de téléchargement en fin de portée.
struct DownloadGuard;

impl Drop for DownloadGuard {
    fn drop(&mut self) {
        DOWNLOADING.store(false, Ordering::SeqCst);
    }
}

/// Télécharge (ou met à jour) les îles et amers d'une région.
#[tauri::command]
pub async fn download_pois(
    state: State<'_, AppState>,
    region_id: String,
) -> CommandResult<Vec<PoiRegionView>> {
    let region = nav_poi::region(&region_id)
        .ok_or_else(|| nav_poi::PoiError::UnknownRegion(region_id.clone()).to_string())?;
    if DOWNLOADING.swap(true, Ordering::SeqCst) {
        return Err("un téléchargement est déjà en cours".to_owned());
    }
    let _guard = DownloadGuard;
    let parsed = nav_poi::download_region(region, nav_poi::DEFAULT_ENDPOINT)
        .await
        .map_err(|e| e.to_string())?;
    state
        .db
        .replace_region_pois(region.id, Utc::now(), "overpass-api.de", &parsed.pois)
        .await
        .map_err(|e| e.to_string())?;
    regions_view(&state).await
}

/// Supprime les points d'une région.
#[tauri::command]
pub async fn delete_pois(
    state: State<'_, AppState>,
    region_id: String,
) -> CommandResult<Vec<PoiRegionView>> {
    if nav_poi::region(&region_id).is_none() {
        return Err(nav_poi::PoiError::UnknownRegion(region_id).to_string());
    }
    state
        .db
        .delete_poi_region(&region_id)
        .await
        .map_err(|e| e.to_string())?;
    regions_view(&state).await
}

/// Points enregistrés, pour la carte (hors ligne).
#[tauri::command]
pub async fn get_pois(state: State<'_, AppState>) -> CommandResult<PoiCollectionView> {
    Ok(PoiCollectionView {
        pois: state.db.list_pois().await.map_err(|e| e.to_string())?,
        attribution: nav_poi::ATTRIBUTION,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_view_serializes_camel_case() {
        let region = nav_poi::region("fidji").expect("région");
        let view = PoiRegionView {
            id: region.id,
            name: region.name,
            boxes: region.boxes,
            fetched_at: None,
            poi_count: 0,
        };
        let json = serde_json::to_value(view).expect("json");
        assert_eq!(json["boxes"].as_array().map(Vec::len), Some(2));
        assert!(json.get("fetchedAt").is_some());
        assert_eq!(json["poiCount"], 0);
    }

    #[test]
    fn guard_releases_the_download_lock() {
        DOWNLOADING.store(true, Ordering::SeqCst);
        drop(DownloadGuard);
        assert!(!DOWNLOADING.load(Ordering::SeqCst));
    }
}
