//! Construction de la géométrie des objets S-57 à partir de la topologie
//! vectorielle : nœuds isolés (sondes, objets ponctuels), nœuds connectés
//! et arcs (lignes, contours de surfaces).
//!
//! Les arcs sont enchaînés en coordonnées entières (avant division par
//! le facteur COMF) : la jonction de deux arcs se vérifie à l'identique.
//! Une référence manquante, un contour non fermé ou une coordonnée hors
//! des limites WGS 84 rendent l'objet inutilisable : il est écarté, aucune
//! position n'est inventée.

use std::collections::HashMap;

use super::{Position, S57Geometry, Sounding};

/// Nœud isolé.
pub(crate) const RCNM_ISOLATED_NODE: u8 = 110;
/// Nœud connecté.
pub(crate) const RCNM_CONNECTED_NODE: u8 = 120;
/// Arc.
pub(crate) const RCNM_EDGE: u8 = 130;

/// Primitive point.
pub(crate) const PRIM_POINT: u8 = 1;
/// Primitive ligne.
pub(crate) const PRIM_LINE: u8 = 2;
/// Primitive surface.
pub(crate) const PRIM_AREA: u8 = 3;
/// Pas de géométrie (objets de collection).
pub(crate) const PRIM_NONE: u8 = 255;

/// Sens inverse d'un arc.
const ORNT_REVERSE: u8 = 2;
/// Contour intérieur (trou).
const USAG_INTERIOR: u8 = 2;
/// Arc masqué (bord de cellule, à ne pas tracer).
const MASK_MASKED: u8 = 1;
/// Nœud de début d'un arc.
const TOPI_BEGIN: u8 = 1;
/// Nœud de fin d'un arc.
const TOPI_END: u8 = 2;

/// Coordonnée entière (x = longitude, y = latitude, en unités COMF).
pub(crate) type RawPoint = (i32, i32);

/// Pointeur d'un enregistrement vecteur vers un autre (champ VRPT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VectorPointer {
    pub rcnm: u8,
    pub rcid: u32,
    pub topi: u8,
}

/// Enregistrement vecteur (nœud ou arc).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RawVector {
    /// Coordonnées (SG2D ou SG3D).
    pub coords: Vec<RawPoint>,
    /// Profondeurs (SG3D), une par coordonnée ; vide pour SG2D.
    pub depths: Vec<i32>,
    /// Nœuds de début et de fin (arcs).
    pub pointers: Vec<VectorPointer>,
}

/// Pointeur d'un objet vers sa géométrie (champ FSPT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpatialPointer {
    pub rcnm: u8,
    pub rcid: u32,
    pub ornt: u8,
    pub usag: u8,
    pub mask: u8,
}

/// Enregistrements vecteurs d'une cellule, par (RCNM, RCID).
pub(crate) type Vectors = HashMap<(u8, u32), RawVector>;

/// Facteurs de conversion des coordonnées et des sondes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Scale {
    /// Facteur des coordonnées (COMF).
    pub comf: f64,
    /// Facteur des sondes (SOMF).
    pub somf: f64,
}

impl Scale {
    fn position(self, (x, y): RawPoint) -> Result<Position, String> {
        let lon = f64::from(x) / self.comf;
        let lat = f64::from(y) / self.comf;
        if !(-180.0..=180.0).contains(&lon) || !(-90.0..=90.0).contains(&lat) {
            return Err(format!("coordonnée hors limites ({lat}, {lon})"));
        }
        Ok([lon, lat])
    }

    fn line(self, points: &[RawPoint]) -> Result<Vec<Position>, String> {
        points.iter().map(|&p| self.position(p)).collect()
    }
}

fn vector(vectors: &Vectors, rcnm: u8, rcid: u32) -> Result<&RawVector, String> {
    vectors
        .get(&(rcnm, rcid))
        .ok_or_else(|| format!("enregistrement vecteur {rcnm}/{rcid} introuvable"))
}

