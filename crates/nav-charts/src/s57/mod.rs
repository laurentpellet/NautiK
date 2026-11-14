//! Décodage des cellules ENC S-57 (IHO S-57 édition 3.1, implémentation
//! binaire ISO/IEC 8211 imposée par la spécification de produit ENC).
//!
//! [`decode_cell`] lit un fichier de base (`.000`) : identification du jeu
//! de données (DSID), paramètres (DSPM), enregistrements vecteurs et
//! objets. Chaque objet reçoit sa géométrie en WGS 84 (longitude,
//! latitude) et les sondes en mètres.
//!
//! Garanties :
//! - les données invalides produisent une erreur, jamais de panique ni de
//!   lecture hors limites ;
//! - un objet dont la géométrie est incohérente est écarté et compté
//!   ([`S57Cell::skipped_features`]) : aucune position n'est inventée ;
//! - seules les cellules en WGS 84 et en mètres sont acceptées.
//!
//! Les fichiers de mise à jour (`.001`…) ne sont pas encore appliqués.

pub mod catalogue;
mod geometry;
mod iso8211;
#[cfg(test)]
mod testdata;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use geometry::{RawVector, Scale, SpatialPointer, VectorPointer, Vectors};
use iso8211::{Cursor, Field, Record};

/// Taille maximale d'un fichier de cellule (la spécification ENC limite
/// une cellule à 5 Mo ; marge large).
pub const MAX_CELL_BYTES: u64 = 64 * 1024 * 1024;

/// Nombre maximal de problèmes détaillés conservés par cellule.
pub const MAX_PROBLEMS: usize = 20;

/// Datum horizontal WGS 84 (valeur HDAT).
const HDAT_WGS84: u8 = 2;
/// Unité de profondeur : mètres (valeur DUNI).
const DUNI_METRES: u8 = 1;
/// Instruction « insérer » (RUIN), seule valide dans un fichier de base.
const RUIN_INSERT: u8 = 1;
/// Type d'opération « nouvelle édition » (EXPP) : fichier de base.
const EXPP_NEW: u8 = 1;
/// Niveau lexical UCS-2 (NALL).
const LEXICAL_UCS2: u8 = 2;

/// Erreur de décodage d'une cellule S-57.
#[derive(Debug, Error)]
pub enum S57Error {
    /// Fichier coupé avant la fin d'un enregistrement.
    #[error("fichier S-57 tronqué (enregistrement à l'octet {offset})")]
    Truncated {
        /// Position de l'enregistrement incomplet.
        offset: usize,
    },
    /// Structure invalide.
    #[error("fichier S-57 invalide : {0}")]
    Malformed(String),
    /// Cellule valide mais hors du périmètre pris en charge.
    #[error("cellule S-57 non prise en charge : {0}")]
    Unsupported(String),
    /// Fichier trop volumineux.
    #[error("cellule trop volumineuse ({size} octets, maximum {max})")]
    TooLarge {
        /// Taille du fichier.
        size: u64,
        /// Taille maximale acceptée.
        max: u64,
    },
    /// Lecture du fichier impossible.
    #[error("lecture de la cellule impossible : {0}")]
    Io(#[from] std::io::Error),
}

impl S57Error {
    pub(crate) fn truncated(offset: usize) -> Self {
        Self::Truncated { offset }
    }

    pub(crate) fn malformed(reason: impl Into<String>) -> Self {
        Self::Malformed(reason.into())
    }

    pub(crate) fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported(reason.into())
    }
}

/// Position WGS 84 `[longitude, latitude]` en degrés (ordre GeoJSON).
pub type Position = [f64; 2];

/// Sonde : position et profondeur en mètres (positive vers le bas).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Sounding {
    /// Position de la sonde.
    pub position: Position,
    /// Profondeur réduite au zéro des cartes, en mètres.
    pub depth: f64,
}

