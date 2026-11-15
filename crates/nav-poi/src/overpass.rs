//! Requête Overpass (OpenStreetMap) et analyse stricte de sa réponse.
//!
//! Éléments retenus : îles, îlots, atolls et archipels nommés ; passes
//! (détroits nommés ou noms « Passe … ») ; balises, bouées, feux et amers
//! du balisage maritime (`seamark:*`) ; mouillages et ports.
//!
//! Une réponse marquée incomplète par le service (`remark`, par exemple
//! un dépassement de délai) est refusée : des amers manquants ne doivent
//! pas passer pour une absence d'amers.

use std::collections::{HashMap, HashSet};

use nav_core::GeoPosition;
use serde::Deserialize;

use crate::model::{
    clean_category, clean_text, is_osm_ref, Poi, PoiError, PoiKind, MAX_TEXT_CHARS,
};
use crate::regions::PoiRegion;

/// Nombre maximal de points d'intérêt retenus pour une région.
pub const MAX_POIS: usize = 200_000;

/// Délai accordé au service Overpass (s).
const SERVER_TIMEOUT_S: u32 = 180;

/// Requête Overpass QL d'une région (un bloc par rectangle).
#[must_use]
pub fn query_for(region: &PoiRegion) -> String {
    let mut q = format!("[out:json][timeout:{SERVER_TIMEOUT_S}];\n(\n");
    for b in region.boxes {
        let bbox = format!(
            "({:.4},{:.4},{:.4},{:.4})",
            b.south, b.west, b.north, b.east
        );
        for statement in [
            "nwr[\"place\"~\"^(island|islet|atoll|archipelago)$\"][\"name\"]",
            "nwr[\"natural\"=\"strait\"][\"name\"]",
            "nwr[\"name\"~\"^(Passe|Pass|Hoa) \"]",
            "node[\"seamark:type\"~\"^(beacon|buoy)_\"]",
            "node[\"seamark:type\"~\"^(light|light_major|light_minor|landmark)$\"]",
            "nwr[\"seamark:type\"~\"^(anchorage|harbour)$\"]",
        ] {
            q.push_str("  ");
            q.push_str(statement);
            q.push_str(&bbox);
            q.push_str(";\n");
        }
    }
    q.push_str(");\nout center tags qt;\n");
    q
}

#[derive(Debug, Deserialize)]
struct Response {
    #[serde(default)]
    remark: Option<String>,
    elements: Vec<Element>,
}

#[derive(Debug, Deserialize)]
struct Center {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Deserialize)]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    id: u64,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default)]
    center: Option<Center>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

/// Résultat de l'analyse : points retenus et nombre d'éléments écartés
/// (position invalide, nature non reconnue, doublon).
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPois {
    /// Points d'intérêt valides.
    pub pois: Vec<Poi>,
    /// Éléments écartés.
    pub skipped: usize,
}

