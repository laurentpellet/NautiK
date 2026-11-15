//! Objets CM93 → entités affichées, avec les mêmes catégories et
//! propriétés que les cellules S-57 (`s57::display`).

use super::{AttrValue, Cm93Cell, Cm93Feature, Cm93Geometry};
use crate::s57::display::{category_of, sounding_label, MapFeature, MapGeometry, MapProperties};
use crate::s57::Position;

/// Classe S-57 équivalente d'une classe propre à CM93.
fn s57_class(class: &str) -> &str {
    match class {
        // Zone découvrante : zone de profondeur, sans profondeur connue
        // (dessinée comme petits fonds).
        "ITDARE" => "DEPARE",
        other => other,
    }
}

/// Couleur S-57 (COLOUR) de la première couleur d'un code COLMAR CM93.
fn colmar_colour(code: i64) -> Option<u8> {
    Some(match code {
        1 | 12 | 16 | 18 => 4,
        2 | 7 | 8 | 14 => 2,
        3 | 11 | 13 | 17 => 3,
        4 | 9 | 10 | 15 => 6,
        5 => 1,
        6 => 11,
        _ => return None,
    })
}

/// Aire signée d'un contour (positive : sens antihoraire).
fn signed_area(ring: &[Position]) -> f64 {
    ring.windows(2)
        .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
        .sum::<f64>()
        / 2.0
}

/// Le point est dans le contour (règle pair-impair).
fn contains(ring: &[Position], [x, y]: Position) -> bool {
    let mut inside = false;
    for w in ring.windows(2) {
        let ([x1, y1], [x2, y2]) = (w[0], w[1]);
        if (y1 > y) != (y2 > y) && x < x1 + (y - y1) * (x2 - x1) / (y2 - y1) {
            inside = !inside;
        }
    }
    inside
}

/// Contours d'une surface ordonnés pour l'affichage : le sens des
/// contours CM93 n'est pas fiable, il est déduit de leur imbrication.
/// Chaque contour extérieur (antihoraire) est suivi de ses trous
/// (horaires) ; un îlot dans un trou redevient extérieur.
fn orient_rings(rings: &[Vec<Position>]) -> Vec<Vec<Position>> {
    if rings.len() == 1 {
        return rings.to_vec();
    }
    let mut order: Vec<usize> = (0..rings.len()).collect();
    let area = |i: usize| signed_area(&rings[i]).abs();
    order.sort_by(|&a, &b| area(b).total_cmp(&area(a)));
    // Parent : le plus petit contour plus grand qui contient le premier
    // point ; profondeur : nombre de contours englobants.
    let mut parent = vec![None; rings.len()];
    let mut depth = vec![0usize; rings.len()];
    for (k, &i) in order.iter().enumerate() {
        let Some(&probe) = rings[i].first() else {
            continue;
        };
        for &j in order[..k].iter().rev() {
            if contains(&rings[j], probe) {
                parent[i] = Some(j);
                depth[i] = depth[j] + 1;
                break;
            }
        }
    }
    let oriented = |i: usize, outer: bool| {
        let mut ring = rings[i].clone();
        if (signed_area(&ring) > 0.0) != outer {
            ring.reverse();
        }
        ring
    };
    let mut out = Vec::with_capacity(rings.len());
    for &i in &order {
        if depth[i] % 2 == 0 {
            out.push(oriented(i, true));
            for &h in &order {
                if parent[h] == Some(i) {
                    out.push(oriented(h, false));
                }
            }
        }
    }
    out
}

fn number(feature: &Cm93Feature, name: &str) -> Option<f64> {
    feature.attribute(name).and_then(AttrValue::as_f64)
}

fn colour(feature: &Cm93Feature) -> Option<u8> {
    match feature.attribute("COLOUR") {
        Some(AttrValue::List(list)) => return list.first().copied(),
        Some(AttrValue::Int(v)) => return u8::try_from(*v).ok(),
        _ => {}
    }
    match feature.attribute("COLMAR") {
        Some(AttrValue::Int(code)) => colmar_colour(*code),
        _ => None,
    }
}

fn name(feature: &Cm93Feature) -> Option<String> {
    match feature.attribute("OBJNAM") {
        Some(AttrValue::Text(t)) if !t.trim().is_empty() => Some(t.trim().to_owned()),
        _ => None,
    }
}

