//! Marques personnelles : points nommés posés par l'utilisateur (mouillage,
//! danger, plongée…), avec une icône et une note. Indépendantes des
//! routes.

use chrono::{DateTime, Utc};
use nav_core::GeoPosition;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{validate_description, validate_name, ValidationError};

/// Nombre maximal de marques enregistrées.
pub const MAX_MARKS: usize = 10_000;

/// Icône d'une marque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkIcon {
    /// Repère générique.
    #[default]
    Flag,
    /// Mouillage.
    Anchorage,
    /// Danger (roche, épave, haut-fond).
    Danger,
    /// Plongée.
    Dive,
    /// Pêche.
    Fish,
    /// Carburant, avitaillement.
    Fuel,
    /// Information.
    Info,
}

impl MarkIcon {
    /// Toutes les icônes, dans l'ordre d'affichage.
    pub const ALL: [Self; 7] = [
        Self::Flag,
        Self::Anchorage,
        Self::Danger,
        Self::Dive,
        Self::Fish,
        Self::Fuel,
        Self::Info,
    ];

    /// Code stable (base, KML).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Anchorage => "anchorage",
            Self::Danger => "danger",
            Self::Dive => "dive",
            Self::Fish => "fish",
            Self::Fuel => "fuel",
            Self::Info => "info",
        }
    }

    /// Icône correspondant à un code ; inconnu → `None`.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|i| i.code() == code)
    }
}

/// Marque personnelle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mark {
    /// Identifiant.
    pub id: Uuid,
    /// Nom.
    pub name: String,
    /// Position WGS84.
    pub position: GeoPosition,
    /// Icône.
    pub icon: MarkIcon,
    /// Note libre.
    pub note: Option<String>,
    /// Création (UTC).
    pub created_at: DateTime<Utc>,
    /// Dernière modification (UTC).
    pub updated_at: DateTime<Utc>,
}

impl Mark {
    /// Nouvelle marque validée.
    ///
    /// # Errors
    ///
    /// [`ValidationError`] si le nom ou la note est invalide.
    pub fn new(
        name: &str,
        position: GeoPosition,
        icon: MarkIcon,
        note: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            id: Uuid::new_v4(),
            name: validate_name(name)?,
            position,
            icon,
            note: validate_description(note)?,
            created_at: now,
            updated_at: now,
        })
    }

    /// Modifie la marque (validation comprise).
    ///
    /// # Errors
    ///
    /// [`ValidationError`] si le nom ou la note est invalide ; la marque
    /// est alors inchangée.
    pub fn edit(
        &mut self,
        name: &str,
        position: GeoPosition,
        icon: MarkIcon,
        note: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<(), ValidationError> {
        let name = validate_name(name)?;
        let note = validate_description(note)?;
        self.name = name;
        self.position = position;
        self.icon = icon;
        self.note = note;
        self.updated_at = now;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t")
    }

    fn p() -> GeoPosition {
        GeoPosition::new(-17.5, -149.6).expect("pos")
    }

    #[test]
    fn new_mark_is_validated_and_trimmed() {
        let m = Mark::new("  Roche  ", p(), MarkIcon::Danger, Some("  "), now()).expect("marque");
        assert_eq!(m.name, "Roche");
        assert_eq!(m.note, None);
        assert_eq!(m.created_at, m.updated_at);
        assert_eq!(
            Mark::new(" ", p(), MarkIcon::Flag, None, now()),
            Err(ValidationError::InvalidName)
        );
        let long = "x".repeat(3_000);
        assert_eq!(
            Mark::new("A", p(), MarkIcon::Flag, Some(&long), now()),
            Err(ValidationError::DescriptionTooLong)
        );
    }

    #[test]
    fn edit_keeps_the_mark_on_error() {
        let mut m = Mark::new("A", p(), MarkIcon::Flag, None, now()).expect("marque");
        let before = m.clone();
        assert!(m.edit("", p(), MarkIcon::Dive, None, now()).is_err());
        assert_eq!(m, before);
        let later = now() + chrono::Duration::seconds(5);
        m.edit("B", p(), MarkIcon::Dive, Some("tombant"), later)
            .expect("modifiée");
        assert_eq!((m.name.as_str(), m.icon), ("B", MarkIcon::Dive));
        assert_eq!(m.updated_at, later);
    }

    #[test]
    fn icon_codes_round_trip() {
        for i in MarkIcon::ALL {
            assert_eq!(MarkIcon::from_code(i.code()), Some(i));
        }
        assert_eq!(MarkIcon::from_code("x"), None);
        assert_eq!(
            serde_json::to_string(&MarkIcon::Anchorage).expect("json"),
            "\"anchorage\""
        );
    }
}
