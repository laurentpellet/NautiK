//! Positions géographiques et relevés de position.
//!
//! Les coordonnées sont exprimées en degrés décimaux WGS84 (convention
//! cartographique, identique au modèle SQLite). Les autres grandeurs
//! suivent le système SI : radians pour les angles de route, mètres par
//! seconde pour les vitesses.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Erreur de validation d'une position.
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum PositionError {
    /// Latitude non finie ou hors de l'intervalle [-90, 90].
    #[error("latitude invalide : {0}")]
    InvalidLatitude(f64),
    /// Longitude non finie ou hors de l'intervalle [-180, 180].
    #[error("longitude invalide : {0}")]
    InvalidLongitude(f64),
}

/// Position géographique WGS84 validée.
///
/// Une `GeoPosition` ne peut être construite qu'avec des coordonnées
/// finies et dans les bornes WGS84 : aucune position invalide ne peut
/// circuler dans le moteur.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawGeoPosition", into = "RawGeoPosition")]
pub struct GeoPosition {
    latitude_deg: f64,
    longitude_deg: f64,
}

impl GeoPosition {
    /// Construit une position validée.
    ///
    /// # Errors
    ///
    /// Retourne une [`PositionError`] si une coordonnée est non finie ou
    /// hors des bornes WGS84.
    pub fn new(latitude_deg: f64, longitude_deg: f64) -> Result<Self, PositionError> {
        if !latitude_deg.is_finite() || !(-90.0..=90.0).contains(&latitude_deg) {
            return Err(PositionError::InvalidLatitude(latitude_deg));
        }
        if !longitude_deg.is_finite() || !(-180.0..=180.0).contains(&longitude_deg) {
            return Err(PositionError::InvalidLongitude(longitude_deg));
        }
        Ok(Self {
            latitude_deg,
            longitude_deg,
        })
    }

    /// Latitude en degrés décimaux (positive au nord).
    #[must_use]
    pub fn latitude_deg(&self) -> f64 {
        self.latitude_deg
    }

    /// Longitude en degrés décimaux (positive à l'est).
    #[must_use]
    pub fn longitude_deg(&self) -> f64 {
        self.longitude_deg
    }
}

/// Forme sérialisée de [`GeoPosition`], validée à la désérialisation.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGeoPosition {
    latitude_deg: f64,
    longitude_deg: f64,
}

impl TryFrom<RawGeoPosition> for GeoPosition {
    type Error = PositionError;

    fn try_from(raw: RawGeoPosition) -> Result<Self, Self::Error> {
        Self::new(raw.latitude_deg, raw.longitude_deg)
    }
}

impl From<GeoPosition> for RawGeoPosition {
    fn from(position: GeoPosition) -> Self {
        Self {
            latitude_deg: position.latitude_deg,
            longitude_deg: position.longitude_deg,
        }
    }
}

/// Nature de la source d'une donnée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    /// Donnée produite par le simulateur interne. Ne doit jamais être
    /// présentée comme une position réelle.
    Simulator,
    /// Récepteur NMEA 0183.
    Nmea0183,
    /// Bus NMEA 2000.
    Nmea2000,
    /// Serveur Signal K.
    SignalK,
    /// Rejeu d'un enregistrement : données réelles mais passées. Ne doit
    /// jamais être présentée comme la position actuelle.
    Replay,
}

impl SourceKind {
    /// Indique si la source produit des données qui ne décrivent pas la
    /// situation réelle actuelle (simulation ou rejeu).
    #[must_use]
    pub fn is_simulated(self) -> bool {
        matches!(self, Self::Simulator | Self::Replay)
    }
}

/// Relevé de position horodaté, avec sa source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionFix {
    /// Position WGS84.
    pub position: GeoPosition,
    /// Route fond (COG) vraie, en radians [0, 2π), si connue.
    pub cog_rad: Option<f64>,
    /// Vitesse fond (SOG), en mètres par seconde, si connue.
    pub sog_mps: Option<f64>,
    /// Nature de la source.
    pub source: SourceKind,
    /// Identifiant de la source (pour les logs et l'arbitrage).
    pub source_id: String,
    /// Horodatage UTC de la mesure.
    pub measured_at: DateTime<Utc>,
    /// Horodatage UTC de réception par le moteur.
    pub received_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_and_boundary_coordinates() {
        for (lat, lon) in [
            (0.0, 0.0),
            (90.0, 180.0),
            (-90.0, -180.0),
            (-17.535, -149.569),
        ] {
            let p = GeoPosition::new(lat, lon).expect("position valide");
            assert_eq!(p.latitude_deg(), lat);
            assert_eq!(p.longitude_deg(), lon);
        }
    }

    #[test]
    fn rejects_out_of_range_latitude() {
        assert_eq!(
            GeoPosition::new(90.0001, 0.0),
            Err(PositionError::InvalidLatitude(90.0001))
        );
        assert!(GeoPosition::new(-91.0, 0.0).is_err());
    }

    #[test]
    fn rejects_out_of_range_longitude() {
        assert_eq!(
            GeoPosition::new(0.0, 180.5),
            Err(PositionError::InvalidLongitude(180.5))
        );
        assert!(GeoPosition::new(0.0, -181.0).is_err());
    }

    #[test]
    fn rejects_non_finite_coordinates() {
        assert!(GeoPosition::new(f64::NAN, 0.0).is_err());
        assert!(GeoPosition::new(0.0, f64::INFINITY).is_err());
        assert!(GeoPosition::new(f64::NEG_INFINITY, 0.0).is_err());
    }

    #[test]
    fn serializes_in_camel_case() {
        let json = serde_json::to_value(GeoPosition::new(-17.5, -149.5).expect("valide"))
            .expect("sérialisation");
        assert_eq!(json["latitudeDeg"], -17.5);
        assert_eq!(json["longitudeDeg"], -149.5);
    }

    #[test]
    fn deserialization_validates_coordinates() {
        let ok: Result<GeoPosition, _> =
            serde_json::from_str(r#"{"latitudeDeg":10.0,"longitudeDeg":20.0}"#);
        assert!(ok.is_ok());
        let bad: Result<GeoPosition, _> =
            serde_json::from_str(r#"{"latitudeDeg":100.0,"longitudeDeg":20.0}"#);
        assert!(bad.is_err());
        let missing: Result<GeoPosition, _> = serde_json::from_str(r#"{"latitudeDeg":10.0}"#);
        assert!(missing.is_err());
    }

    #[test]
    fn only_simulator_is_simulated() {
        assert!(SourceKind::Simulator.is_simulated());
        assert!(!SourceKind::Nmea0183.is_simulated());
        assert!(!SourceKind::Nmea2000.is_simulated());
        assert!(!SourceKind::SignalK.is_simulated());
    }
}
