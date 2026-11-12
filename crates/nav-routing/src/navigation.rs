//! Navigation active : suivi d'une route waypoint par waypoint.
//!
//! Calcule à chaque relevé la distance et le relèvement vers le waypoint
//! actif, l'écart de route (XTE), la distance restante et les ETA, et
//! passe au waypoint suivant à l'entrée dans le cercle d'arrivée.

use chrono::{DateTime, Duration, Utc};
use nav_core::{GeoPosition, PositionFix};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::model::Route;

/// Rayon d'arrivée par défaut : 0,1 mille marin.
pub const DEFAULT_ARRIVAL_RADIUS_M: f64 = 185.2;

/// Vitesse fond minimale (m/s, ≈ 0,2 nœud) pour estimer une ETA.
pub const MIN_SPEED_FOR_ETA_MPS: f64 = 0.1;

/// Erreur de configuration de la navigation.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum NavigationError {
    /// Rayon d'arrivée non fini ou hors de ]0, 10 000] m.
    #[error("rayon d'arrivée invalide : {0} m")]
    InvalidArrivalRadius(f64),
}

/// État de navigation calculé pour un relevé.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationStatus {
    /// Route suivie.
    pub route_id: Uuid,
    /// Nom de la route.
    pub route_name: String,
    /// Index du waypoint actif.
    pub target_index: usize,
    /// Nombre de waypoints de la route.
    pub waypoint_count: usize,
    /// Nom du waypoint actif.
    pub target_name: String,
    /// Position du waypoint actif.
    pub target_position: GeoPosition,
    /// Distance au waypoint actif (DTW), en mètres.
    pub distance_to_target_m: f64,
    /// Relèvement vrai vers le waypoint actif (BTW), en radians.
    pub bearing_to_target_rad: Option<f64>,
    /// Écart de route (XTE) en mètres, positif à droite.
    pub cross_track_m: Option<f64>,
    /// Distance restante jusqu'à la fin de la route, en mètres.
    pub remaining_distance_m: f64,
    /// ETA au waypoint actif (UTC), estimée à la vitesse fond courante.
    pub eta_target: Option<DateTime<Utc>>,
    /// ETA à la fin de la route (UTC).
    pub eta_end: Option<DateTime<Utc>>,
    /// Nom du waypoint atteint lors de ce relevé, le cas échéant.
    pub reached_waypoint: Option<String>,
    /// Route terminée (dernier waypoint atteint).
    pub finished: bool,
    /// Le calcul repose sur une position simulée.
    pub simulated: bool,
    /// Horodatage du relevé utilisé (UTC).
    pub computed_at: DateTime<Utc>,
}

/// Suivi d'une route active.
#[derive(Debug, Clone)]
pub struct ActiveNavigation {
    route: Route,
    target_index: usize,
    leg_origin: Option<GeoPosition>,
    arrival_radius_m: f64,
    finished: bool,
}

impl ActiveNavigation {
    /// Active une route, en visant son premier waypoint.
    ///
    /// # Errors
    ///
    /// [`NavigationError::InvalidArrivalRadius`] si le rayon est invalide.
    pub fn new(route: Route, arrival_radius_m: f64) -> Result<Self, NavigationError> {
        if !arrival_radius_m.is_finite() || arrival_radius_m <= 0.0 || arrival_radius_m > 10_000.0 {
            return Err(NavigationError::InvalidArrivalRadius(arrival_radius_m));
        }
        Ok(Self {
            route,
            target_index: 0,
            leg_origin: None,
            arrival_radius_m,
            finished: false,
        })
    }

    /// Route suivie.
    #[must_use]
    pub fn route(&self) -> &Route {
        &self.route
    }

    /// Index du waypoint actif.
    #[must_use]
    pub fn target_index(&self) -> usize {
        self.target_index
    }

    /// Met à jour la navigation avec un relevé de position.
    pub fn update(&mut self, fix: &PositionFix) -> NavigationStatus {
        let position = fix.position;
        if self.leg_origin.is_none() {
            // Le premier segment part de la position à l'activation.
            self.leg_origin = Some(position);
        }
        let mut reached = None;
        if !self.finished {
            let target = &self.route.waypoints[self.target_index];
            if nav_geodesy::distance_m(position, target.position) <= self.arrival_radius_m {
                reached = Some(target.name.clone());
                if self.target_index + 1 < self.route.waypoints.len() {
                    self.target_index += 1;
                } else {
                    self.finished = true;
                }
            }
        }
        self.status(fix, reached)
    }

