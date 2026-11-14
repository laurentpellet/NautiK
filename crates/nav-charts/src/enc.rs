//! Inventaire d'un dossier de cartes ENC S-57 (IHO S-57 édition 3.1).
//!
//! Un jeu d'échange ENC contient un catalogue `CATALOG.031` et des
//! cellules : un fichier de base `NOM.000` et ses mises à jour
//! `NOM.001`, `NOM.002`… dans le même dossier. Ce module repère les
//! fichiers ; le décodage des cellules est fait par [`crate::s57`].
//!
//! Le parcours est borné (profondeur, nombre d'entrées) et ne suit pas
//! les liens symboliques : un dossier choisi par erreur (disque entier)
//! ne bloque pas l'application.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::ChartError;

/// Nom du fichier catalogue d'un jeu d'échange S-57.
pub const ENC_CATALOG_FILE: &str = "CATALOG.031";

/// Profondeur maximale de sous-dossiers parcourus.
pub const MAX_ENC_DEPTH: usize = 8;

/// Nombre maximal d'entrées examinées (fichiers et dossiers).
pub const MAX_ENC_ENTRIES: usize = 50_000;

/// Cellule ENC trouvée (fichier de base `.000`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncCell {
    /// Nom de la cellule (nom du fichier sans extension, ex. `FR501130`).
    pub name: String,
    /// Chemin relatif au dossier ENC, séparateurs `/`.
    pub path: String,
    /// Taille du fichier de base, en octets.
    pub size_bytes: u64,
    /// Nombre de fichiers de mise à jour (`.001` à `.999`) à côté.
    pub updates: u32,
}

/// Inventaire d'un dossier ENC.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncInventory {
    /// Cellules, triées par nom puis par chemin.
    pub cells: Vec<EncCell>,
    /// Nombre de catalogues `CATALOG.031` trouvés.
    pub catalogs: u32,
    /// Sous-dossiers illisibles (droits, support retiré).
    pub unreadable_dirs: u32,
    /// Parcours interrompu par les bornes : inventaire incomplet.
    pub truncated: bool,
    /// Base CM93 trouvée (chemin relatif, `.` pour le dossier lui-même) ;
    /// son contenu n'est pas parcouru ici (voir [`crate::cm93`]).
    pub cm93: Option<String>,
}

/// Base CM93 d'un dossier ENC : le dossier lui-même ou un sous-dossier
/// jusqu'à deux niveaux (le premier trouvé dans l'ordre des noms).
pub fn find_cm93_root(root: &Path) -> Option<PathBuf> {
    if crate::cm93::is_cm93_root(root) {
        return Some(root.to_path_buf());
    }
    let subdirs = |dir: &Path| -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                    .map(|e| e.path())
                    .take(512)
                    .collect()
            })
            .unwrap_or_default();
        dirs.sort();
        dirs
    };
    for child in subdirs(root) {
        if crate::cm93::is_cm93_root(&child) {
            return Some(child);
        }
        if let Some(found) = subdirs(&child)
            .into_iter()
            .find(|d| crate::cm93::is_cm93_root(d))
        {
            return Some(found);
        }
    }
    None
}

/// Extension numérique à trois chiffres (`000` à `999`).
fn numeric_extension(path: &Path) -> Option<u32> {
    let ext = path.extension()?.to_str()?;
    if ext.len() == 3 && ext.bytes().all(|b| b.is_ascii_digit()) {
        ext.parse().ok()
    } else {
        None
    }
}

