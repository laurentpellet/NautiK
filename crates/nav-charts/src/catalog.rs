//! Catalogue des cartes disponibles et service de tuiles.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ChartError;
use crate::mbtiles::MbTiles;
use crate::metadata::{Bounds, Center};
use crate::tile::{parse_tile_path, ChartId, TileFormat, TileRequestError};

/// Extension des fichiers de cartes reconnus.
const MBTILES_EXTENSION: &str = "mbtiles";

/// Description d'une carte, exposée aux clients (Tauri, API distante).
///
/// Ne contient pas le chemin complet du fichier, pour ne pas exposer
/// l'arborescence locale aux clients distants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartInfo {
    /// Identifiant utilisé dans les URL de tuiles.
    pub id: ChartId,
    /// Nom affiché.
    pub name: String,
    /// Nom du fichier.
    pub file_name: String,
    /// Description.
    pub description: Option<String>,
    /// Format des tuiles.
    pub format: TileFormat,
    /// Zoom minimal.
    pub min_zoom: u8,
    /// Zoom maximal.
    pub max_zoom: u8,
    /// Emprise.
    pub bounds: Option<Bounds>,
    /// Centre suggéré.
    pub center: Option<Center>,
    /// Attribution (texte brut).
    pub attribution: Option<String>,
    /// Anomalies non bloquantes.
    pub warnings: Vec<String>,
}

/// Carte qui n'a pas pu être chargée. Signalée à l'utilisateur plutôt
/// que masquée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartLoadFailure {
    /// Nom du fichier.
    pub file_name: String,
    /// Raison de l'échec.
    pub reason: String,
}

/// Réponse à une requête de tuile, indépendante du transport (protocole
/// Tauri, HTTP Axum).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileReply {
    /// Code de statut HTTP.
    pub status: u16,
    /// Type MIME du corps, si présent.
    pub content_type: Option<&'static str>,
    /// Corps de la réponse.
    pub body: Vec<u8>,
}

impl TileReply {
    fn empty(status: u16) -> Self {
        Self {
            status,
            content_type: None,
            body: Vec::new(),
        }
    }
}

struct LoadedChart {
    info: ChartInfo,
    mbtiles: MbTiles,
}

/// Catalogue des cartes chargées depuis un répertoire.
#[derive(Default)]
pub struct ChartCatalog {
    charts: Vec<LoadedChart>,
    failures: Vec<ChartLoadFailure>,
}

impl ChartCatalog {
    /// Charge toutes les cartes `.mbtiles` d'un répertoire (non récursif).
    ///
    /// Une carte invalide n'empêche pas le chargement des autres : elle
    /// est consignée dans [`ChartCatalog::failures`].
    ///
    /// Les cartes sont triées par zoom maximal croissant puis par nom :
    /// les cartes les plus détaillées sont dessinées au-dessus.
    ///
    /// # Errors
    ///
    /// Retourne [`ChartError::DirectoryUnavailable`] si le répertoire ne
    /// peut pas être lu.
    pub async fn load_dir(dir: &Path) -> Result<Self, ChartError> {
        let unavailable = |source| ChartError::DirectoryUnavailable {
            path: dir.to_owned(),
            source,
        };
        let mut entries = tokio::fs::read_dir(dir).await.map_err(unavailable)?;
        let mut paths: Vec<PathBuf> = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(unavailable)? {
            let path = entry.path();
            let is_mbtiles = path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case(MBTILES_EXTENSION));
            if is_mbtiles && entry.file_type().await.is_ok_and(|t| t.is_file()) {
                paths.push(path);
            }
        }
        paths.sort();

        let mut catalog = Self::default();
        let mut used_ids: HashSet<ChartId> = HashSet::new();
        for path in paths {
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            match MbTiles::open(&path).await {
                Ok(mbtiles) => {
                    let stem = path
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let id = unique_id(ChartId::from_file_stem(&stem), &used_ids);
                    used_ids.insert(id.clone());
                    let meta = mbtiles.metadata().clone();
                    tracing::info!(chart = %id, file = %file_name, "carte chargée");
                    catalog.charts.push(LoadedChart {
                        info: ChartInfo {
                            id,
                            name: meta.name,
                            file_name,
                            description: meta.description,
                            format: meta.format,
                            min_zoom: meta.min_zoom,
                            max_zoom: meta.max_zoom,
                            bounds: meta.bounds,
                            center: meta.center,
                            attribution: meta.attribution,
                            warnings: meta.warnings,
                        },
                        mbtiles,
                    });
                }
                Err(error) => {
                    tracing::warn!(file = %file_name, error = %error, "carte ignorée");
                    catalog.failures.push(ChartLoadFailure {
                        file_name,
                        reason: error_chain(&error),
                    });
                }
            }
        }
        catalog
            .charts
            .sort_by(|a, b| (a.info.max_zoom, &a.info.name).cmp(&(b.info.max_zoom, &b.info.name)));
        Ok(catalog)
    }

    /// Cartes chargées, dans l'ordre d'affichage (dessous → dessus).
    pub fn charts(&self) -> impl Iterator<Item = &ChartInfo> {
        self.charts.iter().map(|chart| &chart.info)
    }

    /// Fichiers qui n'ont pas pu être chargés.
    #[must_use]
    pub fn failures(&self) -> &[ChartLoadFailure] {
        &self.failures
    }

    /// Répond à une requête de tuile `/{chart_id}/{z}/{x}/{y}`.
    ///
    /// Statuts : 200 (tuile), 204 (tuile absente dans une carte connue,
    /// affichée vide par MapLibre), 400 (requête invalide), 404 (carte
    /// inconnue), 500 (erreur de lecture).
    pub async fn serve_tile(&self, path: &str) -> TileReply {
        let (id, coord) = match parse_tile_path(path) {
            Ok(request) => request,
            Err(TileRequestError::InvalidChartId) => return TileReply::empty(404),
            Err(error) => {
                tracing::debug!(path, error = %error, "requête de tuile rejetée");
                return TileReply::empty(400);
            }
        };
        let Some(chart) = self.charts.iter().find(|chart| chart.info.id == id) else {
            return TileReply::empty(404);
        };
        match chart.mbtiles.tile(coord).await {
            Ok(Some(tile)) => TileReply {
                status: 200,
                content_type: Some(tile.format.mime_type()),
                body: tile.data,
            },
            Ok(None) => TileReply::empty(204),
            Err(error) => {
                tracing::error!(chart = %id, error = %error_chain(&error), "lecture de tuile échouée");
                TileReply::empty(500)
            }
        }
    }

    /// Ferme toutes les cartes.
    pub async fn close(&self) {
        for chart in &self.charts {
            chart.mbtiles.close().await;
        }
    }
}

fn unique_id(base: ChartId, used: &HashSet<ChartId>) -> ChartId {
    if !used.contains(&base) {
        return base;
    }
    (2..)
        .map(|n| base.with_suffix(n))
        .find(|candidate| !used.contains(candidate))
        .unwrap_or(base)
}

fn error_chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(" : ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_id_adds_suffix_on_collision() {
        let base = ChartId::parse("tahiti").expect("valide");
        let mut used = HashSet::new();
        assert_eq!(unique_id(base.clone(), &used), base);
        used.insert(base.clone());
        assert_eq!(unique_id(base.clone(), &used).as_str(), "tahiti-2");
        used.insert(base.with_suffix(2));
        assert_eq!(unique_id(base, &used).as_str(), "tahiti-3");
    }
}
