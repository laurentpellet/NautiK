//! Import et export KML des marques personnelles (`Placemark` de type
//! `Point`).
//!
//! L'import est strict sur les coordonnées : un point invalide fait
//! échouer tout l'import. Les autres géométries (lignes, polygones) sont
//! écartées avec un avertissement. Le parseur (quick-xml) ne résout
//! aucune entité externe (pas de XXE).

use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use nav_core::GeoPosition;
use quick_xml::escape::{escape, resolve_predefined_entity};
use quick_xml::events::Event;
use quick_xml::Reader;
use thiserror::Error;

use crate::mark::{Mark, MarkIcon, MAX_MARKS};
use crate::model::{MAX_DESCRIPTION_LEN, MAX_NAME_LEN};

/// Profondeur maximale d'imbrication XML.
const MAX_DEPTH: usize = 32;
/// Longueur maximale d'un texte conservé.
const MAX_TEXT_LEN: usize = 10_000;
/// Préfixe du style KML portant l'icône NautiK.
const STYLE_PREFIX: &str = "nautik-";

/// Erreur d'import KML.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum KmlError {
    /// XML mal formé.
    #[error("XML invalide (octet {position}) : {message}")]
    Xml {
        /// Position dans le fichier.
        position: u64,
        /// Message du parseur.
        message: String,
    },
    /// L'élément racine n'est pas `<kml>`.
    #[error("le fichier n'est pas un document KML")]
    NotKml,
    /// Coordonnées absentes, illisibles ou hors bornes.
    #[error("coordonnées invalides dans le repère n° {index} : « {text} »")]
    InvalidCoordinates {
        /// Rang du repère (à partir de 1).
        index: usize,
        /// Texte lu.
        text: String,
    },
    /// Imbrication trop profonde.
    #[error("imbrication XML trop profonde")]
    TooDeep,
    /// Trop de repères.
    #[error("trop de repères (maximum {MAX_MARKS})")]
    TooMany,
}

