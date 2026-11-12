//! État du bateau : dernières valeurs connues et leur fraîcheur.
//!
//! L'état conserve la dernière position et la dernière mesure de chaque
//! grandeur. Il n'extrapole jamais : une donnée trop ancienne est
//! signalée obsolète dans l'instantané, avec son âge.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::measurement::{age, Measurement, Quantity};
use crate::{PositionFix, SourceKind};

/// Âge au-delà duquel la position est obsolète.
pub const POSITION_STALE_AFTER: Duration = Duration::from_secs(5);

/// Position courante et sa fraîcheur.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionStatus {
    /// Dernier relevé.
    pub fix: PositionFix,
    /// Âge (réception), en millisecondes.
    pub age_ms: u64,
    /// Relevé obsolète.
    pub stale: bool,
}

/// Lecture d'instrument et sa fraîcheur.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    /// Grandeur.
    pub quantity: Quantity,
    /// Valeur SI.
    pub value: f64,
    /// Nature de la source.
    pub source: SourceKind,
    /// Identifiant de la source.
    pub source_id: String,
    /// Horodatage de la mesure (UTC).
    pub measured_at: DateTime<Utc>,
    /// Âge (réception), en millisecondes.
    pub age_ms: u64,
    /// Mesure obsolète.
    pub stale: bool,
}

/// Instantané de l'état du bateau, diffusé aux interfaces.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentsSnapshot {
    /// Horodatage de l'instantané (UTC).
    pub computed_at: DateTime<Utc>,
    /// Position, si une a été reçue.
    pub position: Option<PositionStatus>,
    /// Lectures disponibles, dans l'ordre de [`Quantity::ALL`].
    pub readings: Vec<Reading>,
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Dernières valeurs connues du bateau.
#[derive(Debug, Clone, Default)]
pub struct VesselState {
    position: Option<PositionFix>,
    measurements: BTreeMap<Quantity, Measurement>,
}

impl VesselState {
    /// État vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enregistre une mesure. Une mesure plus ancienne (horodatage de
    /// mesure) que celle déjà connue de la même source est ignorée.
    /// Retourne `true` si la mesure a été retenue.
    pub fn update_measurement(&mut self, measurement: Measurement) -> bool {
        if let Some(current) = self.measurements.get(&measurement.quantity) {
            if current.source_id == measurement.source_id
                && measurement.measured_at < current.measured_at
            {
                return false;
            }
        }
        self.measurements.insert(measurement.quantity, measurement);
        true
    }

    /// Enregistre un relevé de position. S'il ne porte ni vitesse ni route
    /// fond, les dernières mesures **fraîches** de même source les
    /// complètent (Signal K les transmet sur des chemins séparés).
    /// Retourne le relevé complété.
    pub fn update_position(&mut self, mut fix: PositionFix) -> PositionFix {
        let fresh = |q: Quantity| {
            self.measurements
                .get(&q)
                .filter(|m| {
                    m.source == fix.source && age(m.received_at, fix.received_at) <= q.stale_after()
                })
                .map(|m| m.value)
        };
        if fix.sog_mps.is_none() {
            fix.sog_mps = fresh(Quantity::SpeedOverGround);
        }
        if fix.cog_rad.is_none() {
            fix.cog_rad = fresh(Quantity::CourseOverGroundTrue);
        }
        self.position = Some(fix.clone());
        fix
    }

    /// Dernier relevé de position.
    #[must_use]
    pub fn position(&self) -> Option<&PositionFix> {
        self.position.as_ref()
    }

    /// Oublie toutes les valeurs (changement de source).
    pub fn clear(&mut self) {
        self.position = None;
        self.measurements.clear();
    }

