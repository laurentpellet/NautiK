//! Liaisons NMEA 0183 de bout en bout : flux enregistré, TCP, UDP,
//! rejeu, erreurs et reconnexion.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use nav_core::{ConnectionState, Sample, SourceKind};
use nav_nmea0183::framer::LineFramer;
use nav_nmea0183::sentence::checksum;
use nav_nmea0183::{decode, run, ClientOptions, DecodeContext, Decoded, NmeaEvent, Transport};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

fn recording() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/tiputa.nmea")
}

fn line(body: &str) -> String {
    format!("${body}*{:02X}\r\n", checksum(body))
}

fn fast() -> ClientOptions {
    ClientOptions {
        connect_timeout: Duration::from_secs(2),
        idle_timeout: Duration::from_secs(5),
        initial_backoff: Duration::from_millis(50),
        max_backoff: Duration::from_millis(200),
        max_sentences_per_second: 1_000,
    }
}

struct Client {
    rx: mpsc::Receiver<NmeaEvent>,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

fn start(transport: Transport, options: ClientOptions) -> Client {
    let (tx, rx) = mpsc::channel(1_024);
    let (stop, stop_rx) = watch::channel(false);
    let task = tokio::spawn(run(transport, options, tx, stop_rx));
    Client { rx, stop, task }
}

impl Client {
    async fn next(&mut self) -> NmeaEvent {
        timeout(Duration::from_secs(10), self.rx.recv())
            .await
            .expect("événement attendu dans les délais")
            .expect("canal ouvert")
    }

    async fn until(&mut self, pred: impl Fn(&NmeaEvent) -> bool) -> NmeaEvent {
        loop {
            let e = self.next().await;
            if pred(&e) {
                return e;
            }
        }
    }

    async fn stop(mut self) {
        let _ = self.stop.send(true);
        let last = self
            .until(|e| matches!(e, NmeaEvent::State(ConnectionState::Stopped)))
            .await;
        assert_eq!(last, NmeaEvent::State(ConnectionState::Stopped));
        timeout(Duration::from_secs(5), self.task)
            .await
            .expect("arrêt")
            .expect("tâche");
    }
}

fn is_position(e: &NmeaEvent) -> bool {
    matches!(e, NmeaEvent::Sample(Sample::Position(_)))
}

#[test]
fn recorded_stream_is_decoded_with_errors_reported() {
    let bytes = std::fs::read(recording()).expect("enregistrement");
    let mut framer = LineFramer::new();
    let mut lines = framer.push(&bytes);
    lines.extend(framer.finish());
    let ctx = DecodeContext {
        origin: "nmea0183",
        source: SourceKind::Nmea0183,
        received_at: Utc::now(),
        use_sentence_time: true,
    };
    let (mut positions, mut measurements, mut errors, mut unsupported, mut ais) = (0, 0, 0, 0, 0);
    for line in lines {
        let line = line.expect("ligne");
        match decode(&line, ctx) {
            Ok(Decoded::Samples(samples)) => {
                for s in samples {
                    match s {
                        Sample::Position(_) => positions += 1,
                        Sample::Measurement(_) => measurements += 1,
                    }
                }
            }
            Ok(Decoded::Unsupported(_)) => unsupported += 1,
            Ok(Decoded::Ais(_)) => ais += 1,
            Err(_) => errors += 1,
        }
    }
    // 10 RMC + 10 GGA valides ; points invalides, estimés et hors
    // bornes écartés.
    assert_eq!(positions, 20);
    assert_eq!(measurements, 100);
    assert_eq!(ais, 1, "AIS");
    assert_eq!(unsupported, 0);
    assert_eq!(
        errors, 5,
        "somme fausse, absente, tronquée, bruit, hors bornes"
    );
}

#[tokio::test]
async fn tcp_stream_reconnects_after_loss() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let data = format!(
                "{}{}$IIHDT,1,T*00\r\n",
                line("GPRMC,183000,A,1458.08,S,14738.28,W,6.5,315.0,121126,,,A"),
                line("IIHDT,312.0,T"),
            );
            socket.write_all(data.as_bytes()).await.expect("write");
            socket.shutdown().await.expect("fin");
        }
    });

    let mut client = start(
        Transport::Tcp {
            host: "127.0.0.1".to_owned(),
            port,
        },
        fast(),
    );
    assert_eq!(
        client.next().await,
        NmeaEvent::State(ConnectionState::Connecting { attempt: 1 })
    );
    let NmeaEvent::State(ConnectionState::Connected { server_name, .. }) = client.next().await
    else {
        panic!("connexion attendue");
    };
    assert_eq!(server_name, Some(format!("TCP 127.0.0.1:{port}")));
    let NmeaEvent::Sample(Sample::Position(fix)) = client.until(is_position).await else {
        unreachable!();
    };
    assert_eq!(fix.source, SourceKind::Nmea0183);
    assert_eq!(fix.source_id, "nmea0183.GP");
    let rejected = client.until(|e| matches!(e, NmeaEvent::Rejected(_))).await;
    assert!(format!("{rejected:?}").contains("somme de contrôle"));

    let lost = client
        .until(|e| matches!(e, NmeaEvent::State(ConnectionState::Disconnected { .. })))
        .await;
    let NmeaEvent::State(ConnectionState::Disconnected {
        reason,
        retry_in_ms,
    }) = lost
    else {
        unreachable!();
    };
    assert!(reason.contains("fermée"), "{reason}");
    assert_eq!(retry_in_ms, 50, "données reçues : reprise rapide");
    // Reconnexion automatique.
    client
        .until(|e| matches!(e, NmeaEvent::State(ConnectionState::Connected { .. })))
        .await;
    client.until(is_position).await;
    client.stop().await;
    server.await.expect("serveur");
}

