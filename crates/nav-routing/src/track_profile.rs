//! Profil d'une trace pour sa relecture : statistiques (distance, durées,
//! vitesses) et vitesse de chaque point, pour la colorer et la rejouer.
//!
//! La vitesse d'un point est la vitesse fond enregistrée ; à défaut, la
//! vitesse moyenne du segment qui y mène (distance / durée). Un segment
//! sans durée n'a pas de vitesse : aucune valeur n'est inventée.

use chrono::{DateTime, Utc};
use nav_core::GeoPosition;
use serde::Serialize;

use crate::model::Track;

/// Vitesse (m/s, ≈ 0,5 nœud) au-dessus de laquelle le bateau fait route.
pub const MOVING_SPEED_MPS: f64 = 0.25;
/// Nombre maximal de points renvoyés (les autres sont sautés
/// régulièrement, le premier et le dernier toujours gardés).
pub const MAX_PROFILE_POINTS: usize = 20_000;

/// Point de la trace relue.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePoint {
    /// Horodatage UTC.
    pub at: DateTime<Utc>,
    /// Position.
    pub position: GeoPosition,
    /// Vitesse (m/s), enregistrée ou déduite du segment.
    pub speed_mps: Option<f64>,
    /// Profondeur enregistrée (m).
    pub depth_m: Option<f64>,
    /// Distance parcourue depuis le début (m).
    pub distance_m: f64,
}

/// Statistiques d'une trace.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackStats {
    /// Distance parcourue (m).
    pub distance_m: f64,
    /// Durée entre le premier et le dernier point (s).
    pub duration_s: f64,
    /// Durée en route, vitesse ≥ [`MOVING_SPEED_MPS`] (s).
    pub moving_s: f64,
    /// Vitesse moyenne sur la durée totale (m/s).
    pub average_mps: Option<f64>,
    /// Vitesse moyenne en route (m/s).
    pub moving_average_mps: Option<f64>,
    /// Vitesse maximale (m/s).
    pub max_mps: Option<f64>,
    /// Profondeur minimale enregistrée (m).
    pub min_depth_m: Option<f64>,
}

/// Profil complet.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackProfile {
    /// Statistiques.
    pub stats: TrackStats,
    /// Points (au plus [`MAX_PROFILE_POINTS`]).
    pub points: Vec<ProfilePoint>,
}

fn max_opt(a: Option<f64>, b: f64) -> Option<f64> {
    Some(a.map_or(b, |a| a.max(b)))
}

fn min_opt(a: Option<f64>, b: f64) -> Option<f64> {
    Some(a.map_or(b, |a| a.min(b)))
}

/// Calcule le profil d'une trace.
#[must_use]
pub fn profile(track: &Track) -> TrackProfile {
    let mut points = Vec::with_capacity(track.points.len());
    let mut distance = 0.0;
    let mut moving_s = 0.0;
    let mut moving_m = 0.0;
    let mut max_mps: Option<f64> = None;
    let mut min_depth: Option<f64> = None;
    let mut previous: Option<&crate::model::TrackPoint> = None;
    for p in &track.points {
        let mut segment_speed = None;
        if let Some(prev) = previous {
            let d = nav_geodesy::distance_m(prev.position, p.position);
            let dt = (p.timestamp - prev.timestamp).num_milliseconds() as f64 / 1000.0;
            distance += d;
            if dt > 0.0 {
                let v = d / dt;
                segment_speed = Some(v);
                if v >= MOVING_SPEED_MPS {
                    moving_s += dt;
                    moving_m += d;
                }
            }
        }
        let speed = p
            .sog_mps
            .filter(|v| v.is_finite() && *v >= 0.0)
            .or(segment_speed);
        if let Some(v) = speed {
            max_mps = max_opt(max_mps, v);
        }
        if let Some(depth) = p.depth_m.filter(|d| d.is_finite()) {
            min_depth = min_opt(min_depth, depth);
        }
        points.push(ProfilePoint {
            at: p.timestamp,
            position: p.position,
            speed_mps: speed,
            depth_m: p.depth_m,
            distance_m: distance,
        });
        previous = Some(p);
    }
    let duration_s = match (track.points.first(), track.points.last()) {
        (Some(a), Some(b)) => {
            ((b.timestamp - a.timestamp).num_milliseconds() as f64 / 1000.0).max(0.0)
        }
        _ => 0.0,
    };
    let stats = TrackStats {
        distance_m: distance,
        duration_s,
        moving_s,
        average_mps: (duration_s > 0.0).then(|| distance / duration_s),
        moving_average_mps: (moving_s > 0.0).then(|| moving_m / moving_s),
        max_mps,
        min_depth_m: min_depth,
    };
    TrackProfile {
        stats,
        points: thin(points),
    }
}

