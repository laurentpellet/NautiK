//! Cellules ENC affichées sur la carte : index (emprise, usage, état des
//! mises à jour) et entités GeoJSON de chaque cellule, avec cache.
//!
//! Une cellule n'est décodée qu'une fois tant que ses fichiers (base et
//! mises à jour) ne changent pas : la signature (taille et date de
//! modification de chaque fichier) est vérifiée à chaque demande. Le
//! nombre de cellules indexées et de cellules gardées en mémoire est
//! borné. Une cellule annulée par son producteur n'est jamais affichée.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use serde::Serialize;

use crate::enc::{resolve_cell_path, EncInventory};
use crate::error::ChartError;
use crate::s57::{self, decode_cell_file, display::map_features, S57Cell};

/// Nombre maximal de cellules indexées.
pub const MAX_INDEXED_CELLS: usize = 200;

/// Nombre maximal de cellules dont les entités restent en mémoire.
pub const MAX_CACHED_CELLS: usize = 32;

/// Cellule de l'index cartographique.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncMapCell {
    /// Chemin relatif (identifiant, voir l'inventaire).
    pub path: String,
    /// Nom de la cellule.
    pub name: String,
    /// Usage prévu (1 vue d'ensemble … 6 accostage).
    pub usage: u8,
    /// Échelle de compilation.
    pub scale: u32,
    /// Emprise `[ouest, sud, est, nord]`.
    pub bounds: Option<[f64; 4]>,
    /// Mises à jour appliquées.
    pub updates_applied: u32,
    /// Mises à jour non appliquées.
    pub updates_pending: u32,
    /// Raison de l'arrêt des mises à jour : cellule pas à jour.
    pub update_error: Option<String>,
    /// Date d'annulation : cellule à ne pas afficher.
    pub cancelled: Option<String>,
    /// Cellule illisible.
    pub error: Option<String>,
}

impl EncMapCell {
    fn unreadable(path: &str, error: String) -> Self {
        Self {
            path: path.to_owned(),
            name: cell_name(path),
            usage: 0,
            scale: 0,
            bounds: None,
            updates_applied: 0,
            updates_pending: 0,
            update_error: None,
            cancelled: None,
            error: Some(error),
        }
    }

    fn from_cell(path: &str, cell: &S57Cell) -> Self {
        let summary = cell.summary();
        Self {
            path: path.to_owned(),
            name: cell_name(path),
            usage: summary.intended_usage,
            scale: summary.compilation_scale,
            bounds: summary.bounds,
            updates_applied: summary.updates_applied,
            updates_pending: summary.updates_pending,
            update_error: summary.update_error,
            cancelled: summary.cancelled,
            error: None,
        }
    }
}

/// Index des cellules affichables.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncMapIndex {
    /// Cellules, dans l'ordre de l'inventaire.
    pub cells: Vec<EncMapCell>,
    /// Inventaire plus grand que [`MAX_INDEXED_CELLS`].
    pub truncated: bool,
}

/// Nom d'une cellule d'après son chemin relatif.
fn cell_name(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.split('.').next().unwrap_or(file).to_owned()
}

/// Signature des fichiers d'une cellule : chemin, taille, date.
type Signature = Vec<(PathBuf, u64, Option<SystemTime>)>;

fn signature(base: &Path) -> std::io::Result<Signature> {
    let mut files = vec![base.to_path_buf()];
    files.extend(
        s57::update_files(base)
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .into_iter()
            .map(|(_, path)| path),
    );
    files
        .into_iter()
        .map(|path| {
            let meta = std::fs::metadata(&path)?;
            Ok((path, meta.len(), meta.modified().ok()))
        })
        .collect()
}

