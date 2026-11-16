//! Aides à la navigation du bateau, calculées à chaque pas du moteur :
//! vecteur de route, ligne de cap, VMG et laylines.
//!
//! Seules des données fraîches sont utilisées : sans position fraîche,
//! rien n'est calculé ; sans vent réel frais, ni VMG au vent ni laylines.

use chrono::{DateTime, Utc};
use nav_core::{GeoPosition, InstrumentsSnapshot, Quantity};
use nav_routing::sailing::{
    self, Drift, Laylines, DEFAULT_GYBE_ANGLE_RAD, DEFAULT_TACK_ANGLE_RAD, DEFAULT_VECTOR_HORIZON_S,
};
use serde::Serialize;

/// Facteur appliqué à la distance de la marque pour la longueur des
/// laylines tracées.
const LAYLINE_LENGTH_FACTOR: f64 = 1.5;

/// Aides à la navigation du bateau, diffusées périodiquement.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnShipSnapshot {
    /// Instant du calcul.
    pub computed_at: DateTime<Utc>,
    /// Les données sont simulées ou rejouées.
    pub simulated: bool,
    /// Horizon du vecteur de route (s).
    pub vector_horizon_s: f64,
    /// Position du bateau utilisée (fraîche).
    pub position: Option<GeoPosition>,
    /// Extrémité du vecteur de route (COG/SOG).
    pub vector_end: Option<GeoPosition>,
    /// Extrémité de la ligne de cap (cap vrai).
    pub heading_end: Option<GeoPosition>,
    /// Direction du vent réel (rad, d'où il vient).
    pub true_wind_direction_rad: Option<f64>,
    /// VMG au vent (m/s, positive en remontant au vent).
    pub vmg_wind_mps: Option<f64>,
    /// VMG vers le waypoint actif (m/s).
    pub vmg_waypoint_mps: Option<f64>,
    /// Route fond sur l'autre amure (rad).
    pub other_tack_cog_rad: Option<f64>,
    /// Laylines du waypoint actif.
    pub laylines: Option<Laylines>,
    /// Dérive : route fond et ligne de foi, tracées à l'avant du bateau.
    pub drift: Option<Drift>,
}

fn fresh(snapshot: &InstrumentsSnapshot, quantity: Quantity) -> Option<f64> {
    snapshot
        .readings
        .iter()
        .find(|r| r.quantity == quantity && !r.stale)
        .map(|r| r.value)
}

