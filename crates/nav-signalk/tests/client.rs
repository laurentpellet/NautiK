//! Tests d'intégration du client Signal K contre un faux serveur local.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
// Signature du rappel de poignée de main imposée par tungstenite.
#![allow(clippy::result_large_err)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nav_core::{Quantity, SourceKind};
use nav_signalk::{
    run, ClientOptions, ConnectionState, Sample, SignalKConfig, SignalKEvent, Token,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

const HELLO: &str =
    r#"{"name":"mock","version":"2.8.0","self":"vessels.urn:mrn:imo:mmsi:227000000"}"#;
const DELTA: &str = r#"{"context":"vessels.urn:mrn:imo:mmsi:227000000","updates":[{"$source":"nmea0183.GP","timestamp":"2026-11-12T10:00:00Z","values":[{"path":"navigation.position","value":{"latitude":-14.968,"longitude":-147.638}},{"path":"environment.depth.belowTransducer","value":12.5}]}]}"#;

fn fast_options() -> ClientOptions {
    ClientOptions {
        connect_timeout: Duration::from_secs(2),
        idle_timeout: Duration::from_secs(2),
        ping_interval: Duration::from_millis(500),
        initial_backoff: Duration::from_millis(50),
        max_backoff: Duration::from_millis(200),
        subscription_period_ms: 1000,
    }
}

async fn read_head(stream: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut byte = [0_u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).await.expect("lecture") == 0 {
            break;
        }
        buf.push(byte[0]);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// Répond à une requête de découverte `GET /signalk`.
async fn serve_discovery(listener: &TcpListener) {
    let port = listener.local_addr().expect("addr").port();
    let (mut stream, _) = listener.accept().await.expect("accept");
    let head = read_head(&mut stream).await;
    assert!(head.starts_with("GET /signalk "), "{head}");
    assert!(
        !head.to_lowercase().contains("authorization"),
        "pas de jeton en découverte"
    );
    let body = format!(
        r#"{{"endpoints":{{"v1":{{"version":"1.0.0","signalk-ws":"ws://127.0.0.1:{port}/signalk/v1/stream"}}}},"server":{{"id":"mock-server","version":"0.0.1"}}}}"#
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .expect("écriture");
    stream.shutdown().await.ok();
}

struct Upgrade {
    ws: WebSocketStream<TcpStream>,
    authorization: Option<String>,
    uri: String,
}

async fn accept_ws(listener: &TcpListener) -> Upgrade {
    let (stream, _) = listener.accept().await.expect("accept");
    let seen: Arc<Mutex<(Option<String>, String)>> = Arc::default();
    let capture = Arc::clone(&seen);
    let ws = tokio_tungstenite::accept_hdr_async(stream, move |req: &Request, resp: Response| {
        let auth = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        *capture.lock().unwrap() = (auth, req.uri().to_string());
        Ok(resp)
    })
    .await
    .expect("handshake");
    let (authorization, uri) = seen.lock().unwrap().clone();
    Upgrade {
        ws,
        authorization,
        uri,
    }
}

async fn expect_subscription(ws: &mut WebSocketStream<TcpStream>) {
    let msg = ws.next().await.expect("message").expect("ok");
    let text = msg.into_text().expect("texte");
    let json: serde_json::Value = serde_json::from_str(text.as_str()).expect("json");
    assert_eq!(json["context"], "vessels.self");
    assert_eq!(json["subscribe"].as_array().expect("tableau").len(), 9);
}

struct Client {
    events: mpsc::Receiver<SignalKEvent>,
    shutdown: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

fn start(port: u16, token: Option<&str>) -> Client {
    let config = SignalKConfig::new(
        &format!("http://127.0.0.1:{port}"),
        token.map(|t| Token::new(t).expect("jeton")),
    )
    .expect("config");
    let (tx, events) = mpsc::channel(64);
    let (shutdown, shutdown_rx) = watch::channel(false);
    let task = tokio::spawn(run(config, fast_options(), tx, shutdown_rx));
    Client {
        events,
        shutdown,
        task,
    }
}

async fn wait_for(
    events: &mut mpsc::Receiver<SignalKEvent>,
    what: &str,
    mut pred: impl FnMut(&SignalKEvent) -> bool,
) -> SignalKEvent {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = events.recv().await.expect("canal ouvert");
            if pred(&event) {
                return event;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("délai dépassé en attendant : {what}"))
}

fn is_state(event: &SignalKEvent, f: impl Fn(&ConnectionState) -> bool) -> bool {
    matches!(event, SignalKEvent::State(s) if f(s))
}

#[tokio::test]
async fn connects_with_token_subscribes_and_receives_data() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = tokio::spawn(async move {
        serve_discovery(&listener).await;
        let mut up = accept_ws(&listener).await;
        assert_eq!(up.authorization.as_deref(), Some("Bearer s3cret-token"));
        assert!(
            up.uri.ends_with("/signalk/v1/stream?subscribe=none"),
            "{}",
            up.uri
        );
        expect_subscription(&mut up.ws).await;
        up.ws.send(Message::text(HELLO)).await.expect("hello");
        up.ws.send(Message::text(DELTA)).await.expect("delta");
        // Garde la connexion ouverte jusqu'à la fermeture par le client.
        while let Some(Ok(_)) = up.ws.next().await {}
    });

    let mut client = start(port, Some("s3cret-token"));
    let connected = wait_for(&mut client.events, "connexion", |e| {
        is_state(e, |s| matches!(s, ConnectionState::Connected { .. }))
    })
    .await;
    assert_eq!(
        connected,
        SignalKEvent::State(ConnectionState::Connected {
            server_name: Some("mock-server".to_owned()),
            server_version: Some("0.0.1".to_owned()),
        })
    );
    let position = wait_for(&mut client.events, "position", |e| {
        matches!(e, SignalKEvent::Sample(Sample::Position(_)))
    })
    .await;
    let SignalKEvent::Sample(Sample::Position(fix)) = position else {
        unreachable!()
    };
    assert_eq!(fix.source, SourceKind::SignalK);
    assert_eq!(fix.source_id, "nmea0183.GP");
    assert!((fix.position.latitude_deg() + 14.968).abs() < 1e-9);
    let depth = wait_for(&mut client.events, "profondeur", |e| {
        matches!(e, SignalKEvent::Sample(Sample::Measurement(_)))
    })
    .await;
    let SignalKEvent::Sample(Sample::Measurement(m)) = depth else {
        unreachable!()
    };
    assert_eq!(m.quantity, Quantity::DepthBelowTransducer);
    assert_eq!(m.value, 12.5);

    client.shutdown.send(true).expect("arrêt");
    wait_for(&mut client.events, "arrêt", |e| {
        is_state(e, |s| *s == ConnectionState::Stopped)
    })
    .await;
    client.task.await.expect("tâche");
    server.await.expect("serveur");
}

#[tokio::test]
async fn reconnects_after_connection_loss() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = tokio::spawn(async move {
        // Première session : données puis coupure brutale.
        serve_discovery(&listener).await;
        let mut first = accept_ws(&listener).await;
        expect_subscription(&mut first.ws).await;
        first.ws.send(Message::text(HELLO)).await.expect("hello");
        first.ws.send(Message::text(DELTA)).await.expect("delta");
        drop(first);
        // Seconde session après reconnexion automatique.
        serve_discovery(&listener).await;
        let mut second = accept_ws(&listener).await;
        expect_subscription(&mut second.ws).await;
        second.ws.send(Message::text(HELLO)).await.expect("hello");
        second.ws.send(Message::text(DELTA)).await.expect("delta");
        while let Some(Ok(_)) = second.ws.next().await {}
    });

    let mut client = start(port, None);
    wait_for(&mut client.events, "1re connexion", |e| {
        is_state(e, |s| matches!(s, ConnectionState::Connected { .. }))
    })
    .await;
    let lost = wait_for(&mut client.events, "perte", |e| {
        is_state(e, |s| matches!(s, ConnectionState::Disconnected { .. }))
    })
    .await;
    let SignalKEvent::State(ConnectionState::Disconnected { retry_in_ms, .. }) = lost else {
        unreachable!()
    };
    assert_eq!(
        retry_in_ms, 50,
        "délai réinitialisé après une connexion réussie"
    );
    wait_for(&mut client.events, "2e connexion", |e| {
        is_state(e, |s| matches!(s, ConnectionState::Connected { .. }))
    })
    .await;
    wait_for(&mut client.events, "données après reconnexion", |e| {
        matches!(e, SignalKEvent::Sample(Sample::Position(_)))
    })
    .await;
    client.shutdown.send(true).expect("arrêt");
    client.task.await.expect("tâche");
    server.await.expect("serveur");
}

#[tokio::test]
async fn reports_rejected_token() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move {
        serve_discovery(&listener).await;
        let (mut stream, _) = listener.accept().await.expect("accept");
        let head = read_head(&mut stream).await;
        assert!(head
            .to_lowercase()
            .contains("authorization: bearer expired"));
        stream
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("écriture");
    });

    let mut client = start(port, Some("expired"));
    let event = wait_for(&mut client.events, "refus", |e| {
        is_state(e, |s| {
            matches!(
                s,
                ConnectionState::Unauthorized { .. } | ConnectionState::Disconnected { .. }
            )
        })
    })
    .await;
    let SignalKEvent::State(ConnectionState::Unauthorized { reason, .. }) = event else {
        panic!("refus d'authentification attendu : {event:?}")
    };
    assert!(reason.contains("401"), "{reason}");
    assert!(
        !reason.contains("expired"),
        "le jeton n'apparaît pas dans le message"
    );
    client.shutdown.send(true).expect("arrêt");
    client.task.await.expect("tâche");
}

