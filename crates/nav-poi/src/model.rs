//! Point d'intérêt nautique validé.

use std::fmt;
use std::str::FromStr;

use nav_core::GeoPosition;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Longueur maximale d'un nom ou d'un détail (caractères).
pub const MAX_TEXT_CHARS: usize = 120;
/// Longueur maximale d'une catégorie (caractères).
pub const MAX_CATEGORY_CHARS: usize = 40;

/// Erreur du module.
#[derive(Debug, Error)]
pub enum PoiError {
    /// Réponse illisible ou incomplète.
    #[error("réponse invalide : {0}")]
    InvalidResponse(String),
    /// Trop d'éléments.
    #[error("réponse trop volumineuse")]
    TooLarge,
    /// Région inconnue.
    #[error("région inconnue : « {0} »")]
    UnknownRegion(String),
    /// Erreur réseau ou HTTP.
    #[error("téléchargement impossible : {0}")]
    Http(String),
}

/// Nature d'un point d'intérêt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoiKind {
    /// Archipel.
    Archipelago,
    /// Île (ou atoll).
    Island,
    /// Îlot (motu).
    Islet,
    /// Passe (chenal dans le récif).
    Pass,
    /// Balise (fixe).
    Beacon,
    /// Bouée (flottante).
    Buoy,
    /// Feu.
    Light,
    /// Amer remarquable.
    Landmark,
    /// Mouillage.
    Anchorage,
    /// Port, marina.
    Harbour,
}

impl PoiKind {
    /// Toutes les natures.
    pub const ALL: [Self; 10] = [
        Self::Archipelago,
        Self::Island,
        Self::Islet,
        Self::Pass,
        Self::Beacon,
        Self::Buoy,
        Self::Light,
        Self::Landmark,
        Self::Anchorage,
        Self::Harbour,
    ];

    /// Code stocké (`snake_case`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Archipelago => "archipelago",
            Self::Island => "island",
            Self::Islet => "islet",
            Self::Pass => "pass",
            Self::Beacon => "beacon",
            Self::Buoy => "buoy",
            Self::Light => "light",
            Self::Landmark => "landmark",
            Self::Anchorage => "anchorage",
            Self::Harbour => "harbour",
        }
    }
}

impl fmt::Display for PoiKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl FromStr for PoiKind {
    type Err = PoiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|k| k.code() == s)
            .ok_or_else(|| PoiError::InvalidResponse(format!("nature inconnue : {s}")))
    }
}

/// Point d'intérêt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Poi {
    /// Référence OpenStreetMap (`n123`, `w45`, `r6`).
    pub osm_ref: String,
    /// Nature.
    pub kind: PoiKind,
    /// Nom (absent pour une balise anonyme).
    pub name: Option<String>,
    /// Position WGS84 (centre pour une surface).
    pub position: GeoPosition,
    /// Catégorie (balisage : `port`, `starboard`, `north`…).
    pub category: Option<String>,
    /// Détails lisibles (caractéristiques d'un feu, couleur…).
    pub details: Option<String>,
}

/// Texte nettoyé : caractères de contrôle remplacés, espaces normalisés,
/// longueur bornée ; vide → `None`.
#[must_use]
pub fn clean_text(text: &str, max_chars: usize) -> Option<String> {
    let replaced: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let joined = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.is_empty() {
        return None;
    }
    Some(joined.chars().take(max_chars).collect())
}

/// Catégorie de balisage : minuscules ASCII, chiffres, `_` et `;` seulement.
#[must_use]
pub fn clean_category(category: &str) -> Option<String> {
    let c = category.trim();
    let ok = !c.is_empty()
        && c.chars().count() <= MAX_CATEGORY_CHARS
        && c.chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == ';');
    ok.then(|| c.to_owned())
}

/// Référence OpenStreetMap valide (`n`, `w` ou `r` suivi d'un entier).
#[must_use]
pub fn is_osm_ref(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('n' | 'w' | 'r'))
        && value.len() > 1
        && value.len() <= 21
        && chars.all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip() {
        for kind in PoiKind::ALL {
            assert_eq!(kind.code().parse::<PoiKind>().ok(), Some(kind));
            assert_eq!(
                serde_json::to_value(kind).ok(),
                Some(serde_json::Value::String(kind.code().to_owned()))
            );
        }
        assert!("volcan".parse::<PoiKind>().is_err());
    }

    #[test]
    fn texts_are_cleaned_and_bounded() {
        assert_eq!(
            clean_text("  Passe\tTiputa\n", 120).as_deref(),
            Some("Passe Tiputa")
        );
        assert_eq!(clean_text("a\u{0}b", 120).as_deref(), Some("a b"));
        assert_eq!(clean_text("   ", 120), None);
        assert_eq!(
            clean_text(&"é".repeat(200), 120).map(|t| t.chars().count()),
            Some(120)
        );
    }

    #[test]
    fn categories_are_restricted() {
        assert_eq!(clean_category("starboard").as_deref(), Some("starboard"));
        assert_eq!(clean_category("north;east").as_deref(), Some("north;east"));
        assert_eq!(clean_category("<script>"), None);
        assert_eq!(clean_category(""), None);
        assert_eq!(clean_category(&"a".repeat(41)), None);
    }

    #[test]
    fn osm_refs_are_checked() {
        assert!(is_osm_ref("n123"));
        assert!(is_osm_ref("w1"));
        assert!(is_osm_ref("r42"));
        assert!(!is_osm_ref("n"));
        assert!(!is_osm_ref("x12"));
        assert!(!is_osm_ref("n12a"));
        assert!(!is_osm_ref(""));
    }
}
