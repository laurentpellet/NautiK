//! Conversion des phrases NMEA 0183 vers le modèle interne (WGS84, SI,
//! UTC).
//!
//! Phrases prises en charge : RMC, GGA, GLL (position), VTG (route et
//! vitesse fond), HDT, HDG, HDM (cap), VHW (vitesse surface), DPT, DBT
//! (profondeur), MWV (vent apparent). Les phrases AIS encapsulées
//! (`!AIVDM`, `!AIVDO`) sont transmises à `nav-ais` sous forme de
//! fragments. Les autres sont ignorées.
//!
//! Aucune position n'est produite si l'équipement la déclare invalide
//! (statut `V`, qualité GGA 0) ou non mesurée : estimée (navigation à
//! l'estime), saisie manuellement ou simulée par le récepteur.

use chrono::{DateTime, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use nav_ais::{AisError, Fragment};
use nav_core::{GeoPosition, Measurement, PositionFix, Quantity, Sample, SourceKind};
use nav_geodesy::units::MPS_PER_KNOT;
use thiserror::Error;

use crate::sentence::{self, Sentence, SentenceError};

/// Mètres par pied.
const METERS_PER_FOOT: f64 = 0.3048;
/// Mètres par brasse.
const METERS_PER_FATHOM: f64 = 1.8288;
/// m/s par km/h.
const MPS_PER_KMH: f64 = 1_000.0 / 3_600.0;
/// m/s par mile terrestre par heure.
const MPS_PER_MPH: f64 = 0.447_04;

/// Erreur de décodage.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    /// Phrase invalide (format, somme de contrôle).
    #[error(transparent)]
    Sentence(#[from] SentenceError),
    /// Phrase AIS invalide.
    #[error(transparent)]
    Ais(#[from] AisError),
    /// Champ invalide.
    #[error("{kind} : champ {field} invalide ({reason})")]
    Field {
        /// Type de phrase.
        kind: String,
        /// Champ concerné.
        field: &'static str,
        /// Cause.
        reason: String,
    },
}

/// Résultat du décodage d'une phrase valide.
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    /// Données extraites (éventuellement aucune : position déclarée
    /// invalide par l'équipement, champs vides).
    Samples(Vec<Sample>),
    /// Fragment de message AIS (VDM, VDO), à réassembler et décoder par
    /// `nav-ais`.
    Ais(Fragment),
    /// Type de phrase non pris en charge.
    Unsupported(String),
}

/// Contexte de décodage.
#[derive(Debug, Clone, Copy)]
pub struct DecodeContext<'a> {
    /// Préfixe de l'identifiant de source (`<origine>.<émetteur>`).
    pub origin: &'a str,
    /// Nature de la source.
    pub source: SourceKind,
    /// Horodatage de réception.
    pub received_at: DateTime<Utc>,
    /// Utiliser l'heure portée par les phrases comme horodatage de
    /// mesure. Désactivé pour un rejeu (l'heure enregistrée est passée).
    pub use_sentence_time: bool,
}

type Result<T> = std::result::Result<T, DecodeError>;

struct Fields<'a> {
    kind: &'a str,
    values: &'a [&'a str],
}

impl<'a> Fields<'a> {
    fn error(&self, field: &'static str, reason: impl Into<String>) -> DecodeError {
        DecodeError::Field {
            kind: self.kind.to_owned(),
            field,
            reason: reason.into(),
        }
    }

