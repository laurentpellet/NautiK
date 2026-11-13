//! Suivi des cibles AIS : fusion des messages par MMSI, fraîcheur,
//! cibles perdues, historique des positions et vecteurs de
//! déplacement.
//!
//! La fraîcheur d'une cible dépend de l'intervalle de compte rendu
//! nominal de son équipement (ITU-R M.1371, tableau 1), estimé d'après
//! sa classe, sa vitesse et son statut : une cible est **obsolète**
//! au-delà de 3 intervalles sans position, **perdue** au-delà de 6
//! (ou si elle n'a jamais donné de position valide), puis **retirée**
//! après [`TrackerConfig::removal_after`] sans aucun message.

use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Duration, Utc};
use nav_core::GeoPosition;
use nav_geodesy::units::MPS_PER_KNOT;
use serde::Serialize;

use crate::message::{AisClass, AisReport, Mmsi};

/// Réglages du suivi.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackerConfig {
    /// Nombre maximal de cibles suivies ; au-delà, la cible reçue le
    /// moins récemment est oubliée (protection contre les flux
    /// excessifs).
    pub max_targets: usize,
    /// Nombre maximal de positions conservées par cible.
    pub history_len: usize,
    /// Intervalle minimal entre deux positions de l'historique.
    pub history_interval: Duration,
    /// Durée de l'historique.
    pub history_span: Duration,
    /// Délai sans aucun message au-delà duquel la cible est retirée.
    pub removal_after: Duration,
    /// Horizon des vecteurs de déplacement.
    pub vector_horizon: Duration,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            max_targets: 2_000,
            history_len: 40,
            history_interval: Duration::seconds(30),
            history_span: Duration::minutes(20),
            removal_after: Duration::minutes(30),
            vector_horizon: Duration::minutes(6),
        }
    }
}

/// État d'une cible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetState {
    /// Position récente.
    Active,
    /// Position obsolète (comptes rendus manqués).
    Stale,
    /// Cible perdue, ou sans position valide.
    Lost,
}

/// Vue d'une cible pour les interfaces.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AisTargetView {
    /// Identité MMSI (9 chiffres).
    pub mmsi: Mmsi,
    /// Classe de l'équipement.
    pub class: AisClass,
    /// Nom du navire.
    pub name: Option<String>,
    /// Indicatif d'appel.
    pub callsign: Option<String>,
    /// Type de navire (ITU-R M.1371).
    pub ship_type: Option<u8>,
    /// Statut de navigation (0 à 14).
    pub navigation_status: Option<u8>,
    /// Dernière position valide.
    pub position: Option<GeoPosition>,
    /// Vitesse fond (m/s).
    pub sog_mps: Option<f64>,
    /// Route fond (rad).
    pub cog_rad: Option<f64>,
    /// Cap vrai (rad).
    pub heading_rad: Option<f64>,
    /// Extrémité du vecteur de déplacement (position estimée à
    /// l'horizon configuré), cible active seulement.
    pub vector_end: Option<GeoPosition>,
    /// Dernier message reçu (tous types).
    pub last_seen_at: DateTime<Utc>,
    /// Âge de la position (ms), si elle existe.
    pub position_age_ms: Option<u64>,
    /// État de la cible.
    pub state: TargetState,
    /// Positions passées, de la plus ancienne à la plus récente.
    pub history: Vec<GeoPosition>,
}

#[derive(Debug, Clone)]
struct Target {
    mmsi: Mmsi,
    class: AisClass,
    name: Option<String>,
    callsign: Option<String>,
    ship_type: Option<u8>,
    navigation_status: Option<u8>,
    position: Option<GeoPosition>,
    position_at: Option<DateTime<Utc>>,
    sog_mps: Option<f64>,
    cog_rad: Option<f64>,
    heading_rad: Option<f64>,
    last_seen_at: DateTime<Utc>,
    history: VecDeque<(GeoPosition, DateTime<Utc>)>,
}

/// Intervalle nominal entre deux comptes rendus de position.
fn reporting_interval(class: AisClass, sog_mps: Option<f64>, status: Option<u8>) -> Duration {
    let knots = sog_mps.map(|v| v / MPS_PER_KNOT);
    match class {
        AisClass::A => match knots {
            // Au mouillage (1) ou amarré (5), à moins de 3 nœuds.
            Some(k) if k <= 3.0 && matches!(status, Some(1 | 5)) => Duration::minutes(3),
            Some(k) if k > 23.0 => Duration::seconds(2),
            Some(k) if k > 14.0 => Duration::seconds(6),
            _ => Duration::seconds(10),
        },
        AisClass::B => match knots {
            Some(k) if k <= 2.0 => Duration::minutes(3),
            _ => Duration::seconds(30),
        },
    }
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.num_milliseconds()).unwrap_or(0)
}

