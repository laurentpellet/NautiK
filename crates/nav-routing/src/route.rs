//! Opérations sur les routes : création, édition, inversion, segments.

use chrono::{DateTime, Utc};
use nav_core::GeoPosition;
use serde::Serialize;
use uuid::Uuid;

use crate::model::{
    validate_description, validate_name, Route, ValidationError, Waypoint, MAX_ROUTE_WAYPOINTS,
};

/// Segment d'une route entre deux waypoints consécutifs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leg {
    /// Index du waypoint de départ.
    pub from_index: usize,
    /// Distance, en mètres.
    pub distance_m: f64,
    /// Relèvement initial vrai, en radians (`None` si waypoints confondus).
    pub bearing_rad: Option<f64>,
}

impl Route {
    /// Crée une route à partir de positions : un waypoint `WPnnn` est créé
    /// pour chaque position.
    ///
    /// # Errors
    ///
    /// [`ValidationError`] si le nom est invalide ou si le nombre de
    /// positions n'est pas compris entre 2 et [`MAX_ROUTE_WAYPOINTS`].
    pub fn from_positions(
        name: &str,
        positions: &[GeoPosition],
        now: DateTime<Utc>,
    ) -> Result<Self, ValidationError> {
        let waypoints = positions
            .iter()
            .enumerate()
            .map(|(i, position)| Waypoint::new(&format!("WP{:03}", i + 1), *position, None, now))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(name, None, waypoints, now)
    }

    /// Crée une route à partir de waypoints existants.
    ///
    /// # Errors
    ///
    /// [`ValidationError`] si le nom, la description ou la longueur sont
    /// invalides.
    pub fn new(
        name: &str,
        description: Option<&str>,
        waypoints: Vec<Waypoint>,
        now: DateTime<Utc>,
    ) -> Result<Self, ValidationError> {
        check_length(waypoints.len())?;
        Ok(Self {
            id: Uuid::new_v4(),
            name: validate_name(name)?,
            description: validate_description(description)?,
            waypoints,
            created_at: now,
            updated_at: now,
        })
    }

    /// Segments de la route.
    #[must_use]
    pub fn legs(&self) -> Vec<Leg> {
        self.waypoints
            .windows(2)
            .enumerate()
            .map(|(from_index, pair)| {
                let inverse = nav_geodesy::inverse(pair[0].position, pair[1].position);
                Leg {
                    from_index,
                    distance_m: inverse.distance_m,
                    bearing_rad: inverse.initial_bearing_rad,
                }
            })
            .collect()
    }

    /// Distance totale, en mètres.
    #[must_use]
    pub fn total_distance_m(&self) -> f64 {
        self.legs().iter().map(|leg| leg.distance_m).sum()
    }

    /// Inverse l'ordre de passage.
    pub fn reverse(&mut self, now: DateTime<Utc>) {
        self.waypoints.reverse();
        self.updated_at = now;
    }

    /// Insère un waypoint à l'index donné.
    ///
    /// # Errors
    ///
    /// [`ValidationError`] si l'index est hors limites ou la route pleine.
    pub fn insert_waypoint(
        &mut self,
        index: usize,
        waypoint: Waypoint,
        now: DateTime<Utc>,
    ) -> Result<(), ValidationError> {
        if index > self.waypoints.len() {
            return Err(ValidationError::IndexOutOfRange(index));
        }
        check_length(self.waypoints.len() + 1)?;
        self.waypoints.insert(index, waypoint);
        self.updated_at = now;
        Ok(())
    }

    /// Retire le waypoint à l'index donné.
    ///
    /// # Errors
    ///
    /// [`ValidationError`] si l'index est hors limites ou si la route
    /// deviendrait trop courte.
    pub fn remove_waypoint(
        &mut self,
        index: usize,
        now: DateTime<Utc>,
    ) -> Result<Waypoint, ValidationError> {
        if index >= self.waypoints.len() {
            return Err(ValidationError::IndexOutOfRange(index));
        }
        check_length(self.waypoints.len() - 1)?;
        self.updated_at = now;
        Ok(self.waypoints.remove(index))
    }

