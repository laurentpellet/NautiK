//! Secrets (jeton Signal K).
//!
//! Le stockage est fourni par l'application : coffre du système pour le
//! desktop, fichier protégé pour le service embarqué. Les secrets ne sont
//! jamais écrits en base ni dans les journaux.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use thiserror::Error;

/// Nom du secret contenant le jeton Signal K.
pub const SIGNALK_TOKEN: &str = "signalk-token";

/// Erreur d'accès au coffre.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("coffre de secrets indisponible : {0}")]
pub struct SecretError(pub String);

/// Stockage de secrets.
pub trait SecretStore: Send + Sync {
    /// Lit un secret (`None` s'il n'existe pas).
    ///
    /// # Errors
    ///
    /// [`SecretError`] si le coffre est inaccessible.
    fn get(&self, name: &str) -> Result<Option<String>, SecretError>;

    /// Enregistre un secret.
    ///
    /// # Errors
    ///
    /// [`SecretError`] si le coffre est inaccessible.
    fn set(&self, name: &str, value: &str) -> Result<(), SecretError>;

    /// Supprime un secret (sans erreur s'il n'existe pas).
    ///
    /// # Errors
    ///
    /// [`SecretError`] si le coffre est inaccessible.
    fn delete(&self, name: &str) -> Result<(), SecretError>;
}

/// Exécute une opération du coffre hors du runtime asynchrone (les
/// appels système peuvent bloquer).
///
/// # Errors
///
/// [`SecretError`] si le coffre est inaccessible.
pub async fn blocking<T, F>(store: &Arc<dyn SecretStore>, op: F) -> Result<T, SecretError>
where
    T: Send + 'static,
    F: FnOnce(&dyn SecretStore) -> Result<T, SecretError> + Send + 'static,
{
    let store = Arc::clone(store);
    tokio::task::spawn_blocking(move || op(store.as_ref()))
        .await
        .map_err(|e| SecretError(e.to_string()))?
}

/// Coffre en mémoire (tests, ou aucun secret persistant).
#[derive(Debug, Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Result<Option<String>, SecretError> {
        Ok(self
            .0
            .lock()
            .map_err(|e| SecretError(e.to_string()))?
            .get(name)
            .cloned())
    }

    fn set(&self, name: &str, value: &str) -> Result<(), SecretError> {
        self.0
            .lock()
            .map_err(|e| SecretError(e.to_string()))?
            .insert(name.to_owned(), value.to_owned());
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), SecretError> {
        self.0
            .lock()
            .map_err(|e| SecretError(e.to_string()))?
            .remove(name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_store_round_trip() {
        let store: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
        assert_eq!(blocking(&store, |s| s.get(SIGNALK_TOKEN)).await, Ok(None));
        blocking(&store, |s| s.set(SIGNALK_TOKEN, "abc"))
            .await
            .expect("set");
        assert_eq!(
            blocking(&store, |s| s.get(SIGNALK_TOKEN)).await,
            Ok(Some("abc".to_owned()))
        );
        blocking(&store, |s| s.delete(SIGNALK_TOKEN))
            .await
            .expect("delete");
        blocking(&store, |s| s.delete(SIGNALK_TOKEN))
            .await
            .expect("idempotent");
        assert_eq!(blocking(&store, |s| s.get(SIGNALK_TOKEN)).await, Ok(None));
    }
}
