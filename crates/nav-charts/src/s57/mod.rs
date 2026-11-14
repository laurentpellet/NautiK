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
//! Les fichiers de mise à jour (`.001`, `.002`…) sont appliqués dans
//! l'ordre par [`decode_cell_file`] (voir le module `update`) : chaque
//! mise à jour est appliquée en entier ou pas du tout, et une mise à jour
//! manquante ou refusée est signalée ([`S57Cell::update_error`]).

pub mod catalogue;
mod geometry;
mod iso8211;
#[cfg(test)]
mod testdata;
mod update;

use std::collections::btree_map::Entry as BTreeEntry;
use std::collections::hash_map::Entry;
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
/// Type d'opération « révision » (EXPP) : fichier de mise à jour.
const EXPP_UPDATE: u8 = 2;
/// Nombre maximal de fichiers de mise à jour (`.001` à `.999`).
pub const MAX_UPDATES: u32 = 999;
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
    /// Numéro de la dernière mise à jour intégrée (UPDN).
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
    /// Mises à jour appliquées.
    pub updates_applied: u32,
    /// Fichiers de mise à jour présents mais non appliqués.
    pub updates_pending: u32,
    /// Date de publication de la dernière mise à jour appliquée.
    pub update_issue_date: Option<String>,
    /// Raison de l'arrêt des mises à jour (fichier manquant, refusé…).
    pub update_error: Option<String>,
    /// Date d'annulation de la cellule par son producteur : une cellule
    /// annulée ne doit plus servir à la navigation.
    pub cancelled: Option<String>,
}

/// Résumé d'une cellule pour l'affichage (Paramètres → Cartes).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S57CellSummary {
    /// Nom du jeu de données.
    pub dataset_name: String,
    /// Édition.
    pub edition: String,
    /// Numéro de la dernière mise à jour intégrée.
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
    /// Mises à jour appliquées.
    pub updates_applied: u32,
    /// Mises à jour présentes mais non appliquées.
    pub updates_pending: u32,
    /// Date de la dernière mise à jour appliquée.
    pub update_issue_date: Option<String>,
    /// Raison de l'arrêt des mises à jour.
    pub update_error: Option<String>,
    /// Date d'annulation de la cellule, le cas échéant.
    pub cancelled: Option<String>,
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
            updates_applied: self.updates_applied,
            updates_pending: self.updates_pending,
            update_issue_date: self.update_issue_date.clone(),
            update_error: self.update_error.clone(),
            cancelled: self.cancelled.clone(),
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
    ("VRPC", &["b11", "b12", "b12"]),
    ("SGCC", &["b11", "b12", "b12"]),
    ("FSPC", &["b11", "b12", "b12"]),
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
#[derive(Debug, Clone, Default)]
pub(crate) struct DatasetId {
    pub name: String,
    pub edition: String,
    pub update: String,
    pub issue_date: String,
    pub usage: u8,
    /// Type d'opération : 1 fichier de base, 2 mise à jour.
    pub expp: u8,
    pub national_level: u8,
    /// Nombres d'enregistrements annoncés (DSSI) : objets, nœuds isolés,
    /// nœuds connectés, arcs.
    pub declared: Option<Declared>,
}

/// Nombres d'enregistrements annoncés par le champ DSSI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Declared {
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
        expp,
        national_level,
        declared,
    })
}

/// Paramètres du jeu de données (DSPM).
#[derive(Debug, Clone, Copy)]
pub(crate) struct DatasetParams {
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

/// Instruction de mise à jour d'une liste (champs VRPC, SGCC, FSPC) :
/// insérer (1), supprimer (2) ou modifier (3) `count` éléments à partir
/// de la position `index` (comptée à partir de 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ListUpdate {
    pub instruction: u8,
    pub index: usize,
    pub count: usize,
}