#[tokio::test]
async fn retries_with_growing_delay_when_server_is_down() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        listener.local_addr().expect("addr").port()
    };
    let mut client = start(port, None);
    let mut delays = Vec::new();
    while delays.len() < 4 {
        if let SignalKEvent::State(ConnectionState::Disconnected {
            retry_in_ms,
            reason,
        }) = wait_for(&mut client.events, "échec", |e| {
            is_state(e, |s| matches!(s, ConnectionState::Disconnected { .. }))
        })
        .await
        {
            assert!(reason.contains("découverte"), "{reason}");
            delays.push(retry_in_ms);
        }
    }
    assert_eq!(delays, [50, 100, 200, 200]);
    client.shutdown.send(true).expect("arrêt");
    client.task.await.expect("tâche");
}

#[tokio::test]
async fn silent_server_is_detected() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = tokio::spawn(async move {
        serve_discovery(&listener).await;
        let mut up = accept_ws(&listener).await;
        expect_subscription(&mut up.ws).await;
        // Le serveur ne lit plus rien : pas de pong, aucune donnée.
        tokio::time::sleep(Duration::from_secs(4)).await;
        drop(up);
    });
    let mut client = start(port, None);
    let event = wait_for(&mut client.events, "silence", |e| {
        is_state(e, |s| matches!(s, ConnectionState::Disconnected { .. }))
    })
    .await;
    let SignalKEvent::State(ConnectionState::Disconnected { reason, .. }) = event else {
        unreachable!()
    };
    assert!(reason.contains("aucune donnée"), "{reason}");
    client.shutdown.send(true).expect("arrêt");
    client.task.await.expect("tâche");
    server.abort();
}

