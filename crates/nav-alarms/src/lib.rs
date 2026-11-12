//! # nav-alarms
//!
//! Règles d'alarme, machine à états, historique et notifications. Fonctionne sans interface graphique.
//!
//! Crate initialisée en phase 0 ; l'implémentation arrive dans une
//! phase ultérieure (voir `TASKS.md`).

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