/// Entités affichables d'une cellule CM93. `cell` : nom de la cellule ;
/// `rank` : rang de son échelle (le plus détaillé dessiné par-dessus).
pub fn map_features(cell: &Cm93Cell, cell_name: &str, rank: u8) -> Vec<MapFeature> {
    let mut out = Vec::new();
    for feature in &cell.features {
        let class = s57_class(&feature.class);
        let Some(cat) = category_of(class) else {
            continue;
        };
        let base = MapProperties {
            cat,
            class: class.to_owned(),
            usage: rank,
            cell: cell_name.to_owned(),
            depth: number(feature, "VALSOU"),
            label: None,
            drval1: number(feature, "DRVAL1"),
            drval2: number(feature, "DRVAL2"),
            valdco: number(feature, "VALDCO"),
            name: name(feature),
            colour: colour(feature),
        };
        let geometry = match &feature.geometry {
            Cm93Geometry::None => continue,
            Cm93Geometry::Point(p) => MapGeometry::Point { coordinates: *p },
            Cm93Geometry::Line(lines) if !lines.is_empty() => MapGeometry::MultiLineString {
                coordinates: lines.clone(),
            },
            Cm93Geometry::Area(rings) if !rings.is_empty() => MapGeometry::Polygon {
                coordinates: orient_rings(rings),
            },
            Cm93Geometry::Line(_) | Cm93Geometry::Area(_) => continue,
            Cm93Geometry::Soundings(soundings) => {
                for &(position, depth) in soundings {
                    out.push(MapFeature {
                        kind: "Feature",
                        geometry: MapGeometry::Point {
                            coordinates: position,
                        },
                        properties: MapProperties {
                            depth: Some(depth),
                            label: Some(sounding_label(depth)),
                            ..base.clone()
                        },
                    });
                }
                continue;
            }
        };
        out.push(MapFeature {
            kind: "Feature",
            geometry,
            properties: base,
        });
    }
    out
}

/// Zone réellement couverte par les données d'une cellule : ses surfaces
/// de source (`_m_sor`, ou `M_COVR` de couverture). Une cellule détaillée
/// ne couvre souvent qu'une partie de son emprise (ports, baies).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cm93Coverage {
    /// Surfaces : contours (règle pair-impair) et leur emprise
    /// `[ouest, sud, est, nord]`.
    areas: Vec<(Vec<Vec<Position>>, [f64; 4])>,
}

fn ring_bounds(rings: &[Vec<Position>]) -> [f64; 4] {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for &[x, y] in rings.iter().flatten() {
        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    }
    b
}

impl Cm93Coverage {
    /// Couverture d'une cellule décodée.
    pub fn of(cell: &Cm93Cell) -> Self {
        let areas = cell
            .features
            .iter()
            .filter(|f| {
                f.class == "_m_sor"
                    || (f.class == "M_COVR"
                        && !matches!(f.attribute("CATCOV"), Some(AttrValue::Int(2))))
            })
            .filter_map(|f| match &f.geometry {
                Cm93Geometry::Area(rings) if !rings.is_empty() => {
                    Some((rings.clone(), ring_bounds(rings)))
                }
                _ => None,
            })
            .collect();
        Self { areas }
    }

    /// Le point est dans la zone couverte (aussi sous une longitude décalée
    /// de 360° : de part et d'autre de l'antiméridien, deux cellules
    /// décrivent le même lieu avec des longitudes différentes).
    pub fn contains(&self, [x, y]: Position) -> bool {
        [x, x - 360.0, x + 360.0]
            .into_iter()
            .any(|x| self.contains_exactly([x, y]))
    }

    fn contains_exactly(&self, [x, y]: Position) -> bool {
        self.areas.iter().any(|(rings, [w, s, e, n])| {
            x >= *w
                && x <= *e
                && y >= *s
                && y <= *n
                && rings.iter().filter(|r| contains(r, [x, y])).count() % 2 == 1
        })
    }
}

/// Points d'une entité affichée.
fn vertices(geometry: &MapGeometry) -> Box<dyn Iterator<Item = &Position> + '_> {
    match geometry {
        MapGeometry::Point { coordinates } => Box::new(std::iter::once(coordinates)),
        MapGeometry::MultiLineString { coordinates } | MapGeometry::Polygon { coordinates } => {
            Box::new(coordinates.iter().flatten())
        }
    }
}