    /// Champ non vide.
    fn get(&self, index: usize) -> Option<&'a str> {
        self.values
            .get(index)
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
    }

    fn number(&self, index: usize, field: &'static str) -> Result<Option<f64>> {
        self.get(index)
            .map(|v| {
                v.parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| self.error(field, format!("« {v} » n'est pas un nombre")))
            })
            .transpose()
    }

    /// Coordonnée `ddmm.mmmm` (`dddmm.mmmm`) et hémisphère.
    fn coordinate(
        &self,
        index: usize,
        field: &'static str,
        max_degrees: f64,
        hemispheres: (&str, &str),
    ) -> Result<Option<f64>> {
        let (Some(raw), Some(hemisphere)) = (self.get(index), self.get(index + 1)) else {
            return if self.get(index).is_some() || self.get(index + 1).is_some() {
                Err(self.error(field, "valeur ou hémisphère manquant"))
            } else {
                Ok(None)
            };
        };
        if !raw.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
            return Err(self.error(field, format!("« {raw} » mal formé")));
        }
        let value: f64 = raw
            .parse()
            .map_err(|_| self.error(field, format!("« {raw} » mal formé")))?;
        let degrees = (value / 100.0).trunc();
        let minutes = value - degrees * 100.0;
        if minutes >= 60.0 {
            return Err(self.error(field, "minutes ≥ 60"));
        }
        let decimal = degrees + minutes / 60.0;
        if decimal > max_degrees {
            return Err(self.error(field, "hors bornes"));
        }
        if hemisphere == hemispheres.0 {
            Ok(Some(decimal))
        } else if hemisphere == hemispheres.1 {
            Ok(Some(-decimal))
        } else {
            Err(self.error(field, format!("hémisphère « {hemisphere} »")))
        }
    }

    /// Heure `hhmmss[.sss]`.
    fn time(&self, index: usize) -> Result<Option<NaiveTime>> {
        let Some(raw) = self.get(index) else {
            return Ok(None);
        };
        let bad = || self.error("heure", format!("« {raw} » mal formée"));
        let (hms, frac) = raw.split_once('.').unwrap_or((raw, ""));
        if hms.len() != 6
            || !hms.bytes().all(|b| b.is_ascii_digit())
            || !frac.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(bad());
        }
        let n = |r: std::ops::Range<usize>| hms[r].parse::<u32>().map_err(|_| bad());
        let (h, m, s) = (n(0..2)?, n(2..4)?, n(4..6)?);
        let millis = format!("{frac:0<3}")
            .get(..3)
            .and_then(|f| f.parse::<u32>().ok())
            .ok_or_else(bad)?;
        // Seconde intercalaire : représentée par chrono au-delà de 59 s.
        let (s, millis) = if s == 60 {
            (59, millis + 1_000)
        } else {
            (s, millis)
        };
        NaiveTime::from_hms_milli_opt(h, m, s, millis)
            .map(Some)
            .ok_or_else(bad)
    }

    /// Date `ddmmyy` (années 80–99 : 19xx, sinon 20xx).
    fn date(&self, index: usize) -> Result<Option<NaiveDate>> {
        let Some(raw) = self.get(index) else {
            return Ok(None);
        };
        let bad = || self.error("date", format!("« {raw} » mal formée"));
        if raw.len() != 6 || !raw.bytes().all(|b| b.is_ascii_digit()) {
            return Err(bad());
        }
        let n = |r: std::ops::Range<usize>| raw[r].parse::<u32>().map_err(|_| bad());
        let (d, m, y) = (n(0..2)?, n(2..4)?, n(4..6)?);
        let year = if y >= 80 { 1900 + y } else { 2000 + y };
        NaiveDate::from_ymd_opt(i32::try_from(year).map_err(|_| bad())?, m, d)
            .map(Some)
            .ok_or_else(bad)
    }

    /// Indicateur de mode NMEA ≥ 2.3 : `true` si la donnée est mesurée.
    /// Estimée (E), manuelle (M), simulée (S) ou invalide (N) : rejetée.
    fn mode_is_measured(&self, index: usize) -> bool {
        !matches!(self.get(index), Some("E" | "M" | "S" | "N"))
    }
}

struct Decoder<'a> {
    ctx: DecodeContext<'a>,
    source_id: String,
    fields: Fields<'a>,
    samples: Vec<Sample>,
}

