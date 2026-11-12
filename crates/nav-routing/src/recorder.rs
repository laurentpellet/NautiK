//! Enregistrement de trace : sélection des relevés à conserver.

use chrono::{DateTime, Utc};
use nav_core::PositionFix;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::TrackPoint;

/// Erreur de configuration de l'enregistreur.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RecorderError {
    /// Intervalle hors de [1, 3600] s.
    #[error("intervalle d'enregistrement invalide : {0} s")]
    InvalidInterval(u32),
    /// Distance minimale non finie ou hors de [0, 10 000] m.
    #[error("distance minimale invalide : {0} m")]
    InvalidDistance(f64),
}

/// Fréquence d'enregistrement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecorderConfig {
    /// Intervalle minimal entre deux points, en secondes.
    pub min_interval_s: u32,
    /// Déplacement minimal entre deux points, en mètres.
    pub min_distance_m: f64,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            min_interval_s: 5,
            min_distance_m: 10.0,
        }
    }
}

impl RecorderConfig {
    /// Valide la configuration.
    ///
    /// # Errors
    ///
    /// [`RecorderError`] si une valeur est hors limites.
    pub fn validate(self) -> Result<Self, RecorderError> {
        if !(1..=3_600).contains(&self.min_interval_s) {
            return Err(RecorderError::InvalidInterval(self.min_interval_s));
        }
        if !self.min_distance_m.is_finite() || !(0.0..=10_000.0).contains(&self.min_distance_m) {
            return Err(RecorderError::InvalidDistance(self.min_distance_m));
        }
        Ok(self)
    }
}

/// Décide quels relevés enregistrer dans une trace.
#[derive(Debug, Clone)]
pub struct TrackRecorder {
    config: RecorderConfig,
    last: Option<TrackPoint>,
}

impl TrackRecorder {
    /// Crée un enregistreur.
    ///
    /// # Errors
    ///
    /// [`RecorderError`] si la configuration est invalide.
    pub fn new(config: RecorderConfig) -> Result<Self, RecorderError> {
        Ok(Self {
            config: config.validate()?,
            last: None,
        })
    }

    /// Dernier point retenu.
    #[must_use]
    pub fn last_point(&self) -> Option<&TrackPoint> {
        self.last.as_ref()
    }

    /// Examine un relevé ; retourne le point à enregistrer s'il est
    /// retenu. Le premier relevé est toujours retenu ; les suivants le
    /// sont si l'intervalle ET le déplacement minimaux sont atteints. Les
    /// relevés non chronologiques sont ignorés.
    pub fn observe(&mut self, fix: &PositionFix) -> Option<TrackPoint> {
        let point = TrackPoint {
            timestamp: fix.measured_at,
            position: fix.position,
            sog_mps: fix.sog_mps.filter(|v| v.is_finite()),
            cog_rad: fix.cog_rad.filter(|v| v.is_finite()),
            depth_m: None,
        };
        let keep = match &self.last {
            None => true,
            Some(last) => self.is_due(last, point.timestamp, &point),
        };
        if keep {
            self.last = Some(point);
            Some(point)
        } else {
            None
        }
    }

    fn is_due(&self, last: &TrackPoint, at: DateTime<Utc>, point: &TrackPoint) -> bool {
        let elapsed = at - last.timestamp;
        if elapsed.num_milliseconds() < i64::from(self.config.min_interval_s) * 1_000 {
            return false;
        }
        nav_geodesy::distance_m(last.position, point.position) >= self.config.min_distance_m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use nav_core::{GeoPosition, SourceKind};

    fn fix(seconds: i64, lat: f64) -> PositionFix {
        let t0 = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t0");
        PositionFix {
            position: GeoPosition::new(lat, 0.0).expect("pos"),
            cog_rad: Some(0.0),
            sog_mps: Some(2.0),
            source: SourceKind::Simulator,
            source_id: "sim".to_owned(),
            measured_at: t0 + Duration::seconds(seconds),
            received_at: t0 + Duration::seconds(seconds),
        }
    }

    #[test]
    fn validates_configuration() {
        assert!(RecorderConfig::default().validate().is_ok());
        let bad_interval = RecorderConfig {
            min_interval_s: 0,
            ..RecorderConfig::default()
        };
        assert_eq!(
            bad_interval.validate(),
            Err(RecorderError::InvalidInterval(0))
        );
        for d in [-1.0, f64::NAN, 20_000.0] {
            let bad = RecorderConfig {
                min_distance_m: d,
                ..RecorderConfig::default()
            };
            assert!(TrackRecorder::new(bad).is_err());
        }
    }

    #[test]
    fn records_first_fix() {
        let mut r = TrackRecorder::new(RecorderConfig::default()).expect("rec");
        assert!(r.observe(&fix(0, 0.0)).is_some());
    }

    #[test]
    fn requires_interval_and_distance() {
        let mut r = TrackRecorder::new(RecorderConfig {
            min_interval_s: 5,
            min_distance_m: 10.0,
        })
        .expect("rec");
        r.observe(&fix(0, 0.0));
        // 1 s plus tard, 111 m : trop tôt.
        assert!(r.observe(&fix(1, 0.001)).is_none());
        // 6 s plus tard, sans déplacement : ignoré.
        assert!(r.observe(&fix(6, 0.0)).is_none());
        // 7 s plus tard, 111 m : retenu.
        let p = r.observe(&fix(7, 0.001)).expect("point");
        assert_eq!(p.sog_mps, Some(2.0));
        assert_eq!(r.last_point(), Some(&p));
    }

    #[test]
    fn stationary_boat_records_single_point() {
        let mut r = TrackRecorder::new(RecorderConfig::default()).expect("rec");
        let kept = (0..100)
            .filter(|s| r.observe(&fix(*s, 0.0)).is_some())
            .count();
        assert_eq!(kept, 1);
    }

    #[test]
    fn ignores_non_chronological_fixes() {
        let mut r = TrackRecorder::new(RecorderConfig::default()).expect("rec");
        r.observe(&fix(100, 0.0));
        assert!(r.observe(&fix(50, 1.0)).is_none());
        assert!(r.observe(&fix(100, 1.0)).is_none());
    }

    #[test]
    fn drops_non_finite_speed_and_course() {
        let mut r = TrackRecorder::new(RecorderConfig::default()).expect("rec");
        let mut f = fix(0, 0.0);
        f.sog_mps = Some(f64::NAN);
        f.cog_rad = Some(f64::INFINITY);
        let p = r.observe(&f).expect("point");
        assert_eq!(p.sog_mps, None);
        assert_eq!(p.cog_rad, None);
    }
}
