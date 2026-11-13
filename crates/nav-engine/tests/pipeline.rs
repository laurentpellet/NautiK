//! Intégration de bout en bout du moteur : liaison NMEA 0183 TCP réelle
//! (positions et AIS) → connecteur → arbitrage → boucle moteur →
//! événements diffusés, puis coupure réseau et reprise.
//!
//! Attendu pendant la coupure : perte de liaison signalée, position
//! conservée mais déclarée obsolète (jamais inventée), alarme de perte
//! de position déclenchée. Après la reprise : reconnexion automatique et
//! position de nouveau fraîche.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use nav_ais::sim::AisSimulator;
use nav_alarms::{AlarmKind, AlarmState};
use nav_core::{ConnectionState, GeoPosition};
use nav_engine::sources::{self, NmeaSettings, SourcesRequest};
use nav_engine::{Engine, EngineEvent, MemoryStore, Service, SourceMode};
use nav_nmea0183::sentence::checksum;
use nav_storage::Db;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpSocket};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::timeout;

/// Position émise par le récepteur GPS simulé (Tiputa, Rangiroa).
const LAT: f64 = -(14.0 + 58.08 / 60.0);
const LON: f64 = -(147.0 + 38.28 / 60.0);

fn rmc() -> String {
    let now = Utc::now();
    let body = format!(
        "GPRMC,{},A,1458.08,S,14738.28,W,0.5,315.0,{},,,A",
        now.format("%H%M%S.00"),
        now.format("%d%m%y")
    );
    format!("${body}*{:02X}\r\n", checksum(&body))
}

/// Écoute TCP réutilisable sur le même port après une coupure.
fn listen(addr: SocketAddr) -> TcpListener {
    let socket = TcpSocket::new_v4().expect("socket");
    socket.set_reuseaddr(true).expect("reuseaddr");
    socket.bind(addr).expect("bind");
    socket.listen(8).expect("listen")
}

/// Multiplexeur NMEA simulé : GPS à 5 Hz et cinq navires AIS.
fn feeder(listener: TcpListener) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let center = GeoPosition::new(LAT, LON).expect("pos");
            let mut fleet = AisSimulator::fleet(center, 5);
            loop {
                let mut data = rmc();
                for line in fleet.step(Utc::now()) {
                    data.push_str(&line);
                    data.push_str("\r\n");
                }
                if socket.write_all(data.as_bytes()).await.is_err() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    })
}

