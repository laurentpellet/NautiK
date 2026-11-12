//! Diffusion de la position vers le frontend.
//!
//! En phase 1, seule la source simulée existe. Chaque relevé émis porte
//! sa source (`simulator`), que l'interface doit afficher.

use std::time::Duration;

use chrono::Utc;
use nav_core::simulation::StaticPositionSimulator;
use nav_core::GeoPosition;
use tauri::{AppHandle, Emitter};

/// Nom de l'événement Tauri portant un [`nav_core::PositionFix`].
pub const POSITION_EVENT: &str = "position-fix";

/// Période d'émission de la position simulée.
const SIMULATION_PERIOD: Duration = Duration::from_secs(1);

/// Position simulée (latitude, longitude en degrés WGS84) : passe de
/// Tiputa, atoll de Rangiroa (Tuamotu), environ 14°58,1′ S 147°38,3′ W.
const SIMULATED_POSITION: (f64, f64) = (-14.9680, -147.6380);

/// Position de départ du simulateur.
#[must_use]
pub fn simulated_start_position() -> Option<GeoPosition> {
    let (lat, lon) = SIMULATED_POSITION;
    GeoPosition::new(lat, lon).ok()
}

/// Démarre la diffusion périodique de la position simulée.
pub fn spawn_simulator(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Some(position) = simulated_start_position() else {
            tracing::error!("position simulée invalide : simulateur non démarré");
            return;
        };
        tracing::warn!(
            lat = position.latitude_deg(),
            lon = position.longitude_deg(),
            "source de position SIMULÉE active"
        );
        let simulator = StaticPositionSimulator::new(position);
        let mut interval = tokio::time::interval(SIMULATION_PERIOD);
        loop {
            interval.tick().await;
            if let Err(error) = app.emit(POSITION_EVENT, simulator.fix_at(Utc::now())) {
                tracing::warn!(error = %error, "émission de position impossible");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulated_position_is_tiputa_pass() {
        let p = simulated_start_position().expect("position valide");
        assert_eq!((p.latitude_deg(), p.longitude_deg()), SIMULATED_POSITION);
        // Passe de Tiputa, nord-est de l'atoll de Rangiroa.
        assert!((-15.0..-14.9).contains(&p.latitude_deg()));
        assert!((-147.7..-147.6).contains(&p.longitude_deg()));
    }
}