/// Géométrie d'un objet S-57.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum S57Geometry {
    /// Objet sans géométrie (collections, métadonnées non spatiales).
    None,
    /// Objet ponctuel.
    Point {
        /// Position.
        position: Position,
    },
    /// Groupe de sondes (SOUNDG).
    Soundings {
        /// Sondes.
        soundings: Vec<Sounding>,
    },
    /// Objet linéaire, en une ou plusieurs parties.
    Line {
        /// Lignes brisées.
        lines: Vec<Vec<Position>>,
    },
    /// Objet surfacique : contour(s) extérieur(s) d'abord, puis trous.
    Area {
        /// Contours fermés (premier point = dernier point).
        rings: Vec<Vec<Position>>,
    },
}

/// Attribut d'un objet (ATTF, ou NATF pour les attributs nationaux).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct S57Attribute {
    /// Code de l'attribut (ATTL).
    pub code: u16,
    /// Valeur textuelle (ATVL) ; vide = valeur inconnue.
    pub value: String,
}

/// Objet S-57 décodé.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S57Feature {
    /// Identifiant de l'enregistrement (RCID).
    pub record_id: u32,
    /// Code de la classe d'objets (OBJL), ex. 42 = DEPARE.
    pub object_class: u16,
    /// Groupe (GRUP) : 1 = données de base (surfaces « sol »), 2 = autres.
    pub group: u8,
    /// Attributs.
    pub attributes: Vec<S57Attribute>,
    /// Géométrie WGS 84.
    pub geometry: S57Geometry,
}

impl S57Feature {
    /// Acronyme de la classe d'objets, s'il est connu.
    pub fn acronym(&self) -> Option<&'static str> {
        catalogue::object_class_acronym(self.object_class)
    }

    /// Valeur d'un attribut.
    pub fn attribute(&self, code: u16) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.code == code)
            .map(|a| a.value.as_str())
    }
}

/// Cellule S-57 décodée.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S57Cell {
    /// Nom du jeu de données (DSNM), ex. `FR501130.000`.
    pub dataset_name: String,
    /// Numéro d'édition (EDTN).
    pub edition: String,
    /// Numéro de mise à jour (UPDN).
    pub update_number: String,
    /// Date de publication (ISDT, AAAAMMJJ).
    pub issue_date: String,
    /// Usage prévu (INTU) : 1 vue d'ensemble … 6 accostage.
    pub intended_usage: u8,
    /// Échelle de compilation (CSCL), dénominateur.
    pub compilation_scale: u32,
    /// Objets décodés.
    pub features: Vec<S57Feature>,
    /// Objets écartés (géométrie incohérente, instruction invalide).
    pub skipped_features: u32,
    /// Détail des premiers problèmes rencontrés.
    pub problems: Vec<String>,
}

/// Résumé d'une cellule pour l'affichage (Paramètres → Cartes).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S57CellSummary {
    /// Nom du jeu de données.
    pub dataset_name: String,
    /// Édition.
    pub edition: String,
    /// Numéro de mise à jour intégrée au fichier de base.
    pub update_number: String,
    /// Date de publication (AAAAMMJJ).
    pub issue_date: String,
    /// Usage prévu (1 à 6).
    pub intended_usage: u8,
    /// Échelle de compilation.
    pub compilation_scale: u32,
    /// Nombre d'objets décodés.
    pub features: u32,
    /// Nombre d'objets par classe (acronyme, ou code numérique).
    pub classes: BTreeMap<String, u32>,
    /// Nombre de sondes individuelles.
    pub soundings: u32,
    /// Emprise `[ouest, sud, est, nord]` des géométries, en degrés.
    pub bounds: Option<[f64; 4]>,
    /// Objets écartés.
    pub skipped_features: u32,
    /// Premiers problèmes rencontrés.
    pub problems: Vec<String>,
}

