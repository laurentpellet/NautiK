//! # nav-core
//!
//! Types métier partagés du moteur NautiK : état du bateau, mesures
//! normalisées, qualité des données, arbitrage des sources et bus
//! d'événements.
//!
//! Cette crate ne dépend d'aucun framework d'interface (ni Tauri, ni
//! WebView) afin que le moteur puisse fonctionner en service autonome.
//!
//! Conventions internes :
//! - coordonnées en WGS84 ;
//! - horodatages en UTC ;
//! - unités du système SI.

use serde::{Deserialize, Serialize};

pub mod instrument_sim;
pub mod link;
pub mod measurement;
pub mod position;
pub mod simulation;
pub mod vessel;

pub use instrument_sim::InstrumentSimulator;
pub use link::{ConnectionState, Sample};
pub use measurement::{Measurement, MeasurementError, Quantity};
pub use position::{GeoPosition, PositionError, PositionFix, SourceKind};
pub use vessel::{InstrumentsSnapshot, Priorities, VesselState};

/// Version du moteur NautiK (version du workspace).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Nom du produit.
pub const PRODUCT_NAME: &str = "NautiK";

/// Avertissement de sûreté affiché par toutes les interfaces.
///
/// NautiK est une aide à la navigation et ne doit jamais être présenté
/// comme un ECDIS certifié (voir `SECURITY.md`, §11).
pub const SAFETY_NOTICE: &str = "NautiK est une aide à la navigation. \
Ce logiciel n'est pas un ECDIS certifié et ne remplace ni les cartes \
officielles, ni la veille humaine, ni les équipements réglementaires.";

/// Version du format des structures échangées avec les clients
/// (Tauri, API distante). Incrémentée à chaque changement incompatible.
pub const PROTOCOL_VERSION: u32 = 1;

/// Informations d'identification du moteur, exposées aux clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    /// Nom du produit.
    pub name: String,
    /// Version du moteur.
    pub version: String,
    /// Version du protocole d'échange.
    pub protocol_version: u32,
    /// Avertissement de sûreté à afficher.
    pub safety_notice: String,
}

impl EngineInfo {
    /// Construit les informations du moteur courant.
    #[must_use]
    pub fn current() -> Self {
        Self {
            name: PRODUCT_NAME.to_owned(),
            version: VERSION.to_owned(),
            protocol_version: PROTOCOL_VERSION,
            safety_notice: SAFETY_NOTICE.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_info_reflects_crate_version() {
        let info = EngineInfo::current();
        assert_eq!(info.name, "NautiK");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.protocol_version, PROTOCOL_VERSION);
    }

    #[test]
    fn safety_notice_states_not_ecdis() {
        assert!(SAFETY_NOTICE.contains("n'est pas un ECDIS certifié"));
    }

    #[test]
    fn engine_info_serializes_in_camel_case() -> Result<(), serde_json::Error> {
        let json = serde_json::to_value(EngineInfo::current())?;
        assert!(json.get("protocolVersion").is_some());
        assert!(json.get("safetyNotice").is_some());
        assert!(json.get("protocol_version").is_none());
        Ok(())
    }

    #[test]
    fn engine_info_round_trips_through_json() -> Result<(), serde_json::Error> {
        let info = EngineInfo::current();
        let decoded: EngineInfo = serde_json::from_str(&serde_json::to_string(&info)?)?;
        assert_eq!(decoded, info);
        Ok(())
    }

    #[test]
    fn engine_info_rejects_malformed_json() {
        let result = serde_json::from_str::<EngineInfo>(r#"{"name":"NautiK"}"#);
        assert!(result.is_err());
    }
}
