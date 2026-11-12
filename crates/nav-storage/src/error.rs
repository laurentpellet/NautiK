//! Erreurs de persistance.

use thiserror::Error;
use uuid::Uuid;

/// Erreur de persistance.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Erreur SQLite.
    #[error("base de données : {0}")]
    Database(#[from] sqlx::Error),
    /// Échec d'application des migrations.
    #[error("migration du schéma : {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    /// Élément introuvable.
    #[error("élément introuvable : {0}")]
    NotFound(Uuid),
    /// Le waypoint est utilisé par au moins une route.
    #[error("le waypoint {0} est utilisé par une route")]
    WaypointInUse(Uuid),
    /// La trace n'est pas en cours d'enregistrement.
    #[error("la trace {0} n'est pas en cours d'enregistrement")]
    TrackNotRecording(Uuid),
    /// Donnée illisible en base (ne doit pas arriver avec les migrations).
    #[error("donnée corrompue en base : {0}")]
    Corrupt(String),
}