impl Target {
    fn new(report: &AisReport, at: DateTime<Utc>) -> Self {
        Self {
            mmsi: report.mmsi,
            class: report.class,
            name: None,
            callsign: None,
            ship_type: None,
            navigation_status: None,
            position: None,
            position_at: None,
            sog_mps: None,
            cog_rad: None,
            heading_rad: None,
            last_seen_at: at,
            history: VecDeque::new(),
        }
    }

    fn apply(&mut self, report: &AisReport, at: DateTime<Utc>, config: &TrackerConfig) {
        self.class = report.class;
        self.last_seen_at = self.last_seen_at.max(at);
        if let Some(identity) = &report.identity {
            // Les parties A et B du message 24 sont complémentaires :
            // une valeur absente ne remplace pas une valeur connue.
            if identity.name.is_some() {
                self.name.clone_from(&identity.name);
            }
            if identity.callsign.is_some() {
                self.callsign.clone_from(&identity.callsign);
            }
            if identity.ship_type.is_some() {
                self.ship_type = identity.ship_type;
            }
        }
        if let Some(d) = report.dynamic {
            if self.position_at.is_some_and(|p| at < p) {
                // Compte rendu plus ancien que la position connue.
                return;
            }
            // Les grandeurs dynamiques sont remplacées, même par
            // « non disponible » : une valeur périmée n'est pas conservée.
            self.sog_mps = d.sog_mps;
            self.cog_rad = d.cog_rad;
            self.heading_rad = d.heading_rad;
            if report.class == AisClass::A {
                self.navigation_status = d.navigation_status;
            }
            if let Some(position) = d.position {
                self.position = Some(position);
                self.position_at = Some(at);
                self.record_history(position, at, config);
            }
        }
    }

    fn record_history(&mut self, position: GeoPosition, at: DateTime<Utc>, config: &TrackerConfig) {
        let due = self
            .history
            .back()
            .map_or(true, |(_, last)| at - *last >= config.history_interval);
        if due {
            self.history.push_back((position, at));
        }
        while self.history.len() > config.history_len {
            self.history.pop_front();
        }
    }

    fn view(&self, now: DateTime<Utc>, config: &TrackerConfig) -> AisTargetView {
        let interval = reporting_interval(self.class, self.sog_mps, self.navigation_status);
        let position_age = self.position_at.map(|at| (now - at).max(Duration::zero()));
        let state = match position_age {
            None => TargetState::Lost,
            Some(age) if age > interval * 6 => TargetState::Lost,
            Some(age) if age > interval * 3 => TargetState::Stale,
            Some(_) => TargetState::Active,
        };
        let vector_end = match (state, self.position, self.sog_mps, self.cog_rad) {
            (TargetState::Active, Some(p), Some(sog), Some(cog)) if sog > 0.0 => {
                let horizon_s = config.vector_horizon.num_milliseconds() as f64 / 1_000.0;
                nav_geodesy::destination(p, cog, sog * horizon_s)
            }
            _ => None,
        };
        let oldest = now - config.history_span;
        AisTargetView {
            mmsi: self.mmsi,
            class: self.class,
            name: self.name.clone(),
            callsign: self.callsign.clone(),
            ship_type: self.ship_type,
            navigation_status: self.navigation_status,
            position: self.position,
            sog_mps: self.sog_mps,
            cog_rad: self.cog_rad,
            heading_rad: self.heading_rad,
            vector_end,
            last_seen_at: self.last_seen_at,
            position_age_ms: position_age.map(millis),
            state,
            history: self
                .history
                .iter()
                .filter(|(_, at)| *at >= oldest)
                .map(|(p, _)| *p)
                .collect(),
        }
    }
}

/// Cibles AIS suivies.
#[derive(Debug, Clone, Default)]
pub struct AisTracker {
    config: TrackerConfig,
    targets: HashMap<Mmsi, Target>,
}