async fn signature_of(path: PathBuf) -> Result<Signature, String> {
    tokio::task::spawn_blocking(move || signature(&path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[derive(Default)]
struct Inner {
    infos: HashMap<PathBuf, (Signature, EncMapCell)>,
    features: HashMap<PathBuf, (Signature, Arc<String>, u64)>,
    clock: u64,
}

/// Bibliothèque des cellules ENC affichées, avec cache.
#[derive(Default)]
pub struct EncLibrary {
    inner: Mutex<Inner>,
}

impl EncLibrary {
    /// Bibliothèque vide.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Index des cellules de l'inventaire (décodées si nécessaire).
    pub async fn index(&self, root: &Path, inventory: &EncInventory) -> EncMapIndex {
        let mut cells = Vec::new();
        for cell in inventory.cells.iter().take(MAX_INDEXED_CELLS) {
            cells.push(self.info(root, &cell.path).await);
        }
        EncMapIndex {
            cells,
            truncated: inventory.cells.len() > MAX_INDEXED_CELLS,
        }
    }

    async fn info(&self, root: &Path, relative: &str) -> EncMapCell {
        let path = match resolve_cell_path(root, relative) {
            Ok(path) => path,
            Err(error) => return EncMapCell::unreadable(relative, error.to_string()),
        };
        let signature = match signature_of(path.clone()).await {
            Ok(signature) => signature,
            Err(error) => return EncMapCell::unreadable(relative, error),
        };
        if let Some((known, info)) = self.lock().infos.get(&path) {
            if *known == signature {
                return info.clone();
            }
        }
        let info = match decode_cell_file(path.clone()).await {
            Ok(cell) => EncMapCell::from_cell(relative, &cell),
            Err(error) => {
                tracing::warn!(cell = %relative, %error, "cellule ENC illisible");
                EncMapCell::unreadable(relative, error.to_string())
            }
        };
        self.lock().infos.insert(path, (signature, info.clone()));
        info
    }

    /// Entités GeoJSON d'une cellule (tableau JSON sérialisé).
    ///
    /// # Errors
    ///
    /// [`ChartError::InvalidCellPath`] si le chemin est refusé,
    /// [`ChartError::EncUnusable`] si la cellule est illisible ou annulée.
    pub async fn features(&self, root: &Path, relative: &str) -> Result<Arc<String>, ChartError> {
        let path = resolve_cell_path(root, relative)?;
        let signature = signature_of(path.clone())
            .await
            .map_err(ChartError::EncUnusable)?;
        {
            let mut inner = self.lock();
            inner.clock += 1;
            let now = inner.clock;
            if let Some((known, json, used)) = inner.features.get_mut(&path) {
                if *known == signature {
                    *used = now;
                    return Ok(Arc::clone(json));
                }
            }
        }
        let cell = decode_cell_file(path.clone())
            .await
            .map_err(|e| ChartError::EncUnusable(e.to_string()))?;
        if let Some(date) = &cell.cancelled {
            return Err(ChartError::EncUnusable(format!(
                "cellule annulée par son producteur ({date})"
            )));
        }
        let json = serde_json::to_string(&map_features(&cell))
            .map_err(|e| ChartError::EncUnusable(e.to_string()))?;
        let json = Arc::new(json);
        let mut inner = self.lock();
        let info = EncMapCell::from_cell(relative, &cell);
        inner.infos.insert(path.clone(), (signature.clone(), info));
        inner.clock += 1;
        let now = inner.clock;
        if inner.features.len() >= MAX_CACHED_CELLS && !inner.features.contains_key(&path) {
            let oldest = inner
                .features
                .iter()
                .min_by_key(|(_, (_, _, used))| *used)
                .map(|(key, _)| key.clone());
            if let Some(key) = oldest {
                inner.features.remove(&key);
            }
        }
        inner
            .features
            .insert(path, (signature, Arc::clone(&json), now));
        Ok(json)
    }

    /// Nombre de cellules dont les entités sont en mémoire.
    pub fn cached_cells(&self) -> usize {
        self.lock().features.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enc::scan_enc_dir_blocking;
    use crate::s57::testdata::CellBuilder;

    fn folder() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let cell = dir.path().join("TEST0001");
        std::fs::create_dir_all(&cell).unwrap();
        std::fs::write(cell.join("TEST0001.000"), CellBuilder::sample().build()).unwrap();
        std::fs::write(dir.path().join("BROKEN01.000"), b"pas une cellule").unwrap();
        dir
    }

    #[tokio::test]
    async fn indexes_cells_with_bounds_and_errors() {
        let dir = folder();
        let inventory = scan_enc_dir_blocking(dir.path()).unwrap();
        let library = EncLibrary::new();
        let index = library.index(dir.path(), &inventory).await;
        assert!(!index.truncated);
        assert_eq!(index.cells.len(), 2);
        let broken = &index.cells[0];
        assert_eq!(broken.name, "BROKEN01");
        assert!(broken.error.is_some());
        let good = &index.cells[1];
        assert_eq!(good.name, "TEST0001");
        assert_eq!(good.usage, 5);
        assert_eq!(good.scale, 22_000);
        assert!(good.error.is_none() && good.cancelled.is_none());
        let [w, s, e, n] = good.bounds.unwrap();
        assert!(w < e && s < n);
        // Deuxième appel : depuis le cache.
        assert_eq!(library.index(dir.path(), &inventory).await, index);
    }

    #[tokio::test]
    async fn serves_features_and_refreshes_when_files_change() {
        let dir = folder();
        let library = EncLibrary::new();
        let json = library
            .features(dir.path(), "TEST0001/TEST0001.000")
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value.as_array().unwrap().len(), 6);
        assert_eq!(library.cached_cells(), 1);
        let again = library
            .features(dir.path(), "TEST0001/TEST0001.000")
            .await
            .unwrap();
        assert!(Arc::ptr_eq(&json, &again), "servi depuis le cache");

        // Une mise à jour ajoutée change la signature : nouveau décodage.
        let mut cancel = CellBuilder::update(1);
        cancel.edition = "0";
        std::fs::write(dir.path().join("TEST0001/TEST0001.001"), cancel.build()).unwrap();
        let error = library
            .features(dir.path(), "TEST0001/TEST0001.000")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("annulée"), "{error}");
    }

    #[tokio::test]
    async fn refuses_unsafe_or_unreadable_cells() {
        let dir = folder();
        let library = EncLibrary::new();
        for path in ["../x.000", "BROKEN01.000", "ABSENT.000"] {
            assert!(library.features(dir.path(), path).await.is_err(), "{path}");
        }
    }

    #[test]
    fn names_cells_from_paths() {
        assert_eq!(cell_name("FR/1/0/FR501130.000"), "FR501130");
        assert_eq!(cell_name("X.000"), "X");
    }
}