#[tokio::test]
async fn refused_connection_is_reported_with_backoff() {
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        l.local_addr().expect("addr").port()
    };
    let mut client = start(
        Transport::Tcp {
            host: "127.0.0.1".to_owned(),
            port,
        },
        // Windows réessaie ~2 s avant de refuser : délai court.
        ClientOptions {
            connect_timeout: Duration::from_millis(300),
            ..fast()
        },
    );
    let mut delays = Vec::new();
    while delays.len() < 3 {
        if let NmeaEvent::State(ConnectionState::Disconnected {
            reason,
            retry_in_ms,
        }) = client.next().await
        {
            assert!(
                reason.contains("connexion impossible") || reason.contains("délai"),
                "{reason}"
            );
            delays.push(retry_in_ms);
        }
    }
    assert_eq!(delays, vec![50, 100, 200]);
    client.stop().await;
}

#[tokio::test]
async fn silent_link_is_reported() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        // Données sans phrase valide : la liaison reste « muette ».
        for _ in 0..10 {
            if socket.write_all(b"$GPHDT,1,T*00\r\n").await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        socket
    });
    let mut client = start(
        Transport::Tcp {
            host: "127.0.0.1".to_owned(),
            port,
        },
        ClientOptions {
            idle_timeout: Duration::from_millis(300),
            ..fast()
        },
    );
    let lost = client
        .until(|e| matches!(e, NmeaEvent::State(ConnectionState::Disconnected { .. })))
        .await;
    assert!(
        format!("{lost:?}").contains("aucune phrase valide"),
        "{lost:?}"
    );
    client.stop().await;
    drop(server.await);
}

#[tokio::test]
async fn excessive_rate_is_clipped_and_reported() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let burst = line("IIHDT,312.0,T").repeat(50);
        socket.write_all(burst.as_bytes()).await.expect("write");
        tokio::time::sleep(Duration::from_millis(500)).await;
    });
    let mut client = start(
        Transport::Tcp {
            host: "127.0.0.1".to_owned(),
            port,
        },
        ClientOptions {
            max_sentences_per_second: 10,
            ..fast()
        },
    );
    let mut samples = 0;
    let mut warnings = 0;
    loop {
        match client.next().await {
            NmeaEvent::Sample(_) => samples += 1,
            NmeaEvent::Rejected(reason) => {
                assert!(reason.contains("flux excessif"), "{reason}");
                warnings += 1;
            }
            NmeaEvent::State(ConnectionState::Disconnected { .. }) => break,
            NmeaEvent::State(_) => {}
            NmeaEvent::Ais { .. } => panic!("AIS inattendu"),
        }
    }
    assert_eq!(samples, 10);
    assert_eq!(warnings, 1, "signalé une fois par seconde");
    client.stop().await;
    server.await.expect("serveur");
}

#[tokio::test]
async fn udp_datagrams_are_decoded() {
    let port = {
        let s = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind");
        s.local_addr().expect("addr").port()
    };
    let mut client = start(Transport::Udp { port }, fast());
    client
        .until(|e| matches!(e, NmeaEvent::State(ConnectionState::Connected { .. })))
        .await;
    let sender = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
    // Deux phrases, la dernière sans fin de ligne.
    let datagram = format!(
        "{}{}",
        line("SDDPT,18.4,0.5,"),
        line("IIMWV,045.0,R,14.2,N,A").trim_end()
    );
    sender
        .send_to(datagram.as_bytes(), ("127.0.0.1", port))
        .await
        .expect("envoi");
    let mut quantities = Vec::new();
    while quantities.len() < 3 {
        if let NmeaEvent::Sample(Sample::Measurement(m)) = client.next().await {
            quantities.push(m.quantity);
        }
    }
    assert_eq!(quantities.len(), 3);
    client.stop().await;
}

