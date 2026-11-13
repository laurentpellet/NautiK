//! Simulateur de bateau suivant une route.
//!
//! Sert au développement et aux démonstrations. Chaque relevé produit
//! est marqué [`SourceKind::Simulator`] ; il ne doit jamais être présenté
//! comme une position réelle. Sans route, le bateau reste immobile.

use chrono::{DateTime, Utc};
use nav_core::simulation::SIMULATOR_SOURCE_ID;
use nav_core::{GeoPosition, PositionFix, SourceKind};

use crate::model::Route;

/// Pas de temps maximal pris en compte (s) : évite un saut après une
/// suspension de l'application.
const MAX_STEP_S: f64 = 10.0;

/// Bateau simulé.
#[derive(Debug, Clone)]
pub struct RouteSimulator {
    position: GeoPosition,
    speed_mps: f64,
    targets: Vec<GeoPosition>,
    target_index: usize,
    course_rad: Option<f64>,
    last_step: Option<DateTime<Utc>>,
}

impl RouteSimulator {
    /// Crée un bateau immobile à `start`, naviguant à `speed_mps` lorsqu'il
    /// suit une route (vitesse négative ou non finie ramenée à 0).
    #[must_use]
    pub fn new(start: GeoPosition, speed_mps: f64) -> Self {
        Self {
            position: start,
            speed_mps: if speed_mps.is_finite() {
                speed_mps.max(0.0)
            } else {
                0.0
            },
            targets: Vec::new(),
            target_index: 0,
            course_rad: None,
            last_step: None,
        }
    }

    /// Vitesse lorsqu'il suit une route (m/s).
    #[must_use]
    pub fn speed_mps(&self) -> f64 {
        self.speed_mps
    }

    /// Change la vitesse (négative ou non finie ramenée à 0) ; prise en
    /// compte au pas suivant.
    pub fn set_speed(&mut self, speed_mps: f64) {
        self.speed_mps = if speed_mps.is_finite() {
            speed_mps.max(0.0)
        } else {
            0.0
        };
    }

    /// Position courante.
    #[must_use]
    pub fn position(&self) -> GeoPosition {
        self.position
    }

    /// Indique si le bateau fait route.
    #[must_use]
    pub fn is_underway(&self) -> bool {
        self.target_index < self.targets.len() && self.speed_mps > 0.0
    }

    /// Suit une route depuis la position courante (ou s'arrête si `None`).
    pub fn follow(&mut self, route: Option<&Route>) {
        self.targets = route
            .map(|r| r.waypoints.iter().map(|w| w.position).collect())
            .unwrap_or_default();
        self.target_index = 0;
        if self.targets.is_empty() {
            self.course_rad = None;
        }
    }

