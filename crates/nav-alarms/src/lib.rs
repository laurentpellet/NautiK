//! # nav-alarms
//!
//! Règles d'alarme, machine à états, historique et notifications.
//! Fonctionne sans interface graphique et sans E/S : le moteur appelle
//! [`AlarmManager::evaluate`] périodiquement et persiste les
//! [`AlarmEvent`] retournés.
//!
//! Alarmes :
//! - **mouillage** (ALR-001) : cercle d'évitage autour de l'ancre,
//!   temporisation, hystérésis, résolution après un retour stable ;
//! - **perte de position** (ALR-002) : position absente ou obsolète.
//!
//! Principes de sûreté :
//! - une donnée absente ou obsolète ne résout jamais une alarme ;
//! - une alarme acquittée reste affichée tant que la situation dure ;
//! - désactiver une alarme est une action explicite, historisée.

mod machine;
mod manager;
mod rules;

pub use machine::{AlarmState, Condition, Machine, Transition};
pub use manager::{
    AlarmEvent, AlarmKind, AlarmManager, AlarmSettings, AlarmView, AlarmsSnapshot, AnchorView,
    EventDetails, EventKind,
};
pub use rules::{
    AnchorConfig, PositionLostConfig, RuleError, CLEAR_DELAY_S, DEFAULT_ANCHOR_DELAY_S,
    DEFAULT_ANCHOR_RADIUS_M, DEFAULT_POSITION_LOST_DELAY_S, MAX_ANCHOR_RADIUS_M, MAX_DELAY_S,
    MIN_ANCHOR_RADIUS_M, MIN_HYSTERESIS_M,
};

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-alarms";

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
