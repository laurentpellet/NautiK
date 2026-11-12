//! Erreurs de la crate `nav-charts`.

use std::path::PathBuf;

use thiserror::Error;

/// Erreur de lecture ou de validation d'une carte.
#[derive(Debug, Error)]
pub enum ChartError {
    /// Le répertoire des cartes est inaccessible.
    #[error("répertoire de cartes inaccessible : {path}")]
    DirectoryUnavailable {
        /// Répertoire concerné.
        path: PathBuf,
        /// Cause système.
        #[source]
        source: std::io::Error,
    },

    /// Le fichier n'a pas pu être ouvert comme base SQLite.
    #[error("impossible d'ouvrir la carte {path}")]
    Open {
        /// Fichier concerné.
        path: PathBuf,
        /// Cause SQLite.
        #[source]
        source: sqlx::Error,
    },

    /// Le fichier n'est pas un MBTiles valide (tables absentes, base
    /// corrompue…).
    #[error("fichier MBTiles invalide {path} : {reason}")]
    InvalidMbtiles {
        /// Fichier concerné.
        path: PathBuf,
        /// Description du problème.
        reason: String,
    },

    /// La carte ne contient aucune tuile.
    #[error("la carte {path} ne contient aucune tuile")]
    EmptyChart {
        /// Fichier concerné.
        path: PathBuf,
    },

    /// Format de tuile non supporté (ex. tuiles vectorielles en V1).
    #[error("format de tuile non supporté : {0}")]
    UnsupportedFormat(String),

    /// Plage de zoom invalide ou absente.
    #[error("plage de zoom invalide : {0}")]
    InvalidZoomRange(String),

    /// Erreur de lecture d'une tuile.
    #[error("erreur de lecture de tuile")]
    TileRead(#[source] sqlx::Error),
}
