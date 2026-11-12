//! Boucle moteur : sources, état du bateau, navigation active, trace.
//!
//! Ce module est volontairement indépendant de Tauri (aucune E/S) : il
//! combine les briques de `nav-core` et `nav-routing` et peut être
//! réutilisé tel quel par le service `nautikd`. La couche Tauri
//! (`runtime`) l'alimente et se charge des événements et de la
//! persistance.
//!
//! Deux modes de source, exclusifs :
//! - **simulateur** : un bateau simulé suit la route active ; chaque
//!   relevé est marqué simulé ;
//! - **Signal K** : les relevés viennent du serveur. En cas de perte du
//!   flux, aucune position n'est inventée : la dernière position connue
//!   devient obsolète et le reste jusqu'au retour des données.

use chrono::{DateTime, Utc};
use nav_core::{GeoPosition, InstrumentsSnapshot, Measurement, PositionFix, VesselState};
use nav_routing::navigation::{NavigationError, DEFAULT_ARRIVAL_RADIUS_M};
use nav_routing::recorder::RecorderError;
use nav_routing::{
    ActiveNavigation, NavigationStatus, RecorderConfig, Route, RouteSimulator, TrackPoint,
    TrackRecorder,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Erreur d'une opération moteur.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum EngineError {
    /// Un enregistrement de trace est déjà en cours.
    #[error("un enregistrement de trace est déjà en cours")]
    AlreadyRecording,
    /// Configuration de navigation invalide.
    #[error(transparent)]
    Navigation(#[from] NavigationError),
    /// Configuration d'enregistrement invalide.
    #[error(transparent)]
    Recorder(#[from] RecorderError),
}

/// Source des données de navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceMode {
    /// Bateau simulé (développement, démonstration).
    #[default]
    Simulator,
    /// Serveur Signal K.
    SignalK,
}

/// Conséquences du traitement d'un relevé de position.
#[derive(Debug, Clone, PartialEq)]
pub struct FixUpdate {
    /// Relevé retenu (complété par les vitesses et routes fraîches).
    pub fix: PositionFix,
    /// État de navigation, si une route est active.
    pub navigation: Option<NavigationStatus>,
    /// Point à ajouter à la trace en cours (identifiant de trace, point).
    pub track_point: Option<(Uuid, TrackPoint)>,
}

/// Résultat d'un pas périodique de la boucle moteur.
#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    /// Relevé simulé traité pendant ce pas (mode simulateur seulement).
    pub update: Option<FixUpdate>,
    /// Instantané de l'état du bateau, avec la fraîcheur des données.
    pub snapshot: InstrumentsSnapshot,
}

#[derive(Debug, Clone)]
struct Recording {
    track_id: Uuid,
    recorder: TrackRecorder,
}

/// État du moteur.
#[derive(Debug, Clone)]
pub struct Engine {
    mode: SourceMode,
    simulator: RouteSimulator,
    vessel: VesselState,
    navigation: Option<ActiveNavigation>,
    recording: Option<Recording>,
}

impl Engine {
    /// Crée un moteur en mode simulateur, bateau immobile à `start`.
    #[must_use]
    pub fn new(start: GeoPosition, simulated_speed_mps: f64) -> Self {
        Self {
            mode: SourceMode::Simulator,
            simulator: RouteSimulator::new(start, simulated_speed_mps),
            vessel: VesselState::new(),
            navigation: None,
            recording: None,
        }
    }

    /// Mode de source courant.
    #[must_use]
    pub fn mode(&self) -> SourceMode {
        self.mode
    }

