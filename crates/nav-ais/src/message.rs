//! Décodage des messages AIS (ITU-R M.1371-5).
//!
//! Messages pris en charge :
//! - 1, 2, 3 : position, classe A ;
//! - 5 : données statiques et de voyage, classe A ;
//! - 18 : position, classe B ;
//! - 19 : position et identité étendues, classe B ;
//! - 24 : données statiques, classe B (parties A et B).
//!
//! Les autres types sont ignorés. Les valeurs « non disponible » et les
//! valeurs réservées ou hors bornes deviennent `None` : aucune position,
//! vitesse ou route n'est inventée.

use std::fmt;

use nav_core::GeoPosition;
use nav_geodesy::units::MPS_PER_KNOT;
use serde::{Deserialize, Serialize};

use crate::armor::Bits;
use crate::AisError;

/// Identité MMSI validée (1 à 999 999 999).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mmsi(u32);

impl Mmsi {
    /// Valeur maximale (9 chiffres).
    pub const MAX: u32 = 999_999_999;

    /// Valide une identité MMSI.
    ///
    /// # Errors
    ///
    /// [`AisError::InvalidMmsi`] si la valeur est nulle ou dépasse
    /// 9 chiffres.
    pub fn new(value: u32) -> Result<Self, AisError> {
        if value == 0 || value > Self::MAX {
            return Err(AisError::InvalidMmsi(value));
        }
        Ok(Self(value))
    }

    /// Valeur numérique.
    #[must_use]
    pub fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for Mmsi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:09}", self.0)
    }
}

impl Serialize for Mmsi {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Classe de l'équipement AIS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AisClass {
    /// Classe A (navires SOLAS).
    A,
    /// Classe B (plaisance, pêche).
    B,
}

/// Données dynamiques (messages 1, 2, 3, 18, 19).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Dynamic {
    /// Statut de navigation (0 à 14 ; 15 « non défini » → `None`).
    pub navigation_status: Option<u8>,
    /// Position WGS84.
    pub position: Option<GeoPosition>,
    /// Vitesse fond (m/s).
    pub sog_mps: Option<f64>,
    /// Route fond vraie (rad, [0, 2π[).
    pub cog_rad: Option<f64>,
    /// Cap vrai (rad, [0, 2π[).
    pub heading_rad: Option<f64>,
}

/// Données d'identité (messages 5, 19, 24).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Identity {
    /// Nom du navire.
    pub name: Option<String>,
    /// Indicatif d'appel.
    pub callsign: Option<String>,
    /// Type de navire et de cargaison (1 à 255 ; 0 → `None`).
    pub ship_type: Option<u8>,
}

/// Message AIS décodé.
#[derive(Debug, Clone, PartialEq)]
pub struct AisReport {
    /// Type de message (1 à 27).
    pub message_type: u8,
    /// Émetteur.
    pub mmsi: Mmsi,
    /// Classe de l'équipement.
    pub class: AisClass,
    /// Message émis par le navire propre (phrase VDO).
    pub own_vessel: bool,
    /// Données dynamiques, si le message en porte.
    pub dynamic: Option<Dynamic>,
    /// Données d'identité, si le message en porte.
    pub identity: Option<Identity>,
}

/// Longitude ou latitude en 1/10 000 de minute ; `None` si non
/// disponible (181° / 91°) ou hors bornes.
fn coordinate(raw: i32, max_deg: f64) -> Option<f64> {
    let deg = f64::from(raw) / 600_000.0;
    (deg.abs() <= max_deg).then_some(deg)
}

fn position(bits: &Bits, lon_at: usize, lat_at: usize) -> Option<GeoPosition> {
    let lon = coordinate(bits.signed(lon_at, 28)?, 180.0)?;
    let lat = coordinate(bits.signed(lat_at, 27)?, 90.0)?;
    GeoPosition::new(lat, lon).ok()
}

/// Vitesse en 1/10 de nœud ; 1023 = non disponible.
fn sog(raw: u32) -> Option<f64> {
    (raw < 1023).then(|| f64::from(raw) / 10.0 * MPS_PER_KNOT)
}

/// Route en 1/10 de degré ; 3600 = non disponible, au-delà réservé.
fn cog(raw: u32) -> Option<f64> {
    (raw < 3600).then(|| (f64::from(raw) / 10.0).to_radians())
}

