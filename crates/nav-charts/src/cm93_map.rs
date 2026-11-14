//! Cartes CM93 affichées : base trouvée dans le dossier ENC, cellules
//! choisies pour la vue, entités des cellules avec cache.
//!
//! L'index vient des noms de fichiers (aucun décodage) ; il est relu par
//! [`Cm93Library::refresh`]. Le frontend ne peut demander qu'une cellule
//! de l'index : aucun chemin arbitraire n'est ouvert.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;

use crate::cm93::{
    decode_cell_file_blocking, map_features, scan_cm93_cells, Cm93Dictionary, Cm93Index, Cm93Scale,
    ViewBounds,
};
use crate::enc::find_cm93_root;
use crate::enc_map::EncMapCell;
use crate::error::ChartError;
use crate::s57::display::MapFeature;

/// Nombre maximal de cellules CM93 dessinées pour une vue.
pub const MAX_CM93_VIEW_CELLS: usize = 16;

/// Nombre maximal de cellules CM93 dont les entités restent en mémoire.
pub const MAX_CACHED_CM93_CELLS: usize = 48;

/// Base CM93 chargée.
struct Base {
    /// Dossier ENC choisi (clé).
    enc_root: PathBuf,
    /// Racine de la base.
    root: PathBuf,
    dict: Arc<Cm93Dictionary>,
    index: Cm93Index,
    paths: HashSet<String>,
}

/// Cellules CM93 d'une vue.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cm93View {
    /// Base trouvée (chemin relatif au dossier ENC, `.` : le dossier
    /// lui-même) ; `None` : pas de base CM93.
    pub base: Option<String>,
    /// Cellules à dessiner, de la moins à la plus détaillée (`usage` :
    /// rang d'échelle 1 à 8, `scale` : dénominateur).
    pub cells: Vec<EncMapCell>,
    /// Cellules de la vue non dessinées faute de place.
    pub omitted: usize,
    /// Index limité : base incomplète.
    pub truncated: bool,
}

/// Entités d'une cellule CM93 et ce qu'il faut en signaler.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cm93CellFeatures<'a> {
    /// Entités GeoJSON.
    pub features: &'a [MapFeature],
    /// Date de la donnée la plus récente (`AAAAMMJJ`).
    pub data_date: Option<&'a str>,
    /// Décalage WGS 84 déclaré par la cellule, non appliqué (mètres).
    pub declared_offset_m: f64,
}

#[derive(Default)]
struct Inner {
    base: Option<Arc<Base>>,
    features: HashMap<String, (Arc<String>, u64)>,
    clock: u64,
}

/// Bibliothèque des cartes CM93 affichées.
#[derive(Default)]
pub struct Cm93Library {
    inner: Mutex<Inner>,
}

fn unusable(e: impl std::fmt::Display) -> ChartError {
    ChartError::EncUnusable(e.to_string())
}

fn relative_label(enc_root: &Path, root: &Path) -> String {
    match root.strip_prefix(enc_root) {
        Ok(rel) if rel.as_os_str().is_empty() => ".".into(),
        Ok(rel) => rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => root.display().to_string(),
    }
}

impl Cm93Library {
    /// Bibliothèque vide.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Recherche (à nouveau) la base CM93 du dossier ENC et relit son
    /// index ; le cache des entités est vidé. `Ok(None)` : pas de base.
    ///
    /// # Errors
    ///
    /// [`ChartError::EncUnusable`] si la base trouvée est illisible
    /// (dictionnaires absents ou mal formés).
    pub async fn refresh(&self, enc_root: &Path) -> Result<Option<String>, ChartError> {
        let enc = enc_root.to_path_buf();
        let loaded = tokio::task::spawn_blocking(move || -> Result<Option<Base>, ChartError> {
            let Some(root) = find_cm93_root(&enc) else {
                return Ok(None);
            };
            let dict = Cm93Dictionary::load(&root).map_err(unusable)?;
            let index = scan_cm93_cells(&root).map_err(unusable)?;
            let paths = index.cells.iter().map(|c| c.path.clone()).collect();
            Ok(Some(Base {
                enc_root: enc,
                root,
                dict: Arc::new(dict),
                index,
                paths,
            }))
        })
        .await
        .map_err(unusable)?;
        let mut inner = self.lock();
        inner.features.clear();
        match loaded {
            Ok(Some(base)) => {
                let label = relative_label(&base.enc_root, &base.root);
                tracing::info!(base = %base.root.display(), cells = base.index.cells.len(), "base CM93");
                inner.base = Some(Arc::new(base));
                Ok(Some(label))
            }
            Ok(None) => {
                inner.base = None;
                Ok(None)
            }
            Err(error) => {
                inner.base = None;
                Err(error)
            }
        }
    }