fn read_list_update(field: &Field<'_>) -> Result<ListUpdate, S57Error> {
    let mut c = Cursor::new(field);
    Ok(ListUpdate {
        instruction: c.u8()?,
        index: usize::from(c.u16()?),
        count: usize::from(c.u16()?),
    })
}

/// Enregistrement vecteur lu dans un fichier.
#[derive(Debug, Clone)]
pub(crate) struct VectorRecord {
    pub rcnm: u8,
    pub rcid: u32,
    /// Version de l'enregistrement (RVER).
    pub version: u16,
    /// Instruction (RUIN) : 1 insérer, 2 supprimer, 3 modifier.
    pub instruction: u8,
    /// Pointeurs et coordonnées portés par l'enregistrement.
    pub vector: RawVector,
    /// Mise à jour des pointeurs (VRPC).
    pub pointer_update: Option<ListUpdate>,
    /// Mise à jour des coordonnées (SGCC).
    pub coord_update: Option<ListUpdate>,
}

fn read_vector(record: &Record<'_>, vrid: &Field<'_>) -> Result<VectorRecord, S57Error> {
    let mut c = Cursor::new(vrid);
    let rcnm = c.u8()?;
    let rcid = c.u32()?;
    let version = c.u16()?;
    let instruction = c.u8()?;
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
    Ok(VectorRecord {
        rcnm,
        rcid,
        version,
        instruction,
        vector,
        pointer_update: record.field("VRPC").map(read_list_update).transpose()?,
        coord_update: record.field("SGCC").map(read_list_update).transpose()?,
    })
}

/// Objet lu dans un fichier.
#[derive(Debug, Clone)]
pub(crate) struct FeatureRecord {
    pub rcid: u32,
    /// Version de l'enregistrement (RVER).
    pub version: u16,
    /// Instruction (RUIN) : 1 insérer, 2 supprimer, 3 modifier.
    pub instruction: u8,
    pub prim: u8,
    pub group: u8,
    pub objl: u16,
    /// Attributs (ATTF puis NATF) ; en mise à jour, la valeur `DEL`
    /// (0x7F) supprime l'attribut.
    pub attributes: Vec<S57Attribute>,
    pub pointers: Vec<SpatialPointer>,
    /// Mise à jour des pointeurs (FSPC).
    pub pointer_update: Option<ListUpdate>,
}

impl FeatureRecord {
    /// Libellé pour les messages : acronyme (ou code) et identifiant.
    pub fn label(&self) -> String {
        let class = catalogue::object_class_acronym(self.objl)
            .map_or_else(|| self.objl.to_string(), str::to_owned);
        format!("objet {class} {}", self.rcid)
    }
}

fn read_feature(
    record: &Record<'_>,
    frid: &Field<'_>,
    national_level: u8,
) -> Result<FeatureRecord, S57Error> {
    let mut c = Cursor::new(frid);
    let _rcnm = c.u8()?;
    let rcid = c.u32()?;
    let prim = c.u8()?;
    let group = c.u8()?;
    let objl = c.u16()?;
    let version = c.u16()?;
    let instruction = c.u8()?;
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
    Ok(FeatureRecord {
        rcid,
        version,
        instruction,
        prim,
        group,
        objl,
        attributes,
        pointers,
        pointer_update: record.field("FSPC").map(read_list_update).transpose()?,
    })
}

/// Contenu d'un fichier S-57 (cellule de base ou mise à jour).
#[derive(Debug, Clone)]
pub(crate) struct Dataset {
    pub id: DatasetId,
    pub params: Option<DatasetParams>,
    pub vectors: Vec<VectorRecord>,
    pub features: Vec<FeatureRecord>,
}