impl Decoder<'_> {
    /// Horodatage de mesure à partir de l'heure (et de la date) de la
    /// phrase. Sans date, le jour de réception le plus proche est retenu.
    fn measured_at(&self, date: Option<NaiveDate>, time: Option<NaiveTime>) -> DateTime<Utc> {
        let received = self.ctx.received_at;
        let Some(time) = time.filter(|_| self.ctx.use_sentence_time) else {
            return received;
        };
        match date {
            Some(date) => Utc.from_utc_datetime(&date.and_time(time)),
            None => {
                let same_day = Utc.from_utc_datetime(&received.date_naive().and_time(time));
                if same_day - received > Duration::hours(12) {
                    same_day - Duration::days(1)
                } else if received - same_day > Duration::hours(12) {
                    same_day + Duration::days(1)
                } else {
                    same_day
                }
            }
        }
    }

    fn measurement(
        &mut self,
        quantity: Quantity,
        value: Option<f64>,
        measured_at: DateTime<Utc>,
    ) -> Result<()> {
        let Some(value) = value else {
            return Ok(());
        };
        let m = Measurement::new(
            quantity,
            value,
            self.ctx.source,
            &self.source_id,
            measured_at,
            self.ctx.received_at,
        )
        .map_err(|e| self.fields.error(quantity_field(quantity), e.to_string()))?;
        self.samples.push(Sample::Measurement(m));
        Ok(())
    }

    fn position(
        &mut self,
        lat: Option<f64>,
        lon: Option<f64>,
        sog_mps: Option<f64>,
        cog_rad: Option<f64>,
        measured_at: DateTime<Utc>,
    ) -> Result<()> {
        let (Some(lat), Some(lon)) = (lat, lon) else {
            return Ok(());
        };
        let position =
            GeoPosition::new(lat, lon).map_err(|e| self.fields.error("position", e.to_string()))?;
        self.samples.push(Sample::Position(PositionFix {
            position,
            cog_rad,
            sog_mps,
            source: self.ctx.source,
            source_id: self.source_id.clone(),
            measured_at,
            received_at: self.ctx.received_at,
        }));
        Ok(())
    }

    fn latitude(&self, index: usize) -> Result<Option<f64>> {
        self.fields.coordinate(index, "latitude", 90.0, ("N", "S"))
    }

    fn longitude(&self, index: usize) -> Result<Option<f64>> {
        self.fields
            .coordinate(index, "longitude", 180.0, ("E", "W"))
    }

    fn knots(&self, index: usize, field: &'static str) -> Result<Option<f64>> {
        Ok(self
            .fields
            .number(index, field)?
            .map(|kn| kn * MPS_PER_KNOT))
    }

    /// Angle en degrés dans [0, 360], converti en radians.
    fn degrees(&self, index: usize, field: &'static str) -> Result<Option<f64>> {
        Ok(self.angle(index, field)?.map(f64::to_radians))
    }

    /// Angle en degrés dans [0, 360] (NMEA : jamais négatif).
    fn angle(&self, index: usize, field: &'static str) -> Result<Option<f64>> {
        match self.fields.number(index, field)? {
            Some(deg) if !(0.0..=360.0).contains(&deg) => {
                Err(self.fields.error(field, format!("{deg}° hors de [0, 360]")))
            }
            other => Ok(other),
        }
    }

    /// RMC : heure, statut, position, vitesse et route fond, date.
    fn rmc(&mut self) -> Result<()> {
        let f = &self.fields;
        if f.get(1) != Some("A") || !f.mode_is_measured(11) {
            return Ok(());
        }
        let measured_at = self.measured_at(f.date(8)?, f.time(0)?);
        let (lat, lon) = (self.latitude(2)?, self.longitude(4)?);
        let sog = self.knots(6, "vitesse fond")?;
        let cog = self.degrees(7, "route fond")?;
        self.position(lat, lon, sog, cog, measured_at)?;
        self.measurement(Quantity::SpeedOverGround, sog, measured_at)?;
        self.measurement(Quantity::CourseOverGroundTrue, cog, measured_at)
    }

    /// GGA : heure, position, qualité du point.
    fn gga(&mut self) -> Result<()> {
        // 1 GPS, 2 différentiel, 3 PPS, 4 RTK, 5 RTK flottant. 0
        // invalide, 6 estimé, 7 manuel, 8 simulation : rejetés.
        if !matches!(self.fields.get(5), Some("1" | "2" | "3" | "4" | "5")) {
            return Ok(());
        }
        let measured_at = self.measured_at(None, self.fields.time(0)?);
        let (lat, lon) = (self.latitude(1)?, self.longitude(3)?);
        self.position(lat, lon, None, None, measured_at)
    }

    /// GLL : position, heure, statut.
    fn gll(&mut self) -> Result<()> {
        let f = &self.fields;
        // Statut absent : phrase antérieure à NMEA 2.0, acceptée.
        if matches!(f.get(5), Some(s) if s != "A") || !f.mode_is_measured(6) {
            return Ok(());
        }
        let measured_at = self.measured_at(None, f.time(4)?);
        let (lat, lon) = (self.latitude(0)?, self.longitude(2)?);
        self.position(lat, lon, None, None, measured_at)
    }

    /// VTG : route fond vraie, vitesse fond (nœuds, sinon km/h).
    fn vtg(&mut self) -> Result<()> {
        if !self.fields.mode_is_measured(8) {
            return Ok(());
        }
        let at = self.ctx.received_at;
        let cog = self.degrees(0, "route fond")?;
        let sog = match self.knots(4, "vitesse fond")? {
            Some(v) => Some(v),
            None => self
                .fields
                .number(6, "vitesse fond")?
                .map(|kmh| kmh * MPS_PER_KMH),
        };
        self.measurement(Quantity::CourseOverGroundTrue, cog, at)?;
        self.measurement(Quantity::SpeedOverGround, sog, at)
    }

    fn hdt(&mut self) -> Result<()> {
        let heading = self.degrees(0, "cap vrai")?;
        self.measurement(Quantity::HeadingTrue, heading, self.ctx.received_at)
    }

    fn hdm(&mut self) -> Result<()> {
        let heading = self.degrees(0, "cap magnétique")?;
        self.measurement(Quantity::HeadingMagnetic, heading, self.ctx.received_at)
    }

    /// HDG : cap du capteur corrigé de la déviation (Est positive) →
    /// cap magnétique. La variation n'est pas appliquée.
    fn hdg(&mut self) -> Result<()> {
        let Some(sensor) = self.angle(0, "cap")? else {
            return Ok(());
        };
        let deviation = match (self.fields.number(1, "déviation")?, self.fields.get(2)) {
            (None, _) => 0.0,
            (Some(d), Some("E")) => d,
            (Some(d), Some("W")) => -d,
            (Some(_), _) => return Err(self.fields.error("déviation", "sens E/W manquant")),
        };
        let heading = (sensor + deviation).to_radians();
        self.measurement(
            Quantity::HeadingMagnetic,
            Some(heading),
            self.ctx.received_at,
        )
    }

    /// VHW : caps (facultatifs) et vitesse surface.
    fn vhw(&mut self) -> Result<()> {
        let at = self.ctx.received_at;
        let stw = match self.knots(4, "vitesse surface")? {
            Some(v) => Some(v),
            None => self
                .fields
                .number(6, "vitesse surface")?
                .map(|kmh| kmh * MPS_PER_KMH),
        };
        let heading_true = self.degrees(0, "cap vrai")?;
        let heading_magnetic = self.degrees(2, "cap magnétique")?;
        self.measurement(Quantity::SpeedThroughWater, stw, at)?;
        self.measurement(Quantity::HeadingTrue, heading_true, at)?;
        self.measurement(Quantity::HeadingMagnetic, heading_magnetic, at)
    }

    /// DPT : profondeur sous le transducteur (m).
    fn dpt(&mut self) -> Result<()> {
        let depth = self.fields.number(0, "profondeur")?;
        self.measurement(Quantity::DepthBelowTransducer, depth, self.ctx.received_at)
    }

    /// DBT : profondeur sous le transducteur, en mètres de préférence.
    fn dbt(&mut self) -> Result<()> {
        let f = &self.fields;
        let depth = if let Some(m) = f.number(2, "profondeur (m)")? {
            Some(m)
        } else if let Some(ft) = f.number(0, "profondeur (pieds)")? {
            Some(ft * METERS_PER_FOOT)
        } else {
            f.number(4, "profondeur (brasses)")?
                .map(|fa| fa * METERS_PER_FATHOM)
        };
        self.measurement(Quantity::DepthBelowTransducer, depth, self.ctx.received_at)
    }

    /// MWV : vent relatif (apparent) uniquement ; le vent vrai calculé
    /// par l'équipement est ignoré.
    fn mwv(&mut self) -> Result<()> {
        let f = &self.fields;
        if f.get(4) != Some("A") || f.get(1) != Some("R") {
            return Ok(());
        }
        let angle = self.angle(0, "angle du vent")?.map(|deg| {
            let signed = if deg > 180.0 { deg - 360.0 } else { deg };
            signed.to_radians()
        });
        let speed = match (f.number(2, "vitesse du vent")?, f.get(3)) {
            (None, _) => None,
            (Some(v), Some("N")) => Some(v * MPS_PER_KNOT),
            (Some(v), Some("M")) => Some(v),
            (Some(v), Some("K")) => Some(v * MPS_PER_KMH),
            (Some(v), Some("S")) => Some(v * MPS_PER_MPH),
            (Some(_), unit) => {
                return Err(f.error(
                    "unité de vitesse du vent",
                    format!("« {} »", unit.unwrap_or_default()),
                ))
            }
        };
        let at = self.ctx.received_at;
        self.measurement(Quantity::WindAngleApparent, angle, at)?;
        self.measurement(Quantity::WindSpeedApparent, speed, at)
    }
}

