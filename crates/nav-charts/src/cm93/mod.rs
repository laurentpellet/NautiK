//! Cartes CM93 (C-MAP, version 2), lues hors ligne.
//!
//! Une base CM93 est un dossier contenant les dictionnaires
//! `CM93OBJ.DIC` (classes d'objets, acronymes S-57) et `CM93ATTR.DIC`
//! (attributs et types), et des cellules rangées par blocs :
//! `LLLLGGGG/<échelle>/LLLLGGGG.<échelle>`. Les huit niveaux d'échelle
//! vont de Z (1:20 000 000) à G (1:7 500). Chaque octet des cellules est
//! substitué par une table fixe (voir [`table`]).
//!
//! Une cellule contient, après un prologue de contrôle et un en-tête de
//! 128 octets (emprise géographique, emprise en Mercator sphérique,
//! nombres d'enregistrements), des segments, des groupes de sondes, des
//! points et des objets. Les coordonnées sont des entiers 16 bits dans
//! l'emprise Mercator de la cellule.
//!
//! Ces cartes ne sont pas officielles et souvent anciennes : l'interface
//! le signale. Aucune donnée n'est inventée : un objet incohérent est
//! écarté et compté, une cellule incohérente est refusée.

mod display;
mod library;
mod table;

#[cfg(test)]
pub(crate) mod testdata;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, PI};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::s57::Position;

pub use display::map_features;
pub use library::{
    cell_bounds, scan_cm93_cells, Cm93CellRef, Cm93Index, Cm93Selection, ViewBounds,
};

/// Demi-grand axe de la sphère de projection des cellules (ellipsoïde
/// international de 1924), en mètres.
pub const CM93_RADIUS_M: f64 = 6_378_388.0;

/// Taille du prologue et de l'en-tête.
const HEADER_LEN: usize = 138;

/// Taille maximale acceptée pour une cellule.
pub const MAX_CELL_BYTES: usize = 16 * 1024 * 1024;

/// Erreur de lecture CM93.
#[derive(Debug, Error)]
pub enum Cm93Error {
    /// Dictionnaire absent ou mal formé.
    #[error("dictionnaire CM93 illisible : {0}")]
    Dictionary(String),
    /// Cellule plus courte que ce qu'elle annonce.
    #[error("cellule CM93 tronquée ({0})")]
    Truncated(&'static str),
    /// Cellule incohérente.
    #[error("cellule CM93 corrompue : {0}")]
    Corrupt(String),
    /// Nom de fichier ou de dossier non conforme.
    #[error("nom de cellule CM93 invalide : {0}")]
    InvalidName(String),
    /// Lecture impossible.
    #[error("lecture impossible de {path} : {source}")]
    Io {
        /// Fichier concerné.
        path: PathBuf,
        /// Cause système.
        #[source]
        source: std::io::Error,
    },
}

/// Niveau d'échelle d'une cellule, du moins au plus détaillé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cm93Scale {
    /// 1:20 000 000.
    Z,
    /// 1:3 000 000.
    A,
    /// 1:1 000 000.
    B,
    /// 1:200 000.
    C,
    /// 1:100 000.
    D,
    /// 1:50 000.
    E,
    /// 1:20 000.
    F,
    /// 1:7 500.
    G,
}

impl Cm93Scale {
    /// Tous les niveaux, du moins au plus détaillé.
    pub const ALL: [Cm93Scale; 8] = [
        Self::Z,
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::E,
        Self::F,
        Self::G,
    ];

