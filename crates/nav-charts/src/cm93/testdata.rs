//! Fabrication de cellules CM93 pour les tests.

use super::table::ENCODE;
use super::{Cm93Dictionary, CM93_RADIUS_M};

/// Dictionnaires réduits (codes de la base réelle).
pub(crate) const OBJECTS: &str = "\
BOYLAT|18|P|8|Buoy, lateral|COLMAR|OBJNAM\r\n\
COALNE|35|L|7|Coastline\r\n\
DEPARE|44|A|1|Depth area|DRVAL1|DRVAL2\r\n\
LNDARE|81|PA|1|Land area\r\n\
SOUNDG|147|S|7|Sounding\r\n\
WRECKS|176|PA|8|Wreck|VALSOU|OBJNAM\r\n\
ITDARE|190|A|1|Intertidal area\r\n\
_m_sor|217|A|0|Source\r\n\
\r\n";

pub(crate) const ATTRIBUTES: &str = "\
COLMAR|60|aBYTE|1|18\r\n\
COLOUR|59|aLIST|1|55\r\n\
DRVAL1|66|aFLOAT|0.0|12000.0\r\n\
DRVAL2|67|aFLOAT|0.0|12000.0\r\n\
OBJNAM|80|aSTRING\r\n\
RECDAT|96|aSTRING\r\n\
SCAMIN|99|aNotUsed\r\n\
VALSOU|116|aWORD10|0|12000.0\r\n\
_wgsox|178|aFLOAT|-5000.0|5000.0\r\n\
_wgsoy|179|aFLOAT|-5000.0|5000.0\r\n\
_cmplx|181|aCMPLX\r\n\
_long1|182|aLONG\r\n";

pub(crate) fn dictionary() -> Cm93Dictionary {
    Cm93Dictionary::parse(OBJECTS, ATTRIBUTES).unwrap()
}

/// Mercator sphérique d'une position.
pub(crate) fn mercator(lon: f64, lat: f64) -> (f64, f64) {
    let e = CM93_RADIUS_M * lon.to_radians();
    let n = CM93_RADIUS_M
        * (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0)
            .tan()
            .ln();
    (e, n)
}

/// Attribut codé : code et octets de la valeur.
pub(crate) type Attr = (u8, Vec<u8>);

/// Cellule à fabriquer (contenu en clair).
pub(crate) struct CellBuilder {
    /// Ouest, sud, est, nord (longitudes 0 à 360).
    pub bounds: [f64; 4],
    pub vectors: Vec<Vec<(u16, u16)>>,
    pub points3d: Vec<Vec<(u16, u16, u16)>>,
    pub points2d: Vec<(u16, u16)>,
    pub features: Vec<Vec<u8>>,
}

/// Enregistrement d'objet.
pub(crate) fn feature(class: u8, geotype: u8, geometry: &[u16], attrs: &[Attr]) -> Vec<u8> {
    let mut body = Vec::new();
    for v in geometry {
        body.extend_from_slice(&v.to_le_bytes());
    }
    if geotype & 0x80 != 0 {
        body.push(attrs.len() as u8);
        for (code, value) in attrs {
            body.push(*code);
            body.extend_from_slice(value);
        }
    }
    let mut record = vec![class, geotype];
    record.extend_from_slice(&((body.len() + 4) as u16).to_le_bytes());
    record.extend(body);
    record
}