fn quantity_field(quantity: Quantity) -> &'static str {
    match quantity {
        Quantity::SpeedOverGround => "vitesse fond",
        Quantity::CourseOverGroundTrue => "route fond",
        Quantity::HeadingTrue => "cap vrai",
        Quantity::HeadingMagnetic => "cap magnétique",
        Quantity::SpeedThroughWater => "vitesse surface",
        Quantity::DepthBelowTransducer => "profondeur",
        Quantity::WindSpeedApparent => "vitesse du vent",
        Quantity::WindAngleApparent => "angle du vent",
    }
}

/// Décode une phrase déjà validée.
///
/// # Errors
///
/// [`DecodeError::Field`] si un champ est mal formé ou hors bornes.
pub fn decode_sentence(sentence: &Sentence<'_>, ctx: DecodeContext<'_>) -> Result<Decoded> {
    let mut decoder = Decoder {
        ctx,
        source_id: format!("{}.{}", ctx.origin, sentence.talker),
        fields: Fields {
            kind: sentence.kind,
            values: &sentence.fields,
        },
        samples: Vec::new(),
    };
    match (sentence.encapsulated, sentence.kind) {
        (false, "RMC") => decoder.rmc()?,
        (false, "GGA") => decoder.gga()?,
        (false, "GLL") => decoder.gll()?,
        (false, "VTG") => decoder.vtg()?,
        (false, "HDT") => decoder.hdt()?,
        (false, "HDM") => decoder.hdm()?,
        (false, "HDG") => decoder.hdg()?,
        (false, "VHW") => decoder.vhw()?,
        (false, "DPT") => decoder.dpt()?,
        (false, "DBT") => decoder.dbt()?,
        (false, "MWV") => decoder.mwv()?,
        (true, kind @ ("VDM" | "VDO")) => {
            return Ok(Decoded::Ais(Fragment::parse(kind, &sentence.fields)?));
        }
        (_, kind) => return Ok(Decoded::Unsupported(kind.to_owned())),
    }
    Ok(Decoded::Samples(decoder.samples))
}

/// Heure portée par une phrase de position (RMC, GGA, GLL), si la ligne
/// est valide. Sert à cadencer un rejeu.
#[must_use]
pub fn sentence_time(line: &str) -> Option<NaiveTime> {
    let sentence = sentence::parse(line).ok()?;
    let index = match sentence.kind {
        "RMC" | "GGA" => 0,
        "GLL" => 4,
        _ => return None,
    };
    Fields {
        kind: sentence.kind,
        values: &sentence.fields,
    }
    .time(index)
    .ok()
    .flatten()
}

/// Valide puis décode une ligne.
///
/// # Errors
///
/// [`DecodeError`] si la phrase ou l'un de ses champs est invalide.
pub fn decode(line: &str, ctx: DecodeContext<'_>) -> Result<Decoded> {
    let sentence = sentence::parse(line)?;
    decode_sentence(&sentence, ctx)
}