#[tokio::test]
async fn malformed_messages_are_reported_without_disconnecting() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = tokio::spawn(async move {
        serve_discovery(&listener).await;
        let mut up = accept_ws(&listener).await;
        expect_subscription(&mut up.ws).await;
        up.ws.send(Message::text(HELLO)).await.expect("hello");
        up.ws
            .send(Message::text("{pas du json"))
            .await
            .expect("envoi");
        up.ws
            .send(Message::text(
                r#"{"updates":[{"values":[{"path":"navigation.position","value":{"latitude":123,"longitude":0}}]}]}"#,
            ))
            .await
            .expect("envoi");
        up.ws.send(Message::text(DELTA)).await.expect("envoi");
        while let Some(Ok(_)) = up.ws.next().await {}
    });
    let mut client = start(port, None);
    let mut rejected = 0;
    wait_for(&mut client.events, "position valide", |e| {
        if matches!(e, SignalKEvent::Rejected(_)) {
            rejected += 1;
        }
        assert!(
            !is_state(e, |s| matches!(s, ConnectionState::Disconnected { .. })),
            "pas de déconnexion : {e:?}"
        );
        matches!(e, SignalKEvent::Sample(Sample::Position(_)))
    })
    .await;
    assert_eq!(rejected, 2);
    client.shutdown.send(true).expect("arrêt");
    client.task.await.expect("tâche");
    server.await.expect("serveur");
}
