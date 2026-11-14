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