/// Coordonnée unique d'un nœud.
fn node_point(vectors: &Vectors, rcnm: u8, rcid: u32) -> Result<RawPoint, String> {
    if rcnm != RCNM_ISOLATED_NODE && rcnm != RCNM_CONNECTED_NODE {
        return Err(format!("{rcnm}/{rcid} n'est pas un nœud"));
    }
    match vector(vectors, rcnm, rcid)?.coords.as_slice() {
        [point] => Ok(*point),
        _ => Err(format!(
            "le nœud {rcnm}/{rcid} n'a pas une coordonnée unique"
        )),
    }
}

/// Points d'un arc : nœud de début, points intermédiaires, nœud de fin,
/// dans le sens demandé.
fn edge_points(vectors: &Vectors, pointer: &SpatialPointer) -> Result<Vec<RawPoint>, String> {
    if pointer.rcnm != RCNM_EDGE {
        return Err(format!(
            "{}/{} n'est pas un arc",
            pointer.rcnm, pointer.rcid
        ));
    }
    let edge = vector(vectors, RCNM_EDGE, pointer.rcid)?;
    let begin = edge
        .pointers
        .iter()
        .find(|p| p.topi == TOPI_BEGIN)
        .or_else(|| edge.pointers.first())
        .ok_or_else(|| format!("arc {} sans nœud de début", pointer.rcid))?;
    let end = edge
        .pointers
        .iter()
        .find(|p| p.topi == TOPI_END)
        .or_else(|| edge.pointers.get(1))
        .ok_or_else(|| format!("arc {} sans nœud de fin", pointer.rcid))?;
    let mut points = Vec::with_capacity(edge.coords.len() + 2);
    points.push(node_point(vectors, begin.rcnm, begin.rcid)?);
    points.extend_from_slice(&edge.coords);
    points.push(node_point(vectors, end.rcnm, end.rcid)?);
    if pointer.ornt == ORNT_REVERSE {
        points.reverse();
    }
    Ok(points)
}

