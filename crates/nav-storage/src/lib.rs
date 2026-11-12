//! # nav-storage
//!
//! Persistance SQLite : migrations, repositories et transactions. Seul
//! point d'accès à la base.
//!
//! Conventions de stockage :
//! - identifiants : UUID texte ;
//! - coordonnées : degrés WGS84 ;
//! - horodatages : RFC 3339 UTC à 9 décimales, de sorte que l'ordre
//!   lexicographique soit l'ordre chronologique ;
//! - unités SI (m, m/s, rad).

mod codec;
mod db;
mod error;
mod import;
mod routes;
mod settings;
mod tracks;
mod waypoints;

pub use db::Db;
pub use error::StorageError;
pub use tracks::TrackSummary;

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-storage";

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