    /// Instantané à `now`, avec la fraîcheur de chaque donnée.
    #[must_use]
    pub fn snapshot(&self, now: DateTime<Utc>) -> InstrumentsSnapshot {
        let position = self.position.as_ref().map(|fix| {
            let a = age(fix.received_at, now);
            PositionStatus {
                fix: fix.clone(),
                age_ms: millis(a),
                stale: a > POSITION_STALE_AFTER,
            }
        });
        let readings = Quantity::ALL
            .iter()
            .filter_map(|q| self.measurements.get(q))
            .map(|m| {
                let a = age(m.received_at, now);
                Reading {
                    quantity: m.quantity,
                    value: m.value,
                    source: m.source,
                    source_id: m.source_id.clone(),
                    measured_at: m.measured_at,
                    age_ms: millis(a),
                    stale: a > m.quantity.stale_after(),
                }
            })
            .collect();
        InstrumentsSnapshot {
            computed_at: now,
            position,
            readings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GeoPosition;
    use chrono::Duration as ChronoDuration;

    fn t(s: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000 + s, 0).expect("t")
    }

    fn m(q: Quantity, v: f64, source_id: &str, measured: i64, received: i64) -> Measurement {
        Measurement::new(
            q,
            v,
            SourceKind::SignalK,
            source_id,
            t(measured),
            t(received),
        )
        .expect("m")
    }

    fn fix(received: i64) -> PositionFix {
        PositionFix {
            position: GeoPosition::new(-14.968, -147.638).expect("pos"),
            cog_rad: None,
            sog_mps: None,
            source: SourceKind::SignalK,
            source_id: "gps".to_owned(),
            measured_at: t(received),
            received_at: t(received),
        }
    }

    #[test]
    fn snapshot_flags_stale_data() {
        let mut v = VesselState::new();
        v.update_measurement(m(Quantity::SpeedOverGround, 3.0, "gps", 0, 0));
        v.update_measurement(m(Quantity::DepthBelowTransducer, 12.0, "sonde", 0, 0));
        v.update_position(fix(0));

        let fresh = v.snapshot(t(4));
        assert!(!fresh.position.as_ref().expect("pos").stale);
        assert!(fresh.readings.iter().all(|r| !r.stale));

        let later = v.snapshot(t(6));
        let pos = later.position.expect("pos");
        assert!(pos.stale, "position > 5 s");
        assert_eq!(pos.age_ms, 6_000);
        let sog = &later.readings[0];
        assert_eq!(sog.quantity, Quantity::SpeedOverGround);
        assert!(sog.stale);
        let depth = &later.readings[1];
        assert!(!depth.stale, "profondeur : seuil 10 s");
    }

    #[test]
    fn empty_state_has_no_invented_values() {
        let snap = VesselState::new().snapshot(t(0));
        assert!(snap.position.is_none());
        assert!(snap.readings.is_empty());
    }

    #[test]
    fn position_is_completed_with_fresh_speed_and_course() {
        let mut v = VesselState::new();
        v.update_measurement(m(Quantity::SpeedOverGround, 3.0, "gps", 10, 10));
        v.update_measurement(m(Quantity::CourseOverGroundTrue, 1.0, "gps", 10, 10));
        let completed = v.update_position(fix(11));
        assert_eq!(completed.sog_mps, Some(3.0));
        assert_eq!(completed.cog_rad, Some(1.0));
        // Mesures trop anciennes : non utilisées.
        let old = v.update_position(fix(30));
        assert_eq!(old.sog_mps, None);
        assert_eq!(old.cog_rad, None);
    }

    #[test]
    fn out_of_order_measurements_from_same_source_are_ignored() {
        let mut v = VesselState::new();
        assert!(v.update_measurement(m(Quantity::HeadingTrue, 1.0, "compas", 10, 10)));
        assert!(!v.update_measurement(m(Quantity::HeadingTrue, 2.0, "compas", 5, 11)));
        assert!(v.update_measurement(m(Quantity::HeadingTrue, 2.0, "autre", 5, 11)));
        assert_eq!(v.snapshot(t(11)).readings[0].value, 2.0);
    }

    #[test]
    fn clear_forgets_everything() {
        let mut v = VesselState::new();
        v.update_position(fix(0));
        v.update_measurement(m(Quantity::HeadingTrue, 1.0, "c", 0, 0));
        v.clear();
        assert!(v.position().is_none());
        assert!(v
            .snapshot(t(0) + ChronoDuration::seconds(1))
            .readings
            .is_empty());
    }
}
