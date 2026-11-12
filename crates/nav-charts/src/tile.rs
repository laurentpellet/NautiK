//! Coordonnées de tuiles, formats et analyse des requêtes de tuiles.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Niveau de zoom maximal accepté.
pub const MAX_ZOOM: u8 = 24;

/// Longueur maximale d'un identifiant de carte.
pub const MAX_CHART_ID_LEN: usize = 64;

/// Erreur d'analyse d'une requête de tuile.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TileRequestError {
    /// Chemin mal formé (nombre de segments, caractères invalides…).
    #[error("chemin de tuile mal formé")]
    MalformedPath,
    /// Identifiant de carte invalide.
    #[error("identifiant de carte invalide")]
    InvalidChartId,
    /// Coordonnées hors limites pour le niveau de zoom.
    #[error("coordonnées de tuile hors limites : z={z} x={x} y={y}")]
    OutOfRange {
        /// Zoom demandé.
        z: u32,
        /// Colonne demandée.
        x: u32,
        /// Ligne demandée.
        y: u32,
    },
}

/// Identifiant stable d'une carte dans le catalogue.
///
/// Composé uniquement de `[a-z0-9_-]`, non vide, de longueur limitée :
/// il peut être utilisé sans risque dans une URL.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ChartId(String);

impl ChartId {
    /// Valide un identifiant existant.
    ///
    /// # Errors
    ///
    /// Retourne [`TileRequestError::InvalidChartId`] si l'identifiant est
    /// vide, trop long ou contient des caractères non autorisés.
    pub fn parse(value: &str) -> Result<Self, TileRequestError> {
        let valid = !value.is_empty()
            && value.len() <= MAX_CHART_ID_LEN
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
        if valid {
            Ok(Self(value.to_owned()))
        } else {
            Err(TileRequestError::InvalidChartId)
        }
    }

    /// Dérive un identifiant à partir d'un nom de fichier (sans extension).
    #[must_use]
    pub fn from_file_stem(stem: &str) -> Self {
        let mut id = String::with_capacity(stem.len());
        for c in stem.chars().flat_map(char::to_lowercase) {
            let mapped = if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' {
                c
            } else {
                '-'
            };
            if !(mapped == '-' && (id.is_empty() || id.ends_with('-'))) {
                id.push(mapped);
            }
        }
        let mut id: String = id
            .trim_end_matches('-')
            .chars()
            .take(MAX_CHART_ID_LEN)
            .collect();
        while id.ends_with('-') {
            id.pop();
        }
        if id.is_empty() {
            id.push_str("chart");
        }
        Self(id)
    }

    /// Identifiant sous forme de chaîne.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Retourne une variante suffixée (`id-2`, `id-3`…) pour lever un
    /// conflit d'identifiants.
    #[must_use]
    pub fn with_suffix(&self, n: usize) -> Self {
        let suffix = format!("-{n}");
        let base: String = self
            .0
            .chars()
            .take(MAX_CHART_ID_LEN - suffix.len())
            .collect();
        Self(format!("{base}{suffix}"))
    }
}

impl fmt::Display for ChartId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ChartId {
    type Error = TileRequestError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<ChartId> for String {
    fn from(id: ChartId) -> Self {
        id.0
    }
}

/// Coordonnées d'une tuile dans le schéma XYZ (origine en haut à gauche),
/// celui utilisé par MapLibre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCoord {
    z: u8,
    x: u32,
    y: u32,
}

impl TileCoord {
    /// Construit des coordonnées validées.
    ///
    /// # Errors
    ///
    /// Retourne [`TileRequestError::OutOfRange`] si le zoom dépasse
    /// [`MAX_ZOOM`] ou si `x`/`y` sortent de la grille du niveau.
    pub fn new(z: u32, x: u32, y: u32) -> Result<Self, TileRequestError> {
        let out_of_range = TileRequestError::OutOfRange { z, x, y };
        let zoom = u8::try_from(z).map_err(|_| out_of_range.clone())?;
        if zoom > MAX_ZOOM {
            return Err(out_of_range);
        }
        let size = 1_u32 << zoom;
        if x >= size || y >= size {
            return Err(out_of_range);
        }
        Ok(Self { z: zoom, x, y })
    }

