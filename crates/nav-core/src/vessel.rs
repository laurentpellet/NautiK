//! État du bateau : dernières valeurs connues, arbitrage des sources et
//! fraîcheur.
//!
//! Chaque source (identifiant `source_id`) est suivie séparément. Pour
//! la position et pour chaque grandeur, une source est **retenue** :
//! 1. une source fraîche l'emporte sur une source obsolète ;
//! 2. puis la mieux classée dans les priorités configurées ;
//! 3. puis la plus récemment reçue.
//!
//! La source retenue est conservée tant qu'elle reste fraîche et
//! qu'aucune source fraîche mieux classée n'apparaît (pas d'alternance
//! entre deux récepteurs équivalents). L'état n'extrapole jamais : une
//! donnée trop ancienne est signalée obsolète dans l'instantané, avec
//! son âge.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::measurement::{age, Measurement, Quantity};
use crate::{PositionFix, SourceKind};

/// Âge au-delà duquel la position est obsolète.
pub const POSITION_STALE_AFTER: Duration = Duration::from_secs(5);

/// Nombre maximal de sources suivies pour une même donnée (protection
/// contre les flux excessifs) ; au-delà, la source la plus ancienne est
/// oubliée.
pub const MAX_SOURCES: usize = 16;

/// Position courante et sa fraîcheur.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionStatus {
    /// Relevé de la source retenue.
    pub fix: PositionFix,
    /// Âge (réception), en millisecondes.
    pub age_ms: u64,
    /// Relevé obsolète.
    pub stale: bool,
    /// Nombre de sources de position fraîches.
    pub fresh_sources: usize,
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
    /// Identifiant de la source retenue.
    pub source_id: String,
    /// Horodatage de la mesure (UTC).
    pub measured_at: DateTime<Utc>,
    /// Âge (réception), en millisecondes.
    pub age_ms: u64,
    /// Mesure obsolète.
    pub stale: bool,
    /// Nombre de sources fraîches pour cette grandeur.
    pub fresh_sources: usize,
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

/// Donnée attribuée à une source.
trait Sourced {
    fn source_id(&self) -> &str;
    fn received_at(&self) -> DateTime<Utc>;
    fn measured_at(&self) -> DateTime<Utc>;
}

impl Sourced for PositionFix {
    fn source_id(&self) -> &str {
        &self.source_id
    }
    fn received_at(&self) -> DateTime<Utc> {
        self.received_at
    }
    fn measured_at(&self) -> DateTime<Utc> {
        self.measured_at
    }
}

impl Sourced for Measurement {
    fn source_id(&self) -> &str {
        &self.source_id
    }
    fn received_at(&self) -> DateTime<Utc> {
        self.received_at
    }
    fn measured_at(&self) -> DateTime<Utc> {
        self.measured_at
    }
}

/// Valeurs concurrentes d'une même donnée, par source.
#[derive(Debug, Clone)]
struct Candidates<T> {
    by_source: BTreeMap<String, T>,
    selected: Option<String>,
}

impl<T> Default for Candidates<T> {
    fn default() -> Self {
        Self {
            by_source: BTreeMap::new(),
            selected: None,
        }
    }
}

impl<T: Sourced> Candidates<T> {
    /// Ajoute une valeur. Une valeur plus ancienne (horodatage de mesure)
    /// que celle déjà connue de la même source est ignorée.
    fn insert(&mut self, value: T) -> bool {
        if let Some(current) = self.by_source.get(value.source_id()) {
            if value.measured_at() < current.measured_at() {
                return false;
            }
        } else if self.by_source.len() >= MAX_SOURCES {
            let oldest = self
                .by_source
                .values()
                .min_by_key(|v| v.received_at())
                .map(|v| v.source_id().to_owned());
            if let Some(id) = oldest {
                self.by_source.remove(&id);
                if self.selected.as_deref() == Some(id.as_str()) {
                    self.selected = None;
                }
            }
        }
        self.by_source.insert(value.source_id().to_owned(), value);
        true
    }

