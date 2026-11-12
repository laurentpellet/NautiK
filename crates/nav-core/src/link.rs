//! Types communs aux connecteurs d'acquisition (Signal K, NMEA…) :
//! échantillons normalisés et état de la liaison.

use serde::Serialize;

use crate::{Measurement, PositionFix};

/// Échantillon normalisé produit par un connecteur.
#[derive(Debug, Clone, PartialEq)]
pub enum Sample {
    /// Position du bateau.
    Position(PositionFix),
    /// Mesure d'instrument.
    Measurement(Measurement),
}

/// État d'une liaison d'acquisition, diffusé aux interfaces. Une perte
/// de liaison est toujours signalée, jamais masquée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConnectionState {
    /// Tentative de connexion en cours.
    Connecting {
        /// Numéro de tentative.
        attempt: u32,
    },
    /// Connecté.
    Connected {
        /// Nom du serveur ou description de l'équipement.
        server_name: Option<String>,
        /// Version du serveur, si connue.
        server_version: Option<String>,
    },
    /// Liaison perdue ou impossible ; nouvelle tentative programmée.
    Disconnected {
        /// Cause.
        reason: String,
        /// Délai avant la prochaine tentative (ms).
        retry_in_ms: u64,
    },
    /// Authentification refusée (jeton absent, expiré ou révoqué).
    Unauthorized {
        /// Cause.
        reason: String,
        /// Délai avant la prochaine tentative (ms).
        retry_in_ms: u64,
    },
    /// Connecteur arrêté.
    Stopped,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_state_json_is_tagged_camel_case() {
        let json = serde_json::to_value(ConnectionState::Disconnected {
            reason: "x".to_owned(),
            retry_in_ms: 1000,
        })
        .expect("json");
        assert_eq!(json["state"], "disconnected");
        assert_eq!(json["retryInMs"], 1000);
        let json = serde_json::to_value(ConnectionState::Connected {
            server_name: Some("COM3".to_owned()),
            server_version: None,
        })
        .expect("json");
        assert_eq!(json["serverName"], "COM3");
    }
}
