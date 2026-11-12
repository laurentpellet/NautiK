//! # nav-nmea0183
//!
//! Acquisition NMEA 0183 (série, TCP, UDP), validation des checksums et conversion vers le modèle interne.
//!
//! Crate initialisée en phase 0 ; l'implémentation arrive dans une
//! phase ultérieure (voir `TASKS.md`).

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