fn is_pass_name(name: &str) -> bool {
    ["Passe ", "Pass ", "Hoa "]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Nature d'un élément d'après ses étiquettes.
fn classify(tags: &HashMap<String, String>, name: Option<&str>) -> Option<PoiKind> {
    let tag = |k: &str| tags.get(k).map(String::as_str);
    match tag("place") {
        Some("archipelago") => return Some(PoiKind::Archipelago),
        Some("island" | "atoll") => return Some(PoiKind::Island),
        Some("islet") => return Some(PoiKind::Islet),
        _ => {}
    }
    if let Some(seamark) = tag("seamark:type") {
        if seamark.starts_with("beacon_") {
            return Some(PoiKind::Beacon);
        }
        if seamark.starts_with("buoy_") {
            return Some(PoiKind::Buoy);
        }
        match seamark {
            "light" | "light_major" | "light_minor" => return Some(PoiKind::Light),
            "landmark" => return Some(PoiKind::Landmark),
            "anchorage" => return Some(PoiKind::Anchorage),
            "harbour" => return Some(PoiKind::Harbour),
            _ => {}
        }
    }
    if tag("natural") == Some("strait") || name.is_some_and(is_pass_name) {
        return Some(PoiKind::Pass);
    }
    None
}

/// Catégorie de balisage (`seamark:beacon_lateral:category` → `port`…).
fn category(tags: &HashMap<String, String>) -> Option<String> {
    let seamark = tags.get("seamark:type")?;
    let value = tags
        .get(&format!("seamark:{seamark}:category"))
        .cloned()
        // Le type porte la famille : `beacon_cardinal` → `cardinal`.
        .or_else(|| {
            seamark
                .split_once('_')
                .map(|(_, family)| family.to_owned())
                .filter(|f| *f != "major" && *f != "minor")
        })?;
    clean_category(&value)
}

/// Caractéristiques lisibles : feu `Fl(2) G 6s 8M`, sinon couleurs.
fn details(tags: &HashMap<String, String>) -> Option<String> {
    let tag = |k: &str| tags.get(k).map(String::as_str);
    let mut parts = Vec::new();
    if let Some(character) = tag("seamark:light:character") {
        let group = tag("seamark:light:group").map_or(String::new(), |g| format!("({g})"));
        parts.push(format!("{character}{group}"));
        if let Some(colour) = tag("seamark:light:colour") {
            parts.push(light_colour(colour));
        }
        if let Some(period) = tag("seamark:light:period") {
            parts.push(format!("{period}s"));
        }
        if let Some(range) = tag("seamark:light:range") {
            parts.push(format!("{range}M"));
        }
    } else if let Some(seamark) = tag("seamark:type") {
        if let Some(colour) = tag(&format!("seamark:{seamark}:colour")) {
            parts.push(colour.replace(';', "-"));
        }
    }
    clean_text(&parts.join(" "), MAX_TEXT_CHARS)
}

/// Couleur d'un feu en abréviation usuelle (`green` → `G`).
fn light_colour(colour: &str) -> String {
    colour
        .split(';')
        .map(|c| match c {
            "white" => "W",
            "red" => "R",
            "green" => "G",
            "yellow" => "Y",
            "blue" => "Bu",
            "orange" => "Or",
            "violet" => "Vi",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn element_position(e: &Element) -> Option<GeoPosition> {
    let (lat, lon) = match (&e.center, e.lat, e.lon) {
        (Some(c), _, _) => (c.lat, c.lon),
        (None, Some(lat), Some(lon)) => (lat, lon),
        _ => return None,
    };
    GeoPosition::new(lat, lon).ok()
}

/// Analyse une réponse Overpass JSON.
///
/// # Errors
///
/// [`PoiError::InvalidResponse`] si la réponse est illisible ou signalée
/// incomplète par le service, [`PoiError::TooLarge`] au-delà de
/// [`MAX_POIS`] points retenus.
pub fn parse_overpass(body: &[u8]) -> Result<ParsedPois, PoiError> {
    let response: Response = serde_json::from_slice(body)
        .map_err(|e| PoiError::InvalidResponse(format!("JSON : {e}")))?;
    if let Some(remark) = response.remark.as_deref().and_then(|r| clean_text(r, 200)) {
        return Err(PoiError::InvalidResponse(format!(
            "réponse incomplète du service : {remark}"
        )));
    }
    let mut seen = HashSet::new();
    let mut pois = Vec::new();
    let mut skipped = 0;
    for e in &response.elements {
        let prefix = match e.kind.as_str() {
            "node" => 'n',
            "way" => 'w',
            "relation" => 'r',
            _ => {
                skipped += 1;
                continue;
            }
        };
        let osm_ref = format!("{prefix}{}", e.id);
        let name = e
            .tags
            .get("name:fr")
            .or_else(|| e.tags.get("name"))
            .or_else(|| e.tags.get("seamark:name"))
            .and_then(|n| clean_text(n, MAX_TEXT_CHARS));
        let (Some(position), Some(kind)) =
            (element_position(e), classify(&e.tags, name.as_deref()))
        else {
            skipped += 1;
            continue;
        };
        // Îles et passes sans nom : rien à afficher.
        let needs_name = matches!(
            kind,
            PoiKind::Archipelago | PoiKind::Island | PoiKind::Islet | PoiKind::Pass
        );
        if (needs_name && name.is_none()) || !is_osm_ref(&osm_ref) || !seen.insert(osm_ref.clone())
        {
            skipped += 1;
            continue;
        }
        if pois.len() >= MAX_POIS {
            return Err(PoiError::TooLarge);
        }
        pois.push(Poi {
            osm_ref,
            kind,
            name,
            position,
            category: category(&e.tags),
            details: details(&e.tags),
        });
    }
    Ok(ParsedPois { pois, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regions::region;

    fn parse(json: &str) -> ParsedPois {
        parse_overpass(json.as_bytes()).expect("analyse")
    }

    #[test]
    fn query_covers_each_box_and_feature_family() {
        let fidji = region("fidji").expect("région");
        let q = query_for(fidji);
        assert!(q.starts_with("[out:json][timeout:180];"));
        assert!(q.contains("(-21.5000,176.5000,-12.0000,180.0000)"));
        assert!(q.contains("(-21.5000,-180.0000,-15.5000,-178.0000)"));
        assert_eq!(q.matches("seamark:type").count(), 6);
        assert!(q.ends_with("out center tags qt;\n"));
    }

    #[test]
    fn classifies_islands_passes_and_seamarks() {
        let parsed = parse(
            r#"{"elements":[
              {"type":"relation","id":1,"center":{"lat":-14.97,"lon":-147.63},
               "tags":{"place":"island","name":"Rangiroa","name:fr":"Rangiroa"}},
              {"type":"node","id":2,"lat":-14.96,"lon":-147.62,
               "tags":{"place":"islet","name":"Motu Teavatia"}},
              {"type":"node","id":3,"lat":-14.95,"lon":-147.64,
               "tags":{"natural":"strait","name":"Passe de Tiputa"}},
              {"type":"way","id":4,"center":{"lat":-14.97,"lon":-147.7},
               "tags":{"name":"Passe d'Avatoru","natural":"water"}},
              {"type":"node","id":5,"lat":-17.53,"lon":-149.57,
               "tags":{"seamark:type":"beacon_lateral","seamark:beacon_lateral:category":"starboard",
                       "seamark:beacon_lateral:colour":"green"}},
              {"type":"node","id":6,"lat":-17.54,"lon":-149.58,
               "tags":{"seamark:type":"buoy_cardinal","seamark:buoy_cardinal:category":"north",
                       "seamark:buoy_cardinal:colour":"black;yellow"}},
              {"type":"node","id":7,"lat":-17.5,"lon":-149.5,
               "tags":{"seamark:type":"light_minor","seamark:name":"Feu de la Pointe Vénus",
                       "seamark:light:character":"Fl","seamark:light:group":"2",
                       "seamark:light:colour":"white;red","seamark:light:period":"10",
                       "seamark:light:range":"12"}},
              {"type":"way","id":8,"center":{"lat":-17.53,"lon":-149.56},
               "tags":{"seamark:type":"harbour","name":"Port de Papeete"}},
              {"type":"node","id":9,"lat":-17.5,"lon":-149.8,
               "tags":{"seamark:type":"anchorage"}},
              {"type":"node","id":10,"lat":-17.5,"lon":-149.8,
               "tags":{"seamark:type":"beacon_cardinal"}}
            ]}"#,
        );
        assert_eq!(parsed.skipped, 0);
        let kinds: Vec<_> = parsed.pois.iter().map(|p| p.kind).collect();
        assert_eq!(
            kinds,
            [
                PoiKind::Island,
                PoiKind::Islet,
                PoiKind::Pass,
                PoiKind::Pass,
                PoiKind::Beacon,
                PoiKind::Buoy,
                PoiKind::Light,
                PoiKind::Harbour,
                PoiKind::Anchorage,
                PoiKind::Beacon,
            ]
        );
        let p = &parsed.pois;
        assert_eq!(p[0].osm_ref, "r1");
        assert_eq!(p[0].position.latitude_deg(), -14.97);
        assert_eq!(p[4].category.as_deref(), Some("starboard"));
        assert_eq!(p[4].details.as_deref(), Some("green"));
        assert_eq!(p[4].name, None, "balise anonyme gardée");
        assert_eq!(p[5].details.as_deref(), Some("black-yellow"));
        assert_eq!(p[6].name.as_deref(), Some("Feu de la Pointe Vénus"));
        assert_eq!(p[6].details.as_deref(), Some("Fl(2) WR 10s 12M"));
        assert_eq!(p[6].category, None);
        assert_eq!(p[9].category.as_deref(), Some("cardinal"));
    }

    #[test]
    fn skips_invalid_unnamed_unknown_and_duplicate_elements() {
        let parsed = parse(
            r#"{"elements":[
              {"type":"node","id":1,"lat":95.0,"lon":0.0,"tags":{"place":"island","name":"Hors bornes"}},
              {"type":"node","id":2,"lat":-17.0,"lon":-149.0,"tags":{"place":"island"}},
              {"type":"node","id":3,"lat":-17.0,"lon":-149.0,"tags":{"amenity":"bar","name":"Snack"}},
              {"type":"way","id":4,"tags":{"place":"island","name":"Sans centre"}},
              {"type":"node","id":5,"lat":-17.0,"lon":-149.0,"tags":{"place":"islet","name":"Motu"}},
              {"type":"node","id":5,"lat":-17.0,"lon":-149.0,"tags":{"place":"islet","name":"Motu"}},
              {"type":"area","id":6,"lat":-17.0,"lon":-149.0,"tags":{"place":"islet","name":"Zone"}},
              {"type":"node","id":7,"lat":-17.0,"lon":-149.0,"tags":{"place":"islet","name":"\u0007  "}}
            ]}"#,
        );
        assert_eq!(parsed.pois.len(), 1);
        assert_eq!(parsed.pois[0].name.as_deref(), Some("Motu"));
        assert_eq!(parsed.skipped, 7);
    }

    #[test]
    fn rejects_malformed_or_incomplete_responses() {
        for body in [
            "",
            "{",
            "[]",
            r#"{"elements":"x"}"#,
            r#"{"elements":[{"type":"node","id":-1}]}"#,
            r#"{"elements":[{"type":"node","id":1,"lat":"a"}]}"#,
        ] {
            assert!(
                matches!(
                    parse_overpass(body.as_bytes()),
                    Err(PoiError::InvalidResponse(_))
                ),
                "{body}"
            );
        }
        let err = parse_overpass(
            br#"{"remark":"runtime error: Query timed out in \"query\" at line 3","elements":[]}"#,
        )
        .expect_err("incomplète");
        assert!(err.to_string().contains("incomplète"), "{err}");
    }

    #[test]
    fn names_are_cleaned() {
        let parsed = parse(
            r#"{"elements":[{"type":"node","id":1,"lat":-17,"lon":-149,
                "tags":{"place":"island","name":"  Tahiti\n Nui  "}}]}"#,
        );
        assert_eq!(parsed.pois[0].name.as_deref(), Some("Tahiti Nui"));
    }
}
