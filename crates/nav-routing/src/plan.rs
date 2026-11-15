//! Planification d'un parcours : longueur, durée et heures de passage à une
//! vitesse prévue, et emprise géographique (pour zoomer sur la carte).
//!
//! Calculs purs : la vitesse et l'heure de départ sont fournies par
//! l'utilisateur ; rien ne dépend de la position du bateau.

use chrono::{DateTime, Duration, Utc};
use nav_core::GeoPosition;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::MAX_ROUTE_WAYPOINTS;

/// Vitesse prévue minimale (m/s, environ 0,2 nœud).
pub const MIN_PLANNED_SPEED_MPS: f64 = 0.1;
/// Vitesse prévue maximale (m/s, environ 58 nœuds).
pub const MAX_PLANNED_SPEED_MPS: f64 = 30.0;

/// Erreur de planification.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlanError {
    /// Vitesse hors limites ou non finie.
    #[error("vitesse prévue invalide (0,2 à 58 nœuds)")]
    InvalidSpeed,
    /// Nombre de points hors limites.
    #[error("un parcours contient de 2 à {MAX_ROUTE_WAYPOINTS} points")]
    InvalidLength,
}

/// Emprise géographique, en degrés WGS84.
///
/// Une emprise qui traverse l'antiméridien a `west > east`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoBounds {
    /// Longitude ouest.
    pub west: f64,
    /// Latitude sud.
    pub south: f64,
    /// Longitude est.
    pub east: f64,
    /// Latitude nord.
    pub north: f64,
}

/// Emprise d'un ensemble de positions (`None` s'il est vide). Lorsque les
/// positions sont plus resserrées de part et d'autre de l'antiméridien,
/// l'emprise le traverse (`west > east`).
#[must_use]
pub fn bounds_of(positions: impl IntoIterator<Item = GeoPosition>) -> Option<GeoBounds> {
    let mut south = f64::INFINITY;
    let mut north = f64::NEG_INFINITY;
    // Longitudes dans [-180, 180] et dans [0, 360).
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min360, mut max360) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut any = false;
    for p in positions {
        any = true;
        let (lat, lon) = (p.latitude_deg(), p.longitude_deg());
        south = south.min(lat);
        north = north.max(lat);
        min = min.min(lon);
        max = max.max(lon);
        let shifted = if lon < 0.0 { lon + 360.0 } else { lon };
        min360 = min360.min(shifted);
        max360 = max360.max(shifted);
    }
    if !any {
        return None;
    }
    let normalize = |lon: f64| if lon > 180.0 { lon - 360.0 } else { lon };
    let (west, east) = if max360 - min360 < max - min {
        (normalize(min360), normalize(max360))
    } else {
        (min, max)
    };
    Some(GeoBounds {
        west,
        south,
        east,
        north,
    })
}

/// Segment planifié.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedLeg {
    /// Index du point de départ du segment.
    pub from_index: usize,
    /// Distance, en mètres.
    pub distance_m: f64,
    /// Relèvement initial vrai, en radians (`None` si points confondus).
    pub bearing_rad: Option<f64>,
    /// Durée du segment à la vitesse prévue, en secondes.
    pub duration_s: f64,
    /// Distance depuis le départ jusqu'au bout du segment, en mètres.
    pub cumulative_distance_m: f64,
    /// Durée depuis le départ jusqu'au bout du segment, en secondes.
    pub cumulative_duration_s: f64,
    /// Heure d'arrivée au bout du segment (si une heure de départ est donnée).
    pub arrival: Option<DateTime<Utc>>,
}

/// Parcours planifié.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlan {
    /// Vitesse prévue, en m/s.
    pub speed_mps: f64,
    /// Heure de départ.
    pub departure: Option<DateTime<Utc>>,
    /// Segments.
    pub legs: Vec<PlannedLeg>,
    /// Longueur totale, en mètres.
    pub total_distance_m: f64,
    /// Durée totale, en secondes.
    pub total_duration_s: f64,
    /// Heure d'arrivée (si une heure de départ est donnée).
    pub arrival: Option<DateTime<Utc>>,
    /// Emprise du parcours.
    pub bounds: GeoBounds,
}

fn add_seconds(start: DateTime<Utc>, seconds: f64) -> Option<DateTime<Utc>> {
    // Bornes : 1 000 segments d'au plus un demi-tour du globe à 0,1 m/s
    // restent bien en deçà de la capacité d'une durée en millisecondes.
    #[allow(clippy::cast_possible_truncation)]
    let ms = (seconds * 1_000.0).round() as i64;
    start.checked_add_signed(Duration::milliseconds(ms))
}