    /// Avance jusqu'à `now` et retourne le relevé simulé.
    pub fn step(&mut self, now: DateTime<Utc>) -> PositionFix {
        let dt = self
            .last_step
            .map(|last| ((now - last).num_milliseconds() as f64 / 1000.0).clamp(0.0, MAX_STEP_S))
            .unwrap_or(0.0);
        self.last_step = Some(now);

        let mut budget_m = self.speed_mps * dt;
        while budget_m > 0.0 && self.target_index < self.targets.len() {
            let target = self.targets[self.target_index];
            let inverse = nav_geodesy::inverse(self.position, target);
            match inverse.initial_bearing_rad {
                Some(bearing) if inverse.distance_m > budget_m => {
                    if let Some(next) = nav_geodesy::destination(self.position, bearing, budget_m) {
                        self.position = next;
                    }
                    self.course_rad = Some(bearing);
                    budget_m = 0.0;
                }
                _ => {
                    // Waypoint atteint pendant ce pas.
                    self.position = target;
                    budget_m -= inverse.distance_m;
                    self.target_index += 1;
                }
            }
        }

        let underway = self.is_underway();
        PositionFix {
            position: self.position,
            cog_rad: if underway { self.course_rad } else { None },
            sog_mps: Some(if underway { self.speed_mps } else { 0.0 }),
            source: SourceKind::Simulator,
            source_id: SIMULATOR_SOURCE_ID.to_owned(),
            measured_at: now,
            received_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    fn t(seconds: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000 + seconds, 0).expect("t")
    }

    #[test]
    fn stays_still_without_route() {
        let mut sim = RouteSimulator::new(p(-14.968, -147.638), 3.0);
        sim.step(t(0));
        let fix = sim.step(t(5));
        assert_eq!(fix.position, p(-14.968, -147.638));
        assert_eq!(fix.sog_mps, Some(0.0));
        assert_eq!(fix.cog_rad, None);
        assert_eq!(fix.source, SourceKind::Simulator);
    }

    #[test]
    fn moves_along_route_at_speed() {
        let mut sim = RouteSimulator::new(p(0.0, 0.0), 5.0);
        let route = Route::from_positions("r", &[p(0.0, 0.01), p(0.01, 0.01)], t(0)).expect("r");
        sim.follow(Some(&route));
        sim.step(t(0));
        let fix = sim.step(t(10));
        let travelled = nav_geodesy::distance_m(p(0.0, 0.0), fix.position);
        assert!((travelled - 50.0).abs() < 1e-3, "{travelled}");
        assert_eq!(fix.sog_mps, Some(5.0));
        let cog = fix.cog_rad.expect("cap").to_degrees();
        assert!((cog - 90.0).abs() < 1e-6);
    }

    #[test]
    fn passes_waypoints_and_stops_at_the_end() {
        // ~1113 m puis ~1106 m ; 5 m/s pendant 10 s par pas.
        let mut sim = RouteSimulator::new(p(0.0, 0.0), 5.0);
        let route = Route::from_positions("r", &[p(0.0, 0.01), p(0.01, 0.01)], t(0)).expect("r");
        sim.follow(Some(&route));
        sim.step(t(0));
        let mut last = sim.step(t(0));
        for i in 1..=500 {
            last = sim.step(t(i * 10));
        }
        assert_eq!(last.position, p(0.01, 0.01));
        assert!(!sim.is_underway());
        assert_eq!(last.sog_mps, Some(0.0));
    }

    #[test]
    fn large_time_gaps_are_clamped() {
        let mut sim = RouteSimulator::new(p(0.0, 0.0), 5.0);
        let route = Route::from_positions("r", &[p(0.0, 1.0), p(1.0, 1.0)], t(0)).expect("r");
        sim.follow(Some(&route));
        sim.step(t(0));
        let fix = sim.step(t(0) + Duration::hours(5));
        let travelled = nav_geodesy::distance_m(p(0.0, 0.0), fix.position);
        assert!((travelled - 50.0).abs() < 1e-3);
    }

    #[test]
    fn time_going_backwards_does_not_move() {
        let mut sim = RouteSimulator::new(p(0.0, 0.0), 5.0);
        let route = Route::from_positions("r", &[p(0.0, 1.0), p(1.0, 1.0)], t(0)).expect("r");
        sim.follow(Some(&route));
        sim.step(t(100));
        let fix = sim.step(t(50));
        assert_eq!(fix.position, p(0.0, 0.0));
    }

    #[test]
    fn invalid_speed_is_zeroed() {
        assert!(!RouteSimulator::new(p(0.0, 0.0), f64::NAN).is_underway());
        let mut sim = RouteSimulator::new(p(0.0, 0.0), -3.0);
        let route = Route::from_positions("r", &[p(0.0, 1.0), p(1.0, 1.0)], t(0)).expect("r");
        sim.follow(Some(&route));
        assert!(!sim.is_underway());
    }

    #[test]
    fn stop_following_route() {
        let mut sim = RouteSimulator::new(p(0.0, 0.0), 5.0);
        let route = Route::from_positions("r", &[p(0.0, 1.0), p(1.0, 1.0)], t(0)).expect("r");
        sim.follow(Some(&route));
        assert!(sim.is_underway());
        sim.follow(None);
        assert!(!sim.is_underway());
    }
}