async fn until<T>(
    rx: &mut broadcast::Receiver<EngineEvent>,
    limit: Duration,
    what: &str,
    mut pick: impl FnMut(&EngineEvent) -> Option<T>,
) -> T {
    timeout(limit, async {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Some(found) = pick(&event) {
                        return found;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => panic!("canal fermé"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("délai dépassé : {what}"))
}

fn position_alarm(event: &EngineEvent) -> Option<AlarmState> {
    let EngineEvent::Alarms(snapshot) = event else {
        return None;
    };
    snapshot
        .alarms
        .iter()
        .find(|a| a.kind == AlarmKind::PositionLost)
        .map(|a| a.state)
}

async fn service() -> Arc<Service> {
    let db = Db::open_in_memory().await.expect("db");
    let start = GeoPosition::new(-17.5, -149.5).expect("pos");
    Service::new(
        db,
        Engine::new(start, 0.0),
        Arc::new(MemoryStore::default()),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nmea_tcp_feed_survives_network_cut() {
    let listener = listen("127.0.0.1:0".parse().expect("adresse"));
    let addr = listener.local_addr().expect("adresse");
    let mut server = feeder(listener);

    let service = service().await;
    let mut rx = service.subscribe();
    sources::set(
        &service,
        SourcesRequest {
            mode: SourceMode::Nmea0183,
            signalk_url: None,
            nmea: Some(NmeaSettings::Tcp {
                host: "127.0.0.1".to_owned(),
                port: addr.port(),
            }),
            priorities: Vec::new(),
            token: None,
            clear_token: false,
        },
    )
    .await
    .expect("source NMEA");
    let engine = tokio::spawn(Arc::clone(&service).run());

    // 1. Données reçues, décodées, arbitrées et diffusées.
    until(&mut rx, Duration::from_secs(10), "connexion", |e| {
        matches!(
            e,
            EngineEvent::SourceStatus(Some(ConnectionState::Connected { .. }))
        )
        .then_some(())
    })
    .await;
    let fix = until(
        &mut rx,
        Duration::from_secs(10),
        "position fraîche",
        |e| match e {
            EngineEvent::Instruments(s) => s.position.as_ref().filter(|p| !p.stale).cloned(),
            _ => None,
        },
    )
    .await;
    assert_eq!(fix.fix.source_id, "nmea0183.GP");
    assert!((fix.fix.position.latitude_deg() - LAT).abs() < 1e-6);
    assert!((fix.fix.position.longitude_deg() - LON).abs() < 1e-6);
    let targets = until(
        &mut rx,
        Duration::from_secs(40),
        "cibles AIS",
        |e| match e {
            EngineEvent::Ais(s) if s.targets.len() == 5 => Some(s.clone()),
            _ => None,
        },
    )
    .await;
    assert!(!targets.simulated, "flux réel, pas le simulateur");
    assert!(targets.targets.iter().all(|t| t.position.is_some()));

    // 2. Coupure réseau : le multiplexeur disparaît (liaison et écoute).
    server.abort();
    let _ = server.await;
    until(&mut rx, Duration::from_secs(10), "perte signalée", |e| {
        matches!(
            e,
            EngineEvent::SourceStatus(Some(ConnectionState::Disconnected { .. }))
        )
        .then_some(())
    })
    .await;
    let stale = until(
        &mut rx,
        Duration::from_secs(10),
        "position obsolète",
        |e| match e {
            EngineEvent::Instruments(s) => s.position.as_ref().filter(|p| p.stale).cloned(),
            _ => None,
        },
    )
    .await;
    // Dernière position connue conservée telle quelle, jamais extrapolée.
    assert_eq!(stale.fix.position, fix.fix.position);
    until(&mut rx, Duration::from_secs(15), "alarme de perte", |e| {
        (position_alarm(e) == Some(AlarmState::Active)).then_some(())
    })
    .await;

    // 3. Reprise du réseau : reconnexion automatique, position fraîche.
    server = feeder(listen(addr));
    until(&mut rx, Duration::from_secs(20), "reconnexion", |e| {
        matches!(
            e,
            EngineEvent::SourceStatus(Some(ConnectionState::Connected { .. }))
        )
        .then_some(())
    })
    .await;
    until(
        &mut rx,
        Duration::from_secs(10),
        "position de nouveau fraîche",
        |e| match e {
            EngineEvent::Instruments(s) => s.position.as_ref().filter(|p| !p.stale).map(|_| ()),
            _ => None,
        },
    )
    .await;

    service.stop_source();
    service.shutdown();
    timeout(Duration::from_secs(5), engine)
        .await
        .expect("arrêt de la boucle")
        .expect("tâche");
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unreachable_source_is_reported_without_inventing_a_position() {
    // Port fermé : la liaison échoue dès le départ.
    let closed = {
        let l = listen("127.0.0.1:0".parse().expect("adresse"));
        l.local_addr().expect("adresse")
    };
    let service = service().await;
    let mut rx = service.subscribe();
    sources::set(
        &service,
        SourcesRequest {
            mode: SourceMode::Nmea0183,
            signalk_url: None,
            nmea: Some(NmeaSettings::Tcp {
                host: "127.0.0.1".to_owned(),
                port: closed.port(),
            }),
            priorities: Vec::new(),
            token: None,
            clear_token: false,
        },
    )
    .await
    .expect("source NMEA");
    let engine = tokio::spawn(Arc::clone(&service).run());
    until(&mut rx, Duration::from_secs(10), "échec signalé", |e| {
        matches!(
            e,
            EngineEvent::SourceStatus(Some(ConnectionState::Disconnected { .. }))
        )
        .then_some(())
    })
    .await;
    // Plusieurs pas de boucle : aucune position n'apparaît (ni celle du
    // simulateur, ni une position par défaut). Aucune position n'ayant
    // été reçue de cette source, l'alarme de perte reste indéterminée
    // (règle de la phase 6) : c'est l'état de liaison qui alerte.
    for _ in 0..3 {
        let position = until(
            &mut rx,
            Duration::from_secs(5),
            "instruments",
            |e| match e {
                EngineEvent::Instruments(s) => Some(s.position.clone()),
                _ => None,
            },
        )
        .await;
        assert!(position.is_none(), "aucune position inventée");
    }
    assert!(matches!(
        service.connection(),
        Some(ConnectionState::Disconnected { .. } | ConnectionState::Connecting { .. })
    ));

    service.stop_source();
    service.shutdown();
    timeout(Duration::from_secs(5), engine)
        .await
        .expect("arrêt")
        .expect("tâche");
}
