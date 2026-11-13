//! État partagé du moteur, boucle périodique, connecteurs d'acquisition
//! (Signal K, NMEA 0183) et diffusion des événements.
//!
//! La boucle ([`Service::run`]) appelle [`Engine::tick`] chaque seconde.
//! Tout ce qui change (instruments, navigation, liaison, rejets, AIS,
//! alarmes) est publié sur un canal de diffusion ([`EngineEvent`]) :
//! l'application desktop le relaie en événements Tauri, le service
//! `nautikd` aux clients WebSocket. Aucune interface n'est nécessaire au
//! fonctionnement de la boucle ni des alarmes.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use chrono::{DateTime, Utc};
use nav_ais::Fragment;
use nav_alarms::AlarmsSnapshot;
use nav_core::{ConnectionState, InstrumentsSnapshot, Sample};
use nav_nmea0183::{NmeaEvent, Transport};
use nav_routing::NavigationStatus;
use nav_signalk::{SignalKConfig, SignalKEvent};
use nav_storage::{Db, StorageError};
use serde::Serialize;
use tokio::sync::{broadcast, mpsc, watch};

use crate::engine::{AisSnapshot, Engine, FixUpdate, SourceMode};
use crate::secrets::SecretStore;

/// Période de la boucle moteur.
pub const TICK_PERIOD: Duration = Duration::from_secs(1);

/// Capacité du canal d'événements d'un connecteur.
const SOURCE_CHANNEL: usize = 256;

/// Capacité du canal de diffusion : un abonné trop lent perd les plus
/// anciens événements (il est prévenu et reçoit les suivants).
const EVENT_CHANNEL: usize = 128;

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

/// Événement diffusé par le moteur.
///
/// Sérialisé `{"type": "...", "data": ...}` pour les clients distants.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum EngineEvent {
    /// Position et instruments, avec leur fraîcheur.
    Instruments(InstrumentsSnapshot),
    /// État de navigation (`None` : navigation désactivée).
    Navigation(Option<NavigationStatus>),
    /// État de la liaison d'acquisition (`None` : simulateur ou arrêt).
    SourceStatus(Option<ConnectionState>),
    /// Données rejetées par la liaison courante.
    Rejections(Rejections),
    /// Cibles AIS.
    Ais(AisSnapshot),
    /// État des alarmes.
    Alarms(AlarmsSnapshot),
}

impl EngineEvent {
    /// Nom de l'événement côté desktop (événements Tauri).
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Instruments(_) => "instruments",
            Self::Navigation(_) => "navigation-status",
            Self::SourceStatus(_) => "source-status",
            Self::Rejections(_) => "source-rejections",
            Self::Ais(_) => "ais-targets",
            Self::Alarms(_) => "alarms",
        }
    }
}

/// Dernières valeurs publiées, envoyées à tout nouvel abonné.
#[derive(Debug, Default)]
struct Latest {
    instruments: Option<InstrumentsSnapshot>,
    ais: Option<AisSnapshot>,
    navigation: Option<NavigationStatus>,
    published_rejections: Rejections,
}

/// État partagé du moteur.
pub struct Service {
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
    events: broadcast::Sender<EngineEvent>,
    latest: Mutex<Latest>,
    /// Instant du dernier pas de boucle (ms depuis l'époque Unix, 0 si
    /// aucun) : sert au chien de garde.
    last_tick_ms: AtomicI64,
    shutdown: watch::Sender<bool>,
}

