//! Modèles Waypoint, Route et Track (voir `DATA_MODEL.md`).

use chrono::{DateTime, Utc};
use nav_core::GeoPosition;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Longueur maximale d'un nom (caractères).
pub const MAX_NAME_LEN: usize = 100;
/// Longueur maximale d'une description (caractères).
pub const MAX_DESCRIPTION_LEN: usize = 2_000;
/// Nombre maximal de waypoints dans une route.
pub const MAX_ROUTE_WAYPOINTS: usize = 1_000;

/// Erreur de validation d'un modèle.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidationError {
    /// Nom vide ou trop long.
    #[error("nom invalide : 1 à {MAX_NAME_LEN} caractères attendus")]
    InvalidName,
    /// Description trop longue.
    #[error("description trop longue ({MAX_DESCRIPTION_LEN} caractères maximum)")]
    DescriptionTooLong,
    /// Route trop courte ou trop longue.
    #[error("une route contient de 2 à {MAX_ROUTE_WAYPOINTS} waypoints")]
    InvalidRouteLength,
    /// Index hors limites.
    #[error("index de waypoint hors limites : {0}")]
    IndexOutOfRange(usize),
}

/// Valide et normalise un nom (espaces de bord supprimés).
///
/// # Errors
///
/// [`ValidationError::InvalidName`] si le nom est vide ou trop long.
pub fn validate_name(name: &str) -> Result<String, ValidationError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_NAME_LEN {
        return Err(ValidationError::InvalidName);
    }
    Ok(trimmed.to_owned())
}

/// Valide une description optionnelle (vide → `None`).
///
/// # Errors
///
/// [`ValidationError::DescriptionTooLong`] si elle dépasse la limite.
pub fn validate_description(description: Option<&str>) -> Result<Option<String>, ValidationError> {
    match description.map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) if d.chars().count() > MAX_DESCRIPTION_LEN => {
            Err(ValidationError::DescriptionTooLong)
        }
        other => Ok(other.map(str::to_owned)),
    }
}

/// Point de passage nommé.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Waypoint {
    /// Identifiant.
    pub id: Uuid,
    /// Nom.
    pub name: String,
    /// Position WGS84.
    pub position: GeoPosition,
    /// Description.
    pub description: Option<String>,
    /// Icône (identifiant libre).
    pub icon: Option<String>,
    /// Couleur (`#rrggbb`).
    pub color: Option<String>,
    /// Création (UTC).
    pub created_at: DateTime<Utc>,
    /// Dernière modification (UTC).
    pub updated_at: DateTime<Utc>,
}

impl Waypoint {
    /// Crée un waypoint validé.
    ///
    /// # Errors
    ///
    /// [`ValidationError`] si le nom ou la description sont invalides.
    pub fn new(
        name: &str,
        position: GeoPosition,
        description: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            id: Uuid::new_v4(),
            name: validate_name(name)?,
            position,
            description: validate_description(description)?,
            icon: None,
            color: None,
            created_at: now,
            updated_at: now,
        })
    }
}

/// Route : suite ordonnée de waypoints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    /// Identifiant.
    pub id: Uuid,
    /// Nom.
    pub name: String,
    /// Description.
    pub description: Option<String>,
    /// Waypoints, dans l'ordre de passage.
    pub waypoints: Vec<Waypoint>,
    /// Création (UTC).
    pub created_at: DateTime<Utc>,
    /// Dernière modification (UTC).
    pub updated_at: DateTime<Utc>,
}

/// Point d'une trace.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackPoint {
    /// Horodatage UTC.
    pub timestamp: DateTime<Utc>,
    /// Position WGS84.
    pub position: GeoPosition,
    /// Vitesse fond (m/s).
    pub sog_mps: Option<f64>,
    /// Route fond vraie (rad).
    pub cog_rad: Option<f64>,
    /// Profondeur (m).
    pub depth_m: Option<f64>,
}

/// Trace : historique horodaté des positions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// Identifiant.
    pub id: Uuid,
    /// Nom.
    pub name: String,
    /// Début (UTC).
    pub started_at: DateTime<Utc>,
    /// Fin (UTC), absente pendant l'enregistrement.
    pub ended_at: Option<DateTime<Utc>>,
    /// Points, dans l'ordre chronologique.
    pub points: Vec<TrackPoint>,
}

impl Track {
    /// Longueur parcourue, en mètres.
    #[must_use]
    pub fn length_m(&self) -> f64 {
        self.points
            .windows(2)
            .map(|pair| nav_geodesy::distance_m(pair[0].position, pair[1].position))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(validate_name("  Tiputa  ").as_deref(), Ok("Tiputa"));
        assert_eq!(validate_name("   "), Err(ValidationError::InvalidName));
        assert_eq!(validate_name(""), Err(ValidationError::InvalidName));
        assert!(validate_name(&"é".repeat(MAX_NAME_LEN)).is_ok());
        assert!(validate_name(&"é".repeat(MAX_NAME_LEN + 1)).is_err());
    }

    #[test]
    fn descriptions_are_optional_and_bounded() {
        assert_eq!(validate_description(None), Ok(None));
        assert_eq!(validate_description(Some("  ")), Ok(None));
        assert_eq!(validate_description(Some(" x ")), Ok(Some("x".to_owned())));
        assert_eq!(
            validate_description(Some(&"x".repeat(MAX_DESCRIPTION_LEN + 1))),
            Err(ValidationError::DescriptionTooLong)
        );
    }

    #[test]
    fn waypoint_creation_validates() {
        let pos = GeoPosition::new(-14.968, -147.638).expect("pos");
        let wp = Waypoint::new("WP1", pos, None, Utc::now()).expect("wp");
        assert_eq!(wp.name, "WP1");
        assert!(Waypoint::new(" ", pos, None, Utc::now()).is_err());
    }

    #[test]
    fn track_length_sums_segments() {
        let t0 = Utc::now();
        let point = |lat: f64| TrackPoint {
            timestamp: t0,
            position: GeoPosition::new(lat, 0.0).expect("pos"),
            sog_mps: None,
            cog_rad: None,
            depth_m: None,
        };
        let track = Track {
            id: Uuid::new_v4(),
            name: "t".to_owned(),
            started_at: t0,
            ended_at: None,
            points: vec![point(0.0), point(0.5), point(1.0)],
        };
        assert!((track.length_m() - 110_574.389).abs() < 0.01);
        let empty = Track {
            points: Vec::new(),
            ..track
        };
        assert_eq!(empty.length_m(), 0.0);
    }
}
