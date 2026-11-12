//! Décodage des messages Signal K (hello, deltas) et normalisation.
//!
//! Signal K transmet déjà en unités SI (m, m/s, rad) ; les valeurs sont
//! néanmoins validées par [`Quantity::normalize`]. Seul le contexte du
//! bateau (`vessels.self`) est retenu ; les autres navires relèvent de
//! l'AIS.

use chrono::{DateTime, Utc};
pub use nav_core::link::Sample;
use nav_core::{GeoPosition, Measurement, PositionFix, Quantity, SourceKind};
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

/// Taille maximale d'un message accepté.
pub const MAX_MESSAGE_BYTES: usize = 256 * 1024;
/// Nombre maximal de valeurs traitées par message.
pub const MAX_VALUES_PER_MESSAGE: usize = 1_000;
/// Identifiant de source par défaut.
pub const DEFAULT_SOURCE_ID: &str = "signalk";

/// Chemin Signal K de la position.
pub const POSITION_PATH: &str = "navigation.position";

/// Chemins Signal K pris en charge et grandeurs associées.
pub const SUBSCRIBED_PATHS: [(&str, Option<Quantity>); 9] = [
    (POSITION_PATH, None),
    (
        "navigation.speedOverGround",
        Some(Quantity::SpeedOverGround),
    ),
    (
        "navigation.courseOverGroundTrue",
        Some(Quantity::CourseOverGroundTrue),
    ),
    ("navigation.headingTrue", Some(Quantity::HeadingTrue)),
    (
        "navigation.headingMagnetic",
        Some(Quantity::HeadingMagnetic),
    ),
    (
        "navigation.speedThroughWater",
        Some(Quantity::SpeedThroughWater),
    ),
    (
        "environment.depth.belowTransducer",
        Some(Quantity::DepthBelowTransducer),
    ),
    (
        "environment.wind.speedApparent",
        Some(Quantity::WindSpeedApparent),
    ),
    (
        "environment.wind.angleApparent",
        Some(Quantity::WindAngleApparent),
    ),
];

/// Message illisible.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DeltaError {
    /// Message trop volumineux.
    #[error("message trop volumineux ({0} octets)")]
    TooLarge(usize),
    /// JSON invalide.
    #[error("JSON invalide : {0}")]
    Json(String),
}

/// Informations du message `hello`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerHello {
    /// Nom du serveur.
    pub name: Option<String>,
    /// Version du serveur.
    pub version: Option<String>,
    /// Identifiant du bateau (`vessels.urn:mrn:…`).
    pub self_context: Option<String>,
}

/// Message décodé.
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    /// Message de bienvenue.
    Hello(ServerHello),
    /// Données (et valeurs rejetées).
    Delta {
        /// Échantillons valides.
        samples: Vec<Sample>,
        /// Valeurs rejetées et raisons.
        rejected: Vec<String>,
    },
    /// Autre message (réponse d'abonnement, etc.).
    Ignored,
}

#[derive(Deserialize)]
struct RawMessage {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default, rename = "self")]
    self_id: Option<String>,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    updates: Option<Vec<RawUpdate>>,
}

#[derive(Deserialize)]
struct RawUpdate {
    #[serde(default, rename = "$source")]
    source_ref: Option<String>,
    #[serde(default)]
    source: Option<RawSource>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    values: Vec<RawValue>,
}

#[derive(Deserialize)]
struct RawSource {
    #[serde(default)]
    label: Option<String>,
}

#[derive(Deserialize)]
struct RawValue {
    path: String,
    #[serde(default)]
    value: Value,
}

fn is_self(context: Option<&str>, self_context: Option<&str>) -> bool {
    match context {
        None | Some("vessels.self") => true,
        Some(c) => self_context.is_some_and(|s| s == c),
    }
}

fn quantity_for(path: &str) -> Option<Option<Quantity>> {
    SUBSCRIBED_PATHS
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, q)| *q)
}

