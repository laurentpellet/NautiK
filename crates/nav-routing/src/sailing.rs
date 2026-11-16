//! Aides à la navigation du bateau : vecteur de route (COG/SOG), ligne
//! de cap, VMG et laylines.
//!
//! Fonctions pures. Aucune valeur n'est extrapolée à partir d'une donnée
//! absente : chaque résultat est `None` dès qu'une entrée manque.

use nav_core::GeoPosition;
use nav_geodesy::units::normalize_angle_rad;
use serde::Serialize;
use std::f64::consts::{FRAC_PI_2, PI};

/// Horizon du vecteur de route par défaut (s) : 6 minutes, comme l'AIS.
pub const DEFAULT_VECTOR_HORIZON_S: f64 = 360.0;
/// Vitesse fond minimale (m/s, ≈ 0,2 nœud) pour tracer un vecteur.
pub const MIN_VECTOR_SPEED_MPS: f64 = 0.1;
/// Longueur minimale de la ligne de cap (m) : visible même à l'arrêt.
pub const MIN_HEADING_LINE_M: f64 = 463.0;
/// Angle au vent réel par défaut au près (rad, 45°).
pub const DEFAULT_TACK_ANGLE_RAD: f64 = PI / 4.0;
/// Angle au vent réel par défaut au portant (rad, 150°).
pub const DEFAULT_GYBE_ANGLE_RAD: f64 = 150.0 * PI / 180.0;
/// Longueur maximale tracée d'une layline (m).
pub const MAX_LAYLINE_M: f64 = 50_000.0;

/// Différence angulaire signée `a − b`, dans ]−π, π].
#[must_use]
pub fn angle_diff(a: f64, b: f64) -> f64 {
    let d = normalize_angle_rad(a - b);
    if d > PI {
        d - 2.0 * PI
    } else {
        d
    }
}

/// Extrémité du vecteur de route : position atteinte à la route et à la
/// vitesse fond courantes après `horizon_s`. `None` à l'arrêt ou sans
/// route fond.
#[must_use]
pub fn course_vector(
    position: GeoPosition,
    cog_rad: Option<f64>,
    sog_mps: Option<f64>,
    horizon_s: f64,
) -> Option<GeoPosition> {
    let (cog, sog) = (cog_rad?, sog_mps?);
    if !sog.is_finite() || sog < MIN_VECTOR_SPEED_MPS || !horizon_s.is_finite() || horizon_s <= 0.0
    {
        return None;
    }
    nav_geodesy::destination(position, cog, sog * horizon_s)
}

/// Extrémité de la ligne de cap (cap vrai), de la longueur du vecteur de
/// route et au moins [`MIN_HEADING_LINE_M`].
#[must_use]
pub fn heading_line(
    position: GeoPosition,
    heading_rad: Option<f64>,
    sog_mps: Option<f64>,
    horizon_s: f64,
) -> Option<GeoPosition> {
    let heading = heading_rad?;
    let length = sog_mps
        .filter(|s| s.is_finite() && *s > 0.0)
        .map_or(0.0, |s| s * horizon_s)
        .max(MIN_HEADING_LINE_M);
    nav_geodesy::destination(position, heading, length)
}

/// Dérive du bateau : route fond (COG) et ligne de foi (cap vrai), tracées
/// à l'avant du bateau sur la durée de projection choisie. L'angle entre
/// les deux montre l'effet du courant (et du vent).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drift {
    /// Route fond vraie (rad).
    pub cog_rad: f64,
    /// Cap vrai, direction de l'étrave (rad).
    pub heading_rad: f64,
    /// Vitesse fond (m/s), qui donne la longueur projetée des deux lignes.
    pub sog_mps: f64,
    /// Angle de dérive `COG − cap` (rad, dans ]−π, π]) : positif quand le
    /// bateau est porté sur tribord.
    pub angle_rad: f64,
}

/// Dérive du bateau. `None` sans route fond, sans cap, ou sous
/// [`MIN_VECTOR_SPEED_MPS`] (la route fond n'a alors pas de sens).
#[must_use]
pub fn drift(
    cog_rad: Option<f64>,
    heading_rad: Option<f64>,
    sog_mps: Option<f64>,
) -> Option<Drift> {
    let (cog, heading, sog) = (cog_rad?, heading_rad?, sog_mps?);
    if !cog.is_finite() || !heading.is_finite() || !sog.is_finite() || sog < MIN_VECTOR_SPEED_MPS {
        return None;
    }
    Some(Drift {
        cog_rad: normalize_angle_rad(cog),
        heading_rad: normalize_angle_rad(heading),
        sog_mps: sog,
        angle_rad: angle_diff(cog, heading),
    })
}

/// Vitesse utile (VMG) dans la direction `direction_rad` : projection de
/// la vitesse fond sur cette direction (négative si l'on s'en éloigne).
#[must_use]
pub fn vmg(cog_rad: Option<f64>, sog_mps: Option<f64>, direction_rad: Option<f64>) -> Option<f64> {
    let (cog, sog, dir) = (cog_rad?, sog_mps?, direction_rad?);
    let v = sog * angle_diff(cog, dir).cos();
    v.is_finite().then_some(v)
}

