//! # nav-nmea0183
//!
//! Acquisition NMEA 0183 (série, TCP, UDP, rejeu de fichier), validation
//! des checksums et conversion vers le modèle interne.
//!
//! - [`sentence`] : délimiteurs, adresse, somme de contrôle ;
//! - [`decode`] : RMC, GGA, GLL, VTG, HDT, HDG, HDM, VHW, DPT, DBT, MWV
//!   vers [`nav_core::Sample`] (WGS84, SI, UTC), fragments AIS VDM/VDO
//!   vers `nav-ais` ;
//! - [`framer`] : découpage borné d'un flux en lignes ;
//! - [`client`] : liaisons, reconnexion, écrêtage du débit.
//!
//! Cette crate est en lecture seule : elle n'émet jamais de phrase vers
//! les équipements.

pub mod client;
pub mod config;
pub mod decode;
pub mod framer;
pub mod sentence;

pub use client::{run, serial_ports, ClientOptions, NmeaEvent};
pub use config::{ConfigError, Transport};
pub use decode::{decode, DecodeContext, DecodeError, Decoded};

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-nmea0183";

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