/// Chemin relatif lisible, séparateurs `/` sur tous les systèmes.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Inventorie un dossier ENC (opération bloquante : à appeler hors du
/// runtime asynchrone, voir [`scan_enc_dir`]).
///
/// # Errors
///
/// [`ChartError::DirectoryUnavailable`] si le dossier lui-même est
/// illisible ; un sous-dossier illisible est seulement compté.
pub fn scan_enc_dir_blocking(root: &Path) -> Result<EncInventory, ChartError> {
    let unavailable = |source| ChartError::DirectoryUnavailable {
        path: root.to_owned(),
        source,
    };
    let first = fs::read_dir(root).map_err(unavailable)?;

    let mut inventory = EncInventory::default();
    if crate::cm93::is_cm93_root(root) {
        inventory.cm93 = Some(".".into());
        return Ok(inventory);
    }
    let mut seen = 0usize;
    // Dossiers restant à parcourir (chemin, profondeur) ; le dossier en
    // cours est déjà ouvert.
    let mut pending: Vec<(PathBuf, usize)> = Vec::new();
    let mut current = Some((first, 0usize));

    while let Some((entries, depth)) = current.take() {
        // Cellules du dossier (nom en majuscules → indice) et mises à jour.
        let mut bases: HashMap<String, usize> = HashMap::new();
        let mut updates: HashMap<String, u32> = HashMap::new();
        for entry in entries {
            seen += 1;
            if seen > MAX_ENC_ENTRIES {
                inventory.truncated = true;
                break;
            }
            let Ok(entry) = entry else {
                inventory.unreadable_dirs += 1;
                continue;
            };
            // Type sans suivre les liens symboliques.
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                if crate::cm93::is_cm93_root(&path) {
                    // Base CM93 : signalée, pas parcourue (des dizaines de
                    // milliers de fichiers sans cellule S-57).
                    if inventory.cm93.is_none() {
                        inventory.cm93 = Some(relative(root, &path));
                    }
                } else if depth < MAX_ENC_DEPTH {
                    pending.push((path, depth + 1));
                } else {
                    inventory.truncated = true;
                }
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            let file_name = entry.file_name();
            if file_name
                .to_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(ENC_CATALOG_FILE))
            {
                inventory.catalogs += 1;
                continue;
            }
            let Some(number) = numeric_extension(&path) else {
                continue;
            };
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let key = stem.to_ascii_uppercase();
            if number == 0 {
                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                bases.insert(key, inventory.cells.len());
                inventory.cells.push(EncCell {
                    name: stem.to_owned(),
                    path: relative(root, &path),
                    size_bytes,
                    updates: 0,
                });
            } else {
                *updates.entry(key).or_default() += 1;
            }
        }
        for (key, count) in updates {
            if let Some(&index) = bases.get(&key) {
                if let Some(cell) = inventory.cells.get_mut(index) {
                    cell.updates = count;
                }
            }
        }
        if inventory.truncated && seen > MAX_ENC_ENTRIES {
            break;
        }
        // Dossier suivant ; un sous-dossier illisible est compté, sans
        // interrompre l'inventaire.
        while let Some((next, next_depth)) = pending.pop() {
            match fs::read_dir(&next) {
                Ok(entries) => {
                    current = Some((entries, next_depth));
                    break;
                }
                Err(error) => {
                    tracing::warn!(dir = %next.display(), %error, "dossier ENC illisible");
                    inventory.unreadable_dirs += 1;
                }
            }
        }
    }

    inventory
        .cells
        .sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
    Ok(inventory)
}

/// Inventorie un dossier ENC sans bloquer le runtime asynchrone.
///
/// # Errors
///
/// Voir [`scan_enc_dir_blocking`].
pub async fn scan_enc_dir(root: PathBuf) -> Result<EncInventory, ChartError> {
    let dir = root.clone();
    tokio::task::spawn_blocking(move || scan_enc_dir_blocking(&dir))
        .await
        .map_err(|error| ChartError::DirectoryUnavailable {
            path: root,
            source: std::io::Error::other(error.to_string()),
        })?
}

/// Longueur maximale d'un chemin relatif de cellule.
const MAX_CELL_PATH_LEN: usize = 1024;