/// Décode un message texte reçu du serveur. `self_context` est
/// l'identifiant du bateau annoncé par le `hello`, s'il est connu.
///
/// # Errors
///
/// [`DeltaError`] si le message est trop volumineux ou n'est pas du JSON
/// conforme.
pub fn decode(
    text: &str,
    self_context: Option<&str>,
    received_at: DateTime<Utc>,
) -> Result<Decoded, DeltaError> {
    if text.len() > MAX_MESSAGE_BYTES {
        return Err(DeltaError::TooLarge(text.len()));
    }
    let raw: RawMessage =
        serde_json::from_str(text).map_err(|e| DeltaError::Json(e.to_string()))?;
    let Some(updates) = raw.updates else {
        if raw.self_id.is_some() || raw.version.is_some() {
            return Ok(Decoded::Hello(ServerHello {
                name: raw.name,
                version: raw.version,
                self_context: raw.self_id,
            }));
        }
        return Ok(Decoded::Ignored);
    };
    if !is_self(raw.context.as_deref(), self_context) {
        return Ok(Decoded::Delta {
            samples: Vec::new(),
            rejected: Vec::new(),
        });
    }

    let mut samples = Vec::new();
    let mut rejected = Vec::new();
    let mut seen = 0_usize;
    for update in updates {
        let source_id = update
            .source_ref
            .or_else(|| update.source.and_then(|s| s.label))
            .filter(|s| !s.is_empty())
            .map(|s| s.chars().take(100).collect::<String>())
            .unwrap_or_else(|| DEFAULT_SOURCE_ID.to_owned());
        // Sans horodatage, l'heure de réception est l'estimation la plus
        // prudente de l'heure de mesure.
        let measured_at = match update.timestamp.as_deref() {
            None => received_at,
            Some(ts) => match DateTime::parse_from_rfc3339(ts) {
                Ok(t) => t.with_timezone(&Utc),
                Err(_) => {
                    rejected.push(format!("horodatage invalide « {ts} » ({source_id})"));
                    continue;
                }
            },
        };
        for value in update.values {
            seen += 1;
            if seen > MAX_VALUES_PER_MESSAGE {
                rejected.push("trop de valeurs dans le message".to_owned());
                return Ok(Decoded::Delta { samples, rejected });
            }
            let Some(quantity) = quantity_for(&value.path) else {
                continue;
            };
            if value.value.is_null() {
                // Valeur inconnue côté serveur : rien à retenir.
                continue;
            }
            match to_sample(quantity, &value.value, &source_id, measured_at, received_at) {
                Ok(sample) => samples.push(sample),
                Err(reason) => rejected.push(format!("{} : {reason}", value.path)),
            }
        }
    }
    Ok(Decoded::Delta { samples, rejected })
}

fn to_sample(
    quantity: Option<Quantity>,
    value: &Value,
    source_id: &str,
    measured_at: DateTime<Utc>,
    received_at: DateTime<Utc>,
) -> Result<Sample, String> {
    match quantity {
        None => {
            let lat = value.get("latitude").and_then(Value::as_f64);
            let lon = value.get("longitude").and_then(Value::as_f64);
            let (Some(lat), Some(lon)) = (lat, lon) else {
                return Err("latitude/longitude absentes".to_owned());
            };
            let position = GeoPosition::new(lat, lon).map_err(|e| e.to_string())?;
            Ok(Sample::Position(PositionFix {
                position,
                cog_rad: None,
                sog_mps: None,
                source: SourceKind::SignalK,
                source_id: source_id.to_owned(),
                measured_at,
                received_at,
            }))
        }
        Some(q) => {
            let number = value.as_f64().ok_or_else(|| "nombre attendu".to_owned())?;
            Measurement::new(
                q,
                number,
                SourceKind::SignalK,
                source_id,
                measured_at,
                received_at,
            )
            .map(Sample::Measurement)
            .map_err(|e| e.to_string())
        }
    }
}