impl S57Cell {
    /// Résumé de la cellule.
    pub fn summary(&self) -> S57CellSummary {
        let mut classes = BTreeMap::new();
        let mut soundings = 0u32;
        let mut bounds: Option<[f64; 4]> = None;
        let mut extend = |[lon, lat]: Position| {
            let b = bounds.get_or_insert([lon, lat, lon, lat]);
            b[0] = b[0].min(lon);
            b[1] = b[1].min(lat);
            b[2] = b[2].max(lon);
            b[3] = b[3].max(lat);
        };
        for feature in &self.features {
            let name = feature
                .acronym()
                .map_or_else(|| feature.object_class.to_string(), str::to_owned);
            *classes.entry(name).or_insert(0u32) += 1;
            match &feature.geometry {
                S57Geometry::None => {}
                S57Geometry::Point { position } => extend(*position),
                S57Geometry::Soundings { soundings: list } => {
                    soundings =
                        soundings.saturating_add(u32::try_from(list.len()).unwrap_or(u32::MAX));
                    list.iter().for_each(|s| extend(s.position));
                }
                S57Geometry::Line { lines: parts } | S57Geometry::Area { rings: parts } => {
                    parts.iter().flatten().for_each(|&p| extend(p));
                }
            }
        }
        S57CellSummary {
            dataset_name: self.dataset_name.clone(),
            edition: self.edition.clone(),
            update_number: self.update_number.clone(),
            issue_date: self.issue_date.clone(),
            intended_usage: self.intended_usage,
            compilation_scale: self.compilation_scale,
            features: u32::try_from(self.features.len()).unwrap_or(u32::MAX),
            classes,
            soundings,
            bounds,
            skipped_features: self.skipped_features,
            problems: self.problems.clone(),
        }
    }
}

/// Formats binaires attendus des champs décodés (répétitions développées).
const EXPECTED_FORMATS: &[(&str, &[&str])] = &[
    (
        "DSID",
        &[
            "b11", "b14", "b11", "b11", "A", "A", "A", "A(8)", "A(8)", "R(4)", "b11", "A", "A",
            "b11", "b12", "A",
        ],
    ),
    (
        "DSSI",
        &[
            "b11", "b11", "b11", "b14", "b14", "b14", "b14", "b14", "b14", "b14", "b14",
        ],
    ),
    (
        "DSPM",
        &[
            "b11", "b14", "b11", "b11", "b11", "b14", "b11", "b11", "b11", "b11", "b14", "b14", "A",
        ],
    ),
    ("VRID", &["b11", "b14", "b12", "b11"]),
    ("VRPT", &["B(40)", "b11", "b11", "b11", "b11"]),
    ("SG2D", &["b24", "b24"]),
    ("SG3D", &["b24", "b24", "b24"]),
    ("FRID", &["b11", "b14", "b11", "b11", "b12", "b12", "b11"]),
    ("ATTF", &["b12", "A"]),
    ("NATF", &["b12", "A"]),
    ("FSPT", &["B(40)", "b11", "b11", "b11"]),
];

/// Vérifie que les champs décodés sont décrits en binaire, comme l'impose
/// la spécification ENC (une implémentation ASCII est refusée).
fn check_formats(file: &iso8211::Iso8211File<'_>) -> Result<(), S57Error> {
    for (tag, expected) in EXPECTED_FORMATS {
        if let Some(description) = file.descriptions.get(*tag) {
            if description.formats != *expected {
                return Err(S57Error::unsupported(format!(
                    "format du champ {tag} inattendu ({}) : seule l'implémentation binaire ENC est lue",
                    description.formats.join(",")
                )));
            }
        }
    }
    Ok(())
}

/// Identification du jeu de données (DSID, DSSI).
#[derive(Debug, Default)]
struct DatasetId {
    name: String,
    edition: String,
    update: String,
    issue_date: String,
    usage: u8,
    national_level: u8,
    /// Nombres d'enregistrements annoncés (DSSI) : objets, nœuds isolés,
    /// nœuds connectés, arcs.
    declared: Option<Declared>,
}

/// Nombres d'enregistrements annoncés par le champ DSSI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Declared {
    features: u64,
    isolated_nodes: u64,
    connected_nodes: u64,
    edges: u64,
}

