//! Secrets (jeton Signal K) dans le coffre du système.
//!
//! Windows : Gestionnaire d'identification ; macOS : trousseau ;
//! Linux : trousseau du noyau (keyutils, non persistant au redémarrage).
//! Les secrets ne sont jamais écrits en base ni dans les journaux.

use std::sync::Arc;

use thiserror::Error;

/// Nom du secret contenant le jeton Signal K.
pub const SIGNALK_TOKEN: &str = "signalk-token";

/// Erreur d'accès au coffre.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("coffre de secrets indisponible : {0}")]
pub struct SecretError(String);

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

/// Coffre du système d'exploitation.
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    /// Coffre propre à l'application (`service` = identifiant Tauri).
    #[must_use]
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_owned(),
        }
    }

    fn entry(&self, name: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(&self.service, name).map_err(|e| SecretError(e.to_string()))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, name: &str) -> Result<Option<String>, SecretError> {
        match self.entry(name)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<(), SecretError> {
        self.entry(name)?
            .set_password(value)
            .map_err(|e| SecretError(e.to_string()))
    }

    fn delete(&self, name: &str) -> Result<(), SecretError> {
        match self.entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }
}

/// Exécute une opération du coffre hors du runtime asynchrone (les
/// appels système peuvent bloquer).
pub async fn blocking<T, F>(store: &Arc<dyn SecretStore>, op: F) -> Result<T, SecretError>
where
    T: Send + 'static,
    F: FnOnce(&dyn SecretStore) -> Result<T, SecretError> + Send + 'static,
{
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || op(store.as_ref()))
        .await
        .map_err(|e| SecretError(e.to_string()))?
}

/// Coffre en mémoire, pour les tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<std::collections::HashMap<String, String>>);

#[cfg(test)]
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
