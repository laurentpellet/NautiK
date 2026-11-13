//! Fichiers protégés du service : jeton d'accès à l'API et coffre des
//! secrets (jeton Signal K).
//!
//! Sur un système embarqué sans session graphique, le trousseau du
//! système n'est pas disponible ou pas persistant : les secrets sont
//! dans des fichiers lisibles du seul compte de service (mode `0600`
//! sous Unix). Leur contenu n'est jamais journalisé.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nav_api::ApiToken;
use nav_engine::{SecretError, SecretStore};

/// Nom du fichier contenant le jeton d'accès à l'API.
pub const TOKEN_FILE: &str = "api-token";

/// Nom du fichier coffre des secrets.
pub const SECRETS_FILE: &str = "secrets.json";

/// Écrit un fichier lisible du seul propriétaire, de façon atomique
/// (fichier temporaire puis renommage).
///
/// # Errors
///
/// Erreur d'entrée/sortie.
pub fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    let _ = fs::remove_file(&tmp);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)
}

/// Vérifie que le fichier n'est lisible que de son propriétaire ; corrige
/// et signale sinon.
fn ensure_private(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)?.permissions().mode();
        if mode & 0o077 != 0 {
            tracing::warn!(
                path = %path.display(),
                "fichier secret accessible à d'autres comptes : droits ramenés à 0600"
            );
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Jeton d'accès : lu s'il existe, créé sinon.
///
/// # Errors
///
/// Erreur d'entrée/sortie, ou fichier présent mais jeton invalide (il
/// n'est jamais remplacé silencieusement).
pub fn load_or_create_token(dir: &Path) -> anyhow::Result<ApiToken> {
    let path = dir.join(TOKEN_FILE);
    match fs::read_to_string(&path) {
        Ok(text) => {
            ensure_private(&path)?;
            ApiToken::parse(&text).map_err(|e| {
                anyhow::anyhow!(
                    "{} : {e} (nautikd --rotate-token pour le régénérer)",
                    path.display()
                )
            })
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let token = rotate_token(dir)?;
            tracing::info!(path = %path.display(), "jeton d'accès créé");
            Ok(token)
        }
        Err(e) => Err(anyhow::anyhow!("lecture de {} : {e}", path.display())),
    }
}

/// Génère et enregistre un nouveau jeton (l'ancien est révoqué).
///
/// # Errors
///
/// Erreur d'entrée/sortie.
pub fn rotate_token(dir: &Path) -> io::Result<ApiToken> {
    let token = ApiToken::generate();
    let mut contents = token.expose().to_owned();
    contents.push('\n');
    write_private(&dir.join(TOKEN_FILE), contents.as_bytes())?;
    Ok(token)
}

/// Coffre des secrets dans un fichier JSON protégé.
#[derive(Debug)]
pub struct FileStore {
    path: PathBuf,
    secrets: Mutex<BTreeMap<String, String>>,
}

impl FileStore {
    /// Ouvre le coffre (vide s'il n'existe pas encore).
    ///
    /// # Errors
    ///
    /// Erreur d'entrée/sortie ou fichier illisible (jamais écrasé
    /// silencieusement).
    pub fn open(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(SECRETS_FILE);
        let secrets = match fs::read(&path) {
            Ok(bytes) => {
                ensure_private(&path)?;
                serde_json::from_slice(&bytes)
                    .map_err(|e| anyhow::anyhow!("coffre {} illisible : {e}", path.display()))?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(anyhow::anyhow!("lecture de {} : {e}", path.display())),
        };
        Ok(Self {
            path,
            secrets: Mutex::new(secrets),
        })
    }

    fn update(&self, op: impl FnOnce(&mut BTreeMap<String, String>)) -> Result<(), SecretError> {
        let mut secrets = self
            .secrets
            .lock()
            .map_err(|e| SecretError(e.to_string()))?;
        let mut next = secrets.clone();
        op(&mut next);
        let json = serde_json::to_vec_pretty(&next).map_err(|e| SecretError(e.to_string()))?;
        write_private(&self.path, &json).map_err(|e| SecretError(e.to_string()))?;
        *secrets = next;
        Ok(())
    }
}

impl SecretStore for FileStore {
    fn get(&self, name: &str) -> Result<Option<String>, SecretError> {
        Ok(self
            .secrets
            .lock()
            .map_err(|e| SecretError(e.to_string()))?
            .get(name)
            .cloned())
    }

    fn set(&self, name: &str, value: &str) -> Result<(), SecretError> {
        self.update(|s| {
            s.insert(name.to_owned(), value.to_owned());
        })
    }

    fn delete(&self, name: &str) -> Result<(), SecretError> {
        self.update(|s| {
            s.remove(name);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_created_once_then_reused_and_rotated() {
        let dir = tempfile::tempdir().expect("tmp");
        let first = load_or_create_token(dir.path()).expect("créé");
        let again = load_or_create_token(dir.path()).expect("relu");
        assert_eq!(first, again);
        let rotated = rotate_token(dir.path()).expect("renouvelé");
        assert_ne!(rotated, first);
        assert_eq!(load_or_create_token(dir.path()).expect("relu"), rotated);
        assert!(!dir.path().join("api-token.tmp").exists());
    }

    #[test]
    fn invalid_token_file_is_never_replaced_silently() {
        let dir = tempfile::tempdir().expect("tmp");
        fs::write(dir.path().join(TOKEN_FILE), "court").expect("écrit");
        let error = load_or_create_token(dir.path()).expect_err("refusé");
        assert!(error.to_string().contains("--rotate-token"), "{error}");
        assert_eq!(
            fs::read_to_string(dir.path().join(TOKEN_FILE)).expect("lu"),
            "court"
        );
    }

    #[cfg(unix)]
    #[test]
    fn secret_files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tmp");
        load_or_create_token(dir.path()).expect("créé");
        let path = dir.path().join(TOKEN_FILE);
        let mode = fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
        load_or_create_token(dir.path()).expect("relu");
        let mode = fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "droits corrigés");
    }

    #[test]
    fn file_store_persists_secrets() {
        let dir = tempfile::tempdir().expect("tmp");
        let store = FileStore::open(dir.path()).expect("ouvert");
        assert_eq!(store.get("signalk-token"), Ok(None));
        store.set("signalk-token", "abc").expect("set");
        let reopened = FileStore::open(dir.path()).expect("rouvert");
        assert_eq!(reopened.get("signalk-token"), Ok(Some("abc".to_owned())));
        reopened.delete("signalk-token").expect("delete");
        reopened.delete("signalk-token").expect("idempotent");
        assert_eq!(
            FileStore::open(dir.path())
                .expect("rouvert")
                .get("signalk-token"),
            Ok(None)
        );
    }

    #[test]
    fn corrupt_store_is_reported_not_erased() {
        let dir = tempfile::tempdir().expect("tmp");
        fs::write(dir.path().join(SECRETS_FILE), "{pas du json").expect("écrit");
        assert!(FileStore::open(dir.path()).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join(SECRETS_FILE)).expect("lu"),
            "{pas du json"
        );
    }
}