    /// Base chargée pour ce dossier ENC (chargée au premier appel).
    async fn base(&self, enc_root: &Path) -> Result<Option<Arc<Base>>, ChartError> {
        if let Some(base) = self.lock().base.clone() {
            if base.enc_root == enc_root {
                return Ok(Some(base));
            }
        }
        self.refresh(enc_root).await?;
        Ok(self.lock().base.clone())
    }

    /// Cellules CM93 à dessiner pour une vue.
    ///
    /// # Errors
    ///
    /// [`ChartError::EncUnusable`] si la base est illisible ;
    /// [`ChartError::InvalidCellPath`] si la vue est invalide.
    pub async fn view(&self, enc_root: &Path, view: ViewBounds) -> Result<Cm93View, ChartError> {
        let [w, s, e, n] = view;
        let valid = view.iter().all(|v| v.is_finite())
            && (-90.0..=90.0).contains(&s)
            && (-90.0..=90.0).contains(&n)
            && s <= n
            && (-180.0..=180.0).contains(&w)
            && (-180.0..=180.0).contains(&e);
        if !valid {
            return Err(ChartError::InvalidCellPath("vue invalide".into()));
        }
        let Some(base) = self.base(enc_root).await? else {
            return Ok(Cm93View::default());
        };
        let selection = base.index.select(&view, MAX_CM93_VIEW_CELLS);
        Ok(Cm93View {
            base: Some(relative_label(&base.enc_root, &base.root)),
            cells: selection
                .cells
                .iter()
                .map(|c| EncMapCell {
                    path: c.path.clone(),
                    name: c.name.clone(),
                    usage: c.scale.rank(),
                    scale: c.scale.denominator(),
                    bounds: Some(c.bounds),
                    updates_applied: 0,
                    updates_pending: 0,
                    update_error: None,
                    cancelled: None,
                    error: None,
                })
                .collect(),
            omitted: selection.omitted,
            truncated: base.index.truncated,
        })
    }

    /// Entités d'une cellule de l'index (objet JSON sérialisé, voir
    /// [`Cm93CellFeatures`]).
    ///
    /// # Errors
    ///
    /// [`ChartError::InvalidCellPath`] si la cellule n'est pas dans
    /// l'index ; [`ChartError::EncUnusable`] si elle est illisible.
    pub async fn features(&self, enc_root: &Path, path: &str) -> Result<Arc<String>, ChartError> {
        let base = self
            .base(enc_root)
            .await?
            .ok_or_else(|| ChartError::InvalidCellPath("aucune base CM93".into()))?;
        if !base.paths.contains(path) {
            return Err(ChartError::InvalidCellPath(format!(
                "{path} absente de la base CM93"
            )));
        }
        {
            let mut inner = self.lock();
            inner.clock += 1;
            let now = inner.clock;
            if let Some((json, used)) = inner.features.get_mut(path) {
                *used = now;
                return Ok(Arc::clone(json));
            }
        }
        let file = base.root.join(path);
        let dict = Arc::clone(&base.dict);
        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
        let rank = name
            .rsplit('.')
            .next()
            .and_then(|e| e.chars().next())
            .and_then(Cm93Scale::from_letter)
            .map_or(1, Cm93Scale::rank);
        let json = tokio::task::spawn_blocking(move || -> Result<String, ChartError> {
            let cell = decode_cell_file_blocking(&file, &dict).map_err(unusable)?;
            let features = map_features(&cell, &name, rank);
            serde_json::to_string(&Cm93CellFeatures {
                features: &features,
                data_date: cell.data_date.as_deref(),
                declared_offset_m: cell.declared_offset_m,
            })
            .map_err(unusable)
        })
        .await
        .map_err(unusable)?
        .inspect_err(|error| tracing::warn!(cell = %path, %error, "cellule CM93 illisible"))?;
        let json = Arc::new(json);
        let mut inner = self.lock();
        inner.clock += 1;
        let now = inner.clock;
        if inner.features.len() >= MAX_CACHED_CM93_CELLS && !inner.features.contains_key(path) {
            let oldest = inner
                .features
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| key.clone());
            if let Some(key) = oldest {
                inner.features.remove(&key);
            }
        }
        inner
            .features
            .insert(path.to_owned(), (Arc::clone(&json), now));
        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cm93::testdata::{CellBuilder, ATTRIBUTES, OBJECTS};