pub(crate) fn text(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

impl CellBuilder {
    /// Cellule au 1:20 000 devant Papeete : une bouée, un trait de côte
    /// en deux segments (le second à l'envers), une zone de profondeur
    /// avec un trou, des sondes, une épave, une couverture déclarant un
    /// décalage WGS 84, un objet de classe inconnue et un objet aux
    /// références hors table.
    pub(crate) fn sample() -> Self {
        let square = |a: u16, b: u16| vec![(a, a), (b, a), (b, b), (a, b), (a, a)];
        Self {
            bounds: [210.333_333, -17.666_667, 210.666_667, -17.333_333],
            vectors: vec![
                vec![(0, 0), (100, 100)],
                vec![(300, 300), (100, 100)],
                square(1000, 60000),
                vec![
                    (2000, 2000),
                    (2000, 3000),
                    (3000, 3000),
                    (3000, 2000),
                    (2000, 2000),
                ],
            ],
            points3d: vec![vec![(30000, 30000, 55), (31000, 31000, 12034)]],
            points2d: vec![(32767, 32767), (40000, 20000)],
            features: vec![
                // Bouée latérale verte (COLMAR 1), nommée.
                feature(18, 0x81, &[0], &[(60, vec![1]), (80, text("PA 3"))]),
                // Trait de côte : segment 0, puis segment 1 à l'envers.
                feature(35, 0x02, &[2, 0, 1 | 0x8000], &[]),
                // Zone 5–10 m : contour, puis trou à l'envers.
                feature(
                    44,
                    0x84,
                    &[2, 2, 3 | 0x8000],
                    &[
                        (66, 5.0f32.to_le_bytes().to_vec()),
                        (67, 10.0f32.to_le_bytes().to_vec()),
                    ],
                ),
                // Sondes.
                feature(147, 0x08, &[0], &[]),
                // Épave par 3,5 m.
                feature(176, 0x81, &[1], &[(116, 35u16.to_le_bytes().to_vec())]),
                // Couverture : décalage déclaré, date.
                feature(
                    217,
                    0x84,
                    &[1, 2],
                    &[
                        (178, (-12.5f32).to_le_bytes().to_vec()),
                        (179, 40.0f32.to_le_bytes().to_vec()),
                        (96, text("20140612")),
                    ],
                ),
                // Classe absente du dictionnaire : écartée.
                feature(250, 0x01, &[0], &[]),
                // Référence hors table : écartée.
                feature(81, 0x04, &[1, 9], &[]),
            ],
        }
    }

    /// Octets en clair.
    pub(crate) fn plain(&self) -> Vec<u8> {
        let [w, s, e, n] = self.bounds;
        let (e_min, n_min) = mercator(w, s);
        let (e_max, n_max) = mercator(e, n);
        let mut body = Vec::new();
        for v in [w, s, e, n, e_min, n_min, e_max, n_max] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        let u16le = |b: &mut Vec<u8>, v: usize| b.extend_from_slice(&(v as u16).to_le_bytes());
        let i32le = |b: &mut Vec<u8>, v: usize| b.extend_from_slice(&(v as i32).to_le_bytes());
        let n_points: usize = self.vectors.iter().map(Vec::len).sum();
        u16le(&mut body, self.vectors.len());
        i32le(&mut body, n_points);
        i32le(&mut body, 0);
        i32le(&mut body, 0);
        u16le(&mut body, self.points3d.len());
        i32le(&mut body, 0);
        i32le(&mut body, 0);
        u16le(&mut body, self.points2d.len());
        u16le(&mut body, 0);
        u16le(&mut body, 0);
        u16le(&mut body, self.features.len());
        body.extend_from_slice(&[0; 32]);
        assert_eq!(body.len(), 128);
        let mut tables = Vec::new();
        for v in &self.vectors {
            u16le(&mut tables, v.len());
            for &(x, y) in v {
                u16le(&mut tables, usize::from(x));
                u16le(&mut tables, usize::from(y));
            }
        }
        for v in &self.points3d {
            u16le(&mut tables, v.len());
            for &(x, y, z) in v {
                for c in [x, y, z] {
                    u16le(&mut tables, usize::from(c));
                }
            }
        }
        for &(x, y) in &self.points2d {
            u16le(&mut tables, usize::from(x));
            u16le(&mut tables, usize::from(y));
        }
        let split = tables.len();
        for f in &self.features {
            tables.extend_from_slice(f);
        }
        let mut out = Vec::new();
        u16le(&mut out, 138);
        i32le(&mut out, split);
        i32le(&mut out, tables.len() - split);
        out.extend(body);
        out.extend(tables);
        out
    }

    /// Octets du fichier (substitués).
    pub(crate) fn build(&self) -> Vec<u8> {
        encode(&self.plain())
    }
}

/// Substitue des octets en clair.
pub(crate) fn encode(plain: &[u8]) -> Vec<u8> {
    plain.iter().map(|&b| ENCODE[usize::from(b)]).collect()
}
