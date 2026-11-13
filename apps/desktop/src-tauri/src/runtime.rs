//! État partagé de l'application, boucle périodique et connecteurs
//! d'acquisition (Signal K, NMEA 0183).
//!
//! La boucle appelle [`Engine::tick`] chaque seconde et diffuse
//! l'instantané des instruments. Les relevés (simulés ou reçus) passent
//! par [`publish_update`] : navigation diffusée, points de trace
//! persistés.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use chrono::{DateTime, Utc};
use nav_ais::Fragment;
use nav_core::{ConnectionState, Sample};
use nav_nmea0183::{NmeaEvent, Transport};
use nav_routing::NavigationStatus;
use nav_signalk::{SignalKConfig, SignalKEvent};
use nav_storage::{Db, StorageError};
use serde::Serialize;
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

/// Événement portant l'état de la liaison d'acquisition (ou `null` en
/// mode simulateur).
pub const SOURCE_STATUS_EVENT: &str = "source-status";

/// Événement portant les données rejetées ([`Rejections`]).
pub const SOURCE_REJECTIONS_EVENT: &str = "source-rejections";

/// Événement portant les cibles AIS (`AisSnapshot`).
pub const AIS_EVENT: &str = "ais-targets";

/// Période de la boucle moteur.
const TICK_PERIOD: Duration = Duration::from_secs(1);

/// Capacité du canal d'événements d'un connecteur.
const SOURCE_CHANNEL: usize = 256;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Un verrou empoisonné est récupéré : chaque opération protégée est
    // atomique en mémoire et laisse l'état cohérent.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Données rejetées par le connecteur courant (phrases invalides,
/// valeurs hors bornes, flux excessif).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rejections {
    /// Nombre de rejets depuis le démarrage du connecteur.
    pub count: u64,
    /// Cause du dernier rejet.
    pub last_reason: Option<String>,
}

/// État partagé, géré par Tauri.
pub struct AppState {
    /// Base de données.
    pub db: Db,
    /// Coffre des secrets.
    pub secrets: Arc<dyn SecretStore>,
    engine: Mutex<Engine>,
    connection: Mutex<Option<ConnectionState>>,
    rejections: Mutex<Rejections>,
    source_stop: Mutex<Option<watch::Sender<bool>>>,
    /// Génération du connecteur : les événements d'un connecteur
    /// remplacé sont ignorés.
    source_generation: AtomicU64,
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
            rejections: Mutex::new(Rejections::default()),
            source_stop: Mutex::new(None),
            source_generation: AtomicU64::new(0),
        }
    }

    /// Accès exclusif au moteur. À ne jamais conserver à travers un
    /// `.await`.
    pub fn engine(&self) -> MutexGuard<'_, Engine> {
        lock(&self.engine)
    }

    /// Dernier état connu de la liaison.
    #[must_use]
    pub fn connection(&self) -> Option<ConnectionState> {
        lock(&self.connection).clone()
    }

    /// Rejets du connecteur courant.
    #[must_use]
    pub fn rejections(&self) -> Rejections {
        lock(&self.rejections).clone()
    }

    fn is_current(&self, generation: u64) -> bool {
        self.source_generation.load(Ordering::SeqCst) == generation
    }

    /// Arrête le connecteur éventuel et invalide ses événements.
    /// Retourne la génération du prochain connecteur.
    pub fn stop_source(&self) -> u64 {
        let generation = self.source_generation.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(stop) = lock(&self.source_stop).take() {
            let _ = stop.send(true);
        }
        *lock(&self.connection) = None;
        *lock(&self.rejections) = Rejections::default();
        generation
    }

    fn set_source_stop(&self, stop: watch::Sender<bool>) {
        *lock(&self.source_stop) = Some(stop);
    }
}

/// Diffuse l'état de navigation (ou `null`).
pub fn emit_navigation(app: &AppHandle, status: Option<&NavigationStatus>) {
    if let Err(error) = app.emit(NAVIGATION_EVENT, status) {
        tracing::warn!(error = %error, "émission de l'état de navigation impossible");
    }
}

/// Diffuse l'état de la liaison (ou `null`).
pub fn emit_connection(app: &AppHandle, state: Option<&ConnectionState>) {
    if let Err(error) = app.emit(SOURCE_STATUS_EVENT, state) {
        tracing::warn!(error = %error, "émission de l'état de la source impossible");
    }
}

