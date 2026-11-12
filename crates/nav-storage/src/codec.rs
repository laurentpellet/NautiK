//! Conversions entre types métier et colonnes SQLite.

use chrono::{DateTime, SecondsFormat, Utc};
use nav_core::GeoPosition;
use uuid::Uuid;

use crate::StorageError;

/// Horodatage à largeur fixe : l'ordre texte suit l'ordre chronologique.
pub(crate) fn time_to_db(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

pub(crate) fn time_from_db(value: &str) -> Result<DateTime<Utc>, StorageError> {
    DateTime::parse_from_rfc3339(value)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| StorageError::Corrupt(format!("horodatage « {value} »")))
}

pub(crate) fn uuid_from_db(value: &str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(value).map_err(|_| StorageError::Corrupt(format!("identifiant « {value} »")))
}

pub(crate) fn position_from_db(lat: f64, lon: f64) -> Result<GeoPosition, StorageError> {
    GeoPosition::new(lat, lon).map_err(|_| StorageError::Corrupt(format!("position {lat}, {lon}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_round_trips_and_sorts() {
        let a = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t");
        let b = DateTime::<Utc>::from_timestamp(1_800_000_000, 500_000_000).expect("t");
        assert_eq!(time_from_db(&time_to_db(a)).expect("a"), a);
        assert_eq!(time_from_db(&time_to_db(b)).expect("b"), b);
        assert!(time_to_db(a) < time_to_db(b));
    }

    #[test]
    fn rejects_corrupt_values() {
        assert!(time_from_db("hier").is_err());
        assert!(uuid_from_db("pas-un-uuid").is_err());
        assert!(position_from_db(95.0, 0.0).is_err());
        assert!(position_from_db(f64::NAN, 0.0).is_err());
    }
}