/// Calcule les aides à partir de l'état du bateau et du waypoint actif.
#[must_use]
pub fn compute(
    snapshot: &InstrumentsSnapshot,
    target: Option<GeoPosition>,
    simulated: bool,
) -> OwnShipSnapshot {
    let horizon = DEFAULT_VECTOR_HORIZON_S;
    let mut out = OwnShipSnapshot {
        computed_at: snapshot.computed_at,
        simulated,
        vector_horizon_s: horizon,
        position: None,
        vector_end: None,
        heading_end: None,
        true_wind_direction_rad: None,
        vmg_wind_mps: None,
        vmg_waypoint_mps: None,
        other_tack_cog_rad: None,
        laylines: None,
        drift: None,
    };
    let Some(status) = snapshot.position.as_ref().filter(|p| !p.stale) else {
        return out;
    };
    let fix = &status.fix;
    let here = fix.position;
    let twd = fresh(snapshot, Quantity::WindDirectionTrue);
    let heading = fresh(snapshot, Quantity::HeadingTrue);
    out.position = Some(here);
    out.vector_end = sailing::course_vector(here, fix.cog_rad, fix.sog_mps, horizon);
    out.heading_end = sailing::heading_line(here, heading, fix.sog_mps, horizon);
    out.drift = sailing::drift(fix.cog_rad, heading, fix.sog_mps);
    out.true_wind_direction_rad = twd;
    out.vmg_wind_mps = sailing::vmg(fix.cog_rad, fix.sog_mps, twd);
    out.other_tack_cog_rad = sailing::other_tack_course(fix.cog_rad, twd);
    if let Some(mark) = target {
        let inv = nav_geodesy::inverse(here, mark);
        out.vmg_waypoint_mps = sailing::vmg(fix.cog_rad, fix.sog_mps, inv.initial_bearing_rad);
        out.laylines = sailing::laylines(
            here,
            mark,
            twd,
            DEFAULT_TACK_ANGLE_RAD,
            DEFAULT_GYBE_ANGLE_RAD,
            inv.distance_m * LAYLINE_LENGTH_FACTOR,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use nav_core::{PositionFix, PositionStatus, Reading, SourceKind};

    fn t() -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t")
    }

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    fn reading(quantity: Quantity, value: f64, stale: bool) -> Reading {
        Reading {
            quantity,
            value,
            source: SourceKind::SignalK,
            source_id: "x".to_owned(),
            measured_at: t(),
            age_ms: 0,
            stale,
            fresh_sources: 1,
        }
    }

    fn snapshot(stale: bool, readings: Vec<Reading>) -> InstrumentsSnapshot {
        InstrumentsSnapshot {
            computed_at: t() + Duration::seconds(1),
            position: Some(PositionStatus {
                fix: PositionFix {
                    position: p(0.0, 0.0),
                    cog_rad: Some(45f64.to_radians()),
                    sog_mps: Some(5.0),
                    source: SourceKind::SignalK,
                    source_id: "gps".to_owned(),
                    measured_at: t(),
                    received_at: t(),
                },
                age_ms: 0,
                stale,
                fresh_sources: 1,
            }),
            readings,
        }
    }

    #[test]
    fn fresh_data_give_vectors_vmg_and_laylines() {
        let s = snapshot(
            false,
            vec![
                reading(Quantity::WindDirectionTrue, 0.0, false),
                reading(Quantity::HeadingTrue, 40f64.to_radians(), false),
            ],
        );
        let o = compute(&s, Some(p(0.1, 0.0)), true);
        assert!(o.simulated);
        assert!(o.vector_end.is_some());
        assert!(o.heading_end.is_some());
        let drift = o.drift.expect("dérive");
        assert!((drift.angle_rad - 5f64.to_radians()).abs() < 1e-9);
        let vmg = o.vmg_wind_mps.expect("vmg vent");
        assert!((vmg - 5.0 * 45f64.to_radians().cos()).abs() < 1e-9);
        assert!(o.vmg_waypoint_mps.is_some());
        assert!(o.laylines.expect("laylines").upwind);
        let other = o.other_tack_cog_rad.expect("autre amure").to_degrees();
        assert!((other - 315.0).abs() < 1e-9);
    }

    #[test]
    fn stale_wind_or_heading_are_not_used() {
        let s = snapshot(
            false,
            vec![
                reading(Quantity::WindDirectionTrue, 0.0, true),
                reading(Quantity::HeadingTrue, 0.0, true),
            ],
        );
        let o = compute(&s, Some(p(0.1, 0.0)), false);
        assert!(o.vector_end.is_some(), "COG/SOG du relevé frais");
        assert_eq!(o.heading_end, None);
        assert_eq!(o.drift, None, "sans cap frais");
        assert_eq!(o.vmg_wind_mps, None);
        assert_eq!(o.laylines, None);
        assert!(o.vmg_waypoint_mps.is_some());
    }

    #[test]
    fn stale_position_gives_nothing() {
        let s = snapshot(true, vec![reading(Quantity::WindDirectionTrue, 0.0, false)]);
        let o = compute(&s, Some(p(0.1, 0.0)), false);
        assert_eq!(o.position, None);
        assert_eq!(o.vector_end, None);
        assert_eq!(o.drift, None);
        assert_eq!(o.vmg_wind_mps, None);
        assert_eq!(o.laylines, None);
        let empty = InstrumentsSnapshot {
            computed_at: t(),
            position: None,
            readings: Vec::new(),
        };
        assert_eq!(compute(&empty, None, false).position, None);
    }
}