/// Objet ponctuel : une position, ou des sondes (SOUNDG, SG3D).
fn point(
    vectors: &Vectors,
    pointers: &[SpatialPointer],
    scale: Scale,
) -> Result<S57Geometry, String> {
    let [pointer] = pointers else {
        return Err(format!("objet ponctuel avec {} références", pointers.len()));
    };
    let node = vector(vectors, pointer.rcnm, pointer.rcid)?;
    if !node.depths.is_empty() {
        let soundings = node
            .coords
            .iter()
            .zip(&node.depths)
            .map(|(&p, &d)| {
                Ok(Sounding {
                    position: scale.position(p)?,
                    depth: f64::from(d) / scale.somf,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        return Ok(S57Geometry::Soundings { soundings });
    }
    let position = scale.position(node_point(vectors, pointer.rcnm, pointer.rcid)?)?;
    Ok(S57Geometry::Point { position })
}

/// Objet linéaire : arcs enchaînés ; un arc masqué interrompt la ligne.
fn line(
    vectors: &Vectors,
    pointers: &[SpatialPointer],
    scale: Scale,
) -> Result<S57Geometry, String> {
    let mut lines: Vec<Vec<RawPoint>> = Vec::new();
    let mut current: Vec<RawPoint> = Vec::new();
    for pointer in pointers {
        if pointer.mask == MASK_MASKED {
            if current.len() >= 2 {
                lines.push(std::mem::take(&mut current));
            }
            current.clear();
            continue;
        }
        let points = edge_points(vectors, pointer)?;
        if current.last().is_some() && current.last() == points.first() {
            current.extend_from_slice(&points[1..]);
        } else {
            if current.len() >= 2 {
                lines.push(std::mem::take(&mut current));
            }
            current = points;
        }
    }
    if current.len() >= 2 {
        lines.push(current);
    }
    if lines.is_empty() {
        return Err("objet linéaire sans arc".to_owned());
    }
    let lines = lines
        .iter()
        .map(|l| scale.line(l))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(S57Geometry::Line { lines })
}

/// Objet surfacique : contours fermés, extérieur d'abord. Les arcs
/// masqués font partie du contour (le masque ne concerne que le tracé).
fn area(
    vectors: &Vectors,
    pointers: &[SpatialPointer],
    scale: Scale,
) -> Result<S57Geometry, String> {
    let mut rings: Vec<(bool, Vec<RawPoint>)> = Vec::new();
    let mut current: Vec<RawPoint> = Vec::new();
    let mut exterior = true;
    for pointer in pointers {
        let points = edge_points(vectors, pointer)?;
        if current.is_empty() {
            exterior = pointer.usag != USAG_INTERIOR;
            current = points;
        } else if current.last() == points.first() {
            current.extend_from_slice(&points[1..]);
        } else {
            return Err("contour discontinu".to_owned());
        }
        if current.len() >= 4 && current.first() == current.last() {
            rings.push((exterior, std::mem::take(&mut current)));
        }
    }
    if !current.is_empty() {
        return Err("contour non fermé".to_owned());
    }
    if !rings.iter().any(|(exterior, _)| *exterior) {
        return Err("surface sans contour extérieur".to_owned());
    }
    // Extérieur(s) d'abord, ordre conservé sinon.
    rings.sort_by_key(|(exterior, _)| !exterior);
    let rings = rings
        .iter()
        .map(|(_, r)| scale.line(r))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(S57Geometry::Area { rings })
}

/// Géométrie d'un objet selon sa primitive.
pub(crate) fn build(
    prim: u8,
    pointers: &[SpatialPointer],
    vectors: &Vectors,
    scale: Scale,
) -> Result<S57Geometry, String> {
    match prim {
        PRIM_POINT => point(vectors, pointers, scale),
        PRIM_LINE => line(vectors, pointers, scale),
        PRIM_AREA => area(vectors, pointers, scale),
        PRIM_NONE => Ok(S57Geometry::None),
        other => Err(format!("primitive {other} inconnue")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALE: Scale = Scale {
        comf: 10.0,
        somf: 10.0,
    };

    fn node(vectors: &mut Vectors, rcid: u32, point: RawPoint) {
        vectors.insert(
            (RCNM_CONNECTED_NODE, rcid),
            RawVector {
                coords: vec![point],
                ..RawVector::default()
            },
        );
    }

    fn edge(vectors: &mut Vectors, rcid: u32, begin: u32, end: u32, inner: &[RawPoint]) {
        let to = |rcid, topi| VectorPointer {
            rcnm: RCNM_CONNECTED_NODE,
            rcid,
            topi,
        };
        vectors.insert(
            (RCNM_EDGE, rcid),
            RawVector {
                coords: inner.to_vec(),
                depths: Vec::new(),
                pointers: vec![to(begin, TOPI_BEGIN), to(end, TOPI_END)],
            },
        );
    }

    fn uses(rcid: u32, ornt: u8, usag: u8, mask: u8) -> SpatialPointer {
        SpatialPointer {
            rcnm: RCNM_EDGE,
            rcid,
            ornt,
            usag,
            mask,
        }
    }

    /// Carré découpé en deux arcs (A→B, B→A), trou en un arc fermé.
    fn square() -> Vectors {
        let mut v = Vectors::new();
        node(&mut v, 1, (0, 0));
        node(&mut v, 2, (10, 10));
        node(&mut v, 3, (3, 3));
        edge(&mut v, 10, 1, 2, &[(0, 10)]);
        edge(&mut v, 11, 1, 2, &[(10, 0)]);
        edge(&mut v, 12, 3, 3, &[(3, 5), (5, 5), (5, 3)]);
        v
    }

    #[test]
    fn area_rings_follow_orientation() {
        let v = square();
        let pointers = [uses(12, 1, 2, 2), uses(10, 1, 1, 2), uses(11, 2, 1, 1)];
        let S57Geometry::Area { rings } = build(PRIM_AREA, &pointers, &v, SCALE).unwrap() else {
            panic!("surface attendue");
        };
        assert_eq!(rings.len(), 2);
        // Extérieur d'abord, arc 11 parcouru à l'envers.
        assert_eq!(
            rings[0],
            [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]
        );
        assert_eq!(rings[1].first(), rings[1].last());
        assert_eq!(rings[1].len(), 5);
    }

    #[test]
    fn broken_or_open_areas_are_rejected() {
        let v = square();
        // Deuxième arc dans le mauvais sens : discontinu.
        assert!(build(
            PRIM_AREA,
            &[uses(10, 1, 1, 2), uses(11, 1, 1, 2)],
            &v,
            SCALE
        )
        .is_err());
        // Un seul arc : non fermé.
        assert!(build(PRIM_AREA, &[uses(10, 1, 1, 2)], &v, SCALE).is_err());
        // Trou seul : pas d'extérieur.
        assert!(build(PRIM_AREA, &[uses(12, 1, 2, 2)], &v, SCALE).is_err());
        // Arc inconnu.
        assert!(build(PRIM_AREA, &[uses(99, 1, 1, 2)], &v, SCALE).is_err());
    }

    #[test]
    fn lines_chain_and_break_at_masked_edges() {
        let v = square();
        let S57Geometry::Line { lines } = build(
            PRIM_LINE,
            &[uses(10, 1, 1, 2), uses(11, 2, 1, 2)],
            &v,
            SCALE,
        )
        .unwrap() else {
            panic!("ligne attendue");
        };
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), 5);
        let S57Geometry::Line { lines } = build(
            PRIM_LINE,
            &[uses(10, 1, 1, 2), uses(11, 2, 1, 1)],
            &v,
            SCALE,
        )
        .unwrap() else {
            panic!("ligne attendue");
        };
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), 3);
        assert!(build(PRIM_LINE, &[uses(11, 2, 1, 1)], &v, SCALE).is_err());
    }

    #[test]
    fn points_and_soundings() {
        let mut v = square();
        v.insert(
            (RCNM_ISOLATED_NODE, 50),
            RawVector {
                coords: vec![(1, 2), (3, 4)],
                depths: vec![55, 120],
                pointers: Vec::new(),
            },
        );
        let at = |rcnm, rcid| SpatialPointer {
            rcnm,
            rcid,
            ornt: 255,
            usag: 255,
            mask: 255,
        };
        assert_eq!(
            build(PRIM_POINT, &[at(RCNM_CONNECTED_NODE, 2)], &v, SCALE).unwrap(),
            S57Geometry::Point {
                position: [1.0, 1.0]
            }
        );
        let S57Geometry::Soundings { soundings } =
            build(PRIM_POINT, &[at(RCNM_ISOLATED_NODE, 50)], &v, SCALE).unwrap()
        else {
            panic!("sondes attendues");
        };
        assert_eq!(soundings.len(), 2);
        assert_eq!(soundings[1].position, [0.3, 0.4]);
        assert!((soundings[1].depth - 12.0).abs() < 1e-9);
        // Deux références, arc à la place d'un nœud, nœud inconnu.
        assert!(build(PRIM_POINT, &[at(120, 1), at(120, 2)], &v, SCALE).is_err());
        assert!(build(PRIM_POINT, &[at(RCNM_EDGE, 10)], &v, SCALE).is_err());
        assert!(build(PRIM_POINT, &[at(110, 7)], &v, SCALE).is_err());
        assert!(build(9, &[], &v, SCALE).is_err());
        assert_eq!(build(PRIM_NONE, &[], &v, SCALE).unwrap(), S57Geometry::None);
    }

    #[test]
    fn out_of_range_coordinates_are_rejected() {
        let mut v = Vectors::new();
        node(&mut v, 1, (1_900, 0));
        let p = SpatialPointer {
            rcnm: RCNM_CONNECTED_NODE,
            rcid: 1,
            ornt: 255,
            usag: 255,
            mask: 255,
        };
        assert!(build(PRIM_POINT, &[p], &v, SCALE).is_err());
    }
}
