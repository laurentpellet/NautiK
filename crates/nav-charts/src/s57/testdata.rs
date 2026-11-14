//! Fabrication de cellules S-57 pour les tests : écriture ISO 8211
//! (DDR et DR) conforme à l'implémentation binaire ENC.

use super::iso8211::{FIELD_TERMINATOR, UNIT_TERMINATOR};

/// Champ : étiquette et données (sans fin de champ).
pub type FieldData = (&'static str, Vec<u8>);

/// Écrit un enregistrement ISO 8211 (DDR si `ddr`).
pub fn record(ddr: bool, fields: &[FieldData]) -> Vec<u8> {
    let mut directory = Vec::new();
    let mut area = Vec::new();
    for (tag, data) in fields {
        directory.extend_from_slice(tag.as_bytes());
        directory.extend_from_slice(format!("{:05}{:05}", data.len() + 1, area.len()).as_bytes());
        area.extend_from_slice(data);
        area.push(FIELD_TERMINATOR);
    }
    directory.push(FIELD_TERMINATOR);
    let base = 24 + directory.len();
    let total = base + area.len();
    let leader = if ddr {
        format!("{total:05}3LE1 06{base:05} ! 5504")
    } else {
        format!("{total:05} D     {base:05}   5504")
    };
    assert_eq!(leader.len(), 24);
    let mut out = leader.into_bytes();
    out.extend(directory);
    out.extend(area);
    out
}

/// Description de champ du DDR.
fn description(tag: &'static str, array: &str, format: &str) -> FieldData {
    let mut data = b"1600;&".to_vec();
    data.extend_from_slice(tag.as_bytes());
    data.push(UNIT_TERMINATOR);
    data.extend_from_slice(array.as_bytes());
    data.push(UNIT_TERMINATOR);
    data.extend_from_slice(format.as_bytes());
    (tag, data)
}

/// Champ de contrôle de mise à jour de liste (VRPC, SGCC, FSPC).
fn control((instruction, index, count): (u8, u16, u16)) -> Vec<u8> {
    let mut out = vec![instruction];
    out.extend(index.to_le_bytes());
    out.extend(count.to_le_bytes());
    out
}

/// Texte variable terminé.
fn text(out: &mut Vec<u8>, value: &str) {
    out.extend(
        value
            .chars()
            .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')),
    );
    out.push(UNIT_TERMINATOR);
}

/// Nœud ou arc à écrire.
pub struct Vector {
    pub rcnm: u8,
    pub rcid: u32,
    pub ruin: u8,
    /// (x = longitude, y = latitude) en unités COMF.
    pub coords: Vec<(i32, i32)>,
    /// Profondeurs en unités SOMF (SG3D) ; vide pour SG2D.
    pub depths: Vec<i32>,
    /// Nœuds (RCNM, RCID, TOPI).
    pub nodes: Vec<(u8, u32, u8)>,
    /// Version (RVER).
    pub version: u16,
    /// Instruction VRPC (instruction, position, nombre).
    pub pointer_update: Option<(u8, u16, u16)>,
    /// Instruction SGCC (instruction, position, nombre).
    pub coord_update: Option<(u8, u16, u16)>,
}

/// Objet à écrire.
pub struct Feature {
    pub rcid: u32,
    pub prim: u8,
    pub objl: u16,
    pub ruin: u8,
    pub attributes: Vec<(u16, String)>,
    /// Attributs nationaux (encodés en UCS-2 si `nall` = 2).
    pub national: Vec<(u16, String)>,
    /// (RCNM, RCID, ORNT, USAG, MASK).
    pub pointers: Vec<(u8, u32, u8, u8, u8)>,
    /// Version (RVER).
    pub version: u16,
    /// Instruction FSPC (instruction, position, nombre).
    pub pointer_update: Option<(u8, u16, u16)>,
}

/// Cellule de test paramétrable.
pub struct CellBuilder {
    pub expp: u8,
    pub edition: &'static str,
    pub update: String,
    pub hdat: u8,
    pub duni: u8,
    pub comf: u32,
    pub somf: u32,
    pub nall: u8,
    pub sg2d_format: &'static str,
    pub with_dspm: bool,
    /// Nombre d'objets annoncé ; `None` = nombre réel.
    pub declared_features: Option<u32>,
    pub vectors: Vec<Vector>,
    pub features: Vec<Feature>,
}

/// Facteur de coordonnées de la cellule d'exemple.
pub const COMF: u32 = 10_000_000;

/// Coordonnée en unités COMF (degrés → entier).
pub fn deg(value: f64) -> i32 {
    (value * f64::from(COMF)).round() as i32
}

impl CellBuilder {
    /// Cellule vide, WGS 84, mètres.
    pub fn new() -> Self {
        Self {
            expp: 1,
            edition: "1",
            update: "0".into(),
            hdat: 2,
            duni: 1,
            comf: COMF,
            somf: 10,
            nall: 2,
            sg2d_format: "(2b24)",
            with_dspm: true,
            declared_features: None,
            vectors: Vec::new(),
            features: Vec::new(),
        }
    }

    /// Fichier de mise à jour n° `number` de la cellule d'exemple, vide.
    pub fn update(number: u32) -> Self {
        Self {
            expp: 2,
            update: number.to_string(),
            with_dspm: false,
            ..Self::new()
        }
    }

    /// Cellule d'exemple au large de Tahiti : zone de profondeur carrée
    /// avec un trou, trait de côte, sondes, épave, objet de couverture,
    /// collection, et un obstacle dont le nœud manque (à écarter).
    pub fn sample() -> Self {
        let mut cell = Self::new();
        let node = |rcid, lon, lat| Vector {
            rcnm: 120,
            rcid,
            ruin: 1,
            coords: vec![(deg(lon), deg(lat))],
            depths: Vec::new(),
            nodes: Vec::new(),
            version: 1,
            pointer_update: None,
            coord_update: None,
        };
        let edge = |rcid, begin, end, inner: &[(f64, f64)]| Vector {
            rcnm: 130,
            rcid,
            ruin: 1,
            coords: inner.iter().map(|&(x, y)| (deg(x), deg(y))).collect(),
            depths: Vec::new(),
            nodes: vec![(120, begin, 1), (120, end, 2)],
            version: 1,
            pointer_update: None,
            coord_update: None,
        };
        cell.vectors = vec![
            node(1, -149.6, -17.5),
            node(2, -149.5, -17.4),
            node(3, -149.58, -17.48),
            edge(10, 1, 2, &[(-149.6, -17.4)]),
            edge(11, 1, 2, &[(-149.5, -17.5)]),
            edge(
                12,
                3,
                3,
                &[(-149.58, -17.46), (-149.56, -17.46), (-149.56, -17.48)],
            ),
            Vector {
                rcnm: 110,
                rcid: 50,
                ruin: 1,
                coords: vec![
                    (deg(-149.55), deg(-17.45)),
                    (deg(-149.54), deg(-17.44)),
                    (deg(-149.53), deg(-17.43)),
                ],
                depths: vec![55, 120, 3],
                nodes: Vec::new(),
                version: 1,
                pointer_update: None,
                coord_update: None,
            },
            Vector {
                rcnm: 110,
                rcid: 51,
                ruin: 1,
                coords: vec![(deg(-149.52), deg(-17.42))],
                depths: Vec::new(),
                nodes: Vec::new(),
                version: 1,
                pointer_update: None,
                coord_update: None,
            },
        ];
        let feature = |rcid, prim, objl, pointers: Vec<(u8, u32, u8, u8, u8)>| Feature {
            rcid,
            prim,
            objl,
            ruin: 1,
            attributes: Vec::new(),
            national: Vec::new(),
            pointers,
            version: 1,
            pointer_update: None,
        };
        let mut depare = feature(
            100,
            3,
            42,
            vec![(130, 10, 1, 1, 2), (130, 11, 2, 1, 2), (130, 12, 1, 2, 2)],
        );
        depare.attributes = vec![(87, "5".into()), (88, "10".into())];
        let mut wreck = feature(104, 1, 159, vec![(110, 51, 255, 255, 255)]);
        wreck.attributes = vec![(179, "3.5".into()), (116, "Epave du port".into())];
        wreck.national = vec![(301, "Épave".into())];
        cell.features = vec![
            depare,
            feature(101, 2, 30, vec![(130, 10, 1, 255, 2), (130, 11, 2, 255, 2)]),
            feature(102, 3, 302, vec![(130, 10, 1, 1, 1), (130, 11, 2, 1, 1)]),
            feature(103, 1, 129, vec![(110, 50, 255, 255, 255)]),
            wreck,
            feature(105, 1, 86, vec![(110, 99, 255, 255, 255)]),
            feature(106, 255, 400, Vec::new()),
        ];
        cell
    }

    /// Écrit la cellule.
    pub fn build(&self) -> Vec<u8> {
        let mut out = record(
            true,
            &[
                ("0000", b"0000;&   cellule de test".to_vec()),
                (
                    "0001",
                    b"0100;&ISO 8211 Record Identifier\x1f\x1f(b12)".to_vec(),
                ),
                description(
                    "DSID",
                    "RCNM!RCID!EXPP!INTU!DSNM!EDTN!UPDN!UADT!ISDT!STED!PRSP!PSDN!PRED!PROF!AGEN!COMT",
                    "(b11,b14,2b11,3A,2A(8),R(4),b11,2A,b11,b12,A)",
                ),
                description(
                    "DSSI",
                    "DSTR!AALL!NALL!NOMR!NOCR!NOGR!NOLR!NOIN!NOCN!NOED!NOFA",
                    "(3b11,8b14)",
                ),
                description(
                    "DSPM",
                    "RCNM!RCID!HDAT!VDAT!SDAT!CSCL!DUNI!HUNI!PUNI!COUN!COMF!SOMF!COMT",
                    "(b11,b14,3b11,b14,4b11,2b14,A)",
                ),
                description("VRID", "RCNM!RCID!RVER!RUIN", "(b11,b14,b12,b11)"),
                description("VRPC", "VPUI!VPIX!NVPT", "(b11,2b12)"),
                description("SGCC", "CCUI!CCIX!CCNC", "(b11,2b12)"),
                description("FSPC", "FSUI!FSIX!NSPT", "(b11,2b12)"),
                description("VRPT", "*NAME!ORNT!USAG!TOPI!MASK", "(B(40),4b11)"),
                description("SG2D", "*YCOO!XCOO", self.sg2d_format),
                description("SG3D", "*YCOO!XCOO!VE3D", "(3b24)"),
                description(
                    "FRID",
                    "RCNM!RCID!PRIM!GRUP!OBJL!RVER!RUIN",
                    "(b11,b14,2b11,2b12,b11)",
                ),
                description("FOID", "AGEN!FIDN!FIDS", "(b12,b14,b12)"),
                description("ATTF", "*ATTL!ATVL", "(b12,A)"),
                description("NATF", "*ATTL!ATVL", "(b12,A)"),
                description("FSPT", "*NAME!ORNT!USAG!MASK", "(B(40),3b11)"),
            ],
        );
        let mut id = 0u16;
        let mut next = || {
            id += 1;
            ("0001", id.to_le_bytes().to_vec())
        };

        let count = |rcnm| self.vectors.iter().filter(|v| v.rcnm == rcnm).count() as u32;
        let mut dsid = vec![10];
        dsid.extend(1u32.to_le_bytes());
        dsid.extend([self.expp, 5]);
        text(&mut dsid, "TEST0001.000");
        text(&mut dsid, self.edition);
        text(&mut dsid, &self.update);
        dsid.extend(b"2026110120261102");
        dsid.extend(b"03.1");
        dsid.push(1);
        text(&mut dsid, "");
        text(&mut dsid, "2.0");
        dsid.push(1);
        dsid.extend(550u16.to_le_bytes());
        text(&mut dsid, "");
        let mut dssi = vec![2, 1, self.nall];
        let features = self.declared_features.unwrap_or(self.features.len() as u32);
        for n in [0, features, 0, 0, count(110), count(120), count(130), 0] {
            dssi.extend(n.to_le_bytes());
        }
        out.extend(record(false, &[next(), ("DSID", dsid), ("DSSI", dssi)]));

        if self.with_dspm {
            let mut dspm = vec![20];
            dspm.extend(1u32.to_le_bytes());
            dspm.extend([self.hdat, 17, 23]);
            dspm.extend(22_000u32.to_le_bytes());
            dspm.extend([self.duni, 1, 1, 1]);
            dspm.extend(self.comf.to_le_bytes());
            dspm.extend(self.somf.to_le_bytes());
            dspm.push(UNIT_TERMINATOR);
            out.extend(record(false, &[next(), ("DSPM", dspm)]));
        }

        for v in &self.vectors {
            let mut vrid = vec![v.rcnm];
            vrid.extend(v.rcid.to_le_bytes());
            vrid.extend(v.version.to_le_bytes());
            vrid.push(v.ruin);
            let mut fields = vec![next(), ("VRID", vrid)];
            if let Some(c) = v.pointer_update {
                fields.push(("VRPC", control(c)));
            }
            if !v.nodes.is_empty() {
                let mut vrpt = Vec::new();
                for &(rcnm, rcid, topi) in &v.nodes {
                    vrpt.push(rcnm);
                    vrpt.extend(rcid.to_le_bytes());
                    vrpt.extend([255, 255, topi, 255]);
                }
                fields.push(("VRPT", vrpt));
            }
            if let Some(c) = v.coord_update {
                fields.push(("SGCC", control(c)));
            }
            if !v.coords.is_empty() {
                let mut sg = Vec::new();
                for (i, &(x, y)) in v.coords.iter().enumerate() {
                    sg.extend(y.to_le_bytes());
                    sg.extend(x.to_le_bytes());
                    if let Some(depth) = v.depths.get(i) {
                        sg.extend(depth.to_le_bytes());
                    }
                }
                fields.push((if v.depths.is_empty() { "SG2D" } else { "SG3D" }, sg));
            }
            out.extend(record(false, &fields));
        }

        for f in &self.features {
            let mut frid = vec![100];
            frid.extend(f.rcid.to_le_bytes());
            frid.extend([f.prim, 2]);
            frid.extend(f.objl.to_le_bytes());
            frid.extend(f.version.to_le_bytes());
            frid.push(f.ruin);
            let mut foid = 550u16.to_le_bytes().to_vec();
            foid.extend(f.rcid.to_le_bytes());
            foid.extend(1u16.to_le_bytes());
            let mut fields = vec![next(), ("FRID", frid), ("FOID", foid)];
            if !f.attributes.is_empty() {
                let mut attf = Vec::new();
                for (code, value) in &f.attributes {
                    attf.extend(code.to_le_bytes());
                    text(&mut attf, value);
                }
                fields.push(("ATTF", attf));
            }
            if !f.national.is_empty() {
                let mut natf = Vec::new();
                for (code, value) in &f.national {
                    natf.extend(code.to_le_bytes());
                    if self.nall == 2 {
                        for unit in value.encode_utf16() {
                            natf.extend(unit.to_le_bytes());
                        }
                        natf.extend([UNIT_TERMINATOR, 0]);
                    } else {
                        text(&mut natf, value);
                    }
                }
                fields.push(("NATF", natf));
            }
            if let Some(c) = f.pointer_update {
                fields.push(("FSPC", control(c)));
            }
            if !f.pointers.is_empty() {
                let mut fspt = Vec::new();
                for &(rcnm, rcid, ornt, usag, mask) in &f.pointers {
                    fspt.push(rcnm);
                    fspt.extend(rcid.to_le_bytes());
                    fspt.extend([ornt, usag, mask]);
                }
                fields.push(("FSPT", fspt));
            }
            out.extend(record(false, &fields));
        }
        out
    }
}
