//! # nav-geodesy
//!
//! Calculs géodésiques sur l'ellipsoïde WGS84 et calculs de navigation.
//!
//! Conventions (voir `AGENTS.md`) :
//! - positions : [`GeoPosition`] en degrés WGS84 ;
//! - distances : mètres ;
//! - angles (relèvements, routes) : radians, relèvements vrais dans
//!   [0, 2π), mesurés depuis le nord dans le sens horaire.
//!
//! Les problèmes inverse et direct utilisent l'algorithme de Karney
//! (GeographicLib), précis au nanomètre et convergent partout. L'écart
//! de route (XTE) utilise la formule sphérique appliquée aux distances et
//! relèvements ellipsoïdaux : l'erreur est négligeable pour des segments
//! de navigation usuels (quelques dizaines de milles).

pub mod units;

use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};
use nav_core::GeoPosition;

/// Rayon moyen terrestre (IUGG), en mètres, pour les calculs sphériques.
pub const MEAN_EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Distance en dessous de laquelle deux positions sont considérées
/// confondues et le relèvement indéfini.
pub const COINCIDENT_DISTANCE_M: f64 = 1e-6;

/// Résultat du problème géodésique inverse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Inverse {
    /// Distance géodésique, en mètres.
    pub distance_m: f64,
    /// Relèvement initial vrai, en radians [0, 2π). `None` si les
    /// positions sont confondues.
    pub initial_bearing_rad: Option<f64>,
    /// Route finale vraie à l'arrivée, en radians [0, 2π). `None` si les
    /// positions sont confondues.
    pub final_bearing_rad: Option<f64>,
}

/// Distance et relèvements de `from` vers `to` sur l'ellipsoïde WGS84.
#[must_use]
pub fn inverse(from: GeoPosition, to: GeoPosition) -> Inverse {
    let geodesic = Geodesic::wgs84();
    let (distance_m, azi1, azi2, _a12): (f64, f64, f64, f64) = geodesic.inverse(
        from.latitude_deg(),
        from.longitude_deg(),
        to.latitude_deg(),
        to.longitude_deg(),
    );
    let defined = distance_m > COINCIDENT_DISTANCE_M;
    Inverse {
        distance_m,
        initial_bearing_rad: defined.then(|| units::normalize_angle_rad(azi1.to_radians())),
        final_bearing_rad: defined.then(|| units::normalize_angle_rad(azi2.to_radians())),
    }
}

/// Distance géodésique en mètres.
#[must_use]
pub fn distance_m(from: GeoPosition, to: GeoPosition) -> f64 {
    inverse(from, to).distance_m
}

/// Position atteinte en partant de `from` au relèvement vrai
/// `bearing_rad` sur `distance_m` mètres (problème direct).
///
/// Retourne `None` si les paramètres ne sont pas finis ou si le
/// résultat sort des bornes WGS84.
#[must_use]
pub fn destination(from: GeoPosition, bearing_rad: f64, distance_m: f64) -> Option<GeoPosition> {
    if !bearing_rad.is_finite() || !distance_m.is_finite() {
        return None;
    }
    let geodesic = Geodesic::wgs84();
    let (lat, lon): (f64, f64) = geodesic.direct(
        from.latitude_deg(),
        from.longitude_deg(),
        bearing_rad.to_degrees(),
        distance_m,
    );
    GeoPosition::new(lat, units::normalize_longitude_deg(lon)).ok()
}

/// Position d'un point par rapport à un segment de route.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrossTrack {
    /// Écart de route (XTE) en mètres : positif à droite de la route,
    /// négatif à gauche.
    pub cross_track_m: f64,
    /// Distance parcourue le long de la route depuis le départ, en
    /// mètres (négative si le point est en arrière du départ).
    pub along_track_m: f64,
    /// Longueur du segment, en mètres.
    pub leg_length_m: f64,
}

