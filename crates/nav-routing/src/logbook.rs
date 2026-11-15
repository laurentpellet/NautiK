//! Journal de bord : entrées horodatées, automatiques (relevé périodique)
//! ou manuelles (texte libre), avec les données du bord à cet instant.
//!
//! Seules des valeurs fraîches sont reprises ; une position obsolète est
//! conservée mais marquée comme telle, jamais présentée comme actuelle.

use std::fmt::Write as _;

use chrono::{DateTime, SecondsFormat, Utc};
use nav_core::{GeoPosition, InstrumentsSnapshot, Quantity};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{validate_description, ValidationError};

/// Intervalles proposés pour les entrées automatiques (minutes ; 0 :
/// désactivées).
pub const AUTO_INTERVALS_MIN: [u32; 5] = [0, 15, 30, 60, 240];
/// Intervalle par défaut des entrées automatiques (minutes).
pub const DEFAULT_AUTO_INTERVAL_MIN: u32 = 60;

/// Nature d'une entrée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogKind {
    /// Relevé périodique.
    Auto,
    /// Entrée saisie par l'équipage.
    Manual,
}

impl LogKind {
    /// Code stable (base).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }

    /// Nature correspondant à un code.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "auto" => Some(Self::Auto),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// Entrée du journal de bord.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// Identifiant.
    pub id: Uuid,
    /// Instant (UTC).
    pub at: DateTime<Utc>,
    /// Nature.
    pub kind: LogKind,
    /// Texte libre.
    pub text: Option<String>,
    /// Dernière position connue.
    pub position: Option<GeoPosition>,
    /// La position était obsolète au moment de l'entrée.
    pub position_stale: bool,
    /// Vitesse fond (m/s).
    pub sog_mps: Option<f64>,
    /// Route fond vraie (rad).
    pub cog_rad: Option<f64>,
    /// Cap vrai (rad).
    pub heading_rad: Option<f64>,
    /// Vitesse du vent réel (m/s).
    pub tws_mps: Option<f64>,
    /// Direction du vent réel (rad).
    pub twd_rad: Option<f64>,
    /// Pression atmosphérique (Pa).
    pub pressure_pa: Option<f64>,
    /// Profondeur sous le capteur (m).
    pub depth_m: Option<f64>,
    /// Données simulées ou rejouées.
    pub simulated: bool,
}

fn fresh(snapshot: &InstrumentsSnapshot, quantity: Quantity) -> Option<f64> {
    snapshot
        .readings
        .iter()
        .find(|r| r.quantity == quantity && !r.stale)
        .map(|r| r.value)
}

impl LogEntry {
    /// Entrée prise sur l'état du bord.
    ///
    /// # Errors
    ///
    /// [`ValidationError::DescriptionTooLong`] si le texte est trop long.
    pub fn from_snapshot(
        kind: LogKind,
        text: Option<&str>,
        snapshot: &InstrumentsSnapshot,
        simulated: bool,
        now: DateTime<Utc>,
    ) -> Result<Self, ValidationError> {
        let position = snapshot.position.as_ref();
        let fresh_fix = position.filter(|p| !p.stale).map(|p| &p.fix);
        Ok(Self {
            id: Uuid::new_v4(),
            at: now,
            kind,
            text: validate_description(text)?,
            position: position.map(|p| p.fix.position),
            position_stale: position.is_some_and(|p| p.stale),
            sog_mps: fresh_fix
                .and_then(|f| f.sog_mps)
                .or_else(|| fresh(snapshot, Quantity::SpeedOverGround)),
            cog_rad: fresh_fix
                .and_then(|f| f.cog_rad)
                .or_else(|| fresh(snapshot, Quantity::CourseOverGroundTrue)),
            heading_rad: fresh(snapshot, Quantity::HeadingTrue),
            tws_mps: fresh(snapshot, Quantity::WindSpeedTrue),
            twd_rad: fresh(snapshot, Quantity::WindDirectionTrue),
            pressure_pa: fresh(snapshot, Quantity::AtmosphericPressure),
            depth_m: fresh(snapshot, Quantity::DepthBelowTransducer),
            simulated,
        })
    }
}

fn csv_field(text: &str) -> String {
    // Une formule de tableur ne doit jamais être interprétée.
    let safe = if text.starts_with(['=', '+', '-', '@']) {
        format!("'{text}")
    } else {
        text.to_owned()
    };
    format!("\"{}\"", safe.replace('"', "\"\""))
}

fn num(value: Option<f64>, scale: f64, decimals: usize) -> String {
    value.map_or_else(String::new, |v| format!("{:.*}", decimals, v * scale))
}

fn deg(value: Option<f64>) -> String {
    value.map_or_else(String::new, |v| {
        format!("{:.0}", v.to_degrees().rem_euclid(360.0))
    })
}