    /// Déplace un waypoint de la route.
    ///
    /// # Errors
    ///
    /// [`ValidationError::IndexOutOfRange`] si l'index est hors limites.
    pub fn move_waypoint(
        &mut self,
        index: usize,
        position: GeoPosition,
        now: DateTime<Utc>,
    ) -> Result<(), ValidationError> {
        let waypoint = self
            .waypoints
            .get_mut(index)
            .ok_or(ValidationError::IndexOutOfRange(index))?;
        waypoint.position = position;
        waypoint.updated_at = now;
        self.updated_at = now;
        Ok(())
    }
}

fn check_length(len: usize) -> Result<(), ValidationError> {
    if (2..=MAX_ROUTE_WAYPOINTS).contains(&len) {
        Ok(())
    } else {
        Err(ValidationError::InvalidRouteLength)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    fn route() -> Route {
        Route::from_positions("Test", &[p(0.0, 0.0), p(0.0, 1.0), p(1.0, 1.0)], Utc::now())
            .expect("route")
    }

    #[test]
    fn creates_named_waypoints() {
        let r = route();
        let names: Vec<_> = r.waypoints.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, ["WP001", "WP002", "WP003"]);
    }

    #[test]
    fn legs_and_total_distance() {
        let r = route();
        let legs = r.legs();
        assert_eq!(legs.len(), 2);
        assert!((legs[0].distance_m - 111_319.491).abs() < 0.01);
        assert!((legs[0].bearing_rad.expect("défini") - FRAC_PI_2).abs() < 1e-9);
        let total = r.total_distance_m();
        assert!((total - legs[0].distance_m - legs[1].distance_m).abs() < 1e-9);
    }

    #[test]
    fn reverse_inverts_order_and_keeps_distance() {
        let mut r = route();
        let distance = r.total_distance_m();
        let first = r.waypoints[0].id;
        r.reverse(Utc::now());
        assert_eq!(r.waypoints[2].id, first);
        assert!((r.total_distance_m() - distance).abs() < 1e-6);
    }

    #[test]
    fn rejects_too_short_routes() {
        assert_eq!(
            Route::from_positions("x", &[p(0.0, 0.0)], Utc::now()),
            Err(ValidationError::InvalidRouteLength)
        );
        assert_eq!(
            Route::from_positions("x", &[], Utc::now()),
            Err(ValidationError::InvalidRouteLength)
        );
        let too_many = vec![p(0.0, 0.0); MAX_ROUTE_WAYPOINTS + 1];
        assert!(Route::from_positions("x", &too_many, Utc::now()).is_err());
    }

    #[test]
    fn rejects_invalid_name() {
        assert_eq!(
            Route::from_positions(" ", &[p(0.0, 0.0), p(1.0, 1.0)], Utc::now()),
            Err(ValidationError::InvalidName)
        );
    }

    #[test]
    fn add_and_remove_waypoints() {
        let mut r = route();
        let wp = Waypoint::new("X", p(0.5, 0.5), None, Utc::now()).expect("wp");
        r.insert_waypoint(1, wp.clone(), Utc::now())
            .expect("insert");
        assert_eq!(r.waypoints[1].id, wp.id);
        assert_eq!(r.waypoints.len(), 4);
        assert_eq!(
            r.insert_waypoint(9, wp, Utc::now()),
            Err(ValidationError::IndexOutOfRange(9))
        );
        let removed = r.remove_waypoint(1, Utc::now()).expect("remove");
        assert_eq!(removed.name, "X");
        r.remove_waypoint(0, Utc::now()).expect("remove");
        assert_eq!(
            r.remove_waypoint(0, Utc::now()),
            Err(ValidationError::InvalidRouteLength),
            "une route garde au moins 2 waypoints"
        );
        assert_eq!(
            r.remove_waypoint(5, Utc::now()),
            Err(ValidationError::IndexOutOfRange(5))
        );
    }

    #[test]
    fn move_waypoint_updates_position() {
        let mut r = route();
        r.move_waypoint(0, p(0.0, -1.0), Utc::now()).expect("move");
        assert_eq!(r.waypoints[0].position, p(0.0, -1.0));
        assert!(r.move_waypoint(7, p(0.0, 0.0), Utc::now()).is_err());
    }

    #[test]
    fn coincident_waypoints_have_undefined_bearing() {
        let r = Route::from_positions("x", &[p(1.0, 1.0), p(1.0, 1.0)], Utc::now()).expect("r");
        assert_eq!(r.legs()[0].bearing_rad, None);
        assert_eq!(r.total_distance_m(), 0.0);
    }
}