/// Laylines vers une marque, selon la direction du vent réel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Laylines {
    /// Marque visée.
    pub mark: GeoPosition,
    /// Extrémité de la layline tribord amures (le bateau la rejoint puis
    /// file vers la marque tribord amures).
    pub starboard_end: GeoPosition,
    /// Extrémité de la layline bâbord amures.
    pub port_end: GeoPosition,
    /// La marque est au vent (près, virements) ; sinon sous le vent
    /// (portant, empannages).
    pub upwind: bool,
    /// Angle au vent réel retenu (rad).
    pub wind_angle_rad: f64,
}

/// Laylines d'une marque : droites passant par la marque, que le bateau
/// rejoint pour l'atteindre au près (`tack_angle_rad`) si elle est au
/// vent, ou au portant (`gybe_angle_rad`) si elle est sous le vent.
/// `length_m` : longueur tracée (bornée à [`MAX_LAYLINE_M`]).
#[must_use]
pub fn laylines(
    boat: GeoPosition,
    mark: GeoPosition,
    twd_rad: Option<f64>,
    tack_angle_rad: f64,
    gybe_angle_rad: f64,
    length_m: f64,
) -> Option<Laylines> {
    let twd = twd_rad?;
    if !twd.is_finite() || !length_m.is_finite() || length_m <= 0.0 {
        return None;
    }
    let bearing = nav_geodesy::inverse(boat, mark).initial_bearing_rad?;
    // Marque au vent : le relèvement est à moins de 90° du vent.
    let upwind = angle_diff(bearing, twd).abs() < FRAC_PI_2;
    let angle = if upwind {
        tack_angle_rad
    } else {
        gybe_angle_rad
    };
    // Routes du bateau sur chaque amure vers la marque ; la layline part
    // de la marque dans la direction opposée.
    let starboard_course = twd - angle;
    let port_course = twd + angle;
    let length = length_m.min(MAX_LAYLINE_M);
    Some(Laylines {
        mark,
        starboard_end: nav_geodesy::destination(
            mark,
            normalize_angle_rad(starboard_course + PI),
            length,
        )?,
        port_end: nav_geodesy::destination(mark, normalize_angle_rad(port_course + PI), length)?,
        upwind,
        wind_angle_rad: angle,
    })
}