/// Export CSV (séparateur `;`, unités nautiques), du plus ancien au plus
/// récent.
#[must_use]
pub fn to_csv(entries: &[LogEntry]) -> String {
    let mut out = String::from(
        "heure_utc;type;latitude;longitude;position_obsolete;sog_kn;cog_deg;cap_deg;\
         vent_kn;vent_dir_deg;pression_hpa;profondeur_m;simulation;texte\n",
    );
    let kn = 3600.0 / 1852.0;
    for e in entries {
        let _ = writeln!(
            out,
            "{};{};{};{};{};{};{};{};{};{};{};{};{};{}",
            e.at.to_rfc3339_opts(SecondsFormat::Secs, true),
            e.kind.code(),
            e.position
                .map_or_else(String::new, |p| format!("{:.6}", p.latitude_deg())),
            e.position
                .map_or_else(String::new, |p| format!("{:.6}", p.longitude_deg())),
            if e.position_stale { "oui" } else { "non" },
            num(e.sog_mps, kn, 1),
            deg(e.cog_rad),
            deg(e.heading_rad),
            num(e.tws_mps, kn, 1),
            deg(e.twd_rad),
            num(e.pressure_pa, 0.01, 1),
            num(e.depth_m, 1.0, 1),
            if e.simulated { "oui" } else { "non" },
            csv_field(e.text.as_deref().unwrap_or("")),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nav_core::{PositionFix, PositionStatus, Reading, SourceKind};

    fn t() -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t")
    }

    fn reading(quantity: Quantity, value: f64, stale: bool) -> Reading {
        Reading {
            quantity,
            value,
            source: SourceKind::SignalK,
            source_id: "x".to_owned(),
            measured_at: t(),
            age_ms: 0,
            stale,
            fresh_sources: 1,
        }
    }

    fn snapshot(stale: bool) -> InstrumentsSnapshot {
        InstrumentsSnapshot {
            computed_at: t(),
            position: Some(PositionStatus {
                fix: PositionFix {
                    position: GeoPosition::new(-17.5, -149.6).expect("p"),
                    cog_rad: Some(1.0),
                    sog_mps: Some(3.0),
                    source: SourceKind::SignalK,
                    source_id: "gps".to_owned(),
                    measured_at: t(),
                    received_at: t(),
                },
                age_ms: 0,
                stale,
                fresh_sources: 1,
            }),
            readings: vec![
                reading(Quantity::WindSpeedTrue, 7.0, false),
                reading(Quantity::AtmosphericPressure, 101_300.0, false),
                reading(Quantity::DepthBelowTransducer, 12.0, true),
            ],
        }
    }

    #[test]
    fn entry_keeps_only_fresh_values() {
        let e = LogEntry::from_snapshot(
            LogKind::Manual,
            Some(" Ris pris "),
            &snapshot(false),
            false,
            t(),
        )
        .expect("entrée");
        assert_eq!(e.text.as_deref(), Some("Ris pris"));
        assert_eq!(e.sog_mps, Some(3.0));
        assert_eq!(e.tws_mps, Some(7.0));
        assert_eq!(e.depth_m, None, "profondeur obsolète écartée");
        assert!(!e.position_stale);
    }

    #[test]
    fn stale_position_is_kept_but_flagged() {
        let e = LogEntry::from_snapshot(LogKind::Auto, None, &snapshot(true), true, t())
            .expect("entrée");
        assert!(e.position.is_some());
        assert!(e.position_stale);
        assert_eq!(e.sog_mps, None, "vitesse du relevé obsolète écartée");
        assert!(e.simulated);
        let empty = InstrumentsSnapshot {
            computed_at: t(),
            position: None,
            readings: Vec::new(),
        };
        let e = LogEntry::from_snapshot(LogKind::Auto, None, &empty, false, t()).expect("entrée");
        assert_eq!(e.position, None);
        assert!(!e.position_stale);
    }

    #[test]
    fn text_is_validated() {
        let long = "x".repeat(3_000);
        assert!(LogEntry::from_snapshot(
            LogKind::Manual,
            Some(&long),
            &snapshot(false),
            false,
            t()
        )
        .is_err());
    }

    #[test]
    fn csv_escapes_text_and_formulas() {
        let mut e = LogEntry::from_snapshot(
            LogKind::Manual,
            Some("=1+1; \"vent\""),
            &snapshot(false),
            false,
            t(),
        )
        .expect("entrée");
        e.cog_rad = Some(-0.1);
        let csv = to_csv(&[e]);
        let line = csv.lines().nth(1).expect("ligne");
        assert!(
            line.starts_with("2027-01-15T08:00:00Z;manual;-17.500000;-149.600000;non;5.8;354;"),
            "{line}"
        );
        assert!(line.contains(";1013.0;"));
        assert!(line.ends_with("\"'=1+1; \"\"vent\"\"\""), "{line}");
        assert_eq!(LogKind::from_code("manual"), Some(LogKind::Manual));
        assert_eq!(LogKind::from_code("x"), None);
    }
}