/// Garde au plus [`MAX_PROFILE_POINTS`] points, régulièrement espacés.
fn thin(points: Vec<ProfilePoint>) -> Vec<ProfilePoint> {
    let n = points.len();
    if n <= MAX_PROFILE_POINTS {
        return points;
    }
    let step = n as f64 / (MAX_PROFILE_POINTS - 1) as f64;
    let mut out: Vec<ProfilePoint> = (0..MAX_PROFILE_POINTS - 1)
        .filter_map(|i| points.get((i as f64 * step) as usize).copied())
        .collect();
    if let Some(last) = points.last() {
        out.push(*last);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TrackPoint;
    use chrono::Duration;
    use uuid::Uuid;

    fn t(s: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t") + Duration::seconds(s)
    }

    fn point(s: i64, lat: f64, sog: Option<f64>, depth: Option<f64>) -> TrackPoint {
        TrackPoint {
            timestamp: t(s),
            position: GeoPosition::new(lat, 0.0).expect("p"),
            sog_mps: sog,
            cog_rad: None,
            depth_m: depth,
        }
    }

    fn track(points: Vec<TrackPoint>) -> Track {
        Track {
            id: Uuid::new_v4(),
            name: "T".to_owned(),
            started_at: t(0),
            ended_at: None,
            points,
        }
    }

    #[test]
    fn stats_use_recorded_speed_or_segment_speed() {
        // ~1 105,7 m par 0,01° de latitude.
        let p = profile(&track(vec![
            point(0, 0.0, None, Some(8.0)),
            point(200, 0.01, None, Some(5.5)),
            point(400, 0.01, Some(0.0), None),
            point(500, 0.02, Some(9.0), None),
        ]));
        let s = p.stats;
        assert!(
            (s.distance_m - 2.0 * 1105.7).abs() < 2.0,
            "{}",
            s.distance_m
        );
        assert_eq!(s.duration_s, 500.0);
        assert_eq!(s.moving_s, 300.0, "arrêt entre 200 et 400 s");
        assert!((s.moving_average_mps.expect("moy") - 2211.4 / 300.0).abs() < 0.05);
        assert_eq!(s.max_mps, Some(9.0), "SOG enregistrée prioritaire");
        assert_eq!(s.min_depth_m, Some(5.5));
        assert_eq!(p.points[0].speed_mps, None, "premier point sans vitesse");
        assert!((p.points[1].speed_mps.expect("v") - 5.53).abs() < 0.01);
        assert_eq!(p.points[2].speed_mps, Some(0.0));
        assert!((p.points[3].distance_m - s.distance_m).abs() < 1e-9);
    }

    #[test]
    fn degenerate_tracks_have_no_speed() {
        let empty = profile(&track(Vec::new()));
        assert_eq!(empty.stats.distance_m, 0.0);
        assert_eq!(empty.stats.average_mps, None);
        assert!(empty.points.is_empty());
        // Deux points au même instant : pas de vitesse inventée.
        let same = profile(&track(vec![
            point(0, 0.0, None, None),
            point(0, 0.01, None, None),
        ]));
        assert_eq!(same.stats.max_mps, None);
        assert_eq!(same.stats.average_mps, None);
        let bad = profile(&track(vec![point(0, 0.0, Some(f64::NAN), None)]));
        assert_eq!(bad.points[0].speed_mps, None);
    }

    #[test]
    fn long_tracks_are_thinned_keeping_ends() {
        let n = MAX_PROFILE_POINTS + 5_000;
        let pts = (0..n)
            .map(|i| point(i as i64, (i as f64) * 1e-6, None, None))
            .collect();
        let p = profile(&track(pts));
        assert_eq!(p.points.len(), MAX_PROFILE_POINTS);
        assert_eq!(p.points[0].at, t(0));
        assert_eq!(p.points.last().map(|x| x.at), Some(t(n as i64 - 1)));
    }
}
