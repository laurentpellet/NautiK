//! Paramètres de la source de position simulée.
//!
//! En phase 2, seule la source simulée existe : un bateau qui part de la
//! passe de Tiputa et suit la route active. Chaque relevé porte sa
//! source (`simulator`), que l'interface doit afficher.

use nav_core::GeoPosition;

/// Position simulée de départ (latitude, longitude en degrés WGS84) :
/// passe de Tiputa, atoll de Rangiroa (Tuamotu), environ
/// 14°58,1′ S 147°38,3′ W.
const SIMULATED_POSITION: (f64, f64) = (-14.9680, -147.6380);

/// Vitesse du bateau simulé lorsqu'il suit une route : 8 nœuds, en m/s.
pub const SIMULATED_SPEED_MPS: f64 = 8.0 * 1_852.0 / 3_600.0;

/// Position de départ du simulateur.
#[must_use]
pub fn simulated_start_position() -> Option<GeoPosition> {
    let (lat, lon) = SIMULATED_POSITION;
    GeoPosition::new(lat, lon).ok()
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

    #[test]
    fn simulated_speed_is_eight_knots() {
        assert!((SIMULATED_SPEED_MPS - 4.115_6).abs() < 1e-4);
    }
}
