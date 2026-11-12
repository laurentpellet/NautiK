//! Réglages applicatifs (table `settings`, valeurs JSON).
//!
//! Aucun secret n'est stocké ici : les jetons vont dans le coffre du
//! système (voir `SECURITY.md`, §3).

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{Db, StorageError};

/// Longueur maximale d'une clé.
const MAX_KEY_LEN: usize = 64;

fn check_key(key: &str) -> Result<(), StorageError> {
    if key.is_empty()
        || key.len() > MAX_KEY_LEN
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
    {
        return Err(StorageError::Corrupt(format!(
            "clé de réglage invalide « {key} »"
        )));
    }
    Ok(())
}

impl Db {
    /// Lit un réglage. `None` s'il n'a jamais été enregistré.
    ///
    /// # Errors
    ///
    /// [`StorageError::Corrupt`] si la valeur stockée ne correspond pas au
    /// type attendu.
    pub async fn get_setting<T: DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, StorageError> {
        check_key(key)?;
        let raw: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM settings WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        raw.map(|json| {
            serde_json::from_str(&json)
                .map_err(|e| StorageError::Corrupt(format!("réglage « {key} » : {e}")))
        })
        .transpose()
    }

    /// Enregistre un réglage (remplace la valeur précédente).
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec de sérialisation ou SQLite.
    pub async fn set_setting<T: Serialize + Sync>(
        &self,
        key: &str,
        value: &T,
    ) -> Result<(), StorageError> {
        check_key(key)?;
        let json = serde_json::to_string(value)
            .map_err(|e| StorageError::Corrupt(format!("réglage « {key} » : {e}")))?;
        sqlx::query(
            "INSERT INTO settings (key, value_json) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
        )
        .bind(key)
        .bind(json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sources {
        mode: String,
        url: Option<String>,
    }

    #[tokio::test]
    async fn round_trip_and_overwrite() {
        let db = Db::open_in_memory().await.expect("db");
        assert_eq!(
            db.get_setting::<Sources>("sources").await.expect("get"),
            None
        );
        let a = Sources {
            mode: "signalK".to_owned(),
            url: Some("http://h:3000".to_owned()),
        };
        db.set_setting("sources", &a).await.expect("set");
        assert_eq!(
            db.get_setting::<Sources>("sources").await.expect("get"),
            Some(a)
        );
        let b = Sources {
            mode: "simulator".to_owned(),
            url: None,
        };
        db.set_setting("sources", &b).await.expect("set");
        assert_eq!(
            db.get_setting::<Sources>("sources").await.expect("get"),
            Some(b)
        );
    }

    #[tokio::test]
    async fn mismatched_type_is_reported() {
        let db = Db::open_in_memory().await.expect("db");
        db.set_setting("sources", &42).await.expect("set");
        assert!(matches!(
            db.get_setting::<Sources>("sources").await,
            Err(StorageError::Corrupt(_))
        ));
    }

    #[tokio::test]
    async fn invalid_keys_are_rejected() {
        let db = Db::open_in_memory().await.expect("db");
        for key in ["", "a b", "x'; DROP TABLE settings; --", &"k".repeat(65)] {
            assert!(db.set_setting(key, &1).await.is_err(), "{key}");
            assert!(db.get_setting::<i32>(key).await.is_err(), "{key}");
        }
    }
}