    /// Niveau de zoom.
    #[must_use]
    pub fn z(&self) -> u8 {
        self.z
    }

    /// Colonne.
    #[must_use]
    pub fn x(&self) -> u32 {
        self.x
    }

    /// Ligne (schéma XYZ).
    #[must_use]
    pub fn y(&self) -> u32 {
        self.y
    }

    /// Ligne dans le schéma TMS utilisé par MBTiles (origine en bas).
    #[must_use]
    pub fn tms_row(&self) -> u32 {
        (1_u32 << self.z) - 1 - self.y
    }
}

/// Analyse un chemin de requête de tuile `/{chart_id}/{z}/{x}/{y}`.
///
/// # Errors
///
/// Retourne une [`TileRequestError`] si le chemin est mal formé, si
/// l'identifiant est invalide ou si les coordonnées sont hors limites.
pub fn parse_tile_path(path: &str) -> Result<(ChartId, TileCoord), TileRequestError> {
    let trimmed = path
        .strip_prefix('/')
        .ok_or(TileRequestError::MalformedPath)?;
    let segments: Vec<&str> = trimmed.split('/').collect();
    let [id, z, x, y] = segments.as_slice() else {
        return Err(TileRequestError::MalformedPath);
    };
    let id = ChartId::parse(id)?;
    let coord = TileCoord::new(parse_number(z)?, parse_number(x)?, parse_number(y)?)?;
    Ok((id, coord))
}

/// Analyse un entier décimal strict (chiffres ASCII uniquement).
fn parse_number(segment: &str) -> Result<u32, TileRequestError> {
    if segment.is_empty() || segment.len() > 10 || !segment.bytes().all(|b| b.is_ascii_digit()) {
        return Err(TileRequestError::MalformedPath);
    }
    segment.parse().map_err(|_| TileRequestError::MalformedPath)
}

/// Format d'image d'une tuile raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TileFormat {
    /// PNG.
    Png,
    /// JPEG.
    Jpg,
    /// WebP.
    Webp,
}

