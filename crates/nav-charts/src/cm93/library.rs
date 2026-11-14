//! Index des cellules d'une base CM93, tiré des seuls noms de fichiers
//! (aucune cellule n'est décodée), et choix des cellules d'une vue.
//!
//! Nom d'une cellule : `LLLLGGGG.<échelle>`, où `LLLL / 3 - 90` est la
//! latitude sud et `GGGG / 3` la longitude ouest (0 à 360 vers l'est), en
//! degrés. Le premier chiffre peut être remplacé par une lettre pour les
//! sous-cellules (même emprise, contenu distinct).

use std::path::Path;

use serde::Serialize;

use super::Cm93Scale;

/// Nombre maximal de cellules indexées.
pub const MAX_INDEXED_CM93_CELLS: usize = 100_000;

/// Cellule de l'index.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cm93CellRef {
    /// Chemin relatif à la racine de la base, séparé par `/`.
    pub path: String,
    /// Nom du fichier.
    pub name: String,
    /// Niveau d'échelle.
    #[serde(skip)]
    pub scale: Cm93Scale,
    /// Emprise théorique : ouest, sud, est, nord (degrés, -180 à 180).
    pub bounds: [f64; 4],
}

/// Index d'une base CM93.
#[derive(Debug, Clone, Default)]
pub struct Cm93Index {
    /// Cellules trouvées.
    pub cells: Vec<Cm93CellRef>,
    /// Index limité à [`MAX_INDEXED_CM93_CELLS`].
    pub truncated: bool,
    /// Dossiers ou fichiers au nom non conforme, ignorés.
    pub ignored: usize,
}

/// Emprise d'une cellule d'après son nom (`None` : nom non conforme).
pub fn cell_bounds(stem: &str, scale: Cm93Scale) -> Option<[f64; 4]> {
    let bytes = stem.as_bytes();
    if bytes.len() != 8 || !bytes[1..].iter().all(u8::is_ascii_digit) {
        return None;
    }
    if !(bytes[0].is_ascii_digit() || bytes[0].is_ascii_uppercase()) {
        return None;
    }
    let lat_index: u32 = format!("0{}", &stem[1..4]).parse().ok()?;
    let lon_index: u32 = stem[4..].parse().ok()?;
    if lat_index > 540 || lon_index >= 1080 {
        return None;
    }
    let span = f64::from(scale.cell_span()) / 3.0;
    let south = f64::from(lat_index) / 3.0 - 90.0;
    let mut west = f64::from(lon_index) / 3.0;
    if west >= 180.0 {
        west -= 360.0;
    }
    // Une cellule à cheval sur l'antiméridien est bornée à 180°.
    let east = (west + span).min(180.0);
    let north = (south + span).min(90.0);
    Some([west, south, east, north])
}

/// Inventorie une base CM93 (appel bloquant ; parcours borné).
pub fn scan_cm93_cells(root: &Path) -> std::io::Result<Cm93Index> {
    let mut index = Cm93Index::default();
    let mut blocks: Vec<_> = std::fs::read_dir(root)?.flatten().collect();
    blocks.sort_by_key(|e| e.file_name());
    'blocks: for block in blocks {
        let Some(block_name) = block.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !block.path().is_dir() {
            continue;
        }
        if block_name.len() != 8 || !block_name.bytes().all(|b| b.is_ascii_digit()) {
            index.ignored += 1;
            continue;
        }
        let Ok(levels) = std::fs::read_dir(block.path()) else {
            index.ignored += 1;
            continue;
        };
        for level in levels.flatten() {
            let path = level.path();
            if !path.is_dir() {
                continue;
            }
            let name = level.file_name();
            let Some(scale) = name
                .to_str()
                .filter(|n| n.len() == 1)
                .and_then(|n| n.chars().next())
                .and_then(Cm93Scale::from_letter)
            else {
                index.ignored += 1;
                continue;
            };
            let Ok(files) = std::fs::read_dir(&path) else {
                index.ignored += 1;
                continue;
            };
            for file in files.flatten() {
                let file_name = file.file_name();
                let Some(file_name) = file_name.to_str() else {
                    index.ignored += 1;
                    continue;
                };
                let parsed = file_name.rsplit_once('.').and_then(|(stem, ext)| {
                    let same_scale = ext.len() == 1
                        && ext.chars().next().and_then(Cm93Scale::from_letter) == Some(scale);
                    same_scale.then(|| cell_bounds(stem, scale)).flatten()
                });
                let Some(bounds) = parsed else {
                    index.ignored += 1;
                    continue;
                };
                if index.cells.len() >= MAX_INDEXED_CM93_CELLS {
                    index.truncated = true;
                    break 'blocks;
                }
                index.cells.push(Cm93CellRef {
                    path: format!("{block_name}/{}/{file_name}", scale.letter()),
                    name: file_name.to_owned(),
                    scale,
                    bounds,
                });
            }
        }
    }
    Ok(index)
}