/// L'entité est entièrement dans la zone couverte par des cellules plus
/// détaillées (qui la dessinent mieux) : elle n'est pas dessinée. Une
/// entité qui en déborde, même en partie, reste dessinée.
pub fn covered_by(feature: &MapFeature, finer: &[&Cm93Coverage]) -> bool {
    !finer.is_empty() && vertices(&feature.geometry).all(|&p| finer.iter().any(|c| c.contains(p)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, x1: f64, ccw: bool) -> Vec<Position> {
        let mut ring = vec![[x0, x0], [x1, x0], [x1, x1], [x0, x1], [x0, x0]];
        if !ccw {
            ring.reverse();
        }
        ring
    }

    fn cell(features: Vec<(&str, Cm93Geometry)>) -> Cm93Cell {
        Cm93Cell {
            bounds: [0.0, 0.0, 20.0, 20.0],
            features: features
                .into_iter()
                .map(|(class, geometry)| Cm93Feature {
                    class: class.to_owned(),
                    geometry,
                    attributes: Vec::new(),
                })
                .collect(),
            skipped_features: 0,
            declared_offset_m: 0.0,
            data_date: None,
        }
    }

    #[test]
    fn coarse_features_inside_finer_coverage_are_hidden() {
        // Cellule détaillée : couverture 0–10 avec un trou 4–6 (non
        // couvert), et une surface de terre qui n'est pas une couverture.
        let fine = cell(vec![
            (
                "_m_sor",
                Cm93Geometry::Area(vec![square(0.0, 10.0, true), square(4.0, 6.0, false)]),
            ),
            ("LNDARE", Cm93Geometry::Area(vec![square(12.0, 18.0, true)])),
        ]);
        let coverage = Cm93Coverage::of(&fine);
        assert!(coverage.contains([1.0, 1.0]));
        assert!(!coverage.contains([5.0, 5.0]), "trou");
        assert!(!coverage.contains([15.0, 15.0]), "terre, pas couverture");
        assert!(!coverage.contains([11.0, 1.0]));
        // Même lieu de l'autre côté de l'antiméridien.
        assert!(coverage.contains([361.0, 1.0]));
        assert!(coverage.contains([-359.0, 1.0]));

        let coarse = cell(vec![
            ("LNDARE", Cm93Geometry::Area(vec![square(1.0, 3.0, true)])),
            ("LNDARE", Cm93Geometry::Area(vec![square(8.0, 12.0, true)])),
            ("BOYLAT", Cm93Geometry::Point([2.0, 8.0])),
            ("BOYLAT", Cm93Geometry::Point([5.0, 5.0])),
            (
                "COALNE",
                Cm93Geometry::Line(vec![vec![[1.0, 1.0], [9.0, 1.0]]]),
            ),
            (
                "COALNE",
                Cm93Geometry::Line(vec![vec![[1.0, 1.0], [15.0, 1.0]]]),
            ),
        ]);
        let hidden: Vec<bool> = map_features(&coarse, "C", 5)
            .iter()
            .map(|f| covered_by(f, &[&coverage]))
            .collect();
        // Entièrement couverts : cachés ; débordant ou dans le trou : gardés.
        assert_eq!(hidden, [true, false, true, false, true, false]);
        // Sans cellule plus détaillée, rien n'est caché.
        assert!(map_features(&coarse, "C", 5)
            .iter()
            .all(|f| !covered_by(f, &[])));
        // Une cellule sans couverture déclarée ne cache rien.
        assert!(!Cm93Coverage::of(&cell(vec![])).contains([1.0, 1.0]));
    }
    #[test]
    fn rings_are_ordered_and_oriented_by_nesting() {
        // Îlot (dans le trou), trou, extérieur, autre extérieur : sens
        // quelconques dans le fichier.
        let island = square(4.0, 6.0, false);
        let hole = square(2.0, 8.0, true);
        let outer = square(0.0, 10.0, false);
        let other = square(20.0, 21.0, false);
        let out = orient_rings(&[island.clone(), hole.clone(), outer.clone(), other]);
        assert_eq!(out.len(), 4);
        // Extérieur puis son trou, puis l'îlot, puis l'autre extérieur.
        assert_eq!(out[0][0], [0.0, 0.0]);
        assert!(signed_area(&out[0]) > 0.0);
        assert!(signed_area(&out[1]) < 0.0);
        assert_eq!(signed_area(&out[1]).abs(), 36.0);
        assert!(signed_area(&out[2]) > 0.0);
        assert_eq!(signed_area(&out[2]).abs(), 4.0);
        assert!(signed_area(&out[3]) > 0.0);
        // Un seul contour : inchangé.
        assert_eq!(orient_rings(std::slice::from_ref(&hole)), vec![hole]);
        assert!(contains(&outer, [5.0, 5.0]) && !contains(&outer, [15.0, 5.0]));
    }
}