    fn fresh_count(&self, now: DateTime<Utc>, stale_after: Duration) -> usize {
        self.by_source
            .values()
            .filter(|v| age(v.received_at(), now) <= stale_after)
            .count()
    }

    /// Réévalue la source retenue à `now`.
    fn reselect(&mut self, now: DateTime<Utc>, stale_after: Duration, priorities: &Priorities) {
        let fresh = |v: &T| age(v.received_at(), now) <= stale_after;
        if let Some(current) = self.selected().filter(|v| fresh(v)) {
            let rank = priorities.rank(current.source_id());
            let better = self
                .by_source
                .values()
                .any(|v| fresh(v) && priorities.rank(v.source_id()) < rank);
            if !better {
                return;
            }
        }
        self.selected = self
            .by_source
            .values()
            .max_by(|a, b| {
                let key = |v: &T| {
                    (
                        fresh(v),
                        std::cmp::Reverse(priorities.rank(v.source_id())),
                        v.received_at(),
                    )
                };
                key(a).cmp(&key(b))
            })
            .map(|v| v.source_id().to_owned());
    }

    fn selected(&self) -> Option<&T> {
        self.selected
            .as_deref()
            .and_then(|id| self.by_source.get(id))
    }

    fn clear(&mut self) {
        self.by_source.clear();
        self.selected = None;
    }
}

/// Priorités des sources, de la plus prioritaire à la moins
/// prioritaire. Une entrée désigne un identifiant exact ou un préfixe
/// suivi d'un point (`"nmea0183"` couvre `"nmea0183.GP"`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Priorities(Vec<String>);

impl Priorities {
    /// Crée une liste de priorités.
    #[must_use]
    pub fn new(entries: Vec<String>) -> Self {
        Self(entries)
    }

    /// Rang d'une source (0 = la plus prioritaire ; les sources non
    /// listées viennent après toutes les autres).
    #[must_use]
    pub fn rank(&self, source_id: &str) -> usize {
        self.0
            .iter()
            .position(|p| {
                source_id == p
                    || source_id
                        .strip_prefix(p.as_str())
                        .is_some_and(|rest| rest.starts_with('.'))
            })
            .unwrap_or(usize::MAX)
    }
}

/// Dernières valeurs connues du bateau, par source.
#[derive(Debug, Clone, Default)]
pub struct VesselState {
    priorities: Priorities,
    positions: Candidates<PositionFix>,
    measurements: BTreeMap<Quantity, Candidates<Measurement>>,
}

impl VesselState {
    /// État vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Définit les priorités des sources.
    pub fn set_priorities(&mut self, priorities: Priorities) {
        self.priorities = priorities;
    }

    /// Enregistre une mesure. Une mesure plus ancienne (horodatage de
    /// mesure) que celle déjà connue de la même source est ignorée.
    /// Retourne `true` si la mesure a été retenue.
    pub fn update_measurement(&mut self, measurement: Measurement) -> bool {
        let quantity = measurement.quantity;
        let now = measurement.received_at;
        let candidates = self.measurements.entry(quantity).or_default();
        if !candidates.insert(measurement) {
            return false;
        }
        candidates.reselect(now, quantity.stale_after(), &self.priorities);
        true
    }