/// Planifie un parcours à vitesse constante.
///
/// # Errors
///
/// [`PlanError`] si la vitesse n'est pas comprise entre
/// [`MIN_PLANNED_SPEED_MPS`] et [`MAX_PLANNED_SPEED_MPS`], ou si le nombre
/// de points n'est pas compris entre 2 et [`MAX_ROUTE_WAYPOINTS`].
pub fn plan(
    positions: &[GeoPosition],
    speed_mps: f64,
    departure: Option<DateTime<Utc>>,
) -> Result<RoutePlan, PlanError> {
    if !speed_mps.is_finite()
        || !(MIN_PLANNED_SPEED_MPS..=MAX_PLANNED_SPEED_MPS).contains(&speed_mps)
    {
        return Err(PlanError::InvalidSpeed);
    }
    if !(2..=MAX_ROUTE_WAYPOINTS).contains(&positions.len()) {
        return Err(PlanError::InvalidLength);
    }
    let mut legs = Vec::with_capacity(positions.len() - 1);
    let mut cumulative = 0.0;
    for (from_index, pair) in positions.windows(2).enumerate() {
        let inverse = nav_geodesy::inverse(pair[0], pair[1]);
        cumulative += inverse.distance_m;
        legs.push(PlannedLeg {
            from_index,
            distance_m: inverse.distance_m,
            bearing_rad: inverse.initial_bearing_rad,
            duration_s: inverse.distance_m / speed_mps,
            cumulative_distance_m: cumulative,
            cumulative_duration_s: cumulative / speed_mps,
            arrival: departure.and_then(|d| add_seconds(d, cumulative / speed_mps)),
        });
    }
    let total_duration_s = cumulative / speed_mps;
    let bounds = bounds_of(positions.iter().copied()).ok_or(PlanError::InvalidLength)?;
    Ok(RoutePlan {
        speed_mps,
        departure,
        legs,
        total_distance_m: cumulative,
        total_duration_s,
        arrival: departure.and_then(|d| add_seconds(d, total_duration_s)),
        bounds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    fn t(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .expect("t")
            .with_timezone(&Utc)
    }

    #[test]
    fn plans_duration_and_arrivals() {
        // Un degré d'équateur ≈ 111 319 m, à 5 m/s ≈ 22 264 s.
        let route = [p(0.0, 0.0), p(0.0, 1.0), p(0.0, 2.0)];
        let plan = plan(&route, 5.0, Some(t("2026-11-14T06:00:00Z"))).expect("plan");
        assert_eq!(plan.legs.len(), 2);
        assert!((plan.total_distance_m - 222_638.98).abs() < 0.1);
        assert!((plan.total_duration_s - plan.total_distance_m / 5.0).abs() < 1e-9);
        assert!((plan.legs[1].cumulative_distance_m - plan.total_distance_m).abs() < 1e-9);
        assert!((plan.legs[1].cumulative_duration_s - plan.total_duration_s).abs() < 1e-9);
        let first = plan.legs[0].arrival.expect("arrivée");
        // 111 319,49 m / 5 m/s = 22 263,898 s.
        assert_eq!(
            first,
            t("2026-11-14T06:00:00Z") + Duration::milliseconds(22_263_898)
        );
        assert_eq!(plan.arrival, plan.legs[1].arrival);
        assert_eq!(
            plan.bounds,
            GeoBounds {
                west: 0.0,
                south: 0.0,
                east: 2.0,
                north: 0.0
            }
        );
    }

    #[test]
    fn without_departure_there_is_no_arrival_time() {
        let plan = plan(&[p(0.0, 0.0), p(1.0, 0.0)], 3.0, None).expect("plan");
        assert_eq!(plan.arrival, None);
        assert!(plan.legs.iter().all(|l| l.arrival.is_none()));
        assert!(plan.total_duration_s > 0.0);
    }

    #[test]
    fn rejects_invalid_speed_and_length() {
        let route = [p(0.0, 0.0), p(1.0, 0.0)];
        for speed in [0.0, -1.0, 0.05, 31.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                plan(&route, speed, None),
                Err(PlanError::InvalidSpeed),
                "{speed}"
            );
        }
        assert_eq!(plan(&route[..1], 5.0, None), Err(PlanError::InvalidLength));
        assert_eq!(plan(&[], 5.0, None), Err(PlanError::InvalidLength));
        let too_many = vec![p(0.0, 0.0); MAX_ROUTE_WAYPOINTS + 1];
        assert_eq!(plan(&too_many, 5.0, None), Err(PlanError::InvalidLength));
    }

    #[test]
    fn coincident_points_have_zero_duration() {
        let plan = plan(&[p(1.0, 1.0), p(1.0, 1.0)], 5.0, None).expect("plan");
        assert_eq!(plan.total_duration_s, 0.0);
        assert_eq!(plan.legs[0].bearing_rad, None);
    }

    #[test]
    fn bounds_cross_the_antimeridian_when_tighter() {
        // Fidji : de 178° E à 179° O.
        let b = bounds_of([p(-17.0, 178.0), p(-18.0, -179.0)]).expect("emprise");
        assert_eq!(b.west, 178.0);
        assert_eq!(b.east, -179.0);
        assert_eq!((b.south, b.north), (-18.0, -17.0));
        // Polynésie : pas de traversée.
        let b = bounds_of([p(-17.5, -149.6), p(-16.5, -151.7)]).expect("emprise");
        assert_eq!((b.west, b.east), (-151.7, -149.6));
        assert!(bounds_of([]).is_none());
        let single = bounds_of([p(10.0, 20.0)]).expect("emprise");
        assert_eq!(
            (single.west, single.east, single.south, single.north),
            (20.0, 20.0, 10.0, 10.0)
        );
    }
}