/// Largeur de vue maximale (degrés) pour laquelle un niveau est dessiné.
fn max_view_span(scale: Cm93Scale) -> f64 {
    match scale {
        Cm93Scale::Z => f64::INFINITY,
        Cm93Scale::A => 180.0,
        Cm93Scale::B => 60.0,
        Cm93Scale::C => 20.0,
        Cm93Scale::D => 6.0,
        Cm93Scale::E => 3.0,
        Cm93Scale::F => 1.2,
        Cm93Scale::G => 0.5,
    }
}

/// Emprise d'une vue : ouest, sud, est, nord (l'ouest peut dépasser l'est
/// si la vue est à cheval sur l'antiméridien).
pub type ViewBounds = [f64; 4];

fn intersects(cell: &[f64; 4], view: &ViewBounds) -> bool {
    let [w, s, e, n] = *cell;
    let [vw, vs, ve, vn] = *view;
    if n < vs || s > vn {
        return false;
    }
    let overlaps = |west: f64, east: f64| e >= west && w <= east;
    if vw <= ve {
        overlaps(vw, ve)
    } else {
        overlaps(vw, 180.0) || overlaps(-180.0, ve)
    }
}

/// Cellules d'une vue choisie.
#[derive(Debug, Clone, Default)]
pub struct Cm93Selection<'a> {
    /// Cellules à dessiner, de la moins à la plus détaillée.
    pub cells: Vec<&'a Cm93CellRef>,
    /// Cellules de niveaux adaptés à la vue mais non retenues (trop
    /// nombreuses) : la vue doit être agrandie pour les voir.
    pub omitted: usize,
}

impl Cm93Index {
    /// Cellules à dessiner pour une vue : niveaux adaptés à son étendue,
    /// du moins au plus détaillé, au plus `max` cellules. Un niveau qui ne
    /// tient pas en entier n'est pas dessiné, ni les plus détaillés.
    pub fn select(&self, view: &ViewBounds, max: usize) -> Cm93Selection<'_> {
        let [vw, vs, ve, vn] = *view;
        let width = if vw <= ve { ve - vw } else { ve + 360.0 - vw };
        let extent = width.max(vn - vs);
        let mut selection = Cm93Selection::default();
        let mut full = false;
        for scale in Cm93Scale::ALL {
            if extent > max_view_span(scale) {
                continue;
            }
            let level: Vec<&Cm93CellRef> = self
                .cells
                .iter()
                .filter(|c| c.scale == scale && intersects(&c.bounds, view))
                .collect();
            if full || selection.cells.len() + level.len() > max {
                full = true;
                selection.omitted += level.len();
                continue;
            }
            selection.cells.extend(level);
        }
        selection
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_from_names() {
        // Papeete : 1/3° au 1:20 000.
        let b = cell_bounds("02170631", Cm93Scale::F).unwrap();
        assert!((b[0] - (-149.6667)).abs() < 1e-3 && (b[1] - (-17.6667)).abs() < 1e-3);
        assert!((b[2] - b[0] - 1.0 / 3.0).abs() < 1e-9);
        // Sous-cellule : même emprise.
        assert_eq!(cell_bounds("A2170631", Cm93Scale::F), Some(b));
        // Bloc Z de 40°.
        let z = cell_bounds("00300240", Cm93Scale::Z).unwrap();
        assert_eq!(z, [80.0, -80.0, 120.0, -40.0]);
        // Antiméridien : borné à 180.
        let east = cell_bounds("02100539", Cm93Scale::B).unwrap();
        assert_eq!(east[2], 180.0);
        for bad in [
            "0217063",
            "021706311",
            "02170x31",
            "a2170631",
            "05510000",
            "00001080",
        ] {
            assert_eq!(cell_bounds(bad, Cm93Scale::F), None, "{bad}");
        }
    }

