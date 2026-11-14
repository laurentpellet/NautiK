//! Préparation des objets S-57 pour l'affichage (symbologie simplifiée,
//! inspirée de l'IHO S-52 sans la remplacer).
//!
//! Seules les classes utiles à la sécurité de la navigation sont
//! retenues : terre, zones de profondeur, zones non hydrographiées,
//! isobathes, trait de côte, sondes, dangers (épaves, obstructions,
//! roches), feux, bouées et balises. Chaque objet devient une entité
//! GeoJSON (`[lon, lat]`, WGS 84) dont les propriétés portent la
//! catégorie d'affichage et les valeurs nécessaires au style (profondeurs,
//! couleur, étiquette de sonde).

use serde::Serialize;

use super::catalogue::{attribute_code, object_class_acronym};
use super::{Position, S57Cell, S57Feature, S57Geometry};

/// Catégorie d'affichage simplifiée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayCategory {
    /// Terre (LNDARE).
    Land,
    /// Zone de profondeur (DEPARE, DRGARE).
    Depth,
    /// Zone non hydrographiée (UNSARE).
    Unsurveyed,
    /// Isobathe (DEPCNT).
    Contour,
    /// Trait de côte, ouvrages côtiers (COALNE, SLCONS).
    Coast,
    /// Sonde (SOUNDG).
    Sounding,
    /// Danger : épave, obstruction, roche (WRECKS, OBSTRN, UWTROC).
    Danger,
    /// Feu (LIGHTS).
    Light,
    /// Bouée (BOYxxx).
    Buoy,
    /// Balise (BCNxxx).
    Beacon,
}

/// Catégorie d'affichage d'une classe d'objets ; `None` : non affichée.
pub fn category(object_class: u16) -> Option<DisplayCategory> {
    category_of(object_class_acronym(object_class)?)
}

/// Catégorie d'affichage d'un acronyme de classe S-57 (aussi utilisé par
/// les cartes CM93) ; `None` : non affichée.
pub fn category_of(acronym: &str) -> Option<DisplayCategory> {
    use DisplayCategory as C;
    Some(match acronym {
        "LNDARE" => C::Land,
        "DEPARE" | "DRGARE" => C::Depth,
        "UNSARE" => C::Unsurveyed,
        "DEPCNT" => C::Contour,
        "COALNE" | "SLCONS" => C::Coast,
        "SOUNDG" => C::Sounding,
        "WRECKS" | "OBSTRN" | "UWTROC" => C::Danger,
        "LIGHTS" => C::Light,
        a if a.starts_with("BOY") => C::Buoy,
        a if a.starts_with("BCN") => C::Beacon,
        _ => return None,
    })
}

/// Géométrie GeoJSON.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum MapGeometry {
    /// Point.
    Point {
        /// `[lon, lat]`.
        coordinates: Position,
    },
    /// Lignes.
    MultiLineString {
        /// Lignes brisées.
        coordinates: Vec<Vec<Position>>,
    },
    /// Surface : contours extérieurs et trous (l'orientation S-57 —
    /// extérieur horaire, trou antihoraire — sépare les uns des autres).
    Polygon {
        /// Contours fermés.
        coordinates: Vec<Vec<Position>>,
    },
}

/// Propriétés d'une entité affichée.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapProperties {
    /// Catégorie d'affichage.
    pub cat: DisplayCategory,
    /// Acronyme de la classe d'objets.
    pub class: String,
    /// Usage prévu de la cellule (1 à 6 ; CM93 : rang d'échelle 1 à 8) :
    /// la plus détaillée l'emporte.
    pub usage: u8,
    /// Nom de la cellule.
    pub cell: String,
    /// Profondeur (sonde, ou VALSOU d'un danger), en mètres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<f64>,
    /// Étiquette de sonde (« 5.5 », « 12 »).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Profondeur minimale de la zone (DRVAL1), en mètres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drval1: Option<f64>,
    /// Profondeur maximale de la zone (DRVAL2), en mètres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drval2: Option<f64>,
    /// Valeur de l'isobathe (VALDCO), en mètres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valdco: Option<f64>,
    /// Nom (OBJNAM).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Première couleur (COLOUR) : 1 blanc, 2 noir, 3 rouge, 4 vert,
    /// 6 jaune…
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<u8>,
}

/// Entité GeoJSON affichée.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapFeature {
    /// Toujours `Feature`.
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// Géométrie.
    pub geometry: MapGeometry,
    /// Propriétés.
    pub properties: MapProperties,
}

