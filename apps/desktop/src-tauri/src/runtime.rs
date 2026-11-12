//! État partagé de l'application, boucle périodique et flux Signal K.
//!
//! La boucle appelle [`Engine::tick`] chaque seconde et diffuse
//! l'instantané des instruments. Les relevés (simulés ou Signal K)
//! passent par [`publish_update`] : navigation diffusée, points de trace
//! persistés.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use chrono::Utc;
use nav_routing::NavigationStatus;
use nav_signalk::{ClientOptions, ConnectionState, Sample, SignalKConfig, SignalKEvent};
use nav_storage::{Db, StorageError};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, watch};

use crate::engine::{Engine, FixUpdate, SourceMode};
use crate::secrets::SecretStore;

/// Événement portant un `InstrumentsSnapshot` (position et instruments,
/// avec leur fraîcheur calculée par le moteur).
pub const INSTRUMENTS_EVENT: &str = "instruments";

/// Événement portant un `NavigationStatus`, ou `null` lorsque la
/// navigation est désactivée.
pub const NAVIGATION_EVENT: &str = "navigation-status";

/// Événement portant l'état de la connexion Signal K (ou `null` en mode
/// simulateur).
pub const SIGNALK_STATUS_EVENT: &str = "signalk-status";

/// Période de la boucle moteur.
const TICK_PERIOD: Duration = Duration::from_secs(1);

/// Capacité du canal d'événements Signal K.
const SIGNALK_CHANNEL: usize = 256;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Un verrou empoisonné est récupéré : chaque opération protégée est
    // atomique en mémoire et laisse l'état cohérent.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// État partagé, géré par Tauri.
pub struct AppState {
    /// Base de données.
    pub db: Db,
    /// Coffre des secrets.
    pub secrets: Arc<dyn SecretStore>,
    engine: Mutex<Engine>,
    connection: Mutex<Option<ConnectionState>>,
    signalk_stop: Mutex<Option<watch::Sender<bool>>>,
    /// Génération de la session Signal K : les événements d'une session
    /// remplacée sont ignorés.
    signalk_generation: AtomicU64,
}

impl AppState {
    /// Crée l'état partagé.
    #[must_use]
    pub fn new(db: Db, engine: Engine, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            db,
            secrets,
            engine: Mutex::new(engine),
            connection: Mutex::new(None),
            signalk_stop: Mutex::new(None),
            signalk_generation: AtomicU64::new(0),
        }
    }

    /// Accès exclusif au moteur. À ne jamais conserver à travers un
    /// `.await`.
    pub fn engine(&self) -> MutexGuard<'_, Engine> {
        lock(&self.engine)
    }

    /// Dernier état connu de la connexion Signal K.
    #[must_use]
    pub fn connection(&self) -> Option<ConnectionState> {
        lock(&self.connection).clone()
    }

    fn is_current(&self, generation: u64) -> bool {
        self.signalk_generation.load(Ordering::SeqCst) == generation
    }

    /// Arrête le client Signal K éventuel et invalide ses événements.
    /// Retourne la génération de la prochaine session.
    pub fn stop_signalk(&self) -> u64 {
        let generation = self.signalk_generation.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(stop) = lock(&self.signalk_stop).take() {
            let _ = stop.send(true);
        }
        *lock(&self.connection) = None;
        generation
    }

    fn set_signalk_stop(&self, stop: watch::Sender<bool>) {
        *lock(&self.signalk_stop) = Some(stop);
    }
}

/// Diffuse l'état de navigation (ou `null`).
pub fn emit_navigation(app: &AppHandle, status: Option<&NavigationStatus>) {
    if let Err(error) = app.emit(NAVIGATION_EVENT, status) {
        tracing::warn!(error = %error, "émission de l'état de navigation impossible");
    }
}

/// Diffuse l'état de connexion Signal K (ou `null`).
pub fn emit_connection(app: &AppHandle, state: Option<&ConnectionState>) {
    if let Err(error) = app.emit(SIGNALK_STATUS_EVENT, state) {
        tracing::warn!(error = %error, "émission de l'état Signal K impossible");
    }
}

/// Diffuse les conséquences d'un relevé et persiste le point de trace.
pub async fn publish_update(app: &AppHandle, state: &AppState, update: FixUpdate) {
    if let Some(status) = &update.navigation {
        if let Some(name) = &status.reached_waypoint {
            tracing::info!(waypoint = %name, "waypoint atteint");
        }
        emit_navigation(app, Some(status));
    }
    if let Some((track_id, point)) = update.track_point {
        match state.db.append_track_points(track_id, &[point]).await {
            Ok(()) => {}
            // Arrêt de l'enregistrement entre le relevé et l'écriture.
            Err(StorageError::TrackNotRecording(_)) => {}
            Err(error) => tracing::error!(error = %error, "point de trace non enregistré"),
        }
    }
}

/// Démarre la boucle moteur périodique.
pub fn spawn_engine_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(TICK_PERIOD);
        loop {
            interval.tick().await;
            let Some(state) = app.try_state::<AppState>() else {
                tracing::error!("état applicatif absent : boucle moteur arrêtée");
                return;
            };
            let tick = state.engine().tick(Utc::now());
            if let Some(update) = tick.update {
                publish_update(&app, &state, update).await;
            }
            if let Err(error) = app.emit(INSTRUMENTS_EVENT, &tick.snapshot) {
                tracing::warn!(error = %error, "émission des instruments impossible");
            }
        }
    });
}

/// Démarre un client Signal K et le traitement de ses événements.
pub fn spawn_signalk(app: AppHandle, state: &AppState, config: SignalKConfig, generation: u64) {
    let (tx, mut rx) = mpsc::channel(SIGNALK_CHANNEL);
    let (stop, stop_rx) = watch::channel(false);
    state.set_signalk_stop(stop);
    tracing::info!(url = %config.display_url(), "démarrage du client Signal K");
    tauri::async_runtime::spawn(nav_signalk::run(
        config,
        ClientOptions::default(),
        tx,
        stop_rx,
    ));
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            if !state.is_current(generation) {
                // Session remplacée : ses données ne doivent pas se mêler
                // à celles de la nouvelle source.
                continue;
            }
            match event {
                SignalKEvent::State(connection) => {
                    *lock(&state.connection) = Some(connection.clone());
                    emit_connection(&app, Some(&connection));
                }
                SignalKEvent::Sample(Sample::Position(fix)) => {
                    let update = {
                        let mut engine = state.engine();
                        if engine.mode() != SourceMode::SignalK {
                            continue;
                        }
                        engine.on_fix(fix)
                    };
                    publish_update(&app, &state, update).await;
                }
                SignalKEvent::Sample(Sample::Measurement(measurement)) => {
                    let mut engine = state.engine();
                    if engine.mode() == SourceMode::SignalK {
                        engine.on_measurement(measurement);
                    }
                }
                SignalKEvent::Rejected(reason) => {
                    tracing::debug!(%reason, "donnée Signal K rejetée");
                }
            }
        }
    });
}