impl Service {
    /// Crée l'état partagé.
    #[must_use]
    pub fn new(db: Db, engine: Engine, secrets: Arc<dyn SecretStore>) -> Arc<Self> {
        let (events, _) = broadcast::channel(EVENT_CHANNEL);
        let (shutdown, _) = watch::channel(false);
        Arc::new(Self {
            db,
            secrets,
            engine: Mutex::new(engine),
            connection: Mutex::new(None),
            rejections: Mutex::new(Rejections::default()),
            source_stop: Mutex::new(None),
            source_generation: AtomicU64::new(0),
            events,
            latest: Mutex::new(Latest::default()),
            last_tick_ms: AtomicI64::new(0),
            shutdown,
        })
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

    /// Instant du dernier pas de la boucle moteur.
    #[must_use]
    pub fn last_tick(&self) -> Option<DateTime<Utc>> {
        match self.last_tick_ms.load(Ordering::SeqCst) {
            0 => None,
            ms => DateTime::from_timestamp_millis(ms),
        }
    }

    /// Abonnement aux événements du moteur.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    /// Publie un événement (sans effet s'il n'y a aucun abonné).
    pub fn publish(&self, event: EngineEvent) {
        if let EngineEvent::Navigation(status) = &event {
            lock(&self.latest).navigation.clone_from(status);
        }
        let _ = self.events.send(event);
    }

    /// État courant complet, sous forme d'événements : envoyé à un client
    /// qui vient de s'abonner.
    #[must_use]
    pub fn current_events(&self) -> Vec<EngineEvent> {
        let alarms = self.engine().alarms(Utc::now());
        let latest = lock(&self.latest);
        let mut events = Vec::with_capacity(6);
        if let Some(instruments) = &latest.instruments {
            events.push(EngineEvent::Instruments(instruments.clone()));
        }
        if let Some(ais) = &latest.ais {
            events.push(EngineEvent::Ais(ais.clone()));
        }
        events.push(EngineEvent::Alarms(alarms));
        events.push(EngineEvent::Navigation(latest.navigation.clone()));
        events.push(EngineEvent::SourceStatus(self.connection()));
        events.push(EngineEvent::Rejections(self.rejections()));
        events
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

    /// Demande l'arrêt de la boucle moteur et du connecteur.
    pub fn shutdown(&self) {
        self.stop_source();
        let _ = self.shutdown.send(true);
    }

    /// Diffuse les conséquences d'un relevé et persiste le point de trace.
    pub async fn publish_update(&self, update: FixUpdate) {
        if let Some(status) = update.navigation {
            if let Some(name) = &status.reached_waypoint {
                tracing::info!(waypoint = %name, "waypoint atteint");
            }
            self.publish(EngineEvent::Navigation(Some(status)));
        }
        if let Some((track_id, point)) = update.track_point {
            match self.db.append_track_points(track_id, &[point]).await {
                Ok(()) => {}
                // Arrêt de l'enregistrement entre le relevé et l'écriture.
                Err(StorageError::TrackNotRecording(_)) => {}
                Err(error) => tracing::error!(error = %error, "point de trace non enregistré"),
            }
        }
    }

    /// Un pas de la boucle moteur à l'instant `now` : simulateurs,
    /// alarmes, historique, diffusion.
    pub async fn step(&self, now: DateTime<Utc>) {
        let tick = self.engine().tick(now);
        if let Some(update) = tick.update {
            self.publish_update(update).await;
        }
        if !tick.alarm_events.is_empty() {
            crate::alarms::record(self, &tick.alarm_events).await;
        }
        self.publish(EngineEvent::Alarms(tick.alarms));
        {
            let mut latest = lock(&self.latest);
            latest.instruments = Some(tick.snapshot.clone());
            latest.ais = Some(tick.ais.clone());
        }
        self.publish(EngineEvent::Instruments(tick.snapshot));
        self.publish(EngineEvent::Ais(tick.ais));
        // Rejets : diffusés au plus une fois par pas, s'ils ont changé.
        let rejections = self.rejections();
        let changed = {
            let mut latest = lock(&self.latest);
            let changed = latest.published_rejections != rejections;
            if changed {
                latest.published_rejections = rejections.clone();
            }
            changed
        };
        if changed {
            self.publish(EngineEvent::Rejections(rejections));
        }
        self.last_tick_ms
            .store(now.timestamp_millis(), Ordering::SeqCst);
    }

    /// Boucle moteur périodique, jusqu'à [`Service::shutdown`].
    pub async fn run(self: Arc<Self>) {
        let mut stop = self.shutdown.subscribe();
        let mut interval = tokio::time::interval(TICK_PERIOD);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            if *stop.borrow() {
                break;
            }
            tokio::select! {
                _ = interval.tick() => self.step(Utc::now()).await,
                changed = stop.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
            }
        }
        tracing::info!("boucle moteur arrêtée");
    }

    /// Démarre un client Signal K et le traitement de ses événements.
    pub fn spawn_signalk(self: &Arc<Self>, config: SignalKConfig, generation: u64) {
        let (tx, rx) = mpsc::channel(SOURCE_CHANNEL);
        let (stop, stop_rx) = watch::channel(false);
        self.set_source_stop(stop);
        tracing::info!(url = %config.display_url(), "démarrage du client Signal K");
        tokio::spawn(nav_signalk::run(
            config,
            nav_signalk::ClientOptions::default(),
            tx,
            stop_rx,
        ));
        self.spawn_consumer(SourceMode::SignalK, generation, rx);
    }

    /// Démarre une liaison NMEA 0183 et le traitement de ses événements.
    pub fn spawn_nmea(self: &Arc<Self>, transport: Transport, generation: u64) {
        let (tx, rx) = mpsc::channel(SOURCE_CHANNEL);
        let (stop, stop_rx) = watch::channel(false);
        self.set_source_stop(stop);
        tracing::info!(link = %transport.describe(), "démarrage de la liaison NMEA 0183");
        tokio::spawn(nav_nmea0183::run(
            transport,
            nav_nmea0183::ClientOptions::default(),
            tx,
            stop_rx,
        ));
        self.spawn_consumer(SourceMode::Nmea0183, generation, rx);
    }

    /// Compte une donnée rejetée par le connecteur courant.
    fn reject(&self, reason: String) {
        tracing::debug!(%reason, "donnée rejetée");
        let mut r = lock(&self.rejections);
        r.count = r.count.saturating_add(1);
        r.last_reason = Some(reason);
    }

    fn spawn_consumer<E>(self: &Arc<Self>, mode: SourceMode, generation: u64, rx: mpsc::Receiver<E>)
    where
        E: Into<SourceEvent> + Send + 'static,
    {
        tokio::spawn(Arc::clone(self).consume(mode, generation, rx));
    }

    async fn consume<E>(
        self: Arc<Self>,
        mode: SourceMode,
        generation: u64,
        mut rx: mpsc::Receiver<E>,
    ) where
        E: Into<SourceEvent> + Send + 'static,
    {
        while let Some(event) = rx.recv().await {
            if !self.is_current(generation) {
                // Connecteur remplacé : ses données ne doivent pas se
                // mêler à celles de la nouvelle source.
                continue;
            }
            match event.into() {
                SourceEvent::State(connection) => {
                    *lock(&self.connection) = Some(connection.clone());
                    self.publish(EngineEvent::SourceStatus(Some(connection)));
                }
                SourceEvent::Sample(Sample::Position(fix)) => {
                    let update = {
                        let mut engine = self.engine();
                        if engine.mode() != mode {
                            continue;
                        }
                        engine.on_fix(fix)
                    };
                    if let Some(update) = update {
                        self.publish_update(update).await;
                    }
                }
                SourceEvent::Sample(Sample::Measurement(measurement)) => {
                    let mut engine = self.engine();
                    if engine.mode() == mode {
                        engine.on_measurement(measurement);
                    }
                }
                SourceEvent::Ais(fragment, received_at) => {
                    let result = {
                        let mut engine = self.engine();
                        if engine.mode() != mode {
                            continue;
                        }
                        engine.on_ais(fragment, received_at)
                    };
                    if let Err(error) = result {
                        self.reject(error.to_string());
                    }
                }
                SourceEvent::Rejected(reason) => self.reject(reason),
            }
        }
    }
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

/// Service en mémoire pour les tests (simulateur à Rangiroa).
#[cfg(test)]
pub(crate) async fn test_service() -> Arc<Service> {
    let db = Db::open_in_memory().await.expect("db");
    let start = nav_core::GeoPosition::new(-14.968, -147.638).expect("pos");
    Service::new(
        db,
        Engine::new(start, 4.0),
        Arc::new(crate::secrets::MemoryStore::default()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(events: &[EngineEvent]) -> Vec<&'static str> {
        events.iter().map(EngineEvent::name).collect()
    }

    #[tokio::test]
    async fn step_publishes_state_and_records_heartbeat() {
        let service = test_service().await;
        let mut rx = service.subscribe();
        assert!(service.last_tick().is_none());
        let now = Utc::now();
        service.step(now).await;
        let mut got = Vec::new();
        while let Ok(event) = rx.try_recv() {
            got.push(event);
        }
        assert_eq!(names(&got), ["alarms", "instruments", "ais-targets"]);
        assert_eq!(
            service.last_tick().map(|t| t.timestamp_millis()),
            Some(now.timestamp_millis())
        );
        // Rejets inchangés : non rediffusés.
        let current = service.current_events();
        assert_eq!(
            names(&current),
            [
                "instruments",
                "ais-targets",
                "alarms",
                "navigation-status",
                "source-status",
                "source-rejections"
            ]
        );
    }

    #[tokio::test]
    async fn rejections_are_published_once_when_they_change() {
        let service = test_service().await;
        let mut rx = service.subscribe();
        service.reject("phrase invalide".to_owned());
        service.step(Utc::now()).await;
        service.step(Utc::now()).await;
        let mut count = 0;
        while let Ok(event) = rx.try_recv() {
            if let EngineEvent::Rejections(r) = event {
                assert_eq!(r.count, 1);
                count += 1;
            }
        }
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn events_serialize_with_type_and_data() {
        let json = serde_json::to_value(EngineEvent::Navigation(None)).expect("json");
        assert_eq!(
            json,
            serde_json::json!({"type": "navigation", "data": null})
        );
        let json = serde_json::to_value(EngineEvent::SourceStatus(None)).expect("json");
        assert_eq!(json["type"], "sourceStatus");
    }

    #[tokio::test]
    async fn loop_ticks_until_shutdown() {
        let service = test_service().await;
        let handle = tokio::spawn(Arc::clone(&service).run());
        // Le premier pas est immédiat.
        for _ in 0..50 {
            if service.last_tick().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(service.last_tick().is_some(), "la boucle tourne");
        service.shutdown();
        tokio::time::timeout(TICK_PERIOD, handle)
            .await
            .expect("arrêt rapide")
            .expect("tâche");
    }

    #[tokio::test]
    async fn replaced_connector_events_are_ignored() {
        let service = test_service().await;
        let old = service.stop_source();
        let (tx, rx) = mpsc::channel::<SignalKEvent>(4);
        service.engine().set_mode(SourceMode::SignalK, false);
        let task = tokio::spawn(Arc::clone(&service).consume(SourceMode::SignalK, old, rx));
        service.stop_source();
        tx.send(SignalKEvent::Rejected("ancien".to_owned()))
            .await
            .expect("send");
        drop(tx);
        task.await.expect("tâche");
        assert_eq!(service.rejections().count, 0);
    }
}