/// Écart de route de `point` par rapport au segment `start` → `end`.
///
/// Retourne `None` si le segment est dégénéré (départ et arrivée
/// confondus).
#[must_use]
pub fn cross_track(start: GeoPosition, end: GeoPosition, point: GeoPosition) -> Option<CrossTrack> {
    let leg = inverse(start, end);
    let leg_bearing = leg.initial_bearing_rad?;
    let to_point = inverse(start, point);
    let Some(point_bearing) = to_point.initial_bearing_rad else {
        // Point confondu avec le départ.
        return Some(CrossTrack {
            cross_track_m: 0.0,
            along_track_m: 0.0,
            leg_length_m: leg.distance_m,
        });
    };
    let angular = to_point.distance_m / MEAN_EARTH_RADIUS_M;
    let delta = point_bearing - leg_bearing;
    let xt_angular = (angular.sin() * delta.sin()).clamp(-1.0, 1.0).asin();
    let ratio = (angular.cos() / xt_angular.cos()).clamp(-1.0, 1.0);
    let at_angular = ratio.acos() * delta.cos().signum();
    Some(CrossTrack {
        cross_track_m: xt_angular * MEAN_EARTH_RADIUS_M,
        along_track_m: at_angular * MEAN_EARTH_RADIUS_M,
        leg_length_m: leg.distance_m,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("position valide")
    }

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{actual} != {expected} (±{tolerance})"
        );
    }

    #[test]
    fn one_degree_of_longitude_at_equator() {
        // Valeur de référence WGS84 : 111 319,491 m.
        let inv = inverse(p(0.0, 0.0), p(0.0, 1.0));
        assert_close(inv.distance_m, 111_319.491, 0.001);
        assert_close(inv.initial_bearing_rad.expect("défini"), FRAC_PI_2, 1e-12);
    }

    #[test]
    fn one_degree_of_latitude_along_meridian() {
        // Méridien, de 0° à 1° N : 110 574,389 m.
        let inv = inverse(p(0.0, 0.0), p(1.0, 0.0));
        assert_close(inv.distance_m, 110_574.389, 0.001);
        assert_close(inv.initial_bearing_rad.expect("défini"), 0.0, 1e-12);
    }

    #[test]
    fn papeete_to_rangiroa_is_plausible() {
        // Contrôle de vraisemblance (pas une valeur de référence) :
        // estimation plane locale ≈ 284 km vers le nord et 206 km vers
        // l'est, soit ≈ 351 km au relèvement ≈ 36°.
        let papeete = p(-17.535, -149.569);
        let tiputa = p(-14.968, -147.638);
        let inv = inverse(papeete, tiputa);
        assert!(
            (345_000.0..356_000.0).contains(&inv.distance_m),
            "{}",
            inv.distance_m
        );
        let bearing = inv.initial_bearing_rad.expect("défini").to_degrees();
        assert!((35.0..37.5).contains(&bearing), "{bearing}");
        // Symétrie : la distance retour est identique.
        assert_close(inverse(tiputa, papeete).distance_m, inv.distance_m, 1e-6);
    }

    #[test]
    fn identical_positions_have_zero_distance_and_no_bearing() {
        let inv = inverse(p(-14.968, -147.638), p(-14.968, -147.638));
        assert_eq!(inv.distance_m, 0.0);
        assert_eq!(inv.initial_bearing_rad, None);
        assert_eq!(inv.final_bearing_rad, None);
    }

    #[test]
    fn crossing_the_antimeridian_takes_the_short_way() {
        let inv = inverse(p(0.0, 179.5), p(0.0, -179.5));
        assert_close(inv.distance_m, 111_319.491, 0.001);
        assert_close(inv.initial_bearing_rad.expect("défini"), FRAC_PI_2, 1e-9);
        let back = inverse(p(0.0, -179.5), p(0.0, 179.5));
        assert_close(
            back.initial_bearing_rad.expect("défini"),
            3.0 * FRAC_PI_2,
            1e-9,
        );
    }

    #[test]
    fn nearly_antipodal_points_converge() {
        let inv = inverse(p(0.0, 0.0), p(0.5, 179.7));
        assert!(inv.distance_m.is_finite());
        assert!((19_900_000.0..20_010_000.0).contains(&inv.distance_m));
    }

    #[test]
    fn poles_are_supported() {
        let inv = inverse(p(90.0, 0.0), p(-90.0, 0.0));
        assert_close(inv.distance_m, 20_003_931.459, 0.01);
    }

    #[test]
    fn bearings_are_normalized() {
        let inv = inverse(p(0.0, 0.0), p(-1.0, -1.0));
        let b = inv.initial_bearing_rad.expect("défini");
        assert!((PI..2.0 * PI).contains(&b), "{b}");
    }

    #[test]
    fn destination_inverts_inverse() {
        let from = p(-14.968, -147.638);
        let to = destination(from, 0.7, 25_000.0).expect("destination");
        let inv = inverse(from, to);
        assert_close(inv.distance_m, 25_000.0, 1e-6);
        assert_close(inv.initial_bearing_rad.expect("défini"), 0.7, 1e-9);
    }

    #[test]
    fn destination_normalizes_longitude_across_antimeridian() {
        let to = destination(p(0.0, 179.9), FRAC_PI_2, 50_000.0).expect("destination");
        assert!(to.longitude_deg() < -179.0, "{}", to.longitude_deg());
    }

    #[test]
    fn destination_rejects_non_finite_input() {
        assert!(destination(p(0.0, 0.0), f64::NAN, 10.0).is_none());
        assert!(destination(p(0.0, 0.0), 0.0, f64::INFINITY).is_none());
    }

    #[test]
    fn cross_track_sign_and_magnitude() {
        // Route plein nord le long du méridien 0.
        let start = p(0.0, 0.0);
        let end = p(1.0, 0.0);
        let east = p(0.5, 0.01);
        let west = p(0.5, -0.01);
        let xte_east = cross_track(start, end, east).expect("défini");
        let xte_west = cross_track(start, end, west).expect("défini");
        assert!(
            xte_east.cross_track_m > 0.0,
            "à droite d'une route nord : est"
        );
        assert!(xte_west.cross_track_m < 0.0);
        assert_close(xte_east.cross_track_m, 1_113.0, 5.0);
        assert_close(xte_east.along_track_m, 55_287.0, 200.0);
    }

    #[test]
    fn point_on_track_has_no_cross_track_error() {
        let ct = cross_track(p(0.0, 0.0), p(0.0, 1.0), p(0.0, 0.25)).expect("défini");
        assert_close(ct.cross_track_m, 0.0, 1e-6);
        assert_close(ct.along_track_m, 27_830.0, 50.0);
    }

    #[test]
    fn point_behind_start_has_negative_along_track() {
        let ct = cross_track(p(0.0, 0.0), p(1.0, 0.0), p(-0.1, 0.0)).expect("défini");
        assert!(ct.along_track_m < 0.0);
    }

    #[test]
    fn degenerate_leg_has_no_cross_track() {
        assert!(cross_track(p(1.0, 1.0), p(1.0, 1.0), p(2.0, 2.0)).is_none());
    }

    #[test]
    fn point_at_start_is_on_track() {
        let ct = cross_track(p(0.0, 0.0), p(1.0, 0.0), p(0.0, 0.0)).expect("défini");
        assert_eq!(ct.cross_track_m, 0.0);
        assert_eq!(ct.along_track_m, 0.0);
    }
}