/// Résultat d'un import KML.
#[derive(Debug, Clone, PartialEq)]
pub struct KmlImport {
    /// Marques lues.
    pub marks: Vec<Mark>,
    /// Repères écartés et raisons.
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct Placemark {
    name: Option<String>,
    description: Option<String>,
    style: Option<String>,
    coordinates: Option<String>,
    point: bool,
    other_geometry: bool,
}

fn truncate(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

/// Lit `lon,lat[,alt]`.
fn parse_coordinates(text: &str) -> Option<GeoPosition> {
    let first = text.split_whitespace().next()?;
    let mut parts = first.split(',');
    let lon: f64 = parts.next()?.trim().parse().ok()?;
    let lat: f64 = parts.next()?.trim().parse().ok()?;
    GeoPosition::new(lat, lon).ok()
}

/// Analyse un document KML et en tire les marques.
///
/// # Errors
///
/// [`KmlError`] si le document est mal formé, n'est pas du KML, ou si un
/// repère a des coordonnées invalides.
pub fn parse(xml: &str, now: DateTime<Utc>) -> Result<KmlImport, KmlError> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut current: Option<Placemark> = None;
    let mut index = 0usize;
    let mut marks = Vec::new();
    let mut warnings = Vec::new();
    let mut root_checked = false;
    loop {
        let event = reader.read_event().map_err(|e| KmlError::Xml {
            position: reader.buffer_position(),
            message: e.to_string(),
        })?;
        match event {
            Event::Start(e) => {
                let name = AsRef::<str>::as_ref(&e.local_name()).to_owned();
                if !root_checked {
                    if name != "kml" {
                        return Err(KmlError::NotKml);
                    }
                    root_checked = true;
                }
                if stack.len() >= MAX_DEPTH {
                    return Err(KmlError::TooDeep);
                }
                match name.as_str() {
                    "Placemark" => current = Some(Placemark::default()),
                    "Point" => {
                        if let Some(p) = current.as_mut() {
                            p.point = true;
                        }
                    }
                    "LineString" | "Polygon" | "LinearRing" | "MultiGeometry" | "Track" => {
                        if let Some(p) = current.as_mut() {
                            p.other_geometry = true;
                        }
                    }
                    _ => {}
                }
                stack.push(name);
            }
            Event::Empty(e) => {
                if !root_checked {
                    return Err(KmlError::NotKml);
                }
                let _ = e;
            }
            Event::Text(t) => {
                let Some(p) = current.as_mut() else { continue };
                append_text(p, stack.last().map(String::as_str), &t.xml10_content());
            }
            Event::CData(t) => {
                let Some(p) = current.as_mut() else { continue };
                append_text(p, stack.last().map(String::as_str), &t.xml10_content());
            }
            Event::GeneralRef(r) => {
                let name = r.xml10_content().into_owned();
                let resolved = match r.resolve_char_ref() {
                    Ok(Some(c)) => Some(c.to_string()),
                    _ => resolve_predefined_entity(&name).map(str::to_owned),
                };
                let Some(value) = resolved else {
                    return Err(KmlError::Xml {
                        position: reader.buffer_position(),
                        message: format!("entité inconnue &{name};"),
                    });
                };
                if let Some(p) = current.as_mut() {
                    append_text(p, stack.last().map(String::as_str), &value);
                }
            }
            Event::End(_) => {
                let name = stack.pop().unwrap_or_default();
                if name == "Placemark" {
                    if let Some(p) = current.take() {
                        index += 1;
                        if let Some(mark) = placemark_to_mark(p, index, now, &mut warnings)? {
                            if marks.len() >= MAX_MARKS {
                                return Err(KmlError::TooMany);
                            }
                            marks.push(mark);
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !root_checked {
        return Err(KmlError::NotKml);
    }
    Ok(KmlImport { marks, warnings })
}

fn append_text(p: &mut Placemark, element: Option<&str>, text: &str) {
    let target = match element {
        Some("name") => &mut p.name,
        Some("description") => &mut p.description,
        Some("styleUrl") => &mut p.style,
        Some("coordinates") => &mut p.coordinates,
        _ => return,
    };
    let joined = match target.take() {
        Some(prev) => prev + text,
        None => text.to_owned(),
    };
    *target = Some(joined.chars().take(MAX_TEXT_LEN).collect());
}

fn placemark_to_mark(
    p: Placemark,
    index: usize,
    now: DateTime<Utc>,
    warnings: &mut Vec<String>,
) -> Result<Option<Mark>, KmlError> {
    let label = p
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("Repère {index}"));
    if !p.point {
        if p.other_geometry {
            warnings.push(format!("« {label} » écarté : ce n'est pas un point"));
        } else {
            warnings.push(format!("« {label} » écarté : aucune géométrie"));
        }
        return Ok(None);
    }
    let text = p.coordinates.unwrap_or_default();
    let position = parse_coordinates(&text).ok_or_else(|| KmlError::InvalidCoordinates {
        index,
        text: truncate(&text, 60),
    })?;
    let name = truncate(&label, MAX_NAME_LEN);
    let note = p
        .description
        .map(|d| truncate(&d, MAX_DESCRIPTION_LEN))
        .filter(|d| !d.is_empty());
    let icon = p
        .style
        .as_deref()
        .and_then(|s| s.trim_start_matches('#').strip_prefix(STYLE_PREFIX))
        .and_then(MarkIcon::from_code)
        .unwrap_or_default();
    match Mark::new(&name, position, icon, note.as_deref(), now) {
        Ok(mark) => Ok(Some(mark)),
        Err(e) => {
            warnings.push(format!("« {label} » écarté : {e}"));
            Ok(None)
        }
    }
}

/// Écrit des marques en KML 2.2 (un `Placemark` par marque).
#[must_use]
pub fn write(marks: &[Mark]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <kml xmlns=\"http://www.opengis.net/kml/2.2\">\n<Document>\n<name>NautiK — marques</name>\n",
    );
    for mark in marks {
        let _ = write!(
            out,
            "<Placemark>\n<name>{}</name>\n",
            escape(mark.name.as_str())
        );
        if let Some(note) = &mark.note {
            let _ = writeln!(out, "<description>{}</description>", escape(note.as_str()));
        }
        let _ = write!(
            out,
            "<styleUrl>#{STYLE_PREFIX}{}</styleUrl>\n<Point><coordinates>{:.7},{:.7}</coordinates></Point>\n</Placemark>\n",
            mark.icon.code(),
            mark.position.longitude_deg(),
            mark.position.latitude_deg()
        );
    }
    out.push_str("</Document>\n</kml>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t")
    }

    #[test]
    fn round_trip_keeps_names_notes_icons_and_positions() {
        let a = Mark::new(
            "Passe <Teavapiti> & co",
            GeoPosition::new(-16.5, -151.75).expect("p"),
            MarkIcon::Danger,
            Some("Courant fort \"à l'étale\""),
            now(),
        )
        .expect("a");
        let b = Mark::new(
            "Mouillage",
            GeoPosition::new(-17.0, 179.9).expect("p"),
            MarkIcon::Anchorage,
            None,
            now(),
        )
        .expect("b");
        let xml = write(&[a.clone(), b.clone()]);
        let back = parse(&xml, now()).expect("relu");
        assert!(back.warnings.is_empty());
        assert_eq!(back.marks.len(), 2);
        let (ra, rb) = (&back.marks[0], &back.marks[1]);
        assert_eq!(ra.name, a.name);
        assert_eq!(ra.note, a.note);
        assert_eq!(ra.icon, MarkIcon::Danger);
        assert!((ra.position.latitude_deg() + 16.5).abs() < 1e-7);
        assert_eq!(rb.icon, MarkIcon::Anchorage);
        assert!((rb.position.longitude_deg() - 179.9).abs() < 1e-7);
    }

    #[test]
    fn foreign_kml_with_lines_and_cdata() {
        let xml = r#"<?xml version="1.0"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document><Folder>
 <Placemark><name>Bouée</name><description><![CDATA[<b>verte</b>]]></description>
  <styleUrl>#autre</styleUrl><Point><coordinates> -149.5,-17.5,0 </coordinates></Point></Placemark>
 <Placemark><name>Chenal</name><LineString><coordinates>0,0 1,1</coordinates></LineString></Placemark>
 <Placemark><Point><coordinates>10,20</coordinates></Point></Placemark>
</Folder></Document></kml>"#;
        let r = parse(xml, now()).expect("lu");
        assert_eq!(r.marks.len(), 2);
        assert_eq!(r.marks[0].note.as_deref(), Some("<b>verte</b>"));
        assert_eq!(r.marks[0].icon, MarkIcon::Flag, "style inconnu");
        assert_eq!(r.marks[1].name, "Repère 3");
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].contains("Chenal"));
    }

    #[test]
    fn malformed_documents_are_rejected() {
        assert_eq!(parse("<gpx></gpx>", now()), Err(KmlError::NotKml));
        assert_eq!(parse("", now()), Err(KmlError::NotKml));
        assert!(matches!(
            parse("<kml><Placemark>", now()),
            Err(KmlError::Xml { .. }) | Ok(_)
        ));
        let bad = "<kml><Placemark><Point><coordinates>abc</coordinates></Point></Placemark></kml>";
        assert!(matches!(
            parse(bad, now()),
            Err(KmlError::InvalidCoordinates { index: 1, .. })
        ));
        let out =
            "<kml><Placemark><Point><coordinates>200,95</coordinates></Point></Placemark></kml>";
        assert!(matches!(
            parse(out, now()),
            Err(KmlError::InvalidCoordinates { .. })
        ));
        let deep = format!("<kml>{}{}</kml>", "<a>".repeat(40), "</a>".repeat(40));
        assert_eq!(parse(&deep, now()), Err(KmlError::TooDeep));
        // Entité externe : jamais résolue.
        let xxe = r#"<!DOCTYPE kml [<!ENTITY x SYSTEM "file:///etc/passwd">]>
<kml><Placemark><name>&x;</name><Point><coordinates>1,1</coordinates></Point></Placemark></kml>"#;
        if let Ok(r) = parse(xxe, now()) {
            assert!(r.marks.iter().all(|m| !m.name.contains("root")));
        }
    }
}
