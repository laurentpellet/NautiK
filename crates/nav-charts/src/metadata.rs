//! Interprétation et validation des métadonnées MBTiles.
//!
//! Les métadonnées d'un fichier MBTiles sont souvent incomplètes ou
//! approximatives. Les champs optionnels mal formés sont ignorés avec un
//! avertissement ; seuls les défauts empêchant l'affichage (format non
//! supporté, plage de zoom inexploitable) sont des erreurs.

use serde::{Deserialize, Serialize};

use crate::error::ChartError;
use crate::tile::{TileFormat, MAX_ZOOM};

/// Longueur maximale conservée pour les champs texte libres.
const MAX_TEXT_LEN: usize = 2_000;

/// Emprise géographique WGS84 en degrés : ouest, sud, est, nord.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    /// Longitude ouest.
    pub west: f64,
    /// Latitude sud.
    pub south: f64,
    /// Longitude est.
    pub east: f64,
    /// Latitude nord.
    pub north: f64,
}

/// Centre de vue suggéré par la carte.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Center {
    /// Longitude en degrés.
    pub longitude: f64,
    /// Latitude en degrés.
    pub latitude: f64,
    /// Zoom suggéré.
    pub zoom: Option<u8>,
}

/// Faits mesurés directement dans la table `tiles`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileFacts {
    /// Zoom minimal présent.
    pub min_zoom: u8,
    /// Zoom maximal présent.
    pub max_zoom: u8,
    /// Format détecté sur une tuile échantillon.
    pub sniffed_format: Option<TileFormat>,
}

/// Métadonnées validées d'une carte raster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartMetadata {
    /// Nom affiché.
    pub name: String,
    /// Description libre.
    pub description: Option<String>,
    /// Format des tuiles.
    pub format: TileFormat,
    /// Zoom minimal.
    pub min_zoom: u8,
    /// Zoom maximal.
    pub max_zoom: u8,
    /// Emprise géographique.
    pub bounds: Option<Bounds>,
    /// Centre suggéré.
    pub center: Option<Center>,
    /// Attribution (texte brut, à échapper par l'interface).
    pub attribution: Option<String>,
    /// Anomalies non bloquantes relevées dans les métadonnées.
    pub warnings: Vec<String>,
}

