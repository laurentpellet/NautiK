//! Règles d'alarme : mouillage et perte de position.

use chrono::Duration;
use nav_core::{GeoPosition, PositionStatus};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::machine::Condition;

/// Rayon minimal du cercle d'évitage (m).
pub const MIN_ANCHOR_RADIUS_M: f64 = 10.0;
/// Rayon maximal du cercle d'évitage (m).
pub const MAX_ANCHOR_RADIUS_M: f64 = 2_000.0;
/// Rayon proposé par défaut (m).
pub const DEFAULT_ANCHOR_RADIUS_M: f64 = 50.0;
/// Hystérésis minimale (m) : la précision d'un GPS courant.
pub const MIN_HYSTERESIS_M: f64 = 5.0;
/// Temporisation maximale d'une alarme (s).
pub const MAX_DELAY_S: u32 = 300;
/// Temporisation par défaut de l'alarme de mouillage (s).
pub const DEFAULT_ANCHOR_DELAY_S: u32 = 10;
/// Temporisation par défaut de l'alarme de perte de position (s), qui
/// s'ajoute au délai d'obsolescence de la position.
pub const DEFAULT_POSITION_LOST_DELAY_S: u32 = 5;
/// Durée pendant laquelle la situation doit être redevenue normale
/// avant la résolution (s).
pub const CLEAR_DELAY_S: i64 = 5;

/// Configuration invalide.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RuleError {
    /// Rayon hors bornes.
    #[error("rayon invalide : {MIN_ANCHOR_RADIUS_M} à {MAX_ANCHOR_RADIUS_M} m")]
    Radius,
    /// Hystérésis hors bornes.
    #[error("hystérésis invalide : {MIN_HYSTERESIS_M} m à la moitié du rayon")]
    Hysteresis,
    /// Temporisation hors bornes.
    #[error("temporisation invalide : 0 à {MAX_DELAY_S} s")]
    Delay,
}

fn check_delay(delay_s: u32) -> Result<(), RuleError> {
    if delay_s > MAX_DELAY_S {
        return Err(RuleError::Delay);
    }
    Ok(())
}

/// Surveillance du mouillage : cercle centré sur l'ancre.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorConfig {
    /// Position de l'ancre (WGS84).
    pub position: GeoPosition,
    /// Rayon du cercle d'évitage (m).
    pub radius_m: f64,
    /// Hystérésis (m) : une fois déclenchée, l'alarme ne se résout que
    /// lorsque le bateau est revenu à `radius_m - hysteresis_m`.
    pub hysteresis_m: f64,
    /// Temporisation avant déclenchement (s).
    pub delay_s: u32,
}

impl AnchorConfig {
    /// Configuration validée, hystérésis par défaut (10 % du rayon, au
    /// moins 5 m).
    ///
    /// # Errors
    ///
    /// [`RuleError`] si le rayon ou la temporisation est hors bornes.
    pub fn new(position: GeoPosition, radius_m: f64, delay_s: u32) -> Result<Self, RuleError> {
        let config = Self {
            position,
            radius_m,
            hysteresis_m: (radius_m * 0.1).max(MIN_HYSTERESIS_M),
            delay_s,
        };
        config.validate()?;
        Ok(config)
    }

    /// Vérifie les bornes (configuration relue en base).
    ///
    /// # Errors
    ///
    /// [`RuleError`] si une valeur est hors bornes.
    pub fn validate(&self) -> Result<(), RuleError> {
        if !self.radius_m.is_finite()
            || !(MIN_ANCHOR_RADIUS_M..=MAX_ANCHOR_RADIUS_M).contains(&self.radius_m)
        {
            return Err(RuleError::Radius);
        }
        if !self.hysteresis_m.is_finite()
            || self.hysteresis_m < MIN_HYSTERESIS_M
            || self.hysteresis_m > self.radius_m / 2.0
        {
            return Err(RuleError::Hysteresis);
        }
        check_delay(self.delay_s)
    }

    /// Temporisation.
    #[must_use]
    pub fn delay(&self) -> Duration {
        Duration::seconds(i64::from(self.delay_s))
    }

    /// Distance et relèvement de l'ancre vers le bateau, si la position
    /// est fraîche.
    #[must_use]
    pub fn offset(&self, position: Option<&PositionStatus>) -> Option<(f64, Option<f64>)> {
        let p = position.filter(|p| !p.stale)?;
        let inv = nav_geodesy::inverse(self.position, p.fix.position);
        Some((inv.distance_m, inv.initial_bearing_rad))
    }

    /// Condition : hors du cercle (avec hystérésis si l'alarme est
    /// déjà déclenchée). Inconnue sans position fraîche.
    #[must_use]
    pub fn condition(&self, distance_m: Option<f64>, alerting: bool) -> Condition {
        let Some(d) = distance_m else {
            return Condition::Unknown;
        };
        let limit = if alerting {
            self.radius_m - self.hysteresis_m
        } else {
            self.radius_m
        };
        if d > limit {
            Condition::Triggered
        } else {
            Condition::Clear
        }
    }
}

/// Alarme de perte de position (GPS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionLostConfig {
    /// Temporisation après l'obsolescence de la position (s).
    pub delay_s: u32,
}