    /// Enregistre un relevé de position. S'il ne porte ni vitesse ni route
    /// fond, les mesures **fraîches** retenues de même nature de source
    /// les complètent (Signal K et VTG les transmettent séparément).
    ///
    /// Retourne le relevé complété si sa source est la source de position
    /// retenue, `None` sinon (relevé d'une source secondaire, ou plus
    /// ancien que le dernier relevé de cette source).
    pub fn update_position(&mut self, mut fix: PositionFix) -> Option<PositionFix> {
        let fresh = |q: Quantity| {
            self.measurements
                .get(&q)
                .and_then(Candidates::selected)
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
        let now = fix.received_at;
        if !self.positions.insert(fix.clone()) {
            return None;
        }
        self.positions
            .reselect(now, POSITION_STALE_AFTER, &self.priorities);
        (self.positions.selected.as_deref() == Some(fix.source_id.as_str())).then_some(fix)
    }

    /// Relevé de la source de position retenue.
    #[must_use]
    pub fn position(&self) -> Option<&PositionFix> {
        self.positions.selected()
    }

    /// Oublie toutes les valeurs (changement de source).
    pub fn clear(&mut self) {
        self.positions.clear();
        self.measurements.clear();
    }

    /// Instantané à `now`, avec la fraîcheur de chaque donnée. Les
    /// sources retenues sont réévaluées (une source devenue obsolète
    /// cède la place à une source fraîche).
    pub fn snapshot(&mut self, now: DateTime<Utc>) -> InstrumentsSnapshot {
        self.positions
            .reselect(now, POSITION_STALE_AFTER, &self.priorities);
        for (quantity, candidates) in &mut self.measurements {
            candidates.reselect(now, quantity.stale_after(), &self.priorities);
        }
        let position = self.positions.selected().map(|fix| {
            let a = age(fix.received_at, now);
            PositionStatus {
                fix: fix.clone(),
                age_ms: millis(a),
                stale: a > POSITION_STALE_AFTER,
                fresh_sources: self.positions.fresh_count(now, POSITION_STALE_AFTER),
            }
        });
        let readings = Quantity::ALL
            .iter()
            .filter_map(|q| self.measurements.get(q))
            .filter_map(|c| c.selected().map(|m| (c, m)))
            .map(|(c, m)| {
                let a = age(m.received_at, now);
                let stale_after = m.quantity.stale_after();
                Reading {
                    quantity: m.quantity,
                    value: m.value,
                    source: m.source,
                    source_id: m.source_id.clone(),
                    measured_at: m.measured_at,
                    age_ms: millis(a),
                    stale: a > stale_after,
                    fresh_sources: c.fresh_count(now, stale_after),
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
        let completed = v.update_position(fix(11)).expect("retenu");
        assert_eq!(completed.sog_mps, Some(3.0));
        assert_eq!(completed.cog_rad, Some(1.0));
        // Mesures trop anciennes : non utilisées.
        let old = v.update_position(fix(30)).expect("retenu");
        assert_eq!(old.sog_mps, None);
        assert_eq!(old.cog_rad, None);
    }

    #[test]
    fn out_of_order_data_from_same_source_is_ignored() {
        let mut v = VesselState::new();
        assert!(v.update_measurement(m(Quantity::HeadingTrue, 1.0, "compas", 10, 10)));
        assert!(!v.update_measurement(m(Quantity::HeadingTrue, 2.0, "compas", 5, 11)));
        assert!(v.update_measurement(m(Quantity::HeadingTrue, 2.0, "autre", 5, 11)));
        assert!(v.update_position(fix(10)).is_some());
        let mut older = fix(12);
        older.measured_at = t(5);
        assert!(v.update_position(older).is_none(), "relevé antérieur");
    }

    fn fix_from(source_id: &str, received: i64, lat: f64) -> PositionFix {
        PositionFix {
            position: GeoPosition::new(lat, -147.638).expect("pos"),
            source_id: source_id.to_owned(),
            ..fix(received)
        }
    }

    #[test]
    fn selected_source_is_kept_while_fresh() {
        let mut v = VesselState::new();
        assert!(v.update_position(fix_from("nmea.GP", 0, -14.0)).is_some());
        // Deuxième récepteur équivalent : pas d'alternance.
        assert!(v.update_position(fix_from("nmea.GN", 1, -14.1)).is_none());
        assert!(v.update_position(fix_from("nmea.GP", 2, -14.0)).is_some());
        let snap = v.snapshot(t(2));
        let pos = snap.position.expect("pos");
        assert_eq!(pos.fix.source_id, "nmea.GP");
        assert_eq!(pos.fresh_sources, 2);
    }

    #[test]
    fn stale_source_gives_way_to_fresh_one() {
        let mut v = VesselState::new();
        v.update_position(fix_from("gps1", 0, -14.0));
        for s in 1..=8 {
            assert!(v.update_position(fix_from("gps2", s, -14.1)).is_none() || s > 5);
        }
        // gps1 est obsolète depuis t5 : gps2 est retenu.
        assert!(v.update_position(fix_from("gps2", 9, -14.1)).is_some());
        let pos = v.snapshot(t(9)).position.expect("pos");
        assert_eq!(pos.fix.source_id, "gps2");
        assert!(!pos.stale);
        assert_eq!(pos.fresh_sources, 1);
    }

    #[test]
    fn snapshot_switches_to_fresh_measurement_source() {
        let mut v = VesselState::new();
        v.update_measurement(m(Quantity::HeadingTrue, 1.0, "compas1", 0, 0));
        v.update_measurement(m(Quantity::HeadingTrue, 2.0, "compas2", 4, 4));
        assert_eq!(v.snapshot(t(4)).readings[0].source_id, "compas1");
        // compas1 obsolète (> 5 s) : compas2, encore frais, est retenu.
        let r = &v.snapshot(t(7)).readings[0];
        assert_eq!(r.source_id, "compas2");
        assert!(!r.stale);
    }

    #[test]
    fn all_stale_keeps_most_recent_and_flags_it() {
        let mut v = VesselState::new();
        v.update_measurement(m(Quantity::HeadingTrue, 1.0, "a", 0, 0));
        v.update_measurement(m(Quantity::HeadingTrue, 2.0, "b", 3, 3));
        let r = &v.snapshot(t(60)).readings[0];
        assert!(r.stale);
        assert_eq!(r.fresh_sources, 0);
        assert_eq!(r.source_id, "b", "la plus récente");
        assert_eq!(r.age_ms, 57_000);
    }

    #[test]
    fn priorities_override_recency() {
        let mut v = VesselState::new();
        v.set_priorities(Priorities::new(vec![
            "signalk.gps".to_owned(),
            "nmea0183".to_owned(),
        ]));
        assert!(v
            .update_position(fix_from("nmea0183.GP", 0, -14.0))
            .is_some());
        // Source prioritaire fraîche : elle prend la main immédiatement.
        assert!(v
            .update_position(fix_from("signalk.gps", 1, -14.1))
            .is_some());
        assert!(v
            .update_position(fix_from("nmea0183.GP", 2, -14.0))
            .is_none());
        assert_eq!(v.position().expect("pos").source_id, "signalk.gps");
    }

    #[test]
    fn priority_ranks_use_prefixes() {
        let p = Priorities::new(vec!["nmea0183".to_owned(), "a.b".to_owned()]);
        assert_eq!(p.rank("nmea0183"), 0);
        assert_eq!(p.rank("nmea0183.GP"), 0);
        assert_eq!(p.rank("nmea0183x.GP"), usize::MAX);
        assert_eq!(p.rank("a.b.c"), 1);
        assert_eq!(p.rank("inconnu"), usize::MAX);
    }

    #[test]
    fn number_of_sources_is_bounded() {
        let mut v = VesselState::new();
        for i in 0..(MAX_SOURCES as i64 + 10) {
            v.update_measurement(m(Quantity::HeadingTrue, 1.0, &format!("s{i}"), i, i));
        }
        let c = v.measurements.get(&Quantity::HeadingTrue).expect("c");
        assert_eq!(c.by_source.len(), MAX_SOURCES);
        assert!(
            !c.by_source.contains_key("s0"),
            "la plus ancienne est oubliée"
        );
        assert!(c.selected().is_some());
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