    fn status(&self, fix: &PositionFix, reached: Option<String>) -> NavigationStatus {
        let position = fix.position;
        let target = &self.route.waypoints[self.target_index];
        let to_target = nav_geodesy::inverse(position, target.position);
        let leg_start = if self.target_index == 0 {
            self.leg_origin
        } else {
            Some(self.route.waypoints[self.target_index - 1].position)
        };
        let cross_track_m = if self.finished {
            None
        } else {
            leg_start
                .and_then(|start| nav_geodesy::cross_track(start, target.position, position))
                .map(|ct| ct.cross_track_m)
        };
        let following: f64 = self.route.legs()[self.target_index.min(self.route.legs().len())..]
            .iter()
            .map(|leg| leg.distance_m)
            .sum();
        let remaining_distance_m = if self.finished {
            0.0
        } else {
            to_target.distance_m + following
        };

        let speed = fix
            .sog_mps
            .filter(|s| s.is_finite() && *s >= MIN_SPEED_FOR_ETA_MPS);
        let eta = |distance_m: f64| {
            speed.and_then(|s| {
                let seconds = distance_m / s;
                (seconds < 1e9)
                    .then(|| fix.measured_at + Duration::milliseconds((seconds * 1000.0) as i64))
            })
        };

        NavigationStatus {
            route_id: self.route.id,
            route_name: self.route.name.clone(),
            target_index: self.target_index,
            waypoint_count: self.route.waypoints.len(),
            target_name: target.name.clone(),
            target_position: target.position,
            distance_to_target_m: to_target.distance_m,
            bearing_to_target_rad: to_target.initial_bearing_rad,
            cross_track_m,
            remaining_distance_m,
            eta_target: if self.finished {
                None
            } else {
                eta(to_target.distance_m)
            },
            eta_end: if self.finished {
                None
            } else {
                eta(remaining_distance_m)
            },
            reached_waypoint: reached,
            finished: self.finished,
            simulated: fix.source.is_simulated(),
            computed_at: fix.measured_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nav_core::SourceKind;

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    fn fix(lat: f64, lon: f64, sog: Option<f64>) -> PositionFix {
        PositionFix {
            position: p(lat, lon),
            cog_rad: None,
            sog_mps: sog,
            source: SourceKind::Nmea0183,
            source_id: "test".to_owned(),
            measured_at: Utc::now(),
            received_at: Utc::now(),
        }
    }

    fn nav() -> ActiveNavigation {
        let route =
            Route::from_positions("R", &[p(0.0, 0.1), p(0.0, 0.2), p(0.1, 0.2)], Utc::now())
                .expect("route");
        ActiveNavigation::new(route, DEFAULT_ARRIVAL_RADIUS_M).expect("nav")
    }

    #[test]
    fn rejects_invalid_arrival_radius() {
        let route = nav().route().clone();
        for r in [0.0, -1.0, f64::NAN, 20_000.0] {
            assert!(ActiveNavigation::new(route.clone(), r).is_err());
        }
    }

    #[test]
    fn targets_first_waypoint_from_activation_position() {
        let mut n = nav();
        let s = n.update(&fix(0.0, 0.0, Some(3.0)));
        assert_eq!(s.target_index, 0);
        assert_eq!(s.target_name, "WP001");
        assert!((s.distance_to_target_m - 11_131.95).abs() < 0.1);
        let btw = s.bearing_to_target_rad.expect("défini").to_degrees();
        assert!((btw - 90.0).abs() < 1e-6);
        assert!(s.cross_track_m.expect("défini").abs() < 1e-6);
        assert!(!s.finished);
        assert!(!s.simulated);
    }

    #[test]
    fn computes_eta_from_speed_over_ground() {
        let mut n = nav();
        let f = fix(0.0, 0.0, Some(5.0));
        let s = n.update(&f);
        let eta = s.eta_target.expect("eta");
        let seconds = (eta - f.measured_at).num_seconds();
        assert!((2_225..=2_227).contains(&seconds), "{seconds}");
        assert!(s.eta_end.expect("eta fin") > eta);
    }

    #[test]
    fn no_eta_without_meaningful_speed() {
        let mut n = nav();
        assert!(n.update(&fix(0.0, 0.0, None)).eta_target.is_none());
        assert!(n.update(&fix(0.0, 0.0, Some(0.05))).eta_target.is_none());
        assert!(n
            .update(&fix(0.0, 0.0, Some(f64::NAN)))
            .eta_target
            .is_none());
    }

    #[test]
    fn advances_inside_arrival_circle() {
        let mut n = nav();
        n.update(&fix(0.0, 0.0, None));
        let s = n.update(&fix(0.0005, 0.1, None));
        assert_eq!(s.reached_waypoint.as_deref(), Some("WP001"));
        assert_eq!(s.target_index, 1);
        assert_eq!(s.target_name, "WP002");
    }

    #[test]
    fn cross_track_error_on_second_leg() {
        let mut n = nav();
        n.update(&fix(0.0, 0.1, None));
        // Segment WP001 → WP002 vers l'est ; un point au nord est à gauche.
        let s = n.update(&fix(0.01, 0.15, None));
        let xte = s.cross_track_m.expect("défini");
        assert!(xte < 0.0, "{xte}");
        assert!((xte.abs() - 1_105.0).abs() < 10.0, "{xte}");
    }

    #[test]
    fn remaining_distance_includes_following_legs() {
        let mut n = nav();
        let s = n.update(&fix(0.0, 0.1, None));
        // Sur WP001 : arrivée, cible WP002 ; reste 2 segments de ~11 km.
        let legs: f64 = n.route().total_distance_m();
        assert!((s.remaining_distance_m - legs).abs() < 1.0);
    }

    #[test]
    fn finishes_at_last_waypoint() {
        let mut n = nav();
        n.update(&fix(0.0, 0.1, None));
        n.update(&fix(0.0, 0.2, None));
        let s = n.update(&fix(0.1, 0.2, Some(3.0)));
        assert!(s.finished);
        assert_eq!(s.reached_waypoint.as_deref(), Some("WP003"));
        assert_eq!(s.remaining_distance_m, 0.0);
        assert!(s.eta_target.is_none());
        assert!(s.cross_track_m.is_none());
        let again = n.update(&fix(0.2, 0.2, Some(3.0)));
        assert!(again.finished);
        assert!(again.reached_waypoint.is_none());
    }

    #[test]
    fn flags_simulated_positions() {
        let mut n = nav();
        let mut f = fix(0.0, 0.0, None);
        f.source = SourceKind::Simulator;
        assert!(n.update(&f).simulated);
    }
}
