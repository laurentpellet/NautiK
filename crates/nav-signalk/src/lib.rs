//! # nav-signalk
//!
//! Client Signal K : découverte HTTP, flux WebSocket, authentification
//! par jeton, abonnement aux chemins prioritaires, reconnexion
//! automatique, normalisation des données vers les types de `nav-core`.
//!
//! Le client ne fait que lire : aucune commande n'est envoyée aux
//! équipements du bord.

pub mod client;
pub mod config;
pub mod delta;

pub use client::{run, ClientOptions, ConnectionState, SignalKEvent};
pub use config::{ConfigError, SignalKConfig, Token};
pub use delta::Sample;

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-signalk";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_name_matches_crate() {
        assert_eq!(COMPONENT, env!("CARGO_PKG_NAME"));
    }
}
