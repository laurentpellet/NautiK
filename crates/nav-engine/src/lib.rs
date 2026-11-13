//! # nav-engine
//!
//! Moteur de navigation NautiK, partagé par l'application desktop (Tauri)
//! et le service autonome `nautikd`.
//!
//! - [`engine`] : état du bateau, navigation, trace, AIS et alarmes, sans
//!   aucune E/S ;
//! - [`service`] : état partagé, boucle périodique, connecteurs
//!   d'acquisition et diffusion des événements ([`EngineEvent`]) ;
//! - [`alarms`], [`routing`], [`sources`], [`simulator`] : opérations
//!   validées exposées aux interfaces (commandes Tauri, API distante).
//!
//! Aucune dépendance à Tauri ni au frontend : la logique de navigation et
//! les alarmes fonctionnent sans interface graphique.

pub mod alarms;
pub mod engine;
pub mod position;
pub mod routing;
pub mod secrets;
pub mod service;
pub mod simulator;
pub mod sources;

pub use engine::{AisSnapshot, Engine, EngineError, FixUpdate, SourceMode, Tick};
pub use secrets::{MemoryStore, SecretError, SecretStore};
pub use service::{EngineEvent, Rejections, Service};

/// Résultat d'une opération : message d'erreur lisible par l'utilisateur.
pub type CommandResult<T> = Result<T, String>;

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-engine";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_name_matches_crate() {
        assert_eq!(COMPONENT, env!("CARGO_PKG_NAME"));
    }
}
