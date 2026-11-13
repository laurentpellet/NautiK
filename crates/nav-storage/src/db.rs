//! Ouverture de la base et application des migrations.

use std::path::Path;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::StorageError;

/// Délai d'attente sur un verrou SQLite.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Base NautiK : pool SQLite migré, clés étrangères actives.
#[derive(Debug, Clone)]
pub struct Db {
    pub(crate) pool: SqlitePool,
}

impl Db {
    /// Ouvre (ou crée) la base au chemin donné et applique les migrations.
    ///
    /// # Errors
    ///
    /// [`StorageError`] si le fichier ne peut être ouvert ou migré.
    pub async fn open(path: &Path) -> Result<Self, StorageError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(BUSY_TIMEOUT);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        Self::migrate(pool).await
    }

    /// Ouvre une base en mémoire (tests, démonstration).
    ///
    /// # Errors
    ///
    /// [`StorageError`] si la migration échoue.
    pub async fn open_in_memory() -> Result<Self, StorageError> {
        let options = SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true);
        // Une seule connexion conservée : chaque connexion mémoire est une
        // base distincte.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await?;
        Self::migrate(pool).await
    }

    async fn migrate(pool: SqlitePool) -> Result<Self, StorageError> {
        sqlx::migrate!("../../migrations").run(&pool).await?;
        tracing::debug!(component = crate::COMPONENT, "schéma à jour");
        Ok(Self { pool })
    }

    /// Vérifie l'intégrité de la base (`PRAGMA quick_check`), par exemple
    /// après un arrêt brutal. Retourne les anomalies trouvées (vide si la
    /// base est saine), au plus [`MAX_INTEGRITY_ERRORS`].
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] si la vérification ne peut être menée.
    pub async fn quick_check(&self) -> Result<Vec<String>, StorageError> {
        let rows: Vec<String> =
            sqlx::query_scalar(&format!("PRAGMA quick_check({MAX_INTEGRITY_ERRORS})"))
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().filter(|r| r != "ok").collect())
    }

    /// Vérification au démarrage (après un éventuel arrêt brutal) : une
    /// anomalie est journalisée en erreur, sans empêcher le démarrage (la
    /// navigation et les alarmes restent prioritaires). Retourne `true`
    /// si la base est saine.
    pub async fn check_integrity(&self) -> bool {
        match self.quick_check().await {
            Ok(problems) if problems.is_empty() => true,
            Ok(problems) => {
                tracing::error!(
                    component = crate::COMPONENT,
                    ?problems,
                    "base de données endommagée : sauvegardez-la et vérifiez traces et routes"
                );
                false
            }
            Err(error) => {
                tracing::error!(
                    component = crate::COMPONENT,
                    %error,
                    "vérification de la base impossible"
                );
                false
            }
        }
    }

    /// Ferme proprement les connexions.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

/// Nombre maximal d'anomalies rapportées par [`Db::quick_check`].
pub const MAX_INTEGRITY_ERRORS: u32 = 20;

#[cfg(test)]
mod tests {
    use super::*;

    async fn assert_foreign_keys(db: &Db) {
        let enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&db.pool)
            .await
            .expect("pragma");
        assert_eq!(enabled, 1);
        let orphan = sqlx::query(
            "INSERT INTO track_points (track_id, timestamp_utc, latitude, longitude) \
             VALUES ('absente', '2026-01-01T00:00:00Z', 0, 0)",
        )
        .execute(&db.pool)
        .await;
        assert!(orphan.is_err(), "point orphelin refusé");
    }

    #[tokio::test]
    async fn foreign_keys_enabled_in_memory() {
        let db = Db::open_in_memory().await.expect("db");
        assert_foreign_keys(&db).await;
    }

    #[tokio::test]
    async fn foreign_keys_and_wal_enabled_on_file() {
        let dir = tempfile::tempdir().expect("dir");
        let db = Db::open(&dir.path().join("n.db")).await.expect("db");
        assert_foreign_keys(&db).await;
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&db.pool)
            .await
            .expect("pragma");
        assert_eq!(mode.to_lowercase(), "wal");
        db.close().await;
    }

    #[tokio::test]
    async fn quick_check_reports_healthy_database() {
        let dir = tempfile::tempdir().expect("dir");
        let db = Db::open(&dir.path().join("n.db")).await.expect("db");
        assert!(db.quick_check().await.expect("vérification").is_empty());
        assert!(db.check_integrity().await);
        db.close().await;
        // Base fermée : la vérification échoue et le signale.
        assert!(!db.check_integrity().await);
        let memory = Db::open_in_memory().await.expect("db");
        assert!(memory.quick_check().await.expect("vérification").is_empty());
    }

    #[tokio::test]
    async fn file_that_is_not_a_database_is_refused() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("n.db");
        std::fs::write(&path, vec![0x5A_u8; 8_192]).expect("écriture");
        assert!(Db::open(&path).await.is_err());
    }

    #[tokio::test]
    async fn check_constraints_reject_invalid_coordinates() {
        let db = Db::open_in_memory().await.expect("db");
        let bad = sqlx::query(
            "INSERT INTO waypoints (id, name, latitude, longitude, created_at, updated_at) \
             VALUES ('x', 'x', 91, 0, '', '')",
        )
        .execute(&db.pool)
        .await;
        assert!(bad.is_err());
    }
}
