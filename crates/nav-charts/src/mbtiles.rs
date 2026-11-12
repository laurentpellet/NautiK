//! Lecture d'un fichier MBTiles raster.
//!
//! Le fichier est ouvert en lecture seule : NautiK ne modifie jamais une
//! carte. Les accès sont asynchrones (SQLx) pour ne pas bloquer le
//! runtime Tokio.

use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

use crate::error::ChartError;
use crate::metadata::{parse_metadata, ChartMetadata, TileFacts};
use crate::tile::{TileCoord, TileFormat};

/// Nombre maximal de connexions simultanées par carte.
const MAX_CONNECTIONS: u32 = 4;

/// Tuile lue dans une carte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    /// Données de l'image.
    pub data: Vec<u8>,
    /// Format de l'image.
    pub format: TileFormat,
}

/// Carte MBTiles ouverte en lecture seule.
#[derive(Debug, Clone)]
pub struct MbTiles {
    path: PathBuf,
    pool: SqlitePool,
    metadata: ChartMetadata,
}

impl MbTiles {
    /// Ouvre et valide un fichier MBTiles.
    ///
    /// # Errors
    ///
    /// Retourne une [`ChartError`] si le fichier est absent, corrompu, ne
    /// respecte pas le schéma MBTiles, ne contient aucune tuile ou
    /// utilise un format non supporté.
    pub async fn open(path: &Path) -> Result<Self, ChartError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .create_if_missing(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(MAX_CONNECTIONS)
            .connect_with(options)
            .await
            .map_err(|source| ChartError::Open {
                path: path.to_owned(),
                source,
            })?;

        let invalid = |error: sqlx::Error| ChartError::InvalidMbtiles {
            path: path.to_owned(),
            reason: error.to_string(),
        };

        let entries: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT name, CAST(value AS TEXT) FROM metadata WHERE name IS NOT NULL")
                .fetch_all(&pool)
                .await
                .map_err(invalid)?;

        let (min_zoom, max_zoom): (Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT MIN(zoom_level), MAX(zoom_level) FROM tiles")
                .fetch_one(&pool)
                .await
                .map_err(invalid)?;
        let (Some(min_zoom), Some(max_zoom)) = (min_zoom, max_zoom) else {
            return Err(ChartError::EmptyChart {
                path: path.to_owned(),
            });
        };
        let to_zoom = |z: i64| {
            u8::try_from(z)
                .map_err(|_| ChartError::InvalidZoomRange(format!("{min_zoom}..{max_zoom}")))
        };

        let sample: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT tile_data FROM tiles WHERE tile_data IS NOT NULL LIMIT 1")
                .fetch_optional(&pool)
                .await
                .map_err(invalid)?;

        let facts = TileFacts {
            min_zoom: to_zoom(min_zoom)?,
            max_zoom: to_zoom(max_zoom)?,
            sniffed_format: sample.and_then(|(data,)| TileFormat::sniff(&data)),
        };

        let fallback_name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let metadata = parse_metadata(&entries, &fallback_name, facts)?;
        for warning in &metadata.warnings {
            tracing::warn!(chart = %path.display(), "{warning}");
        }

        Ok(Self {
            path: path.to_owned(),
            pool,
            metadata,
        })
    }

    /// Chemin du fichier.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Métadonnées validées.
    #[must_use]
    pub fn metadata(&self) -> &ChartMetadata {
        &self.metadata
    }

    /// Lit une tuile. Retourne `None` si la tuile n'existe pas.
    ///
    /// Le format retourné est détecté sur les données ; à défaut, le
    /// format déclaré de la carte est utilisé.
    ///
    /// # Errors
    ///
    /// Retourne [`ChartError::TileRead`] en cas d'erreur SQLite.
    pub async fn tile(&self, coord: TileCoord) -> Result<Option<Tile>, ChartError> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as(
            "SELECT tile_data FROM tiles \
             WHERE zoom_level = ?1 AND tile_column = ?2 AND tile_row = ?3 LIMIT 1",
        )
        .bind(i64::from(coord.z()))
        .bind(i64::from(coord.x()))
        .bind(i64::from(coord.tms_row()))
        .fetch_optional(&self.pool)
        .await
        .map_err(ChartError::TileRead)?;

        Ok(row.map(|(data,)| {
            let format = TileFormat::sniff(&data).unwrap_or(self.metadata.format);
            Tile { data, format }
        }))
    }

    /// Ferme les connexions à la carte.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}
