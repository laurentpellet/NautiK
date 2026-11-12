//! Conversions d'unités. Le moteur travaille en SI ; ces conversions
//! servent aux entrées/sorties (NMEA, GPX, affichage).

use std::f64::consts::TAU;

/// Mètres par mille marin international.
pub const METERS_PER_NAUTICAL_MILE: f64 = 1_852.0;

/// Mètres par seconde pour un nœud.
pub const MPS_PER_KNOT: f64 = METERS_PER_NAUTICAL_MILE / 3_600.0;

/// Milles marins → mètres.
#[must_use]
pub fn nautical_miles_to_meters(nm: f64) -> f64 {
    nm * METERS_PER_NAUTICAL_MILE
}

/// Mètres → milles marins.
#[must_use]
pub fn meters_to_nautical_miles(m: f64) -> f64 {
    m / METERS_PER_NAUTICAL_MILE
}

/// Nœuds → mètres par seconde.
#[must_use]
pub fn knots_to_mps(knots: f64) -> f64 {
    knots * MPS_PER_KNOT
}

/// Mètres par seconde → nœuds.
#[must_use]
pub fn mps_to_knots(mps: f64) -> f64 {
    mps / MPS_PER_KNOT
}

/// Ramène un angle dans [0, 2π).
#[must_use]
pub fn normalize_angle_rad(angle: f64) -> f64 {
    let normalized = angle.rem_euclid(TAU);
    // rem_euclid peut retourner TAU pour des valeurs négatives infimes.
    if normalized >= TAU {
        0.0
    } else {
        normalized
    }
}

/// Ramène une longitude dans [-180, 180].
#[must_use]
pub fn normalize_longitude_deg(lon: f64) -> f64 {
    if (-180.0..=180.0).contains(&lon) {
        lon
    } else {
        (lon + 180.0).rem_euclid(360.0) - 180.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn distance_conversions() {
        assert_eq!(nautical_miles_to_meters(1.0), 1_852.0);
        assert_eq!(meters_to_nautical_miles(3_704.0), 2.0);
        assert_eq!(meters_to_nautical_miles(0.0), 0.0);
    }

    #[test]
    fn speed_conversions() {
        assert!((knots_to_mps(1.0) - 0.514_444).abs() < 1e-6);
        assert!((mps_to_knots(knots_to_mps(6.5)) - 6.5).abs() < 1e-12);
    }

    #[test]
    fn angle_normalization() {
        assert_eq!(normalize_angle_rad(0.0), 0.0);
        assert!((normalize_angle_rad(-PI / 2.0) - 3.0 * PI / 2.0).abs() < 1e-12);
        assert!((normalize_angle_rad(5.0 * PI) - PI).abs() < 1e-12);
        assert_eq!(normalize_angle_rad(TAU), 0.0);
        assert!(normalize_angle_rad(-1e-300) < TAU);
    }

    #[test]
    fn longitude_normalization() {
        assert_eq!(normalize_longitude_deg(-180.0), -180.0);
        assert_eq!(normalize_longitude_deg(180.0), 180.0);
        assert!((normalize_longitude_deg(190.0) - -170.0).abs() < 1e-12);
        assert!((normalize_longitude_deg(-190.0) - 170.0).abs() < 1e-12);
        assert!((normalize_longitude_deg(540.0) - -180.0).abs() < 1e-12);
    }
}