fn read_dsid(record: &Record<'_>, dsid: &Field<'_>) -> Result<DatasetId, S57Error> {
    let mut c = Cursor::new(dsid);
    let _rcnm = c.u8()?;
    let _rcid = c.u32()?;
    let expp = c.u8()?;
    let usage = c.u8()?;
    let name = c.variable_text();
    let edition = c.variable_text();
    let update = c.variable_text();
    let _uadt = c.fixed_text(8)?;
    let issue_date = c.fixed_text(8)?;
    if expp != EXPP_NEW {
        return Err(S57Error::unsupported(format!(
            "{name} est un fichier de mise à jour (EXPP {expp}), pas une cellule de base"
        )));
    }
    let mut national_level = 1;
    let mut declared = None;
    if let Some(dssi) = record.field("DSSI") {
        let mut c = Cursor::new(dssi);
        let _dstr = c.u8()?;
        let _aall = c.u8()?;
        national_level = c.u8()?;
        let mut features = 0u64;
        for _ in 0..4 {
            features += u64::from(c.u32()?);
        }
        declared = Some(Declared {
            features,
            isolated_nodes: u64::from(c.u32()?),
            connected_nodes: u64::from(c.u32()?),
            edges: u64::from(c.u32()?),
        });
    }
    Ok(DatasetId {
        name,
        edition,
        update,
        issue_date,
        usage,
        national_level,
        declared,
    })
}

/// Paramètres du jeu de données (DSPM).
#[derive(Debug, Clone, Copy)]
struct DatasetParams {
    scale: u32,
    comf: u32,
    somf: u32,
}

fn read_dspm(dspm: &Field<'_>) -> Result<DatasetParams, S57Error> {
    let mut c = Cursor::new(dspm);
    let _rcnm = c.u8()?;
    let _rcid = c.u32()?;
    let hdat = c.u8()?;
    let _vdat = c.u8()?;
    let _sdat = c.u8()?;
    let scale = c.u32()?;
    let duni = c.u8()?;
    let _huni = c.u8()?;
    let _puni = c.u8()?;
    let _coun = c.u8()?;
    let comf = c.u32()?;
    let somf = c.u32()?;
    if hdat != HDAT_WGS84 {
        return Err(S57Error::unsupported(format!(
            "datum horizontal {hdat} (seul WGS 84 est accepté)"
        )));
    }
    if duni != DUNI_METRES {
        return Err(S57Error::unsupported(format!(
            "unité de profondeur {duni} (seuls les mètres sont acceptés)"
        )));
    }
    if comf == 0 || somf == 0 {
        return Err(S57Error::malformed(
            "facteur de coordonnées ou de sondes nul",
        ));
    }
    Ok(DatasetParams { scale, comf, somf })
}

/// Lit un enregistrement vecteur ; `None` si son instruction n'est pas
/// une insertion.
fn read_vector(
    record: &Record<'_>,
    vrid: &Field<'_>,
) -> Result<(u8, u32, u8, RawVector), S57Error> {
    let mut c = Cursor::new(vrid);
    let rcnm = c.u8()?;
    let rcid = c.u32()?;
    let _rver = c.u16()?;
    let ruin = c.u8()?;
    let mut vector = RawVector::default();
    for field in record.fields("VRPT") {
        let mut c = Cursor::new(field);
        while !c.is_empty() {
            let rcnm = c.u8()?;
            let rcid = c.u32()?;
            let _ornt = c.u8()?;
            let _usag = c.u8()?;
            let topi = c.u8()?;
            let _mask = c.u8()?;
            vector.pointers.push(VectorPointer { rcnm, rcid, topi });
        }
    }
    for field in record.fields("SG2D") {
        let mut c = Cursor::new(field);
        while !c.is_empty() {
            let y = c.i32()?;
            let x = c.i32()?;
            vector.coords.push((x, y));
        }
    }
    for field in record.fields("SG3D") {
        let mut c = Cursor::new(field);
        while !c.is_empty() {
            let y = c.i32()?;
            let x = c.i32()?;
            let depth = c.i32()?;
            vector.coords.push((x, y));
            vector.depths.push(depth);
        }
    }
    if !vector.depths.is_empty() && vector.depths.len() != vector.coords.len() {
        return Err(S57Error::malformed(format!(
            "vecteur {rcnm}/{rcid} : coordonnées 2D et 3D mélangées"
        )));
    }
    Ok((rcnm, rcid, ruin, vector))
}