/// Route fond sur l'autre amure (virement ou empannage), symétrique de la
/// route courante par rapport au vent réel.
#[must_use]
pub fn other_tack_course(cog_rad: Option<f64>, twd_rad: Option<f64>) -> Option<f64> {
    let (cog, twd) = (cog_rad?, twd_rad?);
    let c = normalize_angle_rad(2.0 * twd - cog);
    c.is_finite().then_some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    fn deg(d: f64) -> f64 {
        d.to_radians()
    }

    fn bearing(from: GeoPosition, to: GeoPosition) -> f64 {
        nav_geodesy::inverse(from, to)
            .initial_bearing_rad
            .expect("relèvement")
            .to_degrees()
    }

    #[test]
    fn drift_gives_signed_angle() {
        let d = drift(Some(deg(5.0)), Some(deg(350.0)), Some(3.0)).expect("dérive");
        assert!((d.angle_rad - deg(15.0)).abs() < 1e-9, "portée sur tribord");
        assert!((d.cog_rad - deg(5.0)).abs() < 1e-9);
        assert!((d.heading_rad - deg(350.0)).abs() < 1e-9);
        assert!((d.sog_mps - 3.0).abs() < 1e-12);
        let port = drift(Some(deg(80.0)), Some(deg(90.0)), Some(3.0)).expect("dérive");
        assert!(
            (port.angle_rad + deg(10.0)).abs() < 1e-9,
            "portée sur bâbord"
        );
        let wrapped = drift(Some(deg(-10.0)), Some(deg(370.0)), Some(3.0)).expect("dérive");
        assert!((wrapped.cog_rad - deg(350.0)).abs() < 1e-9);
        assert!((wrapped.angle_rad + deg(20.0)).abs() < 1e-9);
    }

    #[test]
    fn drift_needs_cog_heading_and_speed() {
        assert_eq!(drift(None, Some(0.0), Some(3.0)), None);
        assert_eq!(drift(Some(0.0), None, Some(3.0)), None);
        assert_eq!(drift(Some(0.0), Some(0.0), None), None);
        assert_eq!(drift(Some(0.0), Some(0.0), Some(0.05)), None, "à l'arrêt");
        assert_eq!(drift(Some(f64::NAN), Some(0.0), Some(3.0)), None);
        assert_eq!(drift(Some(0.0), Some(0.0), Some(f64::INFINITY)), None);
    }

    #[test]
    fn angle_diff_wraps() {
        assert!((angle_diff(deg(10.0), deg(350.0)) - deg(20.0)).abs() < 1e-9);
        assert!((angle_diff(deg(350.0), deg(10.0)) + deg(20.0)).abs() < 1e-9);
        assert!((angle_diff(deg(180.0), 0.0).abs() - PI).abs() < 1e-9);
    }

    #[test]
    fn course_vector_needs_motion_and_course() {
        let o = p(0.0, 0.0);
        let end = course_vector(o, Some(0.0), Some(5.0), 360.0).expect("vecteur");
        let d = nav_geodesy::distance_m(o, end);
        assert!((d - 1_800.0).abs() < 0.5, "{d}");
        assert!(bearing(o, end).abs() < 1e-6);
        assert_eq!(course_vector(o, None, Some(5.0), 360.0), None);
        assert_eq!(course_vector(o, Some(0.0), None, 360.0), None);
        assert_eq!(
            course_vector(o, Some(0.0), Some(0.05), 360.0),
            None,
            "arrêt"
        );
        assert_eq!(course_vector(o, Some(0.0), Some(f64::NAN), 360.0), None);
        assert_eq!(course_vector(o, Some(0.0), Some(5.0), 0.0), None);
    }

    #[test]
    fn heading_line_has_a_minimum_length() {
        let o = p(0.0, 0.0);
        let still = heading_line(o, Some(deg(90.0)), None, 360.0).expect("ligne");
        assert!((nav_geodesy::distance_m(o, still) - MIN_HEADING_LINE_M).abs() < 0.5);
        assert!((bearing(o, still) - 90.0).abs() < 1e-6);
        let moving = heading_line(o, Some(0.0), Some(5.0), 360.0).expect("ligne");
        assert!((nav_geodesy::distance_m(o, moving) - 1_800.0).abs() < 0.5);
        assert_eq!(heading_line(o, None, Some(5.0), 360.0), None);
    }

    #[test]
    fn vmg_projects_speed() {
        let v = vmg(Some(deg(45.0)), Some(6.0), Some(0.0)).expect("vmg");
        assert!((v - 6.0 * deg(45.0).cos()).abs() < 1e-9);
        let away = vmg(Some(deg(180.0)), Some(6.0), Some(0.0)).expect("vmg");
        assert!((away + 6.0).abs() < 1e-9);
        assert_eq!(vmg(None, Some(6.0), Some(0.0)), None);
        assert_eq!(vmg(Some(0.0), Some(6.0), None), None);
        assert_eq!(vmg(Some(0.0), Some(f64::INFINITY), Some(0.0)), None);
    }

    #[test]
    fn upwind_laylines_follow_the_tack_angle() {
        // Vent du nord, marque au nord du bateau : marque au vent.
        let boat = p(0.0, 0.0);
        let mark = p(0.1, 0.0);
        let l = laylines(boat, mark, Some(0.0), deg(45.0), deg(150.0), 10_000.0).expect("laylines");
        assert!(l.upwind);
        // Tribord amures : route 315°, layline partant de la marque vers 135°.
        assert!((bearing(mark, l.starboard_end) - 135.0).abs() < 0.01);
        // Bâbord amures : route 45°, layline vers 225°.
        assert!((bearing(mark, l.port_end) - 225.0).abs() < 0.01);
        assert!((nav_geodesy::distance_m(mark, l.port_end) - 10_000.0).abs() < 0.5);
    }

    #[test]
    fn downwind_laylines_use_the_gybe_angle_and_bounded_length() {
        // Vent du nord, marque au sud : portant.
        let boat = p(0.0, 0.0);
        let mark = p(-0.1, 0.0);
        let l = laylines(boat, mark, Some(0.0), deg(45.0), deg(150.0), 1e9).expect("laylines");
        assert!(!l.upwind);
        assert!((l.wind_angle_rad - deg(150.0)).abs() < 1e-12);
        // Tribord : route −150° = 210°, layline vers 30°.
        assert!((bearing(mark, l.starboard_end) - 30.0).abs() < 0.01);
        assert!((bearing(mark, l.port_end) - 330.0).abs() < 0.01);
        assert!((nav_geodesy::distance_m(mark, l.port_end) - MAX_LAYLINE_M).abs() < 1.0);
    }

    #[test]
    fn laylines_need_wind_and_distinct_points() {
        let b = p(0.0, 0.0);
        let m = p(0.1, 0.0);
        assert_eq!(laylines(b, m, None, 0.7, 2.6, 1e4), None);
        assert_eq!(laylines(b, b, Some(0.0), 0.7, 2.6, 1e4), None);
        assert_eq!(laylines(b, m, Some(f64::NAN), 0.7, 2.6, 1e4), None);
        assert_eq!(laylines(b, m, Some(0.0), 0.7, 2.6, -1.0), None);
    }

    #[test]
    fn other_tack_mirrors_around_the_wind() {
        let c = other_tack_course(Some(deg(45.0)), Some(0.0)).expect("route");
        assert!((c.to_degrees() - 315.0).abs() < 1e-9);
        let c = other_tack_course(Some(deg(200.0)), Some(deg(10.0))).expect("route");
        assert!((c.to_degrees() - 180.0).abs() < 1e-9);
        assert_eq!(other_tack_course(None, Some(0.0)), None);
    }
}
