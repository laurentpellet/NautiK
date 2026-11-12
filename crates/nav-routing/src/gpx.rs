//! Import et export GPX 1.1 (waypoints, routes, traces).
//!
//! L'import est strict sur les coordonnées : un point invalide fait
//! échouer tout l'import plutôt que d'être corrigé ou ignoré en silence.
//! Les points de trace sans horodatage ne sont jamais datés arbitrairement :
//! la trace concernée est écartée avec un avertissement.
//!
//! Le parseur (quick-xml) ne résout aucune entité externe (pas de XXE).

use std::fmt::Write as _;

use chrono::{DateTime, SecondsFormat, Utc};
use nav_core::GeoPosition;
use quick_xml::escape::{escape, resolve_predefined_entity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use thiserror::Error;
use uuid::Uuid;

use crate::model::{
    validate_description, validate_name, Route, Track, TrackPoint, ValidationError, Waypoint,
    MAX_DESCRIPTION_LEN, MAX_NAME_LEN,
};

/// Nombre maximal de points (tous types confondus) dans un fichier.
pub const MAX_GPX_POINTS: usize = 1_000_000;
/// Profondeur maximale d'imbrication XML.
const MAX_DEPTH: usize = 32;
/// Longueur maximale d'un texte conservé.
const MAX_TEXT_LEN: usize = 10_000;

/// Erreur d'import GPX.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum GpxError {
    /// XML mal formé.
    #[error("XML invalide (octet {position}) : {message}")]
    Xml {
        /// Position dans le fichier.
        position: u64,
        /// Message du parseur.
        message: String,
    },
    /// L'élément racine n'est pas `<gpx>`.
    #[error("le fichier n'est pas un document GPX")]
    NotGpx,
    /// Attribut `lat`/`lon` absent ou non numérique.
    #[error("coordonnées absentes ou illisibles dans <{element}> n° {index}")]
    MissingCoordinates {
        /// Élément concerné.
        element: String,
        /// Rang de l'élément (à partir de 1).
        index: usize,
    },
    /// Coordonnées hors des bornes WGS84.
    #[error("coordonnées hors bornes dans <{element}> n° {index} : {lat}, {lon}")]
    InvalidCoordinates {
        /// Élément concerné.
        element: String,
        /// Rang de l'élément (à partir de 1).
        index: usize,
        /// Latitude lue.
        lat: f64,
        /// Longitude lue.
        lon: f64,
    },
    /// Horodatage illisible.
    #[error("horodatage invalide : {0}")]
    InvalidTime(String),
    /// Fichier trop volumineux.
    #[error("trop de points (maximum {MAX_GPX_POINTS})")]
    TooManyPoints,
    /// Imbrication excessive.
    #[error("imbrication XML excessive")]
    TooDeep,
}

/// Point GPX lu (wpt, rtept ou trkpt).
#[derive(Debug, Clone, PartialEq)]
pub struct GpxPoint {
    /// Position.
    pub position: GeoPosition,
    /// Nom.
    pub name: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// Horodatage.
    pub time: Option<DateTime<Utc>>,
}

/// Route ou trace GPX lue.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GpxPath {
    /// Nom.
    pub name: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// Points (segments de trace fusionnés).
    pub points: Vec<GpxPoint>,
}

/// Contenu d'un fichier GPX.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GpxDocument {
    /// Waypoints isolés.
    pub waypoints: Vec<GpxPoint>,
    /// Routes.
    pub routes: Vec<GpxPath>,
    /// Traces.
    pub tracks: Vec<GpxPath>,
}

/// Résultat de la conversion d'un document GPX en modèles NautiK.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GpxImport {
    /// Waypoints isolés.
    pub waypoints: Vec<Waypoint>,
    /// Routes.
    pub routes: Vec<Route>,
    /// Traces.
    pub tracks: Vec<Track>,
    /// Éléments écartés et raisons.
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Container {
    Route,
    Track,
}

#[derive(Default)]
struct PendingPoint {
    lat: f64,
    lon: f64,
    name: Option<String>,
    description: Option<String>,
    time: Option<String>,
}