/// Objet brut, avant construction de la géométrie.
struct RawFeature {
    rcid: u32,
    prim: u8,
    group: u8,
    objl: u16,
    ruin: u8,
    attributes: Vec<S57Attribute>,
    pointers: Vec<SpatialPointer>,
}

fn read_feature(
    record: &Record<'_>,
    frid: &Field<'_>,
    national_level: u8,
) -> Result<RawFeature, S57Error> {
    let mut c = Cursor::new(frid);
    let _rcnm = c.u8()?;
    let rcid = c.u32()?;
    let prim = c.u8()?;
    let group = c.u8()?;
    let objl = c.u16()?;
    let _rver = c.u16()?;
    let ruin = c.u8()?;
    let mut attributes = Vec::new();
    for field in record.fields("ATTF") {
        let mut c = Cursor::new(field);
        while !c.is_empty() {
            let code = c.u16()?;
            let value = c.variable_text();
            attributes.push(S57Attribute { code, value });
        }
    }
    for field in record.fields("NATF") {
        let mut c = Cursor::new(field);
        // En UCS-2, le champ se termine par `1E 00` : l'octet nul restant
        // après la fin de champ n'est pas un attribut.
        while !c.is_empty() && !matches!(c.rest(), [0x00] | [0x1E, 0x00]) {
            let code = c.u16()?;
            let value = if national_level == LEXICAL_UCS2 {
                c.variable_ucs2()
            } else {
                c.variable_text()
            };
            attributes.push(S57Attribute { code, value });
        }
    }
    let mut pointers = Vec::new();
    for field in record.fields("FSPT") {
        let mut c = Cursor::new(field);
        while !c.is_empty() {
            pointers.push(SpatialPointer {
                rcnm: c.u8()?,
                rcid: c.u32()?,
                ornt: c.u8()?,
                usag: c.u8()?,
                mask: c.u8()?,
            });
        }
    }
    Ok(RawFeature {
        rcid,
        prim,
        group,
        objl,
        ruin,
        attributes,
        pointers,
    })
}

/// Problèmes rencontrés : compteur d'objets écartés et premiers messages.
#[derive(Default)]
struct Problems {
    skipped: u32,
    messages: Vec<String>,
}

impl Problems {
    fn note(&mut self, message: String) {
        if self.messages.len() < MAX_PROBLEMS {
            self.messages.push(message);
        }
    }

    fn skip(&mut self, message: String) {
        self.skipped = self.skipped.saturating_add(1);
        self.note(message);
    }
}

