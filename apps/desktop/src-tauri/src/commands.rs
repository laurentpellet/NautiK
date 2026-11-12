//! Commandes Tauri exposées au frontend.
//!
//! Chaque commande valide ses entrées et délègue au moteur. Les
//! structures retournées sont celles de `nav-core` (typées et versionnées).

use nav_core::EngineInfo;

/// Retourne les informations d'identification du moteur.
#[tauri::command]
pub fn get_engine_info() -> EngineInfo {
    EngineInfo::current()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_engine_info_delegates_to_core() {
        assert_eq!(get_engine_info(), EngineInfo::current());
    }
}