/// Message d'abonnement aux chemins pris en charge.
#[must_use]
pub fn subscription_message(period_ms: u32) -> String {
    let subscribe: Vec<Value> = SUBSCRIBED_PATHS
        .iter()
        .map(|(path, _)| serde_json::json!({ "path": path, "period": period_ms }))
        .collect();
    serde_json::json!({ "context": "vessels.self", "subscribe": subscribe }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t")
    }

    fn delta(body: &str) -> (Vec<Sample>, Vec<String>) {
        match decode(body, Some("vessels.urn:mrn:imo:mmsi:227000000"), now()).expect("ok") {
            Decoded::Delta { samples, rejected } => (samples, rejected),
            other => panic!("delta attendu : {other:?}"),
        }
    }

    #[test]
    fn decodes_hello() {
        let hello = decode(
            r#"{"name":"signalk-server","version":"2.8.0","self":"vessels.urn:mrn:imo:mmsi:227000000","roles":["master"],"timestamp":"2026-11-12T10:00:00Z"}"#,
            None,
            now(),
        )
        .expect("ok");
        assert_eq!(
            hello,
            Decoded::Hello(ServerHello {
                name: Some("signalk-server".to_owned()),
                version: Some("2.8.0".to_owned()),
                self_context: Some("vessels.urn:mrn:imo:mmsi:227000000".to_owned()),
            })
        );
    }

    #[test]
    fn decodes_position_and_instruments() {
        let (samples, rejected) = delta(
            r#"{"context":"vessels.urn:mrn:imo:mmsi:227000000","updates":[{
                "$source":"nmea0183.GP","timestamp":"2026-11-12T10:00:00.250Z",
                "values":[
                  {"path":"navigation.position","value":{"latitude":-14.968,"longitude":-147.638}},
                  {"path":"navigation.speedOverGround","value":3.2},
                  {"path":"navigation.courseOverGroundTrue","value":-0.5},
                  {"path":"navigation.unknownPath","value":1},
                  {"path":"environment.depth.belowTransducer","value":null}
                ]}]}"#,
        );
        assert!(rejected.is_empty(), "{rejected:?}");
        assert_eq!(samples.len(), 3);
        let Sample::Position(fix) = &samples[0] else {
            panic!("position attendue")
        };
        assert_eq!(fix.source, SourceKind::SignalK);
        assert_eq!(fix.source_id, "nmea0183.GP");
        assert_eq!(
            fix.measured_at.to_rfc3339(),
            "2026-11-12T10:00:00.250+00:00"
        );
        assert_eq!(fix.received_at, now());
        let Sample::Measurement(cog) = &samples[2] else {
            panic!("mesure attendue")
        };
        assert!(cog.value > 5.7, "angle normalisé dans [0, 2π)");
    }

    #[test]
    fn missing_context_and_timestamp_default_to_self_and_reception() {
        let (samples, _) = delta(
            r#"{"updates":[{"source":{"label":"n2k"},"values":[{"path":"navigation.headingTrue","value":1.0}]}]}"#,
        );
        let Sample::Measurement(m) = &samples[0] else {
            panic!("mesure attendue")
        };
        assert_eq!(m.source_id, "n2k");
        assert_eq!(m.measured_at, now());
    }

    #[test]
    fn ignores_other_vessels() {
        let (samples, rejected) = delta(
            r#"{"context":"vessels.urn:mrn:imo:mmsi:999999999","updates":[{"values":[{"path":"navigation.position","value":{"latitude":1,"longitude":2}}]}]}"#,
        );
        assert!(samples.is_empty());
        assert!(rejected.is_empty());
    }

    #[test]
    fn rejects_invalid_values_without_dropping_valid_ones() {
        let (samples, rejected) = delta(
            r#"{"updates":[
              {"timestamp":"hier","values":[{"path":"navigation.headingTrue","value":1}]},
              {"values":[
                {"path":"navigation.position","value":{"latitude":95,"longitude":0}},
                {"path":"navigation.position","value":{"lat":1}},
                {"path":"navigation.speedOverGround","value":"rapide"},
                {"path":"navigation.speedOverGround","value":-2},
                {"path":"environment.depth.belowTransducer","value":4.2}
              ]}]}"#,
        );
        assert_eq!(samples.len(), 1, "seule la profondeur est valide");
        assert_eq!(rejected.len(), 5, "{rejected:?}");
    }

    #[test]
    fn rejects_malformed_messages() {
        assert!(matches!(decode("{", None, now()), Err(DeltaError::Json(_))));
        assert!(matches!(
            decode("[1,2]", None, now()),
            Err(DeltaError::Json(_))
        ));
        assert!(matches!(
            decode(r#"{"updates":[{"values":[{"value":1}]}]}"#, None, now()),
            Err(DeltaError::Json(_))
        ));
        let huge = format!(r#"{{"x":"{}"}}"#, "a".repeat(MAX_MESSAGE_BYTES));
        assert!(matches!(
            decode(&huge, None, now()),
            Err(DeltaError::TooLarge(_))
        ));
        assert_eq!(
            decode(r#"{"requestId":"1"}"#, None, now()),
            Ok(Decoded::Ignored)
        );
    }

    #[test]
    fn caps_values_per_message() {
        let values: Vec<String> = (0..MAX_VALUES_PER_MESSAGE + 10)
            .map(|_| r#"{"path":"navigation.headingTrue","value":1}"#.to_owned())
            .collect();
        let (samples, rejected) = delta(&format!(
            r#"{{"updates":[{{"values":[{}]}}]}}"#,
            values.join(",")
        ));
        assert_eq!(samples.len(), MAX_VALUES_PER_MESSAGE);
        assert_eq!(rejected.len(), 1);
    }

    #[test]
    fn subscription_covers_priority_paths() {
        let msg: Value = serde_json::from_str(&subscription_message(1000)).expect("json");
        assert_eq!(msg["context"], "vessels.self");
        let paths: Vec<&str> = msg["subscribe"]
            .as_array()
            .expect("tableau")
            .iter()
            .filter_map(|s| s["path"].as_str())
            .collect();
        assert_eq!(paths.len(), 9);
        assert!(paths.contains(&"environment.wind.angleApparent"));
    }
}