/// Décode une cellule S-57 de base (contenu d'un fichier `.000`).
///
/// # Errors
///
/// [`S57Error`] si le fichier n'est pas une cellule S-57 lisible, n'est
/// pas un fichier de base, ou n'est pas en WGS 84 et en mètres.
pub fn decode_cell(bytes: &[u8]) -> Result<S57Cell, S57Error> {
    let file = iso8211::read_file(bytes)?;
    check_formats(&file)?;

    let mut dataset: Option<DatasetId> = None;
    let mut params: Option<DatasetParams> = None;
    let mut vectors: Vectors = HashMap::new();
    let mut raw_features = Vec::new();
    let mut problems = Problems::default();
    let mut read = Declared::default();

    for record in &file.records {
        if let Some(dsid) = record.field("DSID") {
            dataset = Some(read_dsid(record, dsid)?);
        } else if let Some(dspm) = record.field("DSPM") {
            params = Some(read_dspm(dspm)?);
        } else if let Some(vrid) = record.field("VRID") {
            let (rcnm, rcid, ruin, vector) = read_vector(record, vrid)?;
            match rcnm {
                geometry::RCNM_ISOLATED_NODE => read.isolated_nodes += 1,
                geometry::RCNM_CONNECTED_NODE => read.connected_nodes += 1,
                geometry::RCNM_EDGE => read.edges += 1,
                _ => {}
            }
            if ruin != RUIN_INSERT {
                problems.note(format!(
                    "vecteur {rcnm}/{rcid} : instruction {ruin} ignorée"
                ));
            } else if vectors.insert((rcnm, rcid), vector).is_some() {
                problems.note(format!("vecteur {rcnm}/{rcid} en double"));
            }
        } else if let Some(frid) = record.field("FRID") {
            let level = dataset.as_ref().map_or(1, |d| d.national_level);
            raw_features.push(read_feature(record, frid, level)?);
            read.features += 1;
        }
    }

    let dataset = dataset
        .ok_or_else(|| S57Error::malformed("identification du jeu de données (DSID) absente"))?;
    // Une cellule coupée entre deux enregistrements reste lisible : les
    // nombres annoncés par DSSI révèlent les enregistrements manquants.
    if let Some(declared) = dataset.declared {
        let short = [
            ("objets", read.features, declared.features),
            ("nœuds isolés", read.isolated_nodes, declared.isolated_nodes),
            (
                "nœuds connectés",
                read.connected_nodes,
                declared.connected_nodes,
            ),
            ("arcs", read.edges, declared.edges),
        ];
        for (what, found, expected) in short {
            if found < expected {
                return Err(S57Error::malformed(format!(
                    "cellule incomplète : {found} {what} sur {expected} annoncés"
                )));
            }
            if found > expected {
                problems.note(format!("{found} {what} pour {expected} annoncés"));
            }
        }
    }
    let params =
        params.ok_or_else(|| S57Error::malformed("paramètres du jeu de données (DSPM) absents"))?;
    let scale = Scale {
        comf: f64::from(params.comf),
        somf: f64::from(params.somf),
    };

    let mut features = Vec::with_capacity(raw_features.len());
    for raw in raw_features {
        let label = catalogue::object_class_acronym(raw.objl)
            .map_or_else(|| raw.objl.to_string(), str::to_owned);
        if raw.ruin != RUIN_INSERT {
            problems.skip(format!(
                "objet {label} {} : instruction {}",
                raw.rcid, raw.ruin
            ));
            continue;
        }
        match geometry::build(raw.prim, &raw.pointers, &vectors, scale) {
            Ok(geometry) => features.push(S57Feature {
                record_id: raw.rcid,
                object_class: raw.objl,
                group: raw.group,
                attributes: raw.attributes,
                geometry,
            }),
            Err(reason) => problems.skip(format!("objet {label} {} : {reason}", raw.rcid)),
        }
    }
    if problems.skipped > 0 {
        tracing::warn!(
            cell = %dataset.name,
            skipped = problems.skipped,
            "objets S-57 écartés"
        );
    }

    Ok(S57Cell {
        dataset_name: dataset.name,
        edition: dataset.edition,
        update_number: dataset.update,
        issue_date: dataset.issue_date,
        intended_usage: dataset.usage,
        compilation_scale: params.scale,
        features,
        skipped_features: problems.skipped,
        problems: problems.messages,
    })
}

/// Lit et décode un fichier de cellule (opération bloquante, voir
/// [`decode_cell_file`]).
///
/// # Errors
///
/// [`S57Error::Io`] si le fichier est illisible, [`S57Error::TooLarge`]
/// s'il dépasse [`MAX_CELL_BYTES`], sinon voir [`decode_cell`].
pub fn decode_cell_file_blocking(path: &Path) -> Result<S57Cell, S57Error> {
    let size = std::fs::metadata(path)?.len();
    if size > MAX_CELL_BYTES {
        return Err(S57Error::TooLarge {
            size,
            max: MAX_CELL_BYTES,
        });
    }
    decode_cell(&std::fs::read(path)?)
}

/// Lit et décode un fichier de cellule sans bloquer le runtime asynchrone.
///
/// # Errors
///
/// Voir [`decode_cell_file_blocking`].
pub async fn decode_cell_file(path: PathBuf) -> Result<S57Cell, S57Error> {
    tokio::task::spawn_blocking(move || decode_cell_file_blocking(&path))
        .await
        .map_err(|e| S57Error::Io(std::io::Error::other(e.to_string())))?
}

#[cfg(test)]
mod tests;
