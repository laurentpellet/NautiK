//! # nav-ais
//!
//! Décodage AIS, suivi des cibles et historique.
//!
//! - [`fragment`] : champs des phrases VDM/VDO et réassemblage borné ;
//! - [`armor`] : charge utile 6 bits ;
//! - [`message`] : messages 1, 2, 3, 5, 18, 19, 24 vers WGS84/SI ;
//! - [`target`] : suivi des cibles (fraîcheur, cibles perdues,
//!   historique, vecteurs) ;
//! - [`sim`] : flux simulé, au format des phrases réelles.
//!
//! Cette crate ne lit aucune liaison : elle reçoit les champs des
//! phrases déjà validées (somme de contrôle) par `nav-nmea0183`. Elle
//! n'émet jamais vers les équipements.
//!
//! CPA/TCPA : version 2 (voir `TASKS.md`, phase 9).

use chrono::{DateTime, Utc};
use thiserror::Error;

pub mod armor;
pub mod fragment;
pub mod message;
pub mod sim;
pub mod target;

pub use fragment::{Assembler, Fragment};
pub use message::{AisClass, AisReport, Dynamic, Identity, Mmsi};
pub use sim::AisSimulator;
pub use target::{AisTargetView, AisTracker, TargetState, TrackerConfig};

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-ais";

/// Erreur de décodage AIS.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AisError {
    /// Champ de phrase VDM/VDO invalide.
    #[error("AIS : {field} invalide (« {value} »)")]
    Field {
        /// Champ concerné.
        field: &'static str,
        /// Valeur reçue (tronquée).
        value: String,
    },
    /// Charge utile invalide.
    #[error("AIS : charge utile invalide ({0})")]
    InvalidPayload(&'static str),
    /// Fragment sans début ou hors ordre.
    #[error("AIS : fragment hors séquence")]
    OutOfSequence,
    /// Message fragmenté abandonné incomplet.
    #[error("AIS : message fragmenté incomplet abandonné")]
    Incomplete,
    /// Message réassemblé trop long.
    #[error("AIS : message trop long")]
    TooLong,
    /// Message trop court pour son type.
    #[error("AIS : message {message_type} trop court ({bits} bits)")]
    TooShort {
        /// Type de message.
        message_type: u8,
        /// Nombre de bits reçus.
        bits: usize,
    },
    /// MMSI invalide.
    #[error("AIS : MMSI invalide ({0})")]
    InvalidMmsi(u32),
}

/// Décodeur d'un flux de fragments : réassemblage puis décodage.
#[derive(Debug, Clone, Default)]
pub struct AisDecoder {
    assembler: Assembler,
}

impl AisDecoder {
    /// Décodeur vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Oublie les messages en cours de réassemblage.
    pub fn clear(&mut self) {
        self.assembler.clear();
    }

    /// Traite un fragment reçu à `now`. Retourne le message décodé s'il
    /// est complet et de type pris en charge.
    ///
    /// # Errors
    ///
    /// [`AisError`] si le fragment ou le message est invalide.
    pub fn push(
        &mut self,
        fragment: Fragment,
        now: DateTime<Utc>,
    ) -> Result<Option<AisReport>, AisError> {
        match self.assembler.push(fragment, now)? {
            Some(assembled) => message::decode(&assembled.bits, assembled.own_vessel),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_name_matches_crate() {
        assert_eq!(COMPONENT, env!("CARGO_PKG_NAME"));
    }

    #[test]
    fn crate_shares_engine_version() {
        assert_eq!(env!("CARGO_PKG_VERSION"), nav_core::VERSION);
    }
}