    /// Dossier ENC contenant une base CM93 d'une cellule au 1:20 000.
    fn folder() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("CM93");
        let level = base.join("02100600").join("F");
        std::fs::create_dir_all(&level).unwrap();
        std::fs::write(base.join("CM93OBJ.DIC"), OBJECTS).unwrap();
        std::fs::write(base.join("CM93ATTR.DIC"), ATTRIBUTES).unwrap();
        std::fs::write(level.join("02170631.F"), CellBuilder::sample().build()).unwrap();
        std::fs::write(level.join("02170632.F"), b"corrompue").unwrap();
        dir
    }

    const PAPEETE: ViewBounds = [-149.6, -17.55, -149.5, -17.45];

    #[tokio::test]
    async fn views_and_features() {
        let dir = folder();
        let library = Cm93Library::new();
        let view = library.view(dir.path(), PAPEETE).await.unwrap();
        assert_eq!(view.base.as_deref(), Some("CM93"));
        assert_eq!(view.cells.len(), 1);
        let cell = &view.cells[0];
        assert_eq!(cell.path, "02100600/F/02170631.F");
        assert_eq!((cell.usage, cell.scale), (7, 20_000));

        let json = library.features(dir.path(), &cell.path).await.unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["features"].as_array().unwrap().len(), 6);
        assert_eq!(value["dataDate"], "20140612");
        assert_eq!(value["declaredOffsetM"], 40.0);
        let again = library.features(dir.path(), &cell.path).await.unwrap();
        assert!(Arc::ptr_eq(&json, &again), "servi depuis le cache");

        // Ailleurs : aucune cellule.
        let far = library
            .view(dir.path(), [0.0, 40.0, 1.0, 41.0])
            .await
            .unwrap();
        assert!(far.cells.is_empty());
    }

    #[tokio::test]
    async fn refuses_paths_outside_the_index_and_broken_cells() {
        let dir = folder();
        let library = Cm93Library::new();
        for path in ["../x.F", "02100600/F/ABSENT.F", "CM93OBJ.DIC", ""] {
            let error = library.features(dir.path(), path).await.unwrap_err();
            assert!(matches!(error, ChartError::InvalidCellPath(_)), "{path}");
        }
        let error = library
            .features(dir.path(), "02100600/F/02170632.F")
            .await
            .unwrap_err();
        assert!(matches!(error, ChartError::EncUnusable(_)));
        for bad in [
            [f64::NAN, 0.0, 1.0, 1.0],
            [0.0, 10.0, 1.0, 5.0],
            [0.0, -95.0, 1.0, 1.0],
            [-200.0, 0.0, 1.0, 1.0],
        ] {
            assert!(library.view(dir.path(), bad).await.is_err());
        }
    }

    #[tokio::test]
    async fn folder_without_base_or_with_broken_base() {
        let dir = tempfile::tempdir().unwrap();
        let library = Cm93Library::new();
        assert_eq!(
            library.view(dir.path(), PAPEETE).await.unwrap(),
            Cm93View::default()
        );
        assert!(library.features(dir.path(), "x").await.is_err());
        // Dictionnaires présents mais vides : base illisible, signalée.
        std::fs::write(dir.path().join("CM93OBJ.DIC"), "").unwrap();
        std::fs::write(dir.path().join("CM93ATTR.DIC"), "").unwrap();
        assert!(library.refresh(dir.path()).await.is_err());
        assert!(library.view(dir.path(), PAPEETE).await.is_err());
        // Réparée puis relue.
        std::fs::write(dir.path().join("CM93OBJ.DIC"), OBJECTS).unwrap();
        std::fs::write(dir.path().join("CM93ATTR.DIC"), ATTRIBUTES).unwrap();
        assert_eq!(
            library.refresh(dir.path()).await.unwrap().as_deref(),
            Some(".")
        );
    }
}
