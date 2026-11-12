//! Adaptation du catalogue de cartes (`nav-charts`) à Tauri : état
//! partagé, protocole local de tuiles et répertoire des cartes.

use std::path::{Path, PathBuf};

use nav_charts::{ChartCatalog, ChartInfo, ChartLoadFailure, TileReply};
use serde::Serialize;
use tauri::http::{header, Response, StatusCode};
use tokio::sync::OnceCell;

/// Nom du protocole local servant les tuiles.
pub const TILE_SCHEME: &str = "nautik-tiles";

/// URL de base des tuiles vue par la WebView. Sous Windows et Android,
/// les protocoles personnalisés sont exposés via `http://<schéma>.localhost`.
#[cfg(any(windows, target_os = "android"))]
pub const TILE_URL_BASE: &str = "http://nautik-tiles.localhost";
/// URL de base des tuiles vue par la WebView.
#[cfg(not(any(windows, target_os = "android")))]
pub const TILE_URL_BASE: &str = "nautik-tiles://localhost";

/// Variable d'environnement permettant de choisir le répertoire des cartes.
pub const CHARTS_DIR_ENV: &str = "NAUTIK_CHARTS_DIR";

/// Résultat du chargement du catalogue (l'erreur est conservée pour être
/// affichée, jamais masquée).
type CatalogLoad = Result<ChartCatalog, String>;

/// État partagé du catalogue, chargé à la première utilisation.
pub struct ChartState {
    dir: PathBuf,
    catalog: OnceCell<CatalogLoad>,
}

/// Vue du catalogue transmise au frontend.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartCatalogView {
    /// Répertoire des cartes (affiché à l'utilisateur local).
    pub charts_dir: String,
    /// URL de base des tuiles : `{base}/{id}/{z}/{x}/{y}`.
    pub tile_url_base: &'static str,
    /// Cartes chargées, dans l'ordre d'affichage.
    pub charts: Vec<ChartInfo>,
    /// Fichiers rejetés.
    pub failures: Vec<ChartLoadFailure>,
    /// Erreur de chargement du répertoire, le cas échéant.
    pub error: Option<String>,
}

impl ChartState {
    /// Crée l'état pour un répertoire de cartes.
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            catalog: OnceCell::new(),
        }
    }

    /// Catalogue, chargé à la première demande.
    pub async fn catalog(&self) -> &CatalogLoad {
        self.catalog
            .get_or_init(|| async {
                ChartCatalog::load_dir(&self.dir).await.map_err(|error| {
                    tracing::error!(dir = %self.dir.display(), error = %error, "catalogue indisponible");
                    error.to_string()
                })
            })
            .await
    }

    /// Construit la vue du catalogue pour le frontend.
    pub async fn view(&self) -> ChartCatalogView {
        let (charts, failures, error) = match self.catalog().await {
            Ok(catalog) => (
                catalog.charts().cloned().collect(),
                catalog.failures().to_vec(),
                None,
            ),
            Err(error) => (Vec::new(), Vec::new(), Some(error.clone())),
        };
        ChartCatalogView {
            charts_dir: self.dir.display().to_string(),
            tile_url_base: TILE_URL_BASE,
            charts,
            failures,
            error,
        }
    }

    /// Répond à une requête du protocole de tuiles.
    pub async fn serve_tile(&self, path: &str) -> TileReply {
        match self.catalog().await {
            Ok(catalog) => catalog.serve_tile(path).await,
            Err(_) => TileReply {
                status: 503,
                content_type: None,
                body: Vec::new(),
            },
        }
    }
}

/// Détermine le répertoire des cartes : variable [`CHARTS_DIR_ENV`] si
/// définie, sinon `<données de l'application>/charts`. Le répertoire est
/// créé s'il n'existe pas.
///
/// # Errors
///
/// Retourne une erreur si le répertoire ne peut pas être créé.
pub fn resolve_charts_dir(app_data_dir: &Path) -> anyhow::Result<PathBuf> {
    let dir = match std::env::var_os(CHARTS_DIR_ENV) {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => app_data_dir.join("charts"),
    };
    std::fs::create_dir_all(&dir)
        .map_err(|e| anyhow::anyhow!("création du répertoire {} : {e}", dir.display()))?;
    Ok(dir)
}

/// Convertit une réponse de tuile en réponse HTTP pour la WebView.
#[must_use]
pub fn to_http_response(reply: TileReply) -> Response<Vec<u8>> {
    let status = StatusCode::from_u16(reply.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = Response::builder()
        .status(status)
        // La page (origine tauri:// ou serveur de dev) charge les tuiles
        // depuis une autre origine locale.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    if let Some(content_type) = reply.content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    builder.body(reply.body).unwrap_or_else(|error| {
        tracing::error!(error = %error, "construction de réponse de tuile impossible");
        let mut fallback = Response::new(Vec::new());
        *fallback.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        fallback
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_reply_is_converted_with_headers() {
        let response = to_http_response(TileReply {
            status: 200,
            content_type: Some("image/png"),
            body: vec![1, 2, 3],
        });
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert_eq!(response.body(), &[1, 2, 3]);
    }

    #[test]
    fn empty_reply_has_no_content_type() {
        let response = to_http_response(TileReply {
            status: 204,
            content_type: None,
            body: Vec::new(),
        });
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(response.headers().get(header::CONTENT_TYPE).is_none());
    }

    #[test]
    fn invalid_status_becomes_server_error() {
        let response = to_http_response(TileReply {
            status: 42,
            content_type: None,
            body: Vec::new(),
        });
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn missing_directory_is_reported_not_hidden() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let state = ChartState::new(dir.path().join("absent"));
        let view = state.view().await;
        assert!(view.error.is_some());
        assert!(view.charts.is_empty());
        assert_eq!(state.serve_tile("/x/0/0/0").await.status, 503);
    }

    #[tokio::test]
    async fn empty_directory_gives_empty_view() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let state = ChartState::new(dir.path().to_owned());
        let view = state.view().await;
        assert!(view.error.is_none());
        assert!(view.charts.is_empty());
        assert_eq!(view.tile_url_base, TILE_URL_BASE);
    }

    #[test]
    fn charts_dir_defaults_to_app_data_and_is_created() {
        // La variable d'environnement n'est pas positionnée dans les tests.
        if std::env::var_os(CHARTS_DIR_ENV).is_some() {
            return;
        }
        let dir = tempfile::TempDir::new().expect("tempdir");
        let resolved = resolve_charts_dir(dir.path()).expect("répertoire");
        assert_eq!(resolved, dir.path().join("charts"));
        assert!(resolved.is_dir());
    }
}