/// Interprète les métadonnées brutes d'un MBTiles.
///
/// `entries` contient les couples `(name, value)` de la table `metadata`,
/// `fallback_name` le nom à utiliser à défaut (nom de fichier), `facts`
/// les informations mesurées dans la table `tiles`.
///
/// # Errors
///
/// Retourne [`ChartError::UnsupportedFormat`] pour un format non raster
/// et [`ChartError::InvalidZoomRange`] si aucune plage de zoom
/// exploitable n'est disponible.
pub fn parse_metadata(
    entries: &[(String, Option<String>)],
    fallback_name: &str,
    facts: TileFacts,
) -> Result<ChartMetadata, ChartError> {
    let mut warnings = Vec::new();
    let get = |key: &str| -> Option<&str> {
        entries
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .and_then(|(_, value)| value.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };

    let name = match get("name") {
        Some(name) => truncate(name),
        None => {
            warnings.push("nom absent : nom de fichier utilisé".to_owned());
            fallback_name.to_owned()
        }
    };

    let format = resolve_format(get("format"), facts.sniffed_format, &mut warnings)?;
    let (min_zoom, max_zoom) = resolve_zoom(get("minzoom"), get("maxzoom"), facts, &mut warnings)?;

    let bounds = get("bounds").and_then(|raw| {
        let parsed = parse_bounds(raw);
        if parsed.is_none() {
            warnings.push(format!("emprise invalide ignorée : {raw}"));
        }
        parsed
    });

    let center = get("center").and_then(|raw| {
        let parsed = parse_center(raw);
        if parsed.is_none() {
            warnings.push(format!("centre invalide ignoré : {raw}"));
        }
        parsed
    });

    Ok(ChartMetadata {
        name,
        description: get("description").map(truncate),
        format,
        min_zoom,
        max_zoom,
        bounds,
        center,
        attribution: get("attribution").map(truncate),
        warnings,
    })
}

fn truncate(value: &str) -> String {
    value.chars().take(MAX_TEXT_LEN).collect()
}

fn resolve_format(
    declared: Option<&str>,
    sniffed: Option<TileFormat>,
    warnings: &mut Vec<String>,
) -> Result<TileFormat, ChartError> {
    match (declared.map(TileFormat::from_metadata), sniffed) {
        (Some(Err(unsupported)), _) => Err(ChartError::UnsupportedFormat(unsupported)),
        (Some(Ok(declared)), Some(sniffed)) if declared != sniffed => {
            warnings.push(format!(
                "format déclaré {declared:?} différent du format réel {sniffed:?}"
            ));
            Ok(sniffed)
        }
        (Some(Ok(declared)), _) => Ok(declared),
        (None, Some(sniffed)) => {
            warnings.push("format absent : détecté sur les tuiles".to_owned());
            Ok(sniffed)
        }
        (None, None) => Err(ChartError::UnsupportedFormat("inconnu".to_owned())),
    }
}

fn resolve_zoom(
    declared_min: Option<&str>,
    declared_max: Option<&str>,
    facts: TileFacts,
    warnings: &mut Vec<String>,
) -> Result<(u8, u8), ChartError> {
    let declared = declared_min
        .and_then(parse_zoom)
        .zip(declared_max.and_then(parse_zoom));
    match declared {
        Some((min, max)) if min <= max => Ok((min, max)),
        _ => {
            if declared_min.is_some() || declared_max.is_some() {
                warnings
                    .push("plage de zoom déclarée invalide : zooms des tuiles utilisés".to_owned());
            }
            if facts.min_zoom <= facts.max_zoom && facts.max_zoom <= MAX_ZOOM {
                Ok((facts.min_zoom, facts.max_zoom))
            } else {
                Err(ChartError::InvalidZoomRange(format!(
                    "{}..{}",
                    facts.min_zoom, facts.max_zoom
                )))
            }
        }
    }
}

fn parse_zoom(raw: &str) -> Option<u8> {
    raw.parse::<u8>().ok().filter(|zoom| *zoom <= MAX_ZOOM)
}

fn parse_floats<const N: usize>(raw: &str) -> Option<[f64; N]> {
    let values: Vec<f64> = raw
        .split(',')
        .map(|part| part.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
        .collect::<Option<_>>()?;
    values.try_into().ok()
}

fn valid_longitude(value: f64) -> bool {
    (-180.0..=180.0).contains(&value)
}

fn valid_latitude(value: f64) -> bool {
    (-90.0..=90.0).contains(&value)
}

/// Analyse une emprise `ouest,sud,est,nord`.
#[must_use]
pub fn parse_bounds(raw: &str) -> Option<Bounds> {
    let [west, south, east, north] = parse_floats::<4>(raw)?;
    let valid = valid_longitude(west)
        && valid_longitude(east)
        && valid_latitude(south)
        && valid_latitude(north)
        && west < east
        && south < north;
    valid.then_some(Bounds {
        west,
        south,
        east,
        north,
    })
}

/// Analyse un centre `longitude,latitude[,zoom]`.
#[must_use]
pub fn parse_center(raw: &str) -> Option<Center> {
    let (coords, zoom) = match raw.rsplit_once(',') {
        Some((head, last)) if head.contains(',') => (head, Some(last.trim())),
        _ => (raw, None),
    };
    let [longitude, latitude] = parse_floats::<2>(coords)?;
    if !valid_longitude(longitude) || !valid_latitude(latitude) {
        return None;
    }
    let zoom = match zoom {
        Some(raw_zoom) => Some(
            raw_zoom
                .parse::<f64>()
                .ok()
                .filter(|z| z.is_finite() && *z >= 0.0 && *z <= f64::from(MAX_ZOOM))
                .map(|z| z.floor() as u8)?,
        ),
        None => None,
    };
    Some(Center {
        longitude,
        latitude,
        zoom,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(pairs: &[(&str, &str)]) -> Vec<(String, Option<String>)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), Some((*v).to_owned())))
            .collect()
    }

    fn facts() -> TileFacts {
        TileFacts {
            min_zoom: 2,
            max_zoom: 9,
            sniffed_format: Some(TileFormat::Png),
        }
    }

    #[test]
    fn parses_complete_metadata() {
        let meta = parse_metadata(
            &entries(&[
                ("name", "Tahiti"),
                ("format", "png"),
                ("minzoom", "3"),
                ("maxzoom", "12"),
                ("bounds", "-149.9,-17.9,-149.1,-17.4"),
                ("center", "-149.5,-17.6,10"),
                ("attribution", "SHOM"),
                ("description", "Carte de test"),
            ]),
            "fichier",
            facts(),
        )
        .expect("métadonnées valides");
        assert_eq!(meta.name, "Tahiti");
        assert_eq!(meta.format, TileFormat::Png);
        assert_eq!((meta.min_zoom, meta.max_zoom), (3, 12));
        assert_eq!(
            meta.bounds,
            Some(Bounds {
                west: -149.9,
                south: -17.9,
                east: -149.1,
                north: -17.4
            })
        );
        assert_eq!(meta.center.and_then(|c| c.zoom), Some(10));
        assert_eq!(meta.attribution.as_deref(), Some("SHOM"));
        assert!(meta.warnings.is_empty(), "{:?}", meta.warnings);
    }

    #[test]
    fn missing_metadata_falls_back_on_tiles() {
        let meta = parse_metadata(&[], "fichier", facts()).expect("valide");
        assert_eq!(meta.name, "fichier");
        assert_eq!(meta.format, TileFormat::Png);
        assert_eq!((meta.min_zoom, meta.max_zoom), (2, 9));
        assert!(meta.bounds.is_none());
        assert_eq!(meta.warnings.len(), 2);
    }

    #[test]
    fn null_and_blank_values_are_treated_as_missing() {
        let raw = vec![
            ("name".to_owned(), None),
            ("attribution".to_owned(), Some("   ".to_owned())),
        ];
        let meta = parse_metadata(&raw, "fichier", facts()).expect("valide");
        assert_eq!(meta.name, "fichier");
        assert!(meta.attribution.is_none());
    }

    #[test]
    fn vector_format_is_rejected() {
        let result = parse_metadata(&entries(&[("format", "pbf")]), "f", facts());
        assert!(matches!(result, Err(ChartError::UnsupportedFormat(f)) if f == "pbf"));
    }

    #[test]
    fn unknown_format_without_sample_is_rejected() {
        let no_sample = TileFacts {
            sniffed_format: None,
            ..facts()
        };
        assert!(matches!(
            parse_metadata(&[], "f", no_sample),
            Err(ChartError::UnsupportedFormat(_))
        ));
    }

    #[test]
    fn real_tile_format_wins_over_declared_format() {
        let jpg = TileFacts {
            sniffed_format: Some(TileFormat::Jpg),
            ..facts()
        };
        let meta = parse_metadata(&entries(&[("format", "png")]), "f", jpg).expect("valide");
        assert_eq!(meta.format, TileFormat::Jpg);
        assert_eq!(meta.warnings.len(), 2);
    }

    #[test]
    fn invalid_declared_zoom_uses_tiles() {
        for (min, max) in [("12", "3"), ("abc", "5"), ("3", "99"), ("-1", "5")] {
            let meta = parse_metadata(
                &entries(&[("format", "png"), ("minzoom", min), ("maxzoom", max)]),
                "f",
                facts(),
            )
            .expect("valide");
            assert_eq!((meta.min_zoom, meta.max_zoom), (2, 9), "{min}..{max}");
            assert!(!meta.warnings.is_empty());
        }
    }

    #[test]
    fn unusable_zoom_range_is_an_error() {
        let bad = TileFacts {
            min_zoom: 5,
            max_zoom: 30,
            sniffed_format: Some(TileFormat::Png),
        };
        assert!(matches!(
            parse_metadata(&entries(&[("format", "png")]), "f", bad),
            Err(ChartError::InvalidZoomRange(_))
        ));
    }

    #[test]
    fn invalid_bounds_are_ignored_with_warning() {
        for raw in [
            "1,2,3",
            "a,b,c,d",
            "10,0,5,1",
            "0,10,1,5",
            "-200,0,0,1",
            "0,-95,1,0",
            "NaN,0,1,1",
        ] {
            assert!(parse_bounds(raw).is_none(), "{raw}");
            let meta = parse_metadata(
                &entries(&[("format", "png"), ("bounds", raw)]),
                "f",
                facts(),
            )
            .expect("valide");
            assert!(meta.bounds.is_none());
            assert!(meta.warnings.iter().any(|w| w.contains("emprise")));
        }
    }

    #[test]
    fn parses_center_with_and_without_zoom() {
        assert_eq!(
            parse_center("-149.5,-17.6"),
            Some(Center {
                longitude: -149.5,
                latitude: -17.6,
                zoom: None
            })
        );
        assert_eq!(parse_center("1,2,7.9").and_then(|c| c.zoom), Some(7));
        assert!(parse_center("1").is_none());
        assert!(parse_center("1,2,3,4").is_none());
        assert!(parse_center("200,0,3").is_none());
        assert!(parse_center("1,2,-3").is_none());
        assert!(parse_center("1,2,x").is_none());
    }

    #[test]
    fn long_text_fields_are_truncated() {
        let long = "x".repeat(MAX_TEXT_LEN + 50);
        let meta = parse_metadata(
            &entries(&[("format", "png"), ("attribution", &long)]),
            "f",
            facts(),
        )
        .expect("valide");
        assert_eq!(meta.attribution.map(|a| a.len()), Some(MAX_TEXT_LEN));
    }
}