impl Default for PositionLostConfig {
    fn default() -> Self {
        Self {
            delay_s: DEFAULT_POSITION_LOST_DELAY_S,
        }
    }
}

impl PositionLostConfig {
    /// Vérifie les bornes.
    ///
    /// # Errors
    ///
    /// [`RuleError::Delay`] si la temporisation est hors bornes.
    pub fn validate(&self) -> Result<(), RuleError> {
        check_delay(self.delay_s)
    }

    /// Temporisation.
    #[must_use]
    pub fn delay(&self) -> Duration {
        Duration::seconds(i64::from(self.delay_s))
    }

    /// Condition : position absente ou obsolète. `armed` : une position
    /// a été reçue depuis le choix de la source, ou le mouillage est
    /// surveillé (une absence de position est alors toujours anormale).
    #[must_use]
    pub fn condition(position: Option<&PositionStatus>, armed: bool) -> Condition {
        match position {
            Some(p) if !p.stale => Condition::Clear,
            _ if armed => Condition::Triggered,
            _ => Condition::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use nav_core::{PositionFix, SourceKind};

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    fn status(position: GeoPosition, stale: bool) -> PositionStatus {
        let t = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t");
        PositionStatus {
            fix: PositionFix {
                position,
                cog_rad: None,
                sog_mps: None,
                source: SourceKind::Simulator,
                source_id: "sim".to_owned(),
                measured_at: t,
                received_at: t,
            },
            age_ms: 0,
            stale,
            fresh_sources: 1,
        }
    }

    #[test]
    fn anchor_config_is_validated() {
        let a = p(-17.5, -149.5);
        let c = AnchorConfig::new(a, 50.0, 10).expect("ok");
        assert_eq!(c.hysteresis_m, 5.0);
        assert_eq!(
            AnchorConfig::new(a, 200.0, 0).expect("ok").hysteresis_m,
            20.0
        );
        assert_eq!(AnchorConfig::new(a, 9.9, 10), Err(RuleError::Radius));
        assert_eq!(AnchorConfig::new(a, 2_001.0, 10), Err(RuleError::Radius));
        assert_eq!(AnchorConfig::new(a, f64::NAN, 10), Err(RuleError::Radius));
        assert_eq!(AnchorConfig::new(a, 50.0, 301), Err(RuleError::Delay));
        let mut bad = c;
        bad.hysteresis_m = 30.0;
        assert_eq!(bad.validate(), Err(RuleError::Hysteresis));
        bad.hysteresis_m = 1.0;
        assert_eq!(bad.validate(), Err(RuleError::Hysteresis));
    }

    #[test]
    fn anchor_condition_uses_hysteresis_once_alerting() {
        let c = AnchorConfig::new(p(0.0, 0.0), 50.0, 10).expect("ok");
        assert_eq!(c.condition(Some(50.0), false), Condition::Clear);
        assert_eq!(c.condition(Some(50.1), false), Condition::Triggered);
        // Déclenchée : retour seulement sous 45 m.
        assert_eq!(c.condition(Some(48.0), true), Condition::Triggered);
        assert_eq!(c.condition(Some(45.0), true), Condition::Clear);
        assert_eq!(c.condition(None, true), Condition::Unknown);
    }

    #[test]
    fn anchor_offset_needs_a_fresh_position() {
        let c = AnchorConfig::new(p(0.0, 0.0), 50.0, 10).expect("ok");
        // ~111 m au nord.
        let fresh = status(p(0.001, 0.0), false);
        let (d, bearing) = c.offset(Some(&fresh)).expect("distance");
        assert!((d - 110.6).abs() < 0.5, "{d}");
        assert!(bearing.expect("relèvement").abs() < 1e-6);
        assert_eq!(c.offset(Some(&status(p(0.001, 0.0), true))), None);
        assert_eq!(c.offset(None), None);
    }

    #[test]
    fn position_lost_condition() {
        let fresh = status(p(0.0, 0.0), false);
        let stale = status(p(0.0, 0.0), true);
        assert_eq!(
            PositionLostConfig::condition(Some(&fresh), false),
            Condition::Clear
        );
        assert_eq!(
            PositionLostConfig::condition(Some(&stale), true),
            Condition::Triggered
        );
        assert_eq!(
            PositionLostConfig::condition(None, true),
            Condition::Triggered
        );
        assert_eq!(
            PositionLostConfig::condition(None, false),
            Condition::Unknown
        );
        assert!(PositionLostConfig { delay_s: 301 }.validate().is_err());
        assert!(PositionLostConfig::default().validate().is_ok());
    }

    #[test]
    fn anchor_config_round_trips_and_rejects_bad_positions() {
        let c = AnchorConfig::new(p(-17.5, -149.5), 80.0, 20).expect("ok");
        let json = serde_json::to_string(&c).expect("json");
        assert_eq!(serde_json::from_str::<AnchorConfig>(&json).expect("lu"), c);
        let bad = json.replace("-17.5", "-97.5");
        assert!(serde_json::from_str::<AnchorConfig>(&bad).is_err());
    }
}