    /// Niveau désigné par sa lettre (extension et dossier).
    pub fn from_letter(letter: char) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|s| s.letter() == letter.to_ascii_uppercase())
    }

    /// Lettre du niveau.
    pub fn letter(self) -> char {
        match self {
            Self::Z => 'Z',
            Self::A => 'A',
            Self::B => 'B',
            Self::C => 'C',
            Self::D => 'D',
            Self::E => 'E',
            Self::F => 'F',
            Self::G => 'G',
        }
    }

    /// Dénominateur d'échelle.
    pub fn denominator(self) -> u32 {
        match self {
            Self::Z => 20_000_000,
            Self::A => 3_000_000,
            Self::B => 1_000_000,
            Self::C => 200_000,
            Self::D => 100_000,
            Self::E => 50_000,
            Self::F => 20_000,
            Self::G => 7_500,
        }
    }

    /// Côté d'une cellule, en tiers de degré.
    pub fn cell_span(self) -> u32 {
        match self {
            Self::Z => 120,
            Self::A => 60,
            Self::B => 30,
            Self::C => 12,
            Self::D => 3,
            Self::E | Self::F | Self::G => 1,
        }
    }

    /// Rang d'affichage (1 à 8) : le plus détaillé est dessiné par-dessus.
    pub fn rank(self) -> u8 {
        self as u8 + 1
    }
}

/// Type d'un attribut (dictionnaire `CM93ATTR.DIC`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrType {
    /// Entier sur un octet.
    Byte,
    /// Entier signé sur 2 octets.
    Int,
    /// Entier sur 2 octets, en dixièmes.
    Word10,
    /// Entier signé sur 4 octets.
    Long,
    /// Réel simple précision.
    Float,
    /// Texte terminé par un octet nul.
    String,
    /// Trois octets puis un texte terminé par un octet nul.
    Complex,
    /// Liste d'octets précédée de sa longueur.
    List,
    /// Taille inconnue : la lecture des attributs de l'objet s'arrête.
    Unknown,
}

impl AttrType {
    fn parse(token: &str) -> Self {
        match token.trim() {
            "aBYTE" => Self::Byte,
            "aINT" => Self::Int,
            "aWORD10" => Self::Word10,
            "aLONG" => Self::Long,
            "aFLOAT" => Self::Float,
            "aSTRING" => Self::String,
            "aCMPLX" => Self::Complex,
            "aLIST" => Self::List,
            _ => Self::Unknown,
        }
    }
}

/// Dictionnaires d'une base CM93.
#[derive(Debug, Clone, Default)]
pub struct Cm93Dictionary {
    classes: HashMap<u8, String>,
    attributes: HashMap<u8, (String, AttrType)>,
}

/// Lignes `NOM|code|…` d'un dictionnaire ; les lignes vides sont ignorées.
fn dictionary_lines(text: &str) -> impl Iterator<Item = Result<(String, u8, Vec<&str>), String>> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|line| {
            let mut tokens = line.split('|');
            let name = tokens.next().unwrap_or("").trim();
            let code = tokens.next().unwrap_or("").trim();
            let valid_name =
                !name.is_empty() && name.len() <= 8 && name.bytes().all(|b| b.is_ascii_graphic());
            match code.parse::<u8>() {
                Ok(code) if valid_name => Ok((name.to_owned(), code, tokens.collect())),
                _ => Err(format!(
                    "ligne invalide « {} »",
                    line.chars().take(40).collect::<String>()
                )),
            }
        })
}

impl Cm93Dictionary {
    /// Lit les dictionnaires depuis leur texte.
    pub fn parse(objects: &str, attributes: &str) -> Result<Self, Cm93Error> {
        let mut dict = Self::default();
        for line in dictionary_lines(objects) {
            let (name, code, _) = line.map_err(Cm93Error::Dictionary)?;
            dict.classes.insert(code, name);
        }
        for line in dictionary_lines(attributes) {
            let (name, code, rest) = line.map_err(Cm93Error::Dictionary)?;
            let kind = AttrType::parse(rest.first().copied().unwrap_or(""));
            dict.attributes.insert(code, (name, kind));
        }
        if dict.classes.is_empty() || dict.attributes.is_empty() {
            return Err(Cm93Error::Dictionary("dictionnaire vide".into()));
        }
        Ok(dict)
    }

