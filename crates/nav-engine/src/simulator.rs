//! Réglages du bateau simulé (vitesse), enregistrés en base.

use serde::{Deserialize, Serialize};

use crate::position::SIMULATED_SPEED_MPS;
use crate::service::Service;
use crate::CommandResult;

/// Clé des réglages du simulateur.
const SIMULATOR_SETTING: &str = "simulator";

/// m/s par nœud.
const MPS_PER_KNOT: f64 = 1_852.0 / 3_600.0;

/// Vitesse simulée maximale : 30 nœuds.
pub const MAX_SIMULATED_SPEED_MPS: f64 = 30.0 * MPS_PER_KNOT;

/// Réglages du simulateur.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulatorSettings {
    /// Vitesse du bateau simulé lorsqu'il suit une route (m/s).
    pub speed_mps: f64,
}

impl Default for SimulatorSettings {
    fn default() -> Self {
        Self {
            speed_mps: SIMULATED_SPEED_MPS,
        }
    }
}

/// Valide une vitesse simulée (0 à 30 nœuds).
///
/// # Errors
///
/// Message d'erreur si la vitesse n'est pas finie ou hors bornes.
pub fn validate(speed_mps: f64) -> Result<SimulatorSettings, String> {
    if !speed_mps.is_finite() || !(0.0..=MAX_SIMULATED_SPEED_MPS).contains(&speed_mps) {
        return Err("vitesse simulée invalide : 0 à 30 nœuds".to_owned());
    }
    Ok(SimulatorSettings { speed_mps })
}

/// Réglages enregistrés ; des réglages illisibles ou hors bornes sont
/// signalés et remplacés par les valeurs par défaut.
pub async fn load(service: &Service) -> SimulatorSettings {
    match service
        .db
        .get_setting::<SimulatorSettings>(SIMULATOR_SETTING)
        .await
    {
        Ok(Some(settings)) => validate(settings.speed_mps).unwrap_or_else(|error| {
            tracing::error!(%error, "réglage du simulateur ignoré");
            SimulatorSettings::default()
        }),
        Ok(None) => SimulatorSettings::default(),
        Err(error) => {
            tracing::error!(%error, "réglages du simulateur illisibles");
            SimulatorSettings::default()
        }
    }
}

/// Applique les réglages enregistrés au moteur (démarrage).
pub async fn restore(service: &Service) {
    let settings = load(service).await;
    service.engine().set_simulated_speed(settings.speed_mps);
}

/// Réglages courants du simulateur.
#[must_use]
pub fn current(service: &Service) -> SimulatorSettings {
    SimulatorSettings {
        speed_mps: service.engine().simulated_speed_mps(),
    }
}

/// Modifie et enregistre la vitesse du bateau simulé.
///
/// # Errors
///
/// Message lisible si la vitesse est invalide ou non enregistrée.
pub async fn set(service: &Service, speed_mps: f64) -> CommandResult<SimulatorSettings> {
    let settings = validate(speed_mps)?;
    service
        .db
        .set_setting(SIMULATOR_SETTING, &settings)
        .await
        .map_err(|e| e.to_string())?;
    service.engine().set_simulated_speed(settings.speed_mps);
    tracing::info!(speed_mps = settings.speed_mps, "vitesse simulée modifiée");
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_service as state;

    #[test]
    fn speed_is_bounded() {
        assert!(validate(0.0).is_ok());
        assert!(validate(MAX_SIMULATED_SPEED_MPS).is_ok());
        assert!(validate(MAX_SIMULATED_SPEED_MPS + 0.01).is_err());
        assert!(validate(-1.0).is_err());
        assert!(validate(f64::NAN).is_err());
    }

    #[tokio::test]
    async fn speed_is_persisted_and_applied() {
        let state = state().await;
        assert_eq!(load(&state).await, SimulatorSettings::default());
        let saved = set(&state, 5.0).await.expect("set");
        assert_eq!(saved.speed_mps, 5.0);
        assert_eq!(state.engine().simulated_speed_mps(), 5.0);
        assert_eq!(load(&state).await.speed_mps, 5.0);
        assert!(set(&state, 100.0).await.is_err());
        assert_eq!(state.engine().simulated_speed_mps(), 5.0, "inchangée");

        state.engine().set_simulated_speed(1.0);
        restore(&state).await;
        assert_eq!(state.engine().simulated_speed_mps(), 5.0);
    }

    #[tokio::test]
    async fn out_of_range_stored_value_falls_back_to_default() {
        let state = state().await;
        state
            .db
            .set_setting(SIMULATOR_SETTING, &SimulatorSettings { speed_mps: 999.0 })
            .await
            .expect("set");
        assert_eq!(load(&state).await, SimulatorSettings::default());
    }
}