    /// Change de source. Les valeurs de l'ancienne source sont oubliées
    /// (aucun mélange de données simulées et réelles).
    pub fn set_mode(&mut self, mode: SourceMode) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        self.vessel.clear();
        let route = match mode {
            SourceMode::Simulator => self.navigation.as_ref().map(ActiveNavigation::route),
            SourceMode::SignalK => None,
        };
        self.simulator.follow(route);
    }

    /// Active une route. En mode simulateur, le bateau simulé la suit.
    ///
    /// # Errors
    ///
    /// [`EngineError::Navigation`] si la configuration est invalide.
    pub fn activate(&mut self, route: Route) -> Result<(), EngineError> {
        let navigation = ActiveNavigation::new(route, DEFAULT_ARRIVAL_RADIUS_M)?;
        if self.mode == SourceMode::Simulator {
            self.simulator.follow(Some(navigation.route()));
        }
        self.navigation = Some(navigation);
        Ok(())
    }

    /// Désactive la navigation (le bateau simulé s'arrête). Retourne
    /// `false` si aucune route n'était active.
    pub fn deactivate(&mut self) -> bool {
        self.simulator.follow(None);
        self.navigation.take().is_some()
    }

    /// Identifiant de la route active.
    #[must_use]
    pub fn active_route_id(&self) -> Option<Uuid> {
        self.navigation.as_ref().map(|n| n.route().id)
    }

    /// Démarre l'enregistrement dans la trace `track_id` (déjà créée).
    ///
    /// # Errors
    ///
    /// [`EngineError::AlreadyRecording`] si un enregistrement est en cours.
    pub fn start_recording(&mut self, track_id: Uuid) -> Result<(), EngineError> {
        if self.recording.is_some() {
            return Err(EngineError::AlreadyRecording);
        }
        let recorder = TrackRecorder::new(RecorderConfig::default())?;
        self.recording = Some(Recording { track_id, recorder });
        Ok(())
    }

    /// Arrête l'enregistrement ; retourne la trace concernée.
    pub fn stop_recording(&mut self) -> Option<Uuid> {
        self.recording.take().map(|r| r.track_id)
    }

    /// Trace en cours d'enregistrement.
    #[must_use]
    pub fn recording_track_id(&self) -> Option<Uuid> {
        self.recording.as_ref().map(|r| r.track_id)
    }

    /// Les données courantes sont-elles simulées ? (Sert à nommer les
    /// traces.)
    #[must_use]
    pub fn is_simulated(&self) -> bool {
        self.mode == SourceMode::Simulator
    }

    /// Traite un relevé de position : état du bateau, navigation, trace.
    pub fn on_fix(&mut self, fix: PositionFix) -> FixUpdate {
        let fix = self.vessel.update_position(fix);
        let navigation = self.navigation.as_mut().map(|n| n.update(&fix));
        let track_point = self
            .recording
            .as_mut()
            .and_then(|r| r.recorder.observe(&fix).map(|p| (r.track_id, p)));
        FixUpdate {
            fix,
            navigation,
            track_point,
        }
    }

    /// Enregistre une mesure d'instrument. Retourne `false` si elle est
    /// ignorée (plus ancienne que la valeur connue).
    pub fn on_measurement(&mut self, measurement: Measurement) -> bool {
        self.vessel.update_measurement(measurement)
    }

    /// Pas périodique : avance le simulateur (en mode simulateur) et
    /// produit l'instantané de l'état du bateau.
    pub fn tick(&mut self, now: DateTime<Utc>) -> Tick {
        let update = match self.mode {
            SourceMode::Simulator => {
                let fix = self.simulator.step(now);
                Some(self.on_fix(fix))
            }
            SourceMode::SignalK => None,
        };
        Tick {
            update,
            snapshot: self.vessel.snapshot(now),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use nav_core::{Quantity, SourceKind};

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    fn t(seconds: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t") + Duration::seconds(seconds)
    }

    fn route() -> Route {
        Route::from_positions("R", &[p(0.0, 0.01), p(0.01, 0.01)], t(0)).expect("route")
    }

    fn signalk_fix(seconds: i64, lat: f64) -> PositionFix {
        PositionFix {
            position: p(lat, 0.0),
            cog_rad: None,
            sog_mps: None,
            source: SourceKind::SignalK,
            source_id: "nmea0183.GP".to_owned(),
            measured_at: t(seconds),
            received_at: t(seconds),
        }
    }

    #[test]
    fn idle_simulator_reports_simulated_position_only() {
        let mut engine = Engine::new(p(0.0, 0.0), 4.0);
        assert!(engine.is_simulated());
        let tick = engine.tick(t(0));
        let update = tick.update.expect("relevé simulé");
        assert_eq!(update.fix.position, p(0.0, 0.0));
        assert!(update.fix.source.is_simulated());
        assert!(update.navigation.is_none());
        assert!(update.track_point.is_none());
        let pos = tick.snapshot.position.expect("position");
        assert!(!pos.stale);
    }

    #[test]
    fn active_route_drives_simulator_and_navigation() {
        let mut engine = Engine::new(p(0.0, 0.0), 4.0);
        let r = route();
        let id = r.id;
        engine.activate(r).expect("activate");
        assert_eq!(engine.active_route_id(), Some(id));
        engine.tick(t(0));
        let update = engine.tick(t(10)).update.expect("relevé");
        let status = update.navigation.expect("navigation");
        assert_eq!(status.route_id, id);
        assert!(status.simulated);
        assert!(status.distance_to_target_m < 1_113.2 - 39.0);
        assert!(engine.deactivate());
        assert!(!engine.deactivate());
        let still = engine.tick(t(20)).update.expect("relevé");
        assert!(still.navigation.is_none());
        assert_eq!(still.fix.sog_mps, Some(0.0), "arrêt à la désactivation");
    }

    #[test]
    fn route_is_followed_to_the_end() {
        let mut engine = Engine::new(p(0.0, 0.0), 10.0);
        engine.activate(route()).expect("activate");
        let mut last = None;
        for s in (0..=600).step_by(5) {
            last = engine.tick(t(s)).update.and_then(|u| u.navigation);
        }
        assert!(last.expect("status").finished);
    }

    #[test]
    fn recording_emits_points_for_its_track() {
        let mut engine = Engine::new(p(0.0, 0.0), 5.0);
        engine.activate(route()).expect("activate");
        let track = Uuid::new_v4();
        engine.start_recording(track).expect("start");
        assert_eq!(
            engine.start_recording(Uuid::new_v4()),
            Err(EngineError::AlreadyRecording)
        );
        assert_eq!(engine.recording_track_id(), Some(track));
        let points: Vec<_> = (0..=60)
            .filter_map(|s| engine.tick(t(s)).update.and_then(|u| u.track_point))
            .collect();
        // Premier point, puis un toutes les ≥ 5 s (25 m parcourus > 10 m).
        assert!((12..=13).contains(&points.len()), "{}", points.len());
        assert!(points.iter().all(|(id, _)| *id == track));
        assert_eq!(engine.stop_recording(), Some(track));
        assert_eq!(engine.stop_recording(), None);
    }

    #[test]
    fn signalk_mode_never_invents_positions() {
        let mut engine = Engine::new(p(0.0, 0.0), 5.0);
        engine.tick(t(0));
        engine.set_mode(SourceMode::SignalK);
        assert!(!engine.is_simulated());
        let tick = engine.tick(t(1));
        assert!(tick.update.is_none(), "pas de relevé simulé");
        assert!(
            tick.snapshot.position.is_none(),
            "données simulées oubliées"
        );

        engine.on_fix(signalk_fix(2, -14.9));
        assert!(!engine.tick(t(3)).snapshot.position.expect("pos").stale);
        // Plus de données : la dernière position devient obsolète.
        let later = engine.tick(t(10)).snapshot.position.expect("pos");
        assert!(later.stale);
        assert_eq!(later.fix.position, p(-14.9, 0.0), "aucune extrapolation");
    }

    #[test]
    fn signalk_fixes_drive_navigation_and_recording() {
        let mut engine = Engine::new(p(0.0, 0.0), 5.0);
        engine.set_mode(SourceMode::SignalK);
        engine.activate(route()).expect("activate");
        engine.start_recording(Uuid::new_v4()).expect("rec");
        let sog = Measurement::new(
            Quantity::SpeedOverGround,
            3.0,
            SourceKind::SignalK,
            "nmea0183.GP",
            t(0),
            t(0),
        )
        .expect("m");
        assert!(engine.on_measurement(sog));
        let update = engine.on_fix(signalk_fix(0, 0.0));
        assert_eq!(update.fix.sog_mps, Some(3.0), "complété par la SOG fraîche");
        let status = update.navigation.expect("navigation");
        assert!(!status.simulated);
        assert!(status.eta_target.is_some());
        assert!(update.track_point.is_some());
        // Le simulateur ne bouge pas en mode Signal K.
        assert!(engine.tick(t(30)).update.is_none());
    }

    #[test]
    fn returning_to_simulator_resumes_active_route() {
        let mut engine = Engine::new(p(0.0, 0.0), 5.0);
        engine.set_mode(SourceMode::SignalK);
        engine.activate(route()).expect("activate");
        engine.set_mode(SourceMode::Simulator);
        engine.tick(t(0));
        let fix = engine.tick(t(10)).update.expect("relevé").fix;
        assert_eq!(fix.sog_mps, Some(5.0));
    }
}
