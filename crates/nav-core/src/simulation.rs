//! Source de position simulée.
//!
//! Utilisée pour le développement et les démonstrations sans
//! récepteur GPS. Chaque relevé produit est marqué
//! [`SourceKind::Simulator`] : il ne doit jamais être présenté comme une
//! position réelle.
//!
//! En phase 1, la position simulée est fixe. Le déplacement simulé
//! (route et vitesse) nécessitera les calculs de `nav-geodesy` (phase 2).

use chrono::{DateTime, Utc};

use crate::position::{GeoPosition, PositionFix, SourceKind};

/// Identifiant de source du simulateur dans les logs.
pub const SIMULATOR_SOURCE_ID: &str = "simulator";

/// Simulateur de position fixe.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticPositionSimulator {
    position: GeoPosition,
}

impl StaticPositionSimulator {
    /// Crée un simulateur produisant toujours la même position.
    #[must_use]
    pub fn new(position: GeoPosition) -> Self {
        Self { position }
    }

    /// Position simulée.
    #[must_use]
    pub fn position(&self) -> GeoPosition {
        self.position
    }

    /// Produit un relevé simulé horodaté à `now`.
    ///
    /// Le bateau simulé est immobile : la vitesse est nulle et la route
    /// fond est inconnue (non définie à vitesse nulle).
    #[must_use]
    pub fn fix_at(&self, now: DateTime<Utc>) -> PositionFix {
        PositionFix {
            position: self.position,
            cog_rad: None,
            sog_mps: Some(0.0),
            source: SourceKind::Simulator,
            source_id: SIMULATOR_SOURCE_ID.to_owned(),
            measured_at: now,
            received_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn simulator() -> StaticPositionSimulator {
        StaticPositionSimulator::new(GeoPosition::new(-17.535, -149.569).expect("valide"))
    }

    #[test]
    fn fix_is_marked_as_simulated() {
        let fix = simulator().fix_at(Utc::now());
        assert_eq!(fix.source, SourceKind::Simulator);
        assert!(fix.source.is_simulated());
        assert_eq!(fix.source_id, SIMULATOR_SOURCE_ID);
    }

    #[test]
    fn fix_uses_given_utc_timestamp() {
        let now = Utc
            .with_ymd_and_hms(2026, 11, 12, 10, 0, 0)
            .single()
            .expect("date");
        let fix = simulator().fix_at(now);
        assert_eq!(fix.measured_at, now);
        assert_eq!(fix.received_at, now);
    }

    #[test]
    fn static_boat_has_zero_speed_and_no_course() {
        let fix = simulator().fix_at(Utc::now());
        assert_eq!(fix.sog_mps, Some(0.0));
        assert_eq!(fix.cog_rad, None);
        assert_eq!(fix.position, simulator().position());
    }

    #[test]
    fn fix_serializes_with_source_and_timestamps() {
        let now = Utc
            .with_ymd_and_hms(2026, 11, 12, 10, 0, 0)
            .single()
            .expect("date");
        let json = serde_json::to_value(simulator().fix_at(now)).expect("sérialisation");
        assert_eq!(json["source"], "simulator");
        assert_eq!(json["measuredAt"], "2026-11-12T10:00:00Z");
        assert!(json["cogRad"].is_null());
        assert_eq!(json["position"]["latitudeDeg"], -17.535);
    }
}