    /// Lit `CM93OBJ.DIC` et `CM93ATTR.DIC` à la racine de la base (casse
    /// des noms ignorée).
    pub fn load(root: &Path) -> Result<Self, Cm93Error> {
        let objects = read_dictionary(root, "CM93OBJ.DIC")?;
        let attributes = read_dictionary(root, "CM93ATTR.DIC")?;
        Self::parse(&objects, &attributes)
    }

    /// Acronyme d'une classe d'objets.
    pub fn class_name(&self, code: u8) -> Option<&str> {
        self.classes.get(&code).map(String::as_str)
    }

    fn attribute(&self, code: u8) -> Option<(&str, AttrType)> {
        self.attributes.get(&code).map(|(n, t)| (n.as_str(), *t))
    }
}

/// Fichier de la racine nommé `name` sans tenir compte de la casse.
pub(crate) fn find_file(root: &Path, name: &str) -> Option<PathBuf> {
    let exact = root.join(name);
    if exact.is_file() {
        return Some(exact);
    }
    std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
}

fn read_dictionary(root: &Path, name: &str) -> Result<String, Cm93Error> {
    let path = find_file(root, name)
        .ok_or_else(|| Cm93Error::Dictionary(format!("{name} introuvable")))?;
    let bytes = std::fs::read(&path).map_err(|source| Cm93Error::Io { path, source })?;
    if bytes.len() > 1024 * 1024 {
        return Err(Cm93Error::Dictionary(format!("{name} trop volumineux")));
    }
    Ok(latin1(&bytes))
}

/// Texte Latin-1 (chaque octet est un caractère).
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// La base CM93 de ce dossier : présence des deux dictionnaires.
pub fn is_cm93_root(dir: &Path) -> bool {
    let has =
        |name: &str| dir.join(name).is_file() || dir.join(name.to_ascii_lowercase()).is_file();
    has("CM93OBJ.DIC") && has("CM93ATTR.DIC")
}

/// Valeur d'un attribut.
#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    /// Entier.
    Int(i64),
    /// Réel.
    Real(f64),
    /// Texte.
    Text(String),
    /// Liste d'entiers.
    List(Vec<u8>),
}

impl AttrValue {
    /// Valeur numérique.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Int(v) => Some(*v as f64),
            Self::Real(v) if v.is_finite() => Some(*v),
            Self::Text(t) => t.trim().parse().ok().filter(|v: &f64| v.is_finite()),
            _ => None,
        }
    }
}

/// Géométrie d'un objet, en WGS 84 (`[lon, lat]`).
#[derive(Debug, Clone, PartialEq)]
pub enum Cm93Geometry {
    /// Sans géométrie.
    None,
    /// Point.
    Point(Position),
    /// Lignes.
    Line(Vec<Vec<Position>>),
    /// Surface : contours fermés.
    Area(Vec<Vec<Position>>),
    /// Sondes : positions et profondeurs (mètres).
    Soundings(Vec<(Position, f64)>),
}

/// Objet d'une cellule.
#[derive(Debug, Clone, PartialEq)]
pub struct Cm93Feature {
    /// Acronyme de la classe (dictionnaire).
    pub class: String,
    /// Géométrie.
    pub geometry: Cm93Geometry,
    /// Attributs, dans l'ordre du fichier.
    pub attributes: Vec<(String, AttrValue)>,
}

impl Cm93Feature {
    /// Valeur d'un attribut.
    pub fn attribute(&self, name: &str) -> Option<&AttrValue> {
        self.attributes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    }
}

/// Cellule décodée.
#[derive(Debug, Clone, PartialEq)]
pub struct Cm93Cell {
    /// Emprise : ouest, sud, est, nord (degrés, telle que déclarée dans
    /// l'en-tête du fichier).
    pub bounds: [f64; 4],
    /// Objets lus.
    pub features: Vec<Cm93Feature>,
    /// Objets écartés (références incohérentes, classe inconnue).
    pub skipped_features: usize,
    /// Décalage WGS 84 maximal déclaré par la cellule (attributs
    /// `_wgsox`, `_wgsoy` de la couverture), en mètres. Non appliqué :
    /// signalé seulement.
    pub declared_offset_m: f64,
    /// Date de la donnée la plus récente déclarée (`AAAAMMJJ`), si connue.
    pub data_date: Option<String>,
}

