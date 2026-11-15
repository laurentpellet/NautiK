//! Commandes du bateau simulé : délèguent à `nav_engine::simulator`.

use nav_core::GeoPosition;
use nav_engine::simulator::{self, SimulatorSettings};
use nav_engine::CommandResult;
use tauri::State;

use crate::AppState;

/// Réglages du simulateur.
#[tauri::command]
pub async fn get_simulator_settings(
    state: State<'_, AppState>,
) -> CommandResult<SimulatorSettings> {
    Ok(simulator::current(&state))
}

/// Modifie la vitesse du bateau simulé.
#[tauri::command]
pub async fn set_simulator_settings(
    state: State<'_, AppState>,
    speed_mps: f64,
) -> CommandResult<SimulatorSettings> {
    simulator::set(&state, speed_mps).await
}

/// Place le bateau simulé (refusé hors du mode simulateur). La position
/// est validée (WGS84) à la désérialisation.
#[tauri::command]
pub async fn place_simulated_boat(
    state: State<'_, AppState>,
    position: GeoPosition,
) -> CommandResult<()> {
    simulator::place_boat(&state, position)
}