/// Résout le chemin relatif d'une cellule (tel que donné par
/// l'inventaire) dans le dossier ENC, sans jamais en sortir.
///
/// Le chemin doit être relatif, à séparateurs `/`, sans composant `.`,
/// `..` ni lecteur, et désigner un fichier de base `.000` existant situé
/// dans le dossier (liens symboliques compris).
///
/// # Errors
///
/// [`ChartError::InvalidCellPath`] si le chemin est refusé ou introuvable.
pub fn resolve_cell_path(root: &Path, relative: &str) -> Result<PathBuf, ChartError> {
    let refuse = |reason: &str| ChartError::InvalidCellPath(format!("{reason} : {relative}"));
    if relative.is_empty() || relative.len() > MAX_CELL_PATH_LEN {
        return Err(refuse("longueur invalide"));
    }
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        let valid =
            !part.is_empty() && part != "." && part != ".." && !part.contains(['\\', ':', '\0']);
        if !valid {
            return Err(refuse("chemin non autorisé"));
        }
        path.push(part);
    }
    if numeric_extension(&path) != Some(0) {
        return Err(refuse("pas une cellule de base .000"));
    }
    let root = fs::canonicalize(root).map_err(|_| refuse("dossier ENC inaccessible"))?;
    let resolved = fs::canonicalize(&path).map_err(|_| refuse("cellule introuvable"))?;
    if !resolved.starts_with(&root) || !resolved.is_file() {
        return Err(refuse("cellule hors du dossier ENC"));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path, bytes: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("dossier de test");
        }
        fs::write(path, vec![0u8; bytes]).expect("fichier de test");
    }

    #[test]
    fn inventories_an_exchange_set() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let root = dir.path();
        touch(&root.join("ENC_ROOT/CATALOG.031"), 10);
        touch(&root.join("ENC_ROOT/FR501130/1/0/FR501130.000"), 1200);
        touch(&root.join("ENC_ROOT/FR501130/1/0/FR501130.001"), 10);
        touch(&root.join("ENC_ROOT/FR501130/1/0/fr501130.002"), 10);
        touch(&root.join("ENC_ROOT/FR401140/FR401140.000"), 800);
        // Bruit : extensions non numériques, mise à jour orpheline.
        touch(&root.join("ENC_ROOT/README.TXT"), 5);
        touch(&root.join("ENC_ROOT/FR401140/FR401140.TIF"), 5);
        touch(&root.join("ENC_ROOT/ORPHAN.003"), 5);
        touch(&root.join("ENC_ROOT/NOTE.0001"), 5);

        let inv = scan_enc_dir_blocking(root).expect("inventaire");
        assert_eq!(inv.catalogs, 1);
        assert!(!inv.truncated);
        assert_eq!(inv.unreadable_dirs, 0);
        assert_eq!(
            inv.cells,
            vec![
                EncCell {
                    name: "FR401140".into(),
                    path: "ENC_ROOT/FR401140/FR401140.000".into(),
                    size_bytes: 800,
                    updates: 0,
                },
                EncCell {
                    name: "FR501130".into(),
                    path: "ENC_ROOT/FR501130/1/0/FR501130.000".into(),
                    size_bytes: 1200,
                    updates: 2,
                },
            ]
        );
    }

    #[test]
    fn cm93_bases_are_found_not_walked() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Cartes").join("CM93");
        std::fs::create_dir_all(base.join("02100600").join("F")).unwrap();
        std::fs::write(base.join("CM93OBJ.DIC"), "x").unwrap();
        std::fs::write(base.join("CM93ATTR.DIC"), "x").unwrap();
        // Un .000 dans la base n'est pas une cellule S-57 de l'inventaire.
        std::fs::write(base.join("02100600").join("F").join("X.000"), "x").unwrap();
        std::fs::write(dir.path().join("FR1.000"), "x").unwrap();
        let inventory = scan_enc_dir_blocking(dir.path()).unwrap();
        assert_eq!(inventory.cells.len(), 1);
        assert_eq!(inventory.cm93.as_deref(), Some("Cartes/CM93"));
        assert_eq!(find_cm93_root(dir.path()), Some(base.clone()));
        // Base choisie directement.
        let inventory = scan_enc_dir_blocking(&base).unwrap();
        assert_eq!(inventory.cm93.as_deref(), Some("."));
        assert!(inventory.cells.is_empty());
        assert_eq!(find_cm93_root(&base), Some(base));
        assert_eq!(find_cm93_root(&dir.path().join("absent")), None);
    }

    #[test]
    fn empty_folder_has_no_cells() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let inv = scan_enc_dir_blocking(dir.path()).expect("inventaire");
        assert_eq!(inv, EncInventory::default());
    }

    #[test]
    fn missing_folder_is_an_error() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let missing = dir.path().join("absent");
        let error = scan_enc_dir_blocking(&missing).expect_err("dossier absent");
        assert!(matches!(error, ChartError::DirectoryUnavailable { .. }));
    }

    #[test]
    fn depth_is_bounded() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let mut deep = dir.path().to_owned();
        for level in 0..=MAX_ENC_DEPTH + 1 {
            deep = deep.join(format!("n{level}"));
        }
        touch(&deep.join("TOODEEP.000"), 1);
        touch(&dir.path().join("n0/SHALLOW.000"), 1);
        let inv = scan_enc_dir_blocking(dir.path()).expect("inventaire");
        assert!(inv.truncated);
        assert_eq!(inv.cells.len(), 1);
        assert_eq!(inv.cells[0].name, "SHALLOW");
    }

    #[tokio::test]
    async fn async_scan_matches_blocking_scan() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        touch(&dir.path().join("US5MA1SK.000"), 4);
        let inv = scan_enc_dir(dir.path().to_owned())
            .await
            .expect("inventaire");
        assert_eq!(inv.cells.len(), 1);
        assert_eq!(inv.cells[0].path, "US5MA1SK.000");
    }
    #[test]
    fn resolves_cells_inside_the_folder_only() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let root = dir.path().join("ENC_ROOT");
        touch(&root.join("FR501130/FR501130.000"), 10);
        touch(&root.join("FR501130/FR501130.001"), 10);
        touch(&dir.path().join("OUT.000"), 10);
        let ok = resolve_cell_path(&root, "FR501130/FR501130.000").expect("cellule");
        assert!(ok.ends_with("FR501130.000"));
        for bad in [
            "",
            "/FR501130/FR501130.000",
            "../OUT.000",
            "FR501130/../../OUT.000",
            "./FR501130/FR501130.000",
            "FR501130//FR501130.000",
            "FR501130\\FR501130.000",
            "C:/OUT.000",
            "FR501130/FR501130.001",
            "FR501130/ABSENT.000",
            "FR501130",
        ] {
            assert!(
                matches!(
                    resolve_cell_path(&root, bad),
                    Err(ChartError::InvalidCellPath(_))
                ),
                "« {bad} » accepté"
            );
        }
        let long = "A/".repeat(600) + "X.000";
        assert!(resolve_cell_path(&root, &long).is_err());
    }
}