/// Lecteur d'octets décodés, borné.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize, what: &'static str) -> Result<&'a [u8], Cm93Error> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or(Cm93Error::Truncated(what))?;
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }
    fn u8(&mut self, what: &'static str) -> Result<u8, Cm93Error> {
        Ok(self.take(1, what)?[0])
    }
    fn u16(&mut self, what: &'static str) -> Result<u16, Cm93Error> {
        let b = self.take(2, what)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn i32(&mut self, what: &'static str) -> Result<i32, Cm93Error> {
        let b = self.take(4, what)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32(&mut self, what: &'static str) -> Result<f32, Cm93Error> {
        let b = self.take(4, what)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f64(&mut self, what: &'static str) -> Result<f64, Cm93Error> {
        let b = self.take(8, what)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(f64::from_le_bytes(a))
    }
    fn cstring(&mut self, what: &'static str) -> Result<String, Cm93Error> {
        let rest = &self.bytes[self.pos..];
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(Cm93Error::Truncated(what))?;
        let text = latin1(&rest[..len]);
        self.pos += len + 1;
        Ok(text)
    }
}

/// En-tête utile d'une cellule.
struct Header {
    bounds: [f64; 4],
    easting_min: f64,
    northing_min: f64,
    x_rate: f64,
    y_rate: f64,
    vectors: usize,
    points3d: usize,
    points2d: usize,
    features: usize,
}

fn read_header(c: &mut Cursor<'_>) -> Result<Header, Cm93Error> {
    let mut v = [0f64; 8];
    for value in &mut v {
        *value = c.f64("en-tête")?;
    }
    let [lon_min, lat_min, lon_max, lat_max, e_min, n_min, e_max, n_max] = v;
    if v.iter().any(|x| !x.is_finite())
        || !(-90.0..=90.0).contains(&lat_min)
        || !(-90.0..=90.0).contains(&lat_max)
        || lat_min > lat_max
        || !(-360.0..=720.0).contains(&lon_min)
        || !(-360.0..=720.0).contains(&lon_max)
    {
        return Err(Cm93Error::Corrupt("emprise invalide".into()));
    }
    let vectors = usize::from(c.u16("en-tête")?);
    c.take(12, "en-tête")?;
    let points3d = usize::from(c.u16("en-tête")?);
    c.take(8, "en-tête")?;
    let points2d = usize::from(c.u16("en-tête")?);
    c.take(4, "en-tête")?;
    let features = usize::from(c.u16("en-tête")?);
    c.take(32, "en-tête")?;
    let mut dx = e_max - e_min;
    if dx < 0.0 {
        dx += CM93_RADIUS_M * 2.0 * PI;
    }
    let dy = n_max - n_min;
    if !(dx > 0.0 && dy > 0.0) {
        return Err(Cm93Error::Corrupt("emprise projetée invalide".into()));
    }
    Ok(Header {
        bounds: [lon_min, lat_min, lon_max, lat_max],
        easting_min: e_min,
        northing_min: n_min,
        x_rate: dx / 65535.0,
        y_rate: dy / 65535.0,
        vectors,
        points3d,
        points2d,
        features,
    })
}

/// Point de la grille de la cellule.
type GridPoint = (u16, u16);

impl Header {
    /// Coordonnées WGS 84 d'un point de la grille (longitude ramenée
    /// entre -180 et 180).
    fn position(&self, (x, y): GridPoint) -> Position {
        let easting = f64::from(x) * self.x_rate + self.easting_min;
        let northing = f64::from(y) * self.y_rate + self.northing_min;
        let lat = (2.0 * (northing / CM93_RADIUS_M).exp().atan() - FRAC_PI_2).to_degrees();
        let mut lon = (easting / CM93_RADIUS_M).to_degrees();
        lon = lon.rem_euclid(360.0);
        if lon > 180.0 {
            lon -= 360.0;
        }
        [lon, lat]
    }
}

/// Bit d'une référence de segment : segment parcouru à l'envers.
const SEGMENT_REVERSED: u16 = 4 << 13;
/// Masque de l'index d'une référence de segment.
const SEGMENT_INDEX: u16 = 0x1fff;

/// Géométrie brute d'un objet, avant conversion.
enum RawGeometry {
    None,
    Point(usize),
    Line(Vec<u16>),
    Area(Vec<u16>),
    Soundings(usize),
}

/// Profondeur d'une sonde codée `z`.
fn sounding_depth(z: u16) -> f64 {
    if z >= 12_000 {
        f64::from(z - 12_000)
    } else {
        f64::from(z) / 10.0
    }
}

/// Segments orientés d'une liste de références ; `None` si une référence
/// sort de la table.
fn oriented<'a>(
    refs: &[u16],
    vectors: &'a [Vec<GridPoint>],
) -> Option<Vec<(bool, &'a [GridPoint])>> {
    refs.iter()
        .map(|&r| {
            let segment = vectors.get(usize::from(r & SEGMENT_INDEX))?;
            Some((r & SEGMENT_REVERSED != 0, segment.as_slice()))
        })
        .collect()
}

fn points_of(reversed: bool, segment: &[GridPoint]) -> Vec<GridPoint> {
    let mut points = segment.to_vec();
    if reversed {
        points.reverse();
    }
    points
}

/// Chaîne les segments ; un segment qui prolonge le précédent le continue.
fn chain_lines(segments: &[(bool, &[GridPoint])]) -> Vec<Vec<GridPoint>> {
    let mut lines: Vec<Vec<GridPoint>> = Vec::new();
    for &(reversed, segment) in segments {
        let points = points_of(reversed, segment);
        if points.len() < 2 {
            continue;
        }
        match lines.last_mut() {
            Some(line) if line.last() == points.first() => line.extend_from_slice(&points[1..]),
            _ => lines.push(points),
        }
    }
    lines
}

/// Assemble les contours d'une surface : un contour se ferme quand le
/// dernier point rejoint le premier. `None` si un contour reste ouvert.
fn chain_rings(segments: &[(bool, &[GridPoint])]) -> Option<Vec<Vec<GridPoint>>> {
    let mut rings = Vec::new();
    let mut ring: Vec<GridPoint> = Vec::new();
    for &(reversed, segment) in segments {
        let points = points_of(reversed, segment);
        if points.is_empty() {
            continue;
        }
        if ring.last() == points.first() {
            ring.extend_from_slice(&points[1..]);
        } else if ring.is_empty() {
            ring = points;
        } else {
            return None;
        }
        if ring.len() >= 4 && ring.first() == ring.last() {
            rings.push(std::mem::take(&mut ring));
        }
    }
    ring.is_empty().then_some(rings)
}

/// Décode les octets substitués d'une cellule.
pub fn deobfuscate(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .map(|&b| table::DECODE[usize::from(b)])
        .collect()
}

/// Décode une cellule (octets du fichier, tels quels).
pub fn decode_cell(bytes: &[u8], dict: &Cm93Dictionary) -> Result<Cm93Cell, Cm93Error> {
    if bytes.len() > MAX_CELL_BYTES {
        return Err(Cm93Error::Corrupt("cellule trop volumineuse".into()));
    }
    let plain = deobfuscate(bytes);
    let mut c = Cursor {
        bytes: &plain,
        pos: 0,
    };
    let header_len = usize::from(c.u16("prologue")?);
    let table1 = c.i32("prologue")?;
    let table2 = c.i32("prologue")?;
    let declared = i64::from(table1) + i64::from(table2) + header_len as i64;
    if header_len != HEADER_LEN || declared != plain.len() as i64 {
        return Err(Cm93Error::Corrupt(format!(
            "longueur annoncée {declared}, fichier de {} octets",
            plain.len()
        )));
    }
    let header = read_header(&mut c)?;

    // Chaque enregistrement occupe au moins deux octets : un nombre
    // d'enregistrements impossible est refusé avant toute allocation.
    let remaining = plain.len() - c.pos;
    if header.vectors + header.points3d + header.points2d + header.features > remaining / 2 {
        return Err(Cm93Error::Truncated("nombre d'enregistrements"));
    }

    let mut vectors = Vec::with_capacity(header.vectors);
    for _ in 0..header.vectors {
        let n = usize::from(c.u16("segments")?);
        let mut points = Vec::with_capacity(n.min(remaining / 4));
        for _ in 0..n {
            points.push((c.u16("segments")?, c.u16("segments")?));
        }
        vectors.push(points);
    }
    let mut points3d = Vec::with_capacity(header.points3d);
    for _ in 0..header.points3d {
        let n = usize::from(c.u16("sondes")?);
        let mut points = Vec::with_capacity(n.min(remaining / 6));
        for _ in 0..n {
            let x = c.u16("sondes")?;
            let y = c.u16("sondes")?;
            let z = c.u16("sondes")?;
            points.push(((x, y), z));
        }
        points3d.push(points);
    }
    let mut points2d = Vec::with_capacity(header.points2d);
    for _ in 0..header.points2d {
        points2d.push((c.u16("points")?, c.u16("points")?));
    }

    let mut cell = Cm93Cell {
        bounds: header.bounds,
        features: Vec::with_capacity(header.features),
        skipped_features: 0,
        declared_offset_m: 0.0,
        data_date: None,
    };
    for _ in 0..header.features {
        let start = c.pos;
        let class = c.u8("objets")?;
        let geotype = c.u8("objets")?;
        let length = usize::from(c.u16("objets")?);
        let end = start
            .checked_add(length)
            .filter(|&e| e <= plain.len() && length >= 4)
            .ok_or(Cm93Error::Truncated("objets"))?;
        let mut record = Cursor {
            bytes: &plain[..end],
            pos: c.pos,
        };
        let parsed = read_feature(&mut record, geotype, dict);
        c.pos = end;
        let Ok((raw, attributes)) = parsed else {
            cell.skipped_features += 1;
            continue;
        };
        let Some(class) = dict.class_name(class) else {
            cell.skipped_features += 1;
            continue;
        };
        let geometry = match raw {
            RawGeometry::None => Some(Cm93Geometry::None),
            RawGeometry::Point(i) => points2d
                .get(i)
                .map(|&p| Cm93Geometry::Point(header.position(p))),
            RawGeometry::Soundings(i) => points3d.get(i).map(|points| {
                Cm93Geometry::Soundings(
                    points
                        .iter()
                        .map(|&(p, z)| (header.position(p), sounding_depth(z)))
                        .collect(),
                )
            }),
            RawGeometry::Line(refs) => oriented(&refs, &vectors).map(|segments| {
                Cm93Geometry::Line(
                    chain_lines(&segments)
                        .into_iter()
                        .map(|l| l.into_iter().map(|p| header.position(p)).collect())
                        .collect(),
                )
            }),
            RawGeometry::Area(refs) => oriented(&refs, &vectors)
                .and_then(|segments| chain_rings(&segments))
                .map(|rings| {
                    Cm93Geometry::Area(
                        rings
                            .into_iter()
                            .map(|r| r.into_iter().map(|p| header.position(p)).collect())
                            .collect(),
                    )
                }),
        };
        let Some(geometry) = geometry else {
            cell.skipped_features += 1;
            continue;
        };
        let feature = Cm93Feature {
            class: class.to_owned(),
            geometry,
            attributes,
        };
        if feature.class == "_m_sor" || feature.class == "M_COVR" {
            for name in ["_wgsox", "_wgsoy"] {
                if let Some(v) = feature.attribute(name).and_then(AttrValue::as_f64) {
                    cell.declared_offset_m = cell.declared_offset_m.max(v.abs());
                }
            }
            for name in ["RECDAT", "_dgdat"] {
                if let Some(AttrValue::Text(date)) = feature.attribute(name) {
                    let date = date.trim();
                    if date.len() == 8
                        && date.bytes().all(|b| b.is_ascii_digit())
                        && cell.data_date.as_deref().map_or(true, |d| d < date)
                    {
                        cell.data_date = Some(date.to_owned());
                    }
                }
            }
        }
        cell.features.push(feature);
    }
    if c.pos != plain.len() {
        return Err(Cm93Error::Corrupt(format!(
            "{} octets inattendus en fin de cellule",
            plain.len() - c.pos
        )));
    }
    Ok(cell)
}

type FeatureParts = (RawGeometry, Vec<(String, AttrValue)>);

fn read_feature(
    c: &mut Cursor<'_>,
    geotype: u8,
    dict: &Cm93Dictionary,
) -> Result<FeatureParts, Cm93Error> {
    let refs = |c: &mut Cursor<'_>| -> Result<Vec<u16>, Cm93Error> {
        let n = usize::from(c.u16("géométrie")?);
        (0..n).map(|_| c.u16("géométrie")).collect()
    };
    let geometry = match geotype & 0x0f {
        1 => RawGeometry::Point(usize::from(c.u16("géométrie")?)),
        2 => RawGeometry::Line(refs(c)?),
        4 => RawGeometry::Area(refs(c)?),
        8 => RawGeometry::Soundings(usize::from(c.u16("géométrie")?)),
        _ => RawGeometry::None,
    };
    if geotype & 0x10 != 0 {
        let n = usize::from(c.u8("objets liés")?);
        c.take(2 * n, "objets liés")?;
    }
    if geotype & 0x20 != 0 {
        c.u16("objets liés")?;
    }
    let mut attributes = Vec::new();
    if geotype & 0x80 != 0 {
        let n = c.u8("attributs")?;
        for _ in 0..n {
            let code = c.u8("attributs")?;
            let Some((name, kind)) = dict.attribute(code) else {
                break;
            };
            let value = match kind {
                AttrType::Byte => AttrValue::Int(i64::from(c.u8("attributs")?)),
                AttrType::Int => AttrValue::Int(i64::from(c.u16("attributs")? as i16)),
                AttrType::Word10 => AttrValue::Real(f64::from(c.u16("attributs")?) / 10.0),
                AttrType::Long => AttrValue::Int(i64::from(c.i32("attributs")?)),
                AttrType::Float => AttrValue::Real(f64::from(c.f32("attributs")?)),
                AttrType::String => AttrValue::Text(c.cstring("attributs")?),
                AttrType::Complex => {
                    c.take(3, "attributs")?;
                    AttrValue::Text(c.cstring("attributs")?)
                }
                AttrType::List => {
                    let n = usize::from(c.u8("attributs")?);
                    AttrValue::List(c.take(n, "attributs")?.to_vec())
                }
                AttrType::Unknown => break,
            };
            attributes.push((name.to_owned(), value));
        }
    }
    Ok((geometry, attributes))
}

/// Lit et décode une cellule (appel bloquant).
pub fn decode_cell_file_blocking(
    path: &Path,
    dict: &Cm93Dictionary,
) -> Result<Cm93Cell, Cm93Error> {
    let io = |source| Cm93Error::Io {
        path: path.to_path_buf(),
        source,
    };
    let len = std::fs::metadata(path).map_err(io)?.len();
    if len > MAX_CELL_BYTES as u64 {
        return Err(Cm93Error::Corrupt("cellule trop volumineuse".into()));
    }
    decode_cell(&std::fs::read(path).map_err(io)?, dict)
}