impl TileFormat {
    /// Interprète la valeur `format` des métadonnées MBTiles.
    ///
    /// # Errors
    ///
    /// Retourne la valeur brute si le format n'est pas un format raster
    /// supporté (ex. `pbf`).
    pub fn from_metadata(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "png" => Ok(Self::Png),
            "jpg" | "jpeg" => Ok(Self::Jpg),
            "webp" => Ok(Self::Webp),
            other => Err(other.to_owned()),
        }
    }

    /// Détecte le format à partir de la signature des données.
    #[must_use]
    pub fn sniff(data: &[u8]) -> Option<Self> {
        if data.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpg)
        } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
            Some(Self::Webp)
        } else {
            None
        }
    }

    /// Type MIME correspondant.
    #[must_use]
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpg => "image/jpeg",
            Self::Webp => "image/webp",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_id_accepts_safe_identifiers() {
        assert!(ChartId::parse("tahiti_2024-v1").is_ok());
    }

    #[test]
    fn chart_id_rejects_unsafe_identifiers() {
        for bad in ["", "Tahiti", "../etc", "a/b", "a b", "é", &"a".repeat(65)] {
            assert_eq!(
                ChartId::parse(bad),
                Err(TileRequestError::InvalidChartId),
                "{bad}"
            );
        }
    }

    #[test]
    fn chart_id_from_file_stem_is_sanitized() {
        assert_eq!(
            ChartId::from_file_stem("Tahiti Nord (2024)").as_str(),
            "tahiti-nord-2024"
        );
        assert_eq!(ChartId::from_file_stem("--Moorea--").as_str(), "moorea");
        assert_eq!(
            ChartId::from_file_stem("Îles Sous-le-Vent").as_str(),
            "les-sous-le-vent"
        );
        assert_eq!(ChartId::from_file_stem("ÉÉÉ").as_str(), "chart");
        assert_eq!(ChartId::from_file_stem("").as_str(), "chart");
        let long = ChartId::from_file_stem(&"x".repeat(200));
        assert_eq!(long.as_str().len(), MAX_CHART_ID_LEN);
    }

    #[test]
    fn derived_chart_ids_are_always_valid() {
        for stem in [
            "Tahiti Nord",
            "a-b-",
            "___",
            "日本",
            "x".repeat(100).as_str(),
        ] {
            let id = ChartId::from_file_stem(stem);
            assert!(ChartId::parse(id.as_str()).is_ok(), "{stem} -> {id}");
            assert!(ChartId::parse(id.with_suffix(12).as_str()).is_ok());
        }
    }

    #[test]
    fn suffix_respects_max_length() {
        let id = ChartId::from_file_stem(&"x".repeat(100)).with_suffix(3);
        assert_eq!(id.as_str().len(), MAX_CHART_ID_LEN);
        assert!(id.as_str().ends_with("-3"));
    }

    #[test]
    fn tile_coord_converts_xyz_to_tms() {
        let coord = TileCoord::new(3, 2, 1).expect("valide");
        assert_eq!(coord.tms_row(), 6);
        assert_eq!(TileCoord::new(0, 0, 0).expect("valide").tms_row(), 0);
    }

    #[test]
    fn tile_coord_rejects_out_of_range() {
        assert!(TileCoord::new(2, 4, 0).is_err());
        assert!(TileCoord::new(2, 0, 4).is_err());
        assert!(TileCoord::new(u32::from(MAX_ZOOM) + 1, 0, 0).is_err());
        assert!(TileCoord::new(300, 0, 0).is_err());
        assert!(TileCoord::new(u32::from(MAX_ZOOM), (1 << MAX_ZOOM) - 1, 0).is_ok());
    }

    #[test]
    fn parses_valid_tile_path() {
        let (id, coord) = parse_tile_path("/tahiti/5/3/7").expect("valide");
        assert_eq!(id.as_str(), "tahiti");
        assert_eq!((coord.z(), coord.x(), coord.y()), (5, 3, 7));
    }

    #[test]
    fn rejects_malformed_tile_paths() {
        for bad in [
            "",
            "tahiti/1/0/0",
            "/tahiti/1/0",
            "/tahiti/1/0/0/0",
            "/tahiti/1/0/0.png",
            "/tahiti/-1/0/0",
            "/tahiti/+1/0/0",
            "/tahiti/1//0",
            "/tahiti/99999999999/0/0",
            "/tahiti/%31/0/0",
        ] {
            assert_eq!(
                parse_tile_path(bad),
                Err(TileRequestError::MalformedPath),
                "{bad}"
            );
        }
        assert_eq!(
            parse_tile_path("/../1/0/0"),
            Err(TileRequestError::InvalidChartId)
        );
        assert!(matches!(
            parse_tile_path("/tahiti/1/2/0"),
            Err(TileRequestError::OutOfRange { .. })
        ));
    }

    #[test]
    fn tile_format_from_metadata() {
        assert_eq!(TileFormat::from_metadata("PNG"), Ok(TileFormat::Png));
        assert_eq!(TileFormat::from_metadata("jpeg"), Ok(TileFormat::Jpg));
        assert_eq!(TileFormat::from_metadata(" webp "), Ok(TileFormat::Webp));
        assert_eq!(TileFormat::from_metadata("pbf"), Err("pbf".to_owned()));
    }

    #[test]
    fn tile_format_sniffing() {
        assert_eq!(
            TileFormat::sniff(b"\x89PNG\r\n\x1a\nrest"),
            Some(TileFormat::Png)
        );
        assert_eq!(
            TileFormat::sniff(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some(TileFormat::Jpg)
        );
        assert_eq!(
            TileFormat::sniff(b"RIFF\0\0\0\0WEBPVP8 "),
            Some(TileFormat::Webp)
        );
        assert_eq!(TileFormat::sniff(b"RIFF"), None);
        assert_eq!(TileFormat::sniff(b""), None);
        assert_eq!(TileFormat::sniff(b"\x1f\x8b gzip pbf"), None);
    }

    #[test]
    fn mime_types() {
        assert_eq!(TileFormat::Png.mime_type(), "image/png");
        assert_eq!(TileFormat::Jpg.mime_type(), "image/jpeg");
        assert_eq!(TileFormat::Webp.mime_type(), "image/webp");
    }
}
