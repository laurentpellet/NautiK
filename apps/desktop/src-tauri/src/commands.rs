//! Commandes Tauri exposées au frontend.
//!
//! Chaque commande valide ses entrées et délègue au moteur. Les
//! structures retournées sont typées et versionnées (`nav-core`).

use nav_core::EngineInfo;
use tauri::State;

use crate::charts::{ChartCatalogView, ChartState};

/// Retourne les informations d'identification du moteur.
#[tauri::command]
pub fn get_engine_info() -> EngineInfo {
    EngineInfo::current()
}

/// Retourne le catalogue des cartes (chargé à la première demande).
#[tauri::command]
pub async fn get_chart_catalog(state: State<'_, ChartState>) -> Result<ChartCatalogView, ()> {
    Ok(state.view().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_engine_info_delegates_to_core() {
        assert_eq!(get_engine_info(), EngineInfo::current());
    }
}