/// Cap en degrés ; 511 = non disponible, 360 à 510 hors bornes.
fn heading(raw: u32) -> Option<f64> {
    (raw < 360).then(|| f64::from(raw).to_radians())
}

/// Texte 6 bits nettoyé (`@` et espaces de complément retirés).
fn text(bits: &Bits, start: usize, chars: usize) -> Option<String> {
    let raw = bits.text(start, chars)?;
    let cleaned = raw.split('@').next().unwrap_or_default().trim().to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}

fn ship_type(raw: u32) -> Option<u8> {
    u8::try_from(raw).ok().filter(|t| *t != 0)
}

struct Reader<'a> {
    bits: &'a Bits,
    message_type: u8,
}

impl Reader<'_> {
    fn require(&self, min_bits: usize) -> Result<(), AisError> {
        if self.bits.len() < min_bits {
            return Err(AisError::TooShort {
                message_type: self.message_type,
                bits: self.bits.len(),
            });
        }
        Ok(())
    }

    fn u(&self, start: usize, len: usize) -> Result<u32, AisError> {
        self.bits.unsigned(start, len).ok_or(AisError::TooShort {
            message_type: self.message_type,
            bits: self.bits.len(),
        })
    }
}

/// Décode un message réassemblé. Retourne `None` pour un type de
/// message non pris en charge.
///
/// # Errors
///
/// - [`AisError::TooShort`] : message plus court que son type ne
///   l'exige ;
/// - [`AisError::InvalidMmsi`] : identité invalide ;
/// - [`AisError::Field`] : partie de message 24 invalide.
pub fn decode(bits: &Bits, own_vessel: bool) -> Result<Option<AisReport>, AisError> {
    let message_type = bits
        .unsigned(0, 6)
        .and_then(|v| u8::try_from(v).ok())
        .ok_or(AisError::TooShort {
            message_type: 0,
            bits: bits.len(),
        })?;
    let r = Reader { bits, message_type };
    let min_bits = match message_type {
        1..=3 | 18 => 168,
        5 => 420,
        19 => 312,
        24 => 160,
        _ => return Ok(None),
    };
    r.require(min_bits)?;
    let mmsi = Mmsi::new(r.u(8, 30)?)?;
    let report = |class, dynamic, identity| AisReport {
        message_type,
        mmsi,
        class,
        own_vessel,
        dynamic,
        identity,
    };

    let decoded = match message_type {
        1..=3 => report(
            AisClass::A,
            Some(Dynamic {
                navigation_status: Some(r.u(38, 4)?)
                    .filter(|s| *s != 15)
                    .and_then(|s| u8::try_from(s).ok()),
                position: position(bits, 61, 89),
                sog_mps: sog(r.u(50, 10)?),
                cog_rad: cog(r.u(116, 12)?),
                heading_rad: heading(r.u(128, 9)?),
            }),
            None,
        ),
        5 => report(
            AisClass::A,
            None,
            Some(Identity {
                callsign: text(bits, 70, 7),
                name: text(bits, 112, 20),
                ship_type: ship_type(r.u(232, 8)?),
            }),
        ),
        18 | 19 => {
            let dynamic = Dynamic {
                navigation_status: None,
                position: position(bits, 57, 85),
                sog_mps: sog(r.u(46, 10)?),
                cog_rad: cog(r.u(112, 12)?),
                heading_rad: heading(r.u(124, 9)?),
            };
            let identity = (message_type == 19).then(|| Identity {
                name: text(bits, 143, 20),
                callsign: None,
                ship_type: r.u(263, 8).ok().and_then(ship_type),
            });
            report(AisClass::B, Some(dynamic), identity)
        }
        _ => {
            // Message 24.
            let identity = match r.u(38, 2)? {
                0 => Identity {
                    name: text(bits, 40, 20),
                    ..Identity::default()
                },
                1 => {
                    r.require(168)?;
                    Identity {
                        ship_type: ship_type(r.u(40, 8)?),
                        callsign: text(bits, 90, 7),
                        name: None,
                    }
                }
                part => {
                    return Err(AisError::Field {
                        field: "partie du message 24",
                        value: part.to_string(),
                    })
                }
            };
            report(AisClass::B, None, Some(identity))
        }
    };
    Ok(Some(decoded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::armor::BitWriter;

    fn bits(w: &BitWriter) -> Bits {
        let (payload, fill) = w.payload();
        Bits::from_payload(&payload, fill).expect("bits")
    }

    fn class_a(mmsi: u32, lon: i32, lat: i32, sog: u32, cog: u32, hdg: u32, status: u32) -> Bits {
        let mut w = BitWriter::new();
        w.unsigned(1, 6)
            .unsigned(0, 2)
            .unsigned(mmsi, 30)
            .unsigned(status, 4)
            .signed(-128, 8)
            .unsigned(sog, 10)
            .unsigned(1, 1)
            .signed(lon, 28)
            .signed(lat, 27)
            .unsigned(cog, 12)
            .unsigned(hdg, 9)
            .unsigned(30, 6)
            .unsigned(0, 25);
        bits(&w)
    }

    #[test]
    fn mmsi_bounds() {
        assert!(Mmsi::new(0).is_err());
        assert!(Mmsi::new(1_000_000_000).is_err());
        assert_eq!(Mmsi::new(2_270_001).expect("mmsi").to_string(), "002270001");
        assert_eq!(
            serde_json::to_string(&Mmsi::new(227_000_001).expect("mmsi")).expect("json"),
            "\"227000001\""
        );
    }

    #[test]
    fn decodes_class_a_position() {
        let b = class_a(227_006_760, -88_582_800, -8_980_800, 123, 450, 44, 0);
        let r = decode(&b, false).expect("ok").expect("report");
        assert_eq!(r.mmsi.value(), 227_006_760);
        assert_eq!(r.class, AisClass::A);
        let d = r.dynamic.expect("dynamic");
        let p = d.position.expect("position");
        assert!((p.longitude_deg() - -147.638).abs() < 1e-9);
        assert!((p.latitude_deg() - -14.968).abs() < 1e-9);
        assert!((d.sog_mps.expect("sog") - 12.3 * MPS_PER_KNOT).abs() < 1e-9);
        assert!((d.cog_rad.expect("cog") - 45_f64.to_radians()).abs() < 1e-9);
        assert!((d.heading_rad.expect("hdg") - 44_f64.to_radians()).abs() < 1e-9);
        assert_eq!(d.navigation_status, Some(0));
        assert!(r.identity.is_none());
    }

    #[test]
    fn unavailable_values_are_none() {
        // Longitude 181°, latitude 91°, SOG 1023, COG 3600, cap 511,
        // statut 15.
        let b = class_a(227_006_760, 108_600_000, 54_600_000, 1023, 3600, 511, 15);
        let d = decode(&b, false)
            .expect("ok")
            .expect("r")
            .dynamic
            .expect("d");
        assert_eq!(d, Dynamic::default());
    }

    #[test]
    fn reserved_values_are_none() {
        // Latitude 95°, COG 3700, cap 400 : réservés / hors bornes.
        let b = class_a(227_006_760, 0, 57_000_000, 0, 3700, 400, 3);
        let d = decode(&b, false)
            .expect("ok")
            .expect("r")
            .dynamic
            .expect("d");
        assert_eq!(d.position, None);
        assert_eq!(d.cog_rad, None);
        assert_eq!(d.heading_rad, None);
        assert_eq!(d.sog_mps, Some(0.0));
        assert_eq!(d.navigation_status, Some(3));
        // Une seule coordonnée disponible : aucune position.
        let b = class_a(227_006_760, 108_600_000, 0, 0, 0, 0, 0);
        assert_eq!(
            decode(&b, false)
                .expect("ok")
                .expect("r")
                .dynamic
                .expect("d")
                .position,
            None
        );
    }

    #[test]
    fn rejects_invalid_mmsi_and_short_messages() {
        let b = class_a(0, 0, 0, 0, 0, 0, 0);
        assert_eq!(decode(&b, false), Err(AisError::InvalidMmsi(0)));
        let b = class_a(1_073_741_823, 0, 0, 0, 0, 0, 0);
        assert!(matches!(decode(&b, false), Err(AisError::InvalidMmsi(_))));
        let mut w = BitWriter::new();
        w.unsigned(1, 6).unsigned(0, 2).unsigned(227_006_760, 30);
        assert!(matches!(
            decode(&bits(&w), false),
            Err(AisError::TooShort {
                message_type: 1,
                ..
            })
        ));
        let empty = Bits::from_payload("0", 4).expect("2 bits");
        assert!(decode(&empty, false).is_err());
    }

    #[test]
    fn ignores_unsupported_types() {
        let mut w = BitWriter::new();
        w.unsigned(21, 6).unsigned(0, 200);
        assert_eq!(decode(&bits(&w), false), Ok(None));
    }

    #[test]
    fn decodes_static_class_a() {
        let mut w = BitWriter::new();
        w.unsigned(5, 6)
            .unsigned(0, 2)
            .unsigned(227_006_760, 30)
            .unsigned(0, 2)
            .unsigned(9_123_456, 30)
            .text("FOPQ", 7)
            .text("ARANUI 5", 20)
            .unsigned(70, 8)
            .unsigned(0, 424 - 240);
        let r = decode(&bits(&w), false).expect("ok").expect("r");
        let i = r.identity.expect("identity");
        assert_eq!(i.name.as_deref(), Some("ARANUI 5"));
        assert_eq!(i.callsign.as_deref(), Some("FOPQ"));
        assert_eq!(i.ship_type, Some(70));
        assert!(r.dynamic.is_none());
    }

    #[test]
    fn decodes_class_b() {
        let mut w = BitWriter::new();
        w.unsigned(19, 6)
            .unsigned(0, 2)
            .unsigned(227_123_456, 30)
            .unsigned(0, 8)
            .unsigned(60, 10)
            .unsigned(0, 1)
            .signed(-88_582_800, 28)
            .signed(-8_980_800, 27)
            .unsigned(900, 12)
            .unsigned(511, 9)
            .unsigned(0, 10)
            .text("MOANA", 20)
            .unsigned(36, 8)
            .unsigned(0, 41);
        let r = decode(&bits(&w), false).expect("ok").expect("r");
        assert_eq!(r.class, AisClass::B);
        let d = r.dynamic.expect("d");
        assert!(d.position.is_some());
        assert_eq!(d.heading_rad, None);
        assert_eq!(d.navigation_status, None);
        let i = r.identity.expect("i");
        assert_eq!((i.name.as_deref(), i.ship_type), (Some("MOANA"), Some(36)));
    }

    #[test]
    fn decodes_message_24_parts() {
        let mut a = BitWriter::new();
        a.unsigned(24, 6)
            .unsigned(0, 2)
            .unsigned(227_123_456, 30)
            .unsigned(0, 2)
            .text("MOANA", 20);
        let i = decode(&bits(&a), false)
            .expect("ok")
            .expect("r")
            .identity
            .expect("i");
        assert_eq!(i.name.as_deref(), Some("MOANA"));

        let mut b = BitWriter::new();
        b.unsigned(24, 6)
            .unsigned(0, 2)
            .unsigned(227_123_456, 30)
            .unsigned(1, 2)
            .unsigned(37, 8)
            .unsigned(0, 42)
            .text("FQ1234", 7)
            .unsigned(0, 36);
        let i = decode(&bits(&b), false)
            .expect("ok")
            .expect("r")
            .identity
            .expect("i");
        assert_eq!(
            (i.ship_type, i.callsign.as_deref(), i.name),
            (Some(37), Some("FQ1234"), None)
        );

        let mut c = BitWriter::new();
        c.unsigned(24, 6)
            .unsigned(0, 2)
            .unsigned(227_123_456, 30)
            .unsigned(2, 2)
            .unsigned(0, 130);
        assert!(matches!(
            decode(&bits(&c), false),
            Err(AisError::Field { .. })
        ));
    }

    #[test]
    fn decodes_reference_sentence() {
        // Exemple public : !AIVDM,1,1,,A,15M67FC000G?ufbE`FepT@3n00Sa,0
        let b = Bits::from_payload("15M67FC000G?ufbE`FepT@3n00Sa", 0).expect("bits");
        let r = decode(&b, false).expect("ok").expect("r");
        assert_eq!(r.message_type, 1);
        assert_eq!(r.class, AisClass::A);
        assert!(r.dynamic.expect("d").position.is_some());
    }
}