/// Lit les enregistrements d'un fichier S-57.
///
/// Un fichier coupé entre deux enregistrements reste lisible : les nombres
/// annoncés par DSSI révèlent les enregistrements manquants.
pub(crate) fn read_dataset(bytes: &[u8]) -> Result<Dataset, S57Error> {
    let file = iso8211::read_file(bytes)?;
    check_formats(&file)?;

    let mut id: Option<DatasetId> = None;
    let mut params = None;
    let mut vectors = Vec::new();
    let mut features = Vec::new();
    let mut read = Declared::default();
    for record in &file.records {
        if let Some(dsid) = record.field("DSID") {
            id = Some(read_dsid(record, dsid)?);
        } else if let Some(dspm) = record.field("DSPM") {
            params = Some(read_dspm(dspm)?);
        } else if let Some(vrid) = record.field("VRID") {
            let vector = read_vector(record, vrid)?;
            match vector.rcnm {
                geometry::RCNM_ISOLATED_NODE => read.isolated_nodes += 1,
                geometry::RCNM_CONNECTED_NODE => read.connected_nodes += 1,
                geometry::RCNM_EDGE => read.edges += 1,
                _ => {}
            }
            vectors.push(vector);
        } else if let Some(frid) = record.field("FRID") {
            let level = id.as_ref().map_or(1, |d| d.national_level);
            features.push(read_feature(record, frid, level)?);
            read.features += 1;
        }
    }
    let id =
        id.ok_or_else(|| S57Error::malformed("identification du jeu de données (DSID) absente"))?;
    if let Some(declared) = id.declared {
        let counts = [
            ("objets", read.features, declared.features),
            ("nœuds isolés", read.isolated_nodes, declared.isolated_nodes),
            (
                "nœuds connectés",
                read.connected_nodes,
                declared.connected_nodes,
            ),
            ("arcs", read.edges, declared.edges),
        ];
        for (what, found, expected) in counts {
            if found < expected {
                return Err(S57Error::malformed(format!(
                    "fichier incomplet : {found} {what} sur {expected} annoncés"
                )));
            }
        }
    }
    Ok(Dataset {
        id,
        params,
        vectors,
        features,
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

/// Objet de la cellule, avant construction de la géométrie.
#[derive(Debug, Clone)]
pub(crate) struct FeatureState {
    pub version: u16,
    pub prim: u8,
    pub group: u8,
    pub objl: u16,
    pub attributes: Vec<S57Attribute>,
    pub pointers: Vec<SpatialPointer>,
}

/// État d'une cellule : fichier de base et mises à jour appliquées.
#[derive(Debug, Clone)]
pub(crate) struct CellState {
    pub id: DatasetId,
    pub params: DatasetParams,
    /// Numéro de la dernière mise à jour intégrée.
    pub update_number: u32,
    pub vectors: Vectors,
    /// Versions (RVER) des enregistrements vecteurs.
    pub vector_versions: HashMap<(u8, u32), u16>,
    /// Objets, par identifiant.
    pub features: BTreeMap<u32, FeatureState>,
}

/// Numéro de mise à jour (UPDN).
pub(crate) fn update_number(text: &str) -> Result<u32, S57Error> {
    text.trim()
        .parse()
        .map_err(|_| S57Error::malformed(format!("numéro de mise à jour « {text} » invalide")))
}

/// Charge le fichier de base.
fn load_base(dataset: Dataset, problems: &mut Problems) -> Result<CellState, S57Error> {
    let id = dataset.id;
    if id.expp != EXPP_NEW {
        return Err(S57Error::unsupported(format!(
            "{} est un fichier de mise à jour (EXPP {}), pas une cellule de base",
            id.name, id.expp
        )));
    }
    let params = dataset
        .params
        .ok_or_else(|| S57Error::malformed("paramètres du jeu de données (DSPM) absents"))?;
    let mut state = CellState {
        update_number: update_number(&id.update)?,
        id,
        params,
        vectors: HashMap::new(),
        vector_versions: HashMap::new(),
        features: BTreeMap::new(),
    };
    for record in dataset.vectors {
        let key = (record.rcnm, record.rcid);
        if record.instruction != RUIN_INSERT {
            problems.note(format!(
                "vecteur {}/{} : instruction {} ignorée",
                record.rcnm, record.rcid, record.instruction
            ));
        } else if let Entry::Vacant(slot) = state.vectors.entry(key) {
            slot.insert(record.vector);
            state.vector_versions.insert(key, record.version);
        } else {
            problems.note(format!("vecteur {}/{} en double", record.rcnm, record.rcid));
        }
    }
    for record in dataset.features {
        if record.instruction != RUIN_INSERT {
            problems.skip(format!(
                "{} : instruction {}",
                record.label(),
                record.instruction
            ));
        } else if let BTreeEntry::Vacant(slot) = state.features.entry(record.rcid) {
            slot.insert(FeatureState {
                version: record.version,
                prim: record.prim,
                group: record.group,
                objl: record.objl,
                attributes: record.attributes,
                pointers: record.pointers,
            });
        } else {
            problems.skip(format!("{} en double", record.label()));
        }
    }
    Ok(state)
}

/// Bilan de l'application des mises à jour.
#[derive(Debug, Default)]
struct UpdateOutcome {
    applied: u32,
    pending: u32,
    issue_date: Option<String>,
    error: Option<String>,
    cancelled: Option<String>,
}

/// Construit la géométrie des objets et produit la cellule décodée.
fn finish(state: CellState, mut problems: Problems, outcome: UpdateOutcome) -> S57Cell {
    let scale = Scale {
        comf: f64::from(state.params.comf),
        somf: f64::from(state.params.somf),
    };
    let mut features = Vec::with_capacity(state.features.len());
    for (rcid, raw) in state.features {
        match geometry::build(raw.prim, &raw.pointers, &state.vectors, scale) {
            Ok(geometry) => features.push(S57Feature {
                record_id: rcid,
                object_class: raw.objl,
                group: raw.group,
                attributes: raw.attributes,
                geometry,
            }),
            Err(reason) => {
                let class = catalogue::object_class_acronym(raw.objl)
                    .map_or_else(|| raw.objl.to_string(), str::to_owned);
                problems.skip(format!("objet {class} {rcid} : {reason}"));
            }
        }
    }
    if problems.skipped > 0 {
        tracing::warn!(
            cell = %state.id.name,
            skipped = problems.skipped,
            "objets S-57 écartés"
        );
    }
    if let Some(error) = &outcome.error {
        tracing::warn!(cell = %state.id.name, %error, "mises à jour S-57 interrompues");
    }
    S57Cell {
        dataset_name: state.id.name,
        edition: state.id.edition,
        update_number: state.update_number.to_string(),
        issue_date: state.id.issue_date,
        intended_usage: state.id.usage,
        compilation_scale: state.params.scale,
        features,
        skipped_features: problems.skipped,
        problems: problems.messages,
        updates_applied: outcome.applied,
        updates_pending: outcome.pending,
        update_issue_date: outcome.issue_date,
        update_error: outcome.error,
        cancelled: outcome.cancelled,
    }
}

/// Fichier de mise à jour : numéro (extension) et contenu, ou erreur de
/// lecture.
type UpdateInput = (u32, Result<Vec<u8>, String>);

/// Décode une cellule de base et applique ses mises à jour dans l'ordre.
/// La première mise à jour illisible, refusée ou manquante arrête
/// l'application : la cellule reste valide mais signalée comme non à jour.
fn decode_with_updates(base: &[u8], updates: Vec<UpdateInput>) -> Result<S57Cell, S57Error> {
    let mut problems = Problems::default();
    let mut state = load_base(read_dataset(base)?, &mut problems)?;
    let mut outcome = UpdateOutcome::default();
    let total = u32::try_from(updates.len()).unwrap_or(u32::MAX);
    for (done, (number, bytes)) in (0u32..).zip(updates) {
        let result = bytes.and_then(|bytes| update::apply_file(&state, &bytes));
        match result {
            Ok(update::Applied::AlreadyIncluded) => {}
            Ok(update::Applied::Cancelled { issue_date }) => {
                tracing::warn!(cell = %state.id.name, date = %issue_date, "cellule S-57 annulée");
                outcome.cancelled = Some(issue_date);
                outcome.pending = total - done - 1;
                break;
            }
            Ok(update::Applied::Updated {
                state: next,
                issue_date,
            }) => {
                state = *next;
                outcome.applied += 1;
                outcome.issue_date = Some(issue_date);
            }
            Err(reason) => {
                outcome.error = Some(format!("mise à jour .{number:03} : {reason}"));
                outcome.pending = total - done;
                break;
            }
        }
    }
    Ok(finish(state, problems, outcome))
}

/// Décode une cellule S-57 de base (contenu d'un fichier `.000`), sans
/// mise à jour.
///
/// # Errors
///
/// [`S57Error`] si le fichier n'est pas une cellule S-57 lisible, n'est
/// pas un fichier de base, ou n'est pas en WGS 84 et en mètres.
pub fn decode_cell(bytes: &[u8]) -> Result<S57Cell, S57Error> {
    decode_with_updates(bytes, Vec::new())
}

/// Décode une cellule de base et applique ses mises à jour (contenus des
/// fichiers `.001`, `.002`…, dans l'ordre).
///
/// # Errors
///
/// Voir [`decode_cell`] : seule la cellule de base peut produire une
/// erreur ; un problème de mise à jour est signalé dans
/// [`S57Cell::update_error`].
pub fn decode_cell_with_updates(base: &[u8], updates: &[&[u8]]) -> Result<S57Cell, S57Error> {
    let inputs = (1u32..)
        .zip(updates)
        .map(|(n, bytes)| (n, Ok(bytes.to_vec())))
        .collect();
    decode_with_updates(base, inputs)
}

/// Lit un fichier en refusant les fichiers trop volumineux.
fn read_limited(path: &Path) -> Result<Vec<u8>, S57Error> {
    let size = std::fs::metadata(path)?.len();
    if size > MAX_CELL_BYTES {
        return Err(S57Error::TooLarge {
            size,
            max: MAX_CELL_BYTES,
        });
    }
    Ok(std::fs::read(path)?)
}

/// Fichiers de mise à jour d'une cellule (`NOM.001`… dans le même
/// dossier, casse ignorée), triés par numéro.
fn update_files(base: &Path) -> Result<Vec<(u32, PathBuf)>, S57Error> {
    let Some(stem) = base.file_stem().and_then(|s| s.to_str()) else {
        return Ok(Vec::new());
    };
    let dir = match base.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let same_cell = path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case(stem));
        let number = path
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| e.len() == 3 && e.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|e| e.parse::<u32>().ok())
            .filter(|n| (1..=MAX_UPDATES).contains(n));
        if let (true, Some(number)) = (same_cell, number) {
            if path.is_file() {
                files.push((number, path));
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Lit et décode un fichier de cellule de base et applique les mises à
/// jour présentes à côté (opération bloquante, voir [`decode_cell_file`]).
///
/// # Errors
///
/// [`S57Error::Io`] si la cellule ou son dossier est illisible,
/// [`S57Error::TooLarge`] si elle dépasse [`MAX_CELL_BYTES`], sinon voir
/// [`decode_cell`]. Une mise à jour illisible est signalée dans
/// [`S57Cell::update_error`].
pub fn decode_cell_file_blocking(path: &Path) -> Result<S57Cell, S57Error> {
    let base = read_limited(path)?;
    let updates = update_files(path)?
        .into_iter()
        .map(|(number, file)| (number, read_limited(&file).map_err(|e| e.to_string())))
        .collect();
    decode_with_updates(&base, updates)
}

/// Lit et décode un fichier de cellule (et ses mises à jour) sans bloquer
/// le runtime asynchrone.
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