#[tokio::test]
async fn simulated_ais_stream_is_decoded() {
    let port = {
        let s = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind");
        s.local_addr().expect("addr").port()
    };
    let mut client = start(Transport::Udp { port }, fast());
    client
        .until(|e| matches!(e, NmeaEvent::State(ConnectionState::Connected { .. })))
        .await;
    let center = nav_core::GeoPosition::new(-14.968, -147.638).expect("pos");
    let lines = nav_ais::AisSimulator::new(center).step(Utc::now());
    assert!(lines.len() > 3);
    let datagram: String = lines
        .iter()
        .map(|l| {
            format!(
                "{l}
"
            )
        })
        .collect();
    let sender = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
    sender
        .send_to(datagram.as_bytes(), ("127.0.0.1", port))
        .await
        .expect("envoi");

    let mut decoder = nav_ais::AisDecoder::new();
    let mut tracker = nav_ais::AisTracker::new();
    let mut fragments = 0;
    while fragments < lines.len() {
        match client.next().await {
            NmeaEvent::Ais {
                fragment,
                received_at,
            } => {
                fragments += 1;
                if let Some(report) = decoder.push(fragment, received_at).expect("décodage") {
                    tracker.update(&report, received_at);
                }
            }
            NmeaEvent::Rejected(reason) => panic!("rejet : {reason}"),
            _ => {}
        }
    }
    let targets = tracker.snapshot(Utc::now());
    assert_eq!(targets.len(), 3);
    assert!(targets
        .iter()
        .all(|t| t.position.is_some() && t.name.as_deref().is_some_and(|n| n.starts_with("SIM"))));
    client.stop().await;
}

#[tokio::test]
async fn replay_is_paced_flagged_and_looped() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = dir.path().join("court.nmea");
    let content = format!(
        "{}{}",
        line("GPRMC,183000.00,A,1458.08,S,14738.28,W,6.5,315.0,121126,,,A"),
        line("GPRMC,183000.30,A,1458.09,S,14738.29,W,6.5,315.0,121126,,,A"),
    );
    std::fs::write(&path, content).expect("écriture");
    let mut client = start(Transport::Replay { path: path.clone() }, fast());
    let NmeaEvent::Sample(Sample::Position(first)) = client.until(is_position).await else {
        unreachable!();
    };
    let t0 = tokio::time::Instant::now();
    let NmeaEvent::Sample(Sample::Position(second)) = client.until(is_position).await else {
        unreachable!();
    };
    assert!(
        t0.elapsed() >= Duration::from_millis(250),
        "rythme enregistré"
    );
    for fix in [&first, &second] {
        assert_eq!(fix.source, SourceKind::Replay);
        assert!(fix.source.is_simulated(), "jamais présenté comme réel");
        assert!(fix.source_id.starts_with("replay."));
        assert_eq!(
            fix.measured_at, fix.received_at,
            "heure enregistrée ignorée"
        );
    }
    let end = client
        .until(|e| matches!(e, NmeaEvent::State(ConnectionState::Disconnected { .. })))
        .await;
    assert!(format!("{end:?}").contains("fin de l'enregistrement"));
    // Le rejeu reprend au début.
    client.until(is_position).await;
    client.stop().await;
}

#[tokio::test]
async fn missing_replay_file_is_reported() {
    let path = std::env::temp_dir().join("nautik-absent-0f3c.nmea");
    let mut client = start(Transport::Replay { path }, fast());
    let e = client
        .until(|e| matches!(e, NmeaEvent::State(ConnectionState::Disconnected { .. })))
        .await;
    assert!(format!("{e:?}").contains("lecture"), "{e:?}");
    client.stop().await;
}

#[tokio::test]
async fn closed_receiver_stops_the_client() {
    let (tx, rx) = mpsc::channel(4);
    let (_stop, stop_rx) = watch::channel(false);
    drop(rx);
    let task = tokio::spawn(run(Transport::Udp { port: 1 }, fast(), tx, stop_rx));
    timeout(Duration::from_secs(5), task)
        .await
        .expect("arrêt sans destinataire")
        .expect("tâche");
}