/// Analyse un document GPX.
///
/// # Errors
///
/// [`GpxError`] si le XML est mal formé, si la racine n'est pas `<gpx>`,
/// si une coordonnée est absente ou invalide, ou si les limites de
/// taille sont dépassées.
pub fn parse(xml: &str) -> Result<GpxDocument, GpxError> {
    // Pas de `trim_text` : il rognerait chaque fragment autour des entités
    // (« a &amp; b » → « a&b »). Le texte complet est rogné à la fermeture.
    let mut reader = Reader::from_str(xml);

    let mut doc = GpxDocument::default();
    let mut stack: Vec<String> = Vec::new();
    let mut root_seen = false;
    let mut container: Option<(Container, GpxPath)> = None;
    let mut point: Option<(String, PendingPoint)> = None;
    let mut text = String::new();
    let mut point_count = 0_usize;
    let mut counters = [0_usize; 3];

    loop {
        let event = reader.read_event().map_err(|e| GpxError::Xml {
            position: reader.error_position(),
            message: e.to_string(),
        })?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let is_empty = matches!(event, Event::Empty(_));
                let name = local_name(e);
                if !root_seen {
                    if name != "gpx" {
                        return Err(GpxError::NotGpx);
                    }
                    root_seen = true;
                }
                match name.as_str() {
                    "rte" if point.is_none() => {
                        container = Some((Container::Route, GpxPath::default()));
                    }
                    "trk" if point.is_none() => {
                        container = Some((Container::Track, GpxPath::default()));
                    }
                    "wpt" | "rtept" | "trkpt" => {
                        point_count += 1;
                        if point_count > MAX_GPX_POINTS {
                            return Err(GpxError::TooManyPoints);
                        }
                        let slot = match name.as_str() {
                            "wpt" => 0,
                            "rtept" => 1,
                            _ => 2,
                        };
                        counters[slot] += 1;
                        let (lat, lon) = read_coordinates(e, &name, counters[slot])?;
                        point = Some((
                            name.clone(),
                            PendingPoint {
                                lat,
                                lon,
                                ..PendingPoint::default()
                            },
                        ));
                    }
                    _ => {}
                }
                text.clear();
                if is_empty {
                    close_element(
                        &name,
                        &mut doc,
                        &mut container,
                        &mut point,
                        &text,
                        &counters,
                    )?;
                } else {
                    stack.push(name);
                    if stack.len() > MAX_DEPTH {
                        return Err(GpxError::TooDeep);
                    }
                }
            }
            Event::Text(t) => {
                push_text(&mut text, &t.xml10_content());
            }
            Event::CData(c) => {
                push_text(&mut text, &c.xml10_content());
            }
            Event::GeneralRef(r) => {
                let name = r.xml10_content().into_owned();
                let resolved = match r.resolve_char_ref() {
                    Ok(Some(c)) => Some(c.to_string()),
                    _ => resolve_predefined_entity(&name).map(str::to_owned),
                };
                match resolved {
                    Some(value) => push_text(&mut text, &value),
                    None => {
                        return Err(GpxError::Xml {
                            position: reader.buffer_position(),
                            message: format!("entité inconnue &{name};"),
                        })
                    }
                }
            }
            Event::End(_) => {
                let name = stack.pop().unwrap_or_default();
                close_element(
                    &name,
                    &mut doc,
                    &mut container,
                    &mut point,
                    &text,
                    &counters,
                )?;
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !root_seen {
        return Err(GpxError::NotGpx);
    }
    Ok(doc)
}

fn local_name(e: &BytesStart<'_>) -> String {
    AsRef::<str>::as_ref(&e.local_name()).to_owned()
}

fn push_text(buffer: &mut String, value: &str) {
    let room = MAX_TEXT_LEN.saturating_sub(buffer.len());
    buffer.extend(value.chars().take(room));
}

fn read_coordinates(
    e: &BytesStart<'_>,
    element: &str,
    index: usize,
) -> Result<(f64, f64), GpxError> {
    let missing = || GpxError::MissingCoordinates {
        element: element.to_owned(),
        index,
    };
    let mut lat = None;
    let mut lon = None;
    for attribute in e.attributes() {
        let attribute = attribute.map_err(|_| missing())?;
        let value: Option<f64> = attribute.value.trim().parse().ok();
        match AsRef::<str>::as_ref(&attribute.key.local_name()) {
            "lat" => lat = value,
            "lon" => lon = value,
            _ => {}
        }
    }
    let (Some(lat), Some(lon)) = (lat, lon) else {
        return Err(missing());
    };
    Ok((lat, lon))
}

fn close_element(
    name: &str,
    doc: &mut GpxDocument,
    container: &mut Option<(Container, GpxPath)>,
    point: &mut Option<(String, PendingPoint)>,
    text: &str,
    counters: &[usize; 3],
) -> Result<(), GpxError> {
    let value = || Some(text.trim().to_owned()).filter(|t| !t.is_empty());
    match name {
        "name" | "desc" | "time" => {
            if let Some((_, p)) = point.as_mut() {
                match name {
                    "name" => p.name = value(),
                    "desc" => p.description = value(),
                    _ => p.time = value(),
                }
            } else if let Some((_, path)) = container.as_mut() {
                match name {
                    "name" => path.name = value(),
                    "desc" => path.description = value(),
                    _ => {}
                }
            }
        }
        "wpt" | "rtept" | "trkpt" => {
            if let Some((element, p)) = point.take() {
                let index = match element.as_str() {
                    "wpt" => counters[0],
                    "rtept" => counters[1],
                    _ => counters[2],
                };
                let position =
                    GeoPosition::new(p.lat, p.lon).map_err(|_| GpxError::InvalidCoordinates {
                        element: element.clone(),
                        index,
                        lat: p.lat,
                        lon: p.lon,
                    })?;
                let time = p
                    .time
                    .map(|t| {
                        DateTime::parse_from_rfc3339(&t)
                            .map(|d| d.with_timezone(&Utc))
                            .map_err(|_| GpxError::InvalidTime(t))
                    })
                    .transpose()?;
                let parsed = GpxPoint {
                    position,
                    name: p.name,
                    description: p.description,
                    time,
                };
                match (element.as_str(), container.as_mut()) {
                    ("wpt", _) => doc.waypoints.push(parsed),
                    ("rtept", Some((Container::Route, path)))
                    | ("trkpt", Some((Container::Track, path))) => path.points.push(parsed),
                    _ => {}
                }
            }
        }
        "rte" | "trk" => {
            if let Some((kind, path)) = container.take() {
                match kind {
                    Container::Route => doc.routes.push(path),
                    Container::Track => doc.tracks.push(path),
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn clean_name(name: Option<&str>, fallback: impl FnOnce() -> String) -> String {
    name.map(|n| n.chars().take(MAX_NAME_LEN).collect::<String>())
        .and_then(|n| validate_name(&n).ok())
        .unwrap_or_else(fallback)
}

fn clean_description(description: Option<&str>) -> Option<String> {
    description
        .map(|d| d.chars().take(MAX_DESCRIPTION_LEN).collect::<String>())
        .and_then(|d| validate_description(Some(&d)).ok().flatten())
}

fn to_waypoint(
    point: &GpxPoint,
    fallback: &str,
    now: DateTime<Utc>,
) -> Result<Waypoint, ValidationError> {
    let mut waypoint = Waypoint::new(
        &clean_name(point.name.as_deref(), || fallback.to_owned()),
        point.position,
        None,
        now,
    )?;
    waypoint.description = clean_description(point.description.as_deref());
    Ok(waypoint)
}

impl GpxDocument {
    /// Convertit le document en modèles NautiK. Les routes de moins de
    /// deux points et les traces sans horodatage complet sont écartées,
    /// avec un avertissement.
    #[must_use]
    pub fn into_models(self, now: DateTime<Utc>) -> GpxImport {
        let mut import = GpxImport::default();
        for (i, point) in self.waypoints.iter().enumerate() {
            match to_waypoint(point, &format!("WPT{:03}", i + 1), now) {
                Ok(w) => import.waypoints.push(w),
                Err(e) => import
                    .warnings
                    .push(format!("waypoint {} écarté : {e}", i + 1)),
            }
        }
        for (i, path) in self.routes.into_iter().enumerate() {
            let name = clean_name(path.name.as_deref(), || format!("Route importée {}", i + 1));
            let waypoints: Result<Vec<_>, _> = path
                .points
                .iter()
                .enumerate()
                .map(|(j, p)| to_waypoint(p, &format!("WP{:03}", j + 1), now))
                .collect();
            let route = waypoints.and_then(|w| {
                let mut route = Route::new(&name, None, w, now)?;
                route.description = clean_description(path.description.as_deref());
                Ok(route)
            });
            match route {
                Ok(route) => import.routes.push(route),
                Err(e) => import
                    .warnings
                    .push(format!("route « {name} » écartée : {e}")),
            }
        }
        for (i, path) in self.tracks.into_iter().enumerate() {
            let name = clean_name(path.name.as_deref(), || format!("Trace importée {}", i + 1));
            let points: Option<Vec<TrackPoint>> = path
                .points
                .iter()
                .map(|p| {
                    p.time.map(|timestamp| TrackPoint {
                        timestamp,
                        position: p.position,
                        sog_mps: None,
                        cog_rad: None,
                        depth_m: None,
                    })
                })
                .collect();
            match points {
                Some(mut points) if !points.is_empty() => {
                    points.sort_by_key(|p| p.timestamp);
                    import.tracks.push(Track {
                        id: Uuid::new_v4(),
                        name,
                        started_at: points[0].timestamp,
                        ended_at: points.last().map(|p| p.timestamp),
                        points,
                    });
                }
                Some(_) => import
                    .warnings
                    .push(format!("trace « {name} » vide écartée")),
                None => import
                    .warnings
                    .push(format!("trace « {name} » écartée : points sans horodatage")),
            }
        }
        import
    }
}

/// Produit un document GPX 1.1.
#[must_use]
pub fn write(waypoints: &[Waypoint], routes: &[Route], tracks: &[Track]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <gpx version=\"1.1\" creator=\"NautiK\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n",
    );
    for w in waypoints {
        write_point(
            &mut out,
            "wpt",
            "  ",
            w.position,
            Some(&w.name),
            w.description.as_deref(),
            None,
        );
    }
    for r in routes {
        out.push_str("  <rte>\n");
        write_text(&mut out, "    ", "name", &r.name);
        if let Some(desc) = &r.description {
            write_text(&mut out, "    ", "desc", desc);
        }
        for w in &r.waypoints {
            write_point(
                &mut out,
                "rtept",
                "    ",
                w.position,
                Some(&w.name),
                w.description.as_deref(),
                None,
            );
        }
        out.push_str("  </rte>\n");
    }
    for t in tracks {
        out.push_str("  <trk>\n");
        write_text(&mut out, "    ", "name", &t.name);
        out.push_str("    <trkseg>\n");
        for p in &t.points {
            write_point(
                &mut out,
                "trkpt",
                "      ",
                p.position,
                None,
                None,
                Some(p.timestamp),
            );
        }
        out.push_str("    </trkseg>\n  </trk>\n");
    }
    out.push_str("</gpx>\n");
    out
}

fn write_text(out: &mut String, indent: &str, tag: &str, value: &str) {
    let _ = writeln!(out, "{indent}<{tag}>{}</{tag}>", escape(value));
}

fn write_point(
    out: &mut String,
    tag: &str,
    indent: &str,
    position: GeoPosition,
    name: Option<&str>,
    description: Option<&str>,
    time: Option<DateTime<Utc>>,
) {
    let _ = write!(
        out,
        "{indent}<{tag} lat=\"{:.7}\" lon=\"{:.7}\">",
        position.latitude_deg(),
        position.longitude_deg()
    );
    if let Some(time) = time {
        let _ = write!(
            out,
            "<time>{}</time>",
            time.to_rfc3339_opts(SecondsFormat::Millis, true)
        );
    }
    if let Some(name) = name {
        let _ = write!(out, "<name>{}</name>", escape(name));
    }
    if let Some(desc) = description {
        let _ = write!(out, "<desc>{}</desc>", escape(desc));
    }
    let _ = writeln!(out, "</{tag}>");
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
  <wpt lat="-14.968" lon="-147.638"><name>Passe de Tiputa</name><desc>Courant &amp; ressac</desc></wpt>
  <wpt lat="-14.95" lon="-147.70"/>
  <rte>
    <name>Rangiroa → Tikehau</name>
    <rtept lat="-14.968" lon="-147.638"><name>Tiputa</name></rtept>
    <rtept lat="-15.0" lon="-148.17"><name><![CDATA[Tikehau <passe>]]></name></rtept>
  </rte>
  <trk>
    <name>Sortie</name>
    <trkseg>
      <trkpt lat="-14.968" lon="-147.638"><time>2026-11-12T10:00:00Z</time></trkpt>
      <trkpt lat="-14.960" lon="-147.630"><time>2026-11-12T10:05:00Z</time></trkpt>
    </trkseg>
    <trkseg>
      <trkpt lat="-14.950" lon="-147.620"><time>2026-11-12T10:10:00+00:00</time></trkpt>
    </trkseg>
  </trk>
</gpx>"#;

    #[test]
    fn parses_waypoints_routes_and_tracks() {
        let doc = parse(SAMPLE).expect("gpx valide");
        assert_eq!(doc.waypoints.len(), 2);
        assert_eq!(doc.waypoints[0].name.as_deref(), Some("Passe de Tiputa"));
        assert_eq!(
            doc.waypoints[0].description.as_deref(),
            Some("Courant & ressac")
        );
        assert_eq!(doc.waypoints[1].name, None);
        assert_eq!(doc.routes.len(), 1);
        assert_eq!(doc.routes[0].name.as_deref(), Some("Rangiroa → Tikehau"));
        assert_eq!(
            doc.routes[0].points[1].name.as_deref(),
            Some("Tikehau <passe>")
        );
        assert_eq!(doc.tracks.len(), 1);
        assert_eq!(doc.tracks[0].points.len(), 3, "segments fusionnés");
    }

    #[test]
    fn converts_to_models() {
        let import = parse(SAMPLE).expect("gpx").into_models(Utc::now());
        assert_eq!(import.waypoints.len(), 2);
        assert_eq!(import.waypoints[1].name, "WPT002");
        assert_eq!(import.routes[0].waypoints.len(), 2);
        let track = &import.tracks[0];
        assert_eq!(track.points.len(), 3);
        assert_eq!(track.started_at.to_rfc3339(), "2026-11-12T10:00:00+00:00");
        assert_eq!(
            track.ended_at.map(|t| t.to_rfc3339()).as_deref(),
            Some("2026-11-12T10:10:00+00:00")
        );
        assert!(import.warnings.is_empty(), "{:?}", import.warnings);
    }

    #[test]
    fn round_trip_preserves_content() {
        let import = parse(SAMPLE).expect("gpx").into_models(Utc::now());
        let xml = write(&import.waypoints, &import.routes, &import.tracks);
        let again = parse(&xml).expect("gpx réécrit").into_models(Utc::now());
        assert_eq!(again.waypoints.len(), import.waypoints.len());
        assert_eq!(
            again.waypoints[0].description,
            import.waypoints[0].description
        );
        assert_eq!(again.routes[0].name, import.routes[0].name);
        assert_eq!(again.routes[0].waypoints[1].name, "Tikehau <passe>");
        let (a, b) = (&again.tracks[0].points, &import.tracks[0].points);
        assert_eq!(a.len(), b.len());
        for (p, q) in a.iter().zip(b) {
            assert_eq!(p.timestamp, q.timestamp);
            assert!((p.position.latitude_deg() - q.position.latitude_deg()).abs() < 1e-7);
            assert!((p.position.longitude_deg() - q.position.longitude_deg()).abs() < 1e-7);
        }
    }

    #[test]
    fn writes_escaped_text() {
        let wp = Waypoint::new(
            "A & <B>",
            GeoPosition::new(1.0, 2.0).expect("pos"),
            Some("\"x\""),
            Utc::now(),
        )
        .expect("wp");
        let xml = write(&[wp], &[], &[]);
        assert!(xml.contains("<name>A &amp; &lt;B&gt;</name>"), "{xml}");
        assert!(xml.contains("lat=\"1.0000000\" lon=\"2.0000000\""));
    }

    #[test]
    fn rejects_malformed_xml() {
        assert!(matches!(
            parse("<gpx><wpt lat=\"1\" lon=\"2\"></gpx>"),
            Err(GpxError::Xml { .. })
        ));
        assert!(matches!(
            parse("<gpx><wpt lat=\"1\" lon=\"2\">"),
            Err(GpxError::Xml { .. }) | Ok(_)
        ));
        assert!(matches!(
            parse("<gpx>&unknown;</gpx>"),
            Err(GpxError::Xml { .. })
        ));
    }

    #[test]
    fn rejects_non_gpx_documents() {
        assert_eq!(parse("<kml></kml>"), Err(GpxError::NotGpx));
        assert_eq!(parse(""), Err(GpxError::NotGpx));
        assert_eq!(parse("pas du xml"), Err(GpxError::NotGpx));
    }

    #[test]
    fn rejects_missing_or_invalid_coordinates() {
        assert_eq!(
            parse(r#"<gpx><wpt lat="1"/></gpx>"#),
            Err(GpxError::MissingCoordinates {
                element: "wpt".to_owned(),
                index: 1
            })
        );
        assert_eq!(
            parse(r#"<gpx><wpt lat="abc" lon="2"/></gpx>"#),
            Err(GpxError::MissingCoordinates {
                element: "wpt".to_owned(),
                index: 1
            })
        );
        assert!(matches!(
            parse(r#"<gpx><wpt lat="1" lon="2"/><wpt lat="91" lon="2"/></gpx>"#),
            Err(GpxError::InvalidCoordinates { index: 2, .. })
        ));
        assert!(matches!(
            parse(r#"<gpx><wpt lat="NaN" lon="2"/></gpx>"#),
            Err(GpxError::InvalidCoordinates { .. })
        ));
    }

    #[test]
    fn rejects_invalid_time() {
        assert!(matches!(
            parse(r#"<gpx><wpt lat="1" lon="2"><time>hier</time></wpt></gpx>"#),
            Err(GpxError::InvalidTime(_))
        ));
    }

    #[test]
    fn rejects_excessive_nesting() {
        let deep = format!("<gpx>{}{}</gpx>", "<a>".repeat(40), "</a>".repeat(40));
        assert_eq!(parse(&deep), Err(GpxError::TooDeep));
    }

    #[test]
    fn does_not_resolve_external_entities() {
        let xxe = r#"<?xml version="1.0"?>
<!DOCTYPE gpx [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<gpx><wpt lat="1" lon="2"><name>&xxe;</name></wpt></gpx>"#;
        assert!(matches!(parse(xxe), Err(GpxError::Xml { .. })));
    }

    #[test]
    fn short_routes_and_untimed_tracks_are_reported() {
        let xml = r#"<gpx>
  <rte><name>Courte</name><rtept lat="1" lon="1"/></rte>
  <trk><name>Sans heure</name><trkseg><trkpt lat="1" lon="1"/></trkseg></trk>
  <trk><name>Vide</name></trk>
</gpx>"#;
        let import = parse(xml).expect("gpx").into_models(Utc::now());
        assert!(import.routes.is_empty());
        assert!(import.tracks.is_empty());
        assert_eq!(import.warnings.len(), 3, "{:?}", import.warnings);
    }

    #[test]
    fn long_names_are_truncated_not_rejected() {
        let xml = format!(
            r#"<gpx><wpt lat="1" lon="1"><name>{}</name></wpt></gpx>"#,
            "n".repeat(500)
        );
        let import = parse(&xml).expect("gpx").into_models(Utc::now());
        assert_eq!(import.waypoints[0].name.chars().count(), MAX_NAME_LEN);
    }

    #[test]
    fn empty_gpx_is_valid() {
        assert_eq!(parse("<gpx/>"), Ok(GpxDocument::default()));
    }
}