    fn cell(stem: &str, scale: Cm93Scale) -> Cm93CellRef {
        Cm93CellRef {
            path: format!("x/{}/{stem}.{}", scale.letter(), scale.letter()),
            name: stem.into(),
            scale,
            bounds: cell_bounds(stem, scale).unwrap(),
        }
    }

    fn index() -> Cm93Index {
        Cm93Index {
            cells: vec![
                cell("02100600", Cm93Scale::Z),
                cell("02100600", Cm93Scale::A),
                cell("02160630", Cm93Scale::C),
                cell("02160630", Cm93Scale::D),
                cell("02170631", Cm93Scale::E),
                cell("02170631", Cm93Scale::F),
                cell("A2170631", Cm93Scale::F),
                cell("02170631", Cm93Scale::G),
                cell("00300240", Cm93Scale::Z),
            ],
            ..Cm93Index::default()
        }
    }

    #[test]
    fn selects_levels_for_the_view() {
        let index = index();
        let names = |s: &Cm93Selection| {
            s.cells
                .iter()
                .map(|c| format!("{}{}", c.scale.letter(), c.name))
                .collect::<Vec<_>>()
        };
        // Port de Papeete : tous les niveaux, du moins au plus détaillé.
        let harbour = [-149.6, -17.55, -149.5, -17.45];
        let s = index.select(&harbour, 16);
        assert_eq!(
            names(&s),
            [
                "Z02100600",
                "A02100600",
                "C02160630",
                "D02160630",
                "E02170631",
                "F02170631",
                "FA2170631",
                "G02170631"
            ]
        );
        // Vue large : niveaux détaillés non dessinés.
        let wide = [-155.0, -22.0, -140.0, -10.0];
        assert_eq!(
            names(&index.select(&wide, 16)),
            ["Z02100600", "A02100600", "C02160630"]
        );
        // Limite atteinte : le niveau qui ne tient pas et les suivants sont
        // omis et comptés.
        let s = index.select(&harbour, 5);
        assert_eq!(s.cells.len(), 5);
        assert_eq!(s.omitted, 3);
        // Ailleurs : rien.
        assert!(index.select(&[0.0, 40.0, 1.0, 41.0], 16).cells.is_empty());
    }

    #[test]
    fn view_across_the_antimeridian() {
        let index = Cm93Index {
            cells: vec![
                cell("02700537", Cm93Scale::E),
                cell("02700540", Cm93Scale::E),
                cell("02700000", Cm93Scale::E),
            ],
            ..Cm93Index::default()
        };
        // Cellules à 179°E et à 180° ; la vue va de 179°E à 179,5°W.
        let s = index.select(&[179.0, 0.0, -179.5, 0.3], 16);
        let names: Vec<_> = s.cells.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["02700537", "02700540"]);
    }

    #[test]
    fn scans_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let level = dir.path().join("02100600").join("F");
        std::fs::create_dir_all(&level).unwrap();
        std::fs::write(level.join("02170631.F"), b"x").unwrap();
        std::fs::write(level.join("A2170631.F"), b"x").unwrap();
        std::fs::write(level.join("02170631.G"), b"x").unwrap();
        std::fs::write(level.join("notes.txt"), b"x").unwrap();
        std::fs::create_dir_all(dir.path().join("divers")).unwrap();
        std::fs::create_dir_all(dir.path().join("02100600").join("XY")).unwrap();
        let index = scan_cm93_cells(dir.path()).unwrap();
        let mut paths: Vec<_> = index.cells.iter().map(|c| c.path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, ["02100600/F/02170631.F", "02100600/F/A2170631.F"]);
        assert_eq!(index.ignored, 4);
        assert!(!index.truncated);
        assert!(scan_cm93_cells(&dir.path().join("absent")).is_err());
    }
}