impl AisTracker {
    /// Suivi avec les réglages par défaut.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(TrackerConfig::default())
    }

    /// Suivi avec des réglages explicites.
    #[must_use]
    pub fn with_config(config: TrackerConfig) -> Self {
        Self {
            config,
            targets: HashMap::new(),
        }
    }

    /// Nombre de cibles suivies.
    #[must_use]
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Aucune cible ?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Oublie toutes les cibles.
    pub fn clear(&mut self) {
        self.targets.clear();
    }

    /// Intègre un message reçu à `at`. Les messages du navire propre
    /// (VDO) sont ignorés. Retourne `true` si une cible a été mise à
    /// jour.
    pub fn update(&mut self, report: &AisReport, at: DateTime<Utc>) -> bool {
        if report.own_vessel {
            return false;
        }
        if !self.targets.contains_key(&report.mmsi) && self.targets.len() >= self.config.max_targets
        {
            let oldest = self
                .targets
                .values()
                .min_by_key(|t| t.last_seen_at)
                .map(|t| t.mmsi);
            if let Some(mmsi) = oldest {
                self.targets.remove(&mmsi);
            }
        }
        let config = self.config;
        self.targets
            .entry(report.mmsi)
            .or_insert_with(|| Target::new(report, at))
            .apply(report, at, &config);
        true
    }

    /// Retire les cibles muettes depuis plus de
    /// [`TrackerConfig::removal_after`] et retourne les autres, triées
    /// par MMSI.
    pub fn snapshot(&mut self, now: DateTime<Utc>) -> Vec<AisTargetView> {
        let removal = self.config.removal_after;
        self.targets.retain(|_, t| now - t.last_seen_at <= removal);
        let mut views: Vec<AisTargetView> = self
            .targets
            .values()
            .map(|t| t.view(now, &self.config))
            .collect();
        views.sort_by_key(|v| v.mmsi);
        views
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Dynamic, Identity};

    fn t(seconds: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t") + Duration::seconds(seconds)
    }

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    fn mmsi(v: u32) -> Mmsi {
        Mmsi::new(v).expect("mmsi")
    }

    fn position(m: u32, class: AisClass, pos: Option<GeoPosition>, knots: f64) -> AisReport {
        AisReport {
            message_type: if class == AisClass::A { 1 } else { 18 },
            mmsi: mmsi(m),
            class,
            own_vessel: false,
            dynamic: Some(Dynamic {
                navigation_status: Some(0),
                position: pos,
                sog_mps: Some(knots * MPS_PER_KNOT),
                cog_rad: Some(0.0),
                heading_rad: None,
            }),
            identity: None,
        }
    }

    fn identity(m: u32, name: Option<&str>, callsign: Option<&str>) -> AisReport {
        AisReport {
            message_type: 24,
            mmsi: mmsi(m),
            class: AisClass::B,
            own_vessel: false,
            dynamic: None,
            identity: Some(Identity {
                name: name.map(str::to_owned),
                callsign: callsign.map(str::to_owned),
                ship_type: None,
            }),
        }
    }

    #[test]
    fn merges_dynamic_and_static_data() {
        let mut tr = AisTracker::new();
        assert!(tr.update(&identity(1, Some("MOANA"), None), t(0)));
        tr.update(&identity(1, None, Some("FQ1")), t(1));
        tr.update(&position(1, AisClass::B, Some(p(-15.0, -147.0)), 6.0), t(2));
        let v = &tr.snapshot(t(3))[0];
        assert_eq!(v.name.as_deref(), Some("MOANA"));
        assert_eq!(v.callsign.as_deref(), Some("FQ1"));
        assert_eq!(v.position, Some(p(-15.0, -147.0)));
        assert_eq!(v.position_age_ms, Some(1_000));
        assert_eq!(v.state, TargetState::Active);
        assert!(v.vector_end.is_some());
        assert_eq!(v.navigation_status, None, "statut ignoré en classe B");
    }

    #[test]
    fn target_without_position_is_lost() {
        let mut tr = AisTracker::new();
        tr.update(&identity(1, Some("X"), None), t(0));
        tr.update(&position(1, AisClass::A, None, 5.0), t(1));
        let v = &tr.snapshot(t(1))[0];
        assert_eq!(v.position, None);
        assert_eq!(v.state, TargetState::Lost);
        assert_eq!(v.vector_end, None);
    }

    #[test]
    fn unavailable_position_keeps_last_known_one_ageing() {
        let mut tr = AisTracker::new();
        tr.update(&position(1, AisClass::A, Some(p(-15.0, -147.0)), 5.0), t(0));
        tr.update(&position(1, AisClass::A, None, 5.0), t(20));
        let v = &tr.snapshot(t(40))[0];
        assert_eq!(v.position, Some(p(-15.0, -147.0)));
        assert_eq!(v.position_age_ms, Some(40_000));
        assert_eq!(v.state, TargetState::Stale);
        assert_eq!(v.vector_end, None, "aucun vecteur sur une cible obsolète");
    }

    #[test]
    fn staleness_follows_reporting_interval() {
        let mut tr = AisTracker::new();
        // Classe A à 10 nœuds : 10 s ; classe B à 6 nœuds : 30 s.
        tr.update(&position(1, AisClass::A, Some(p(0.0, 0.0)), 10.0), t(0));
        tr.update(&position(2, AisClass::B, Some(p(0.0, 0.0)), 6.0), t(0));
        let states = |tr: &mut AisTracker, s| {
            tr.snapshot(t(s))
                .iter()
                .map(|v| v.state)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            states(&mut tr, 30),
            [TargetState::Active, TargetState::Active]
        );
        assert_eq!(
            states(&mut tr, 31),
            [TargetState::Stale, TargetState::Active]
        );
        assert_eq!(
            states(&mut tr, 61),
            [TargetState::Lost, TargetState::Active]
        );
        assert_eq!(states(&mut tr, 91), [TargetState::Lost, TargetState::Stale]);
        assert_eq!(states(&mut tr, 181), [TargetState::Lost, TargetState::Lost]);
    }

    #[test]
    fn anchored_targets_report_slowly() {
        let mut r = position(1, AisClass::A, Some(p(0.0, 0.0)), 0.0);
        if let Some(d) = r.dynamic.as_mut() {
            d.navigation_status = Some(1);
        }
        let mut tr = AisTracker::new();
        tr.update(&r, t(0));
        assert_eq!(
            tr.snapshot(t(9 * 60)).first().map(|v| v.state),
            Some(TargetState::Active)
        );
        assert_eq!(
            tr.snapshot(t(9 * 60 + 1)).first().map(|v| v.state),
            Some(TargetState::Stale)
        );
        assert_eq!(
            tr.snapshot(t(9 * 60)).first().and_then(|v| v.vector_end),
            None
        );
    }

    #[test]
    fn silent_targets_are_removed() {
        let mut tr = AisTracker::new();
        tr.update(&position(1, AisClass::A, Some(p(0.0, 0.0)), 1.0), t(0));
        assert_eq!(tr.snapshot(t(30 * 60)).len(), 1);
        assert!(tr.snapshot(t(30 * 60 + 1)).is_empty());
        assert!(tr.is_empty());
    }

    #[test]
    fn own_vessel_reports_are_ignored() {
        let mut tr = AisTracker::new();
        let mut r = position(1, AisClass::A, Some(p(0.0, 0.0)), 1.0);
        r.own_vessel = true;
        assert!(!tr.update(&r, t(0)));
        assert!(tr.is_empty());
    }

    #[test]
    fn older_report_does_not_replace_position() {
        let mut tr = AisTracker::new();
        tr.update(&position(1, AisClass::A, Some(p(1.0, 1.0)), 1.0), t(10));
        tr.update(&position(1, AisClass::A, Some(p(2.0, 2.0)), 1.0), t(5));
        assert_eq!(tr.snapshot(t(10))[0].position, Some(p(1.0, 1.0)));
    }

    #[test]
    fn history_is_spaced_and_bounded() {
        let config = TrackerConfig {
            history_len: 3,
            ..TrackerConfig::default()
        };
        let mut tr = AisTracker::with_config(config);
        for i in 0..10 {
            let lat = f64::from(i) * 0.001;
            tr.update(
                &position(1, AisClass::A, Some(p(lat, 0.0)), 5.0),
                t(i64::from(i) * 15),
            );
        }
        // Positions retenues toutes les 30 s : 0, 30, 60, 90, 120 s ;
        // les 3 dernières sont conservées.
        let v = &tr.snapshot(t(135))[0];
        assert_eq!(v.history, [p(0.004, 0.0), p(0.006, 0.0), p(0.008, 0.0)]);
        // Au-delà de la durée de l'historique, rien n'est présenté.
        assert!(tr.snapshot(t(135 + 21 * 60))[0].history.is_empty());
    }

    #[test]
    fn target_count_is_bounded() {
        let config = TrackerConfig {
            max_targets: 3,
            ..TrackerConfig::default()
        };
        let mut tr = AisTracker::with_config(config);
        for i in 1..=4 {
            tr.update(
                &position(i, AisClass::A, Some(p(0.0, 0.0)), 1.0),
                t(i64::from(i)),
            );
        }
        let mmsis: Vec<u32> = tr.snapshot(t(5)).iter().map(|v| v.mmsi.value()).collect();
        assert_eq!(mmsis, [2, 3, 4]);
    }

    #[test]
    fn thousand_targets() {
        let mut tr = AisTracker::new();
        for i in 1..=1_000 {
            let lat = f64::from(i) * 0.001;
            tr.update(&position(i, AisClass::A, Some(p(lat, 0.0)), 8.0), t(0));
        }
        let views = tr.snapshot(t(1));
        assert_eq!(views.len(), 1_000);
        assert!(views.iter().all(|v| v.state == TargetState::Active));
    }

    #[test]
    fn view_serializes_as_camel_case() {
        let mut tr = AisTracker::new();
        tr.update(
            &position(227_000_001, AisClass::B, Some(p(-15.0, -147.0)), 6.0),
            t(0),
        );
        let json = serde_json::to_value(&tr.snapshot(t(1))[0]).expect("json");
        assert_eq!(json["mmsi"], "227000001");
        assert_eq!(json["class"], "B");
        assert_eq!(json["state"], "active");
        assert_eq!(json["positionAgeMs"], 1000);
        assert_eq!(json["position"]["latitudeDeg"], -15.0);
        assert!(json["vectorEnd"].is_object());
    }
}