/// Valeur numérique d'un attribut (valeur vide ou invalide : `None`).
fn number(feature: &S57Feature, acronym: &str) -> Option<f64> {
    let code = attribute_code(acronym)?;
    feature
        .attribute(code)?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

fn text(feature: &S57Feature, acronym: &str) -> Option<String> {
    let value = feature.attribute(attribute_code(acronym)?)?.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// Étiquette de sonde : une décimale sous 31 m (si non entière), entier
/// au-delà, comme sur les cartes marines.
pub fn sounding_label(depth: f64) -> String {
    let tenths = (depth * 10.0).round() / 10.0;
    if depth < 31.0 {
        if tenths.fract() == 0.0 {
            format!("{tenths:.0}")
        } else {
            format!("{tenths:.1}")
        }
    } else {
        format!("{:.0}", depth.trunc())
    }
}

/// Entités affichables d'une cellule. Une cellule annulée n'en a aucune.
pub fn map_features(cell: &S57Cell) -> Vec<MapFeature> {
    if cell.cancelled.is_some() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for feature in &cell.features {
        let Some(cat) = category(feature.object_class) else {
            continue;
        };
        let base = MapProperties {
            cat,
            class: feature.acronym().unwrap_or("").to_owned(),
            usage: cell.intended_usage,
            cell: cell.dataset_name.clone(),
            depth: number(feature, "VALSOU"),
            label: None,
            drval1: number(feature, "DRVAL1"),
            drval2: number(feature, "DRVAL2"),
            valdco: number(feature, "VALDCO"),
            name: text(feature, "OBJNAM"),
            colour: text(feature, "COLOUR")
                .and_then(|c| c.split(',').next().and_then(|v| v.trim().parse().ok())),
        };
        let mut push = |geometry| {
            out.push(MapFeature {
                kind: "Feature",
                geometry,
                properties: base.clone(),
            });
        };
        match &feature.geometry {
            S57Geometry::None => {}
            S57Geometry::Point { position } => push(MapGeometry::Point {
                coordinates: *position,
            }),
            S57Geometry::Line { lines } => push(MapGeometry::MultiLineString {
                coordinates: lines.clone(),
            }),
            S57Geometry::Area { rings } => push(MapGeometry::Polygon {
                coordinates: rings.clone(),
            }),
            S57Geometry::Soundings { soundings } => {
                for s in soundings {
                    out.push(MapFeature {
                        kind: "Feature",
                        geometry: MapGeometry::Point {
                            coordinates: s.position,
                        },
                        properties: MapProperties {
                            depth: Some(s.depth),
                            label: Some(sounding_label(s.depth)),
                            ..base.clone()
                        },
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::testdata::CellBuilder;
    use super::super::{decode_cell, decode_cell_with_updates};
    use super::*;

    #[test]
    fn categories_of_known_classes() {
        assert_eq!(category(42), Some(DisplayCategory::Depth));
        assert_eq!(category(71), Some(DisplayCategory::Land));
        assert_eq!(category(159), Some(DisplayCategory::Danger));
        assert_eq!(category(17), Some(DisplayCategory::Buoy));
        assert_eq!(category(7), Some(DisplayCategory::Beacon));
        assert_eq!(category(302), None, "M_COVR non affiché");
        assert_eq!(category(9999), None);
        assert_eq!(category_of("BOYSPP"), Some(DisplayCategory::Buoy));
        assert_eq!(category_of("_m_sor"), None);
    }

    #[test]
    fn sounding_labels() {
        assert_eq!(sounding_label(5.5), "5.5");
        assert_eq!(sounding_label(12.0), "12");
        assert_eq!(sounding_label(0.3), "0.3");
        assert_eq!(sounding_label(-0.9), "-0.9");
        assert_eq!(sounding_label(30.96), "31");
        assert_eq!(sounding_label(45.7), "45");
        assert_eq!(sounding_label(4.04), "4");
    }

    #[test]
    fn converts_sample_cell() {
        let cell = decode_cell(&CellBuilder::sample().build()).unwrap();
        let features = map_features(&cell);
        let of = |cat| features.iter().filter(move |f| f.properties.cat == cat);
        // 3 sondes, une zone de profondeur, un trait de côte, une épave ;
        // M_COVR et la collection ne sont pas affichés.
        assert_eq!(features.len(), 6);
        let soundings: Vec<_> = of(DisplayCategory::Sounding).collect();
        assert_eq!(soundings.len(), 3);
        assert_eq!(soundings[0].properties.label.as_deref(), Some("5.5"));
        assert_eq!(soundings[0].properties.usage, 5);
        let depth = of(DisplayCategory::Depth).next().unwrap();
        assert_eq!(depth.properties.drval1, Some(5.0));
        assert_eq!(depth.properties.drval2, Some(10.0));
        assert!(
            matches!(&depth.geometry, MapGeometry::Polygon { coordinates } if coordinates.len() == 2)
        );
        let wreck = of(DisplayCategory::Danger).next().unwrap();
        assert_eq!(wreck.properties.depth, Some(3.5));
        assert_eq!(wreck.properties.name.as_deref(), Some("Epave du port"));

        let json = serde_json::to_value(&features[0]).unwrap();
        assert_eq!(json["type"], "Feature");
        assert!(json["geometry"]["type"].is_string());
        assert!(
            json["properties"].get("label").is_none() || json["properties"]["cat"] == "sounding"
        );
    }

    #[test]
    fn serializes_as_geojson() {
        let cell = decode_cell(&CellBuilder::sample().build()).unwrap();
        let json = serde_json::to_value(map_features(&cell)).unwrap();
        let area = json
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["properties"]["cat"] == "depth")
            .unwrap();
        assert_eq!(area["geometry"]["type"], "Polygon");
        assert_eq!(area["geometry"]["coordinates"][0][0][0], -149.6);
        assert_eq!(area["properties"]["class"], "DEPARE");
        assert!(area["properties"].get("colour").is_none());
    }

    #[test]
    fn cancelled_cells_are_not_displayed() {
        let base = CellBuilder::sample().build();
        let mut cancel = CellBuilder::update(1);
        cancel.edition = "0";
        let cell = decode_cell_with_updates(&base, &[&cancel.build()]).unwrap();
        assert!(cell.cancelled.is_some());
        assert!(map_features(&cell).is_empty());
    }
}