/// Diffuse les rejets.
pub fn emit_rejections(app: &AppHandle, rejections: &Rejections) {
    if let Err(error) = app.emit(SOURCE_REJECTIONS_EVENT, rejections) {
        tracing::warn!(error = %error, "émission des rejets impossible");
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
        let mut published = Rejections::default();
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
            crate::alarms::emit_alarms(&app, &tick.alarms);
            if !tick.alarm_events.is_empty() {
                crate::alarms::record(&state, &tick.alarm_events).await;
            }
            if let Err(error) = app.emit(INSTRUMENTS_EVENT, &tick.snapshot) {
                tracing::warn!(error = %error, "émission des instruments impossible");
            }
            if let Err(error) = app.emit(AIS_EVENT, &tick.ais) {
                tracing::warn!(error = %error, "émission des cibles AIS impossible");
            }
            // Rejets : diffusés au plus une fois par seconde, s'ils ont
            // changé.
            let rejections = state.rejections();
            if rejections != published {
                emit_rejections(&app, &rejections);
                published = rejections;
            }
        }
    });
}

/// Événement d'un connecteur, quelle que soit sa nature.
enum SourceEvent {
    State(ConnectionState),
    Sample(Sample),
    Ais(Fragment, DateTime<Utc>),
    Rejected(String),
}

impl From<SignalKEvent> for SourceEvent {
    fn from(event: SignalKEvent) -> Self {
        match event {
            SignalKEvent::State(s) => Self::State(s),
            SignalKEvent::Sample(s) => Self::Sample(s),
            SignalKEvent::Rejected(r) => Self::Rejected(r),
        }
    }
}

impl From<NmeaEvent> for SourceEvent {
    fn from(event: NmeaEvent) -> Self {
        match event {
            NmeaEvent::State(s) => Self::State(s),
            NmeaEvent::Sample(s) => Self::Sample(s),
            NmeaEvent::Ais {
                fragment,
                received_at,
            } => Self::Ais(fragment, received_at),
            NmeaEvent::Rejected(r) => Self::Rejected(r),
        }
    }
}

/// Démarre un client Signal K et le traitement de ses événements.
pub fn spawn_signalk(app: AppHandle, state: &AppState, config: SignalKConfig, generation: u64) {
    let (tx, rx) = mpsc::channel(SOURCE_CHANNEL);
    let (stop, stop_rx) = watch::channel(false);
    state.set_source_stop(stop);
    tracing::info!(url = %config.display_url(), "démarrage du client Signal K");
    tauri::async_runtime::spawn(nav_signalk::run(
        config,
        nav_signalk::ClientOptions::default(),
        tx,
        stop_rx,
    ));
    spawn_consumer(app, SourceMode::SignalK, generation, rx);
}

/// Démarre une liaison NMEA 0183 et le traitement de ses événements.
pub fn spawn_nmea(app: AppHandle, state: &AppState, transport: Transport, generation: u64) {
    let (tx, rx) = mpsc::channel(SOURCE_CHANNEL);
    let (stop, stop_rx) = watch::channel(false);
    state.set_source_stop(stop);
    tracing::info!(link = %transport.describe(), "démarrage de la liaison NMEA 0183");
    tauri::async_runtime::spawn(nav_nmea0183::run(
        transport,
        nav_nmea0183::ClientOptions::default(),
        tx,
        stop_rx,
    ));
    spawn_consumer(app, SourceMode::Nmea0183, generation, rx);
}

/// Compte une donnée rejetée par le connecteur courant.
fn reject(state: &AppState, reason: String) {
    tracing::debug!(%reason, "donnée rejetée");
    let mut r = lock(&state.rejections);
    r.count = r.count.saturating_add(1);
    r.last_reason = Some(reason);
}

fn spawn_consumer<E>(app: AppHandle, mode: SourceMode, generation: u64, mut rx: mpsc::Receiver<E>)
where
    E: Into<SourceEvent> + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            if !state.is_current(generation) {
                // Connecteur remplacé : ses données ne doivent pas se
                // mêler à celles de la nouvelle source.
                continue;
            }
            match event.into() {
                SourceEvent::State(connection) => {
                    *lock(&state.connection) = Some(connection.clone());
                    emit_connection(&app, Some(&connection));
                }
                SourceEvent::Sample(Sample::Position(fix)) => {
                    let update = {
                        let mut engine = state.engine();
                        if engine.mode() != mode {
                            continue;
                        }
                        engine.on_fix(fix)
                    };
                    if let Some(update) = update {
                        publish_update(&app, &state, update).await;
                    }
                }
                SourceEvent::Sample(Sample::Measurement(measurement)) => {
                    let mut engine = state.engine();
                    if engine.mode() == mode {
                        engine.on_measurement(measurement);
                    }
                }
                SourceEvent::Ais(fragment, received_at) => {
                    let result = {
                        let mut engine = state.engine();
                        if engine.mode() != mode {
                            continue;
                        }
                        engine.on_ais(fragment, received_at)
                    };
                    if let Err(error) = result {
                        reject(&state, error.to_string());
                    }
                }
                SourceEvent::Rejected(reason) => reject(&state, reason),
            }
        }
    });
}
