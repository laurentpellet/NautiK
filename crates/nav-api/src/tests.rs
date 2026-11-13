//! Tests de l'API : authentification, routes REST (sans réseau) et
//! sessions WebSocket (sur un vrai port local).

use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use nav_core::GeoPosition;
use nav_engine::{Engine, MemoryStore, Service};
use nav_storage::Db;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tower::ServiceExt;

use super::*;

fn token() -> ApiToken {
    ApiToken::parse(&"t".repeat(40)).expect("jeton")
}

async fn service() -> Arc<Service> {
    let db = Db::open_in_memory().await.expect("db");
    let start = GeoPosition::new(-14.968, -147.638).expect("pos");
    Service::new(
        db,
        Engine::new(start, 4.0),
        Arc::new(MemoryStore::default()),
    )
}

async fn app() -> (Router, Arc<Service>, watch::Sender<bool>) {
    let service = service().await;
    let (stop_tx, stop) = watch::channel(false);
    (
        router(Arc::clone(&service), token(), stop),
        service,
        stop_tx,
    )
}

async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    auth: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(token) = auth {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .expect("requête");
    let response = app.clone().oneshot(request).await.expect("réponse");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), MAX_BODY_BYTES)
        .await
        .expect("corps");
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

fn good() -> Option<&'static str> {
    Some("tttttttttttttttttttttttttttttttttttttttt")
}

#[tokio::test]
async fn every_api_route_requires_the_token() {
    let (app, _, _) = app().await;
    for uri in [
        "/api/v1/info",
        "/api/v1/alarms",
        "/api/v1/sources",
        "/api/v1/state",
    ] {
        let (status, body) = call(&app, Method::GET, uri, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert_eq!(body["error"], "authentification requise");
        let (status, _) = call(&app, Method::GET, uri, Some(&"x".repeat(40)), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
    }
    let (status, _) = call(
        &app,
        Method::POST,
        "/api/v1/alarms/anchor",
        None,
        Some(json!({"radiusM": 50, "delayS": 0})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "aucune commande sans jeton"
    );
    let (status, body) = call(&app, Method::GET, "/api/v1/info", good(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["version"], nav_core::VERSION);
}

#[tokio::test]
async fn health_is_public_and_reports_a_stalled_loop() {
    let (app, service, _) = app().await;
    let (status, body) = call(&app, Method::GET, "/health", None, None).await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "boucle jamais lancée"
    );
    assert_eq!(body["status"], "stalled");
    service.step(chrono::Utc::now()).await;
    let (status, body) = call(&app, Method::GET, "/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(body.get("alarms").is_none(), "aucune donnée de navigation");
}

#[tokio::test]
async fn anchor_watch_through_rest() {
    let (app, service, _) = app().await;
    // Sans position : refus explicite.
    let (status, body) = call(
        &app,
        Method::POST,
        "/api/v1/alarms/anchor",
        good(),
        Some(json!({"radiusM": 50, "delayS": 0})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"]
        .as_str()
        .expect("message")
        .contains("position"));

    service.step(chrono::Utc::now()).await;
    let (status, body) = call(
        &app,
        Method::POST,
        "/api/v1/alarms/anchor",
        good(),
        Some(json!({"radiusM": 50, "delayS": 0})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["anchor"]["radiusM"], 50.0);

    let (status, _) = call(
        &app,
        Method::POST,
        "/api/v1/alarms/anchor",
        good(),
        Some(json!({"radiusM": 50, "delayS": 0, "intrus": 1})),
    )
    .await;
    assert!(status.is_client_error(), "champ inconnu refusé");

    let (status, body) = call(
        &app,
        Method::GET,
        "/api/v1/alarms/events?limit=5",
        good(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().map(Vec::len), Some(1));

    let (status, body) = call(&app, Method::DELETE, "/api/v1/alarms/anchor", good(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["anchor"], Value::Null);

    let (status, _) = call(
        &app,
        Method::PUT,
        "/api/v1/alarms/anchor/enabled",
        good(),
        Some(json!({"enabled": false})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "le mouillage ne se désactive pas ainsi"
    );
    let (status, _) = call(
        &app,
        Method::POST,
        "/api/v1/alarms/x/acknowledge",
        good(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn routes_navigation_and_recording_through_rest() {
    let (app, _, _) = app().await;
    let (status, route) = call(
        &app,
        Method::POST,
        "/api/v1/routes",
        good(),
        Some(json!({
            "name": "Passe",
            "positions": [
                {"latitudeDeg": -14.968, "longitudeDeg": -147.638},
                {"latitudeDeg": -14.9, "longitudeDeg": -147.6}
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{route}");
    let id = route["id"].as_str().expect("id").to_owned();

    let (status, _) = call(
        &app,
        Method::POST,
        "/api/v1/routes",
        good(),
        Some(json!({"name": "X", "positions": [{"latitudeDeg": 95.0, "longitudeDeg": 0.0}]})),
    )
    .await;
    assert!(status.is_client_error(), "position hors bornes refusée");

    let (status, _) = call(
        &app,
        Method::PUT,
        "/api/v1/navigation",
        good(),
        Some(json!({"routeId": id})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, nav) = call(&app, Method::GET, "/api/v1/navigation", good(), None).await;
    assert_eq!(nav["activeRouteId"], Value::String(id.clone()));
    let (status, _) = call(
        &app,
        Method::DELETE,
        &format!("/api/v1/routes/{id}"),
        good(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "route active protégée");
    let (status, _) = call(&app, Method::DELETE, "/api/v1/navigation", good(), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(
        &app,
        Method::DELETE,
        &format!("/api/v1/routes/{id}"),
        good(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, track) = call(&app, Method::POST, "/api/v1/recording", good(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(track["isRecording"], true);
    let (status, _) = call(&app, Method::DELETE, "/api/v1/recording", good(), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, tracks) = call(&app, Method::GET, "/api/v1/tracks", good(), None).await;
    assert_eq!(tracks.as_array().map(Vec::len), Some(1));

    let (status, _) = call(&app, Method::GET, "/api/v1/tracks/../../etc", good(), None).await;
    assert!(status.is_client_error());
    let (status, body) = call(&app, Method::GET, "/api/v1/inconnue", good(), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "ressource inconnue");
}

#[tokio::test]
async fn sources_never_expose_the_token_and_refuse_remote_replay() {
    let (app, _, _) = app().await;
    let (status, view) = call(
        &app,
        Method::PUT,
        "/api/v1/sources",
        good(),
        Some(json!({
            "mode": "simulator",
            "signalkUrl": "http://boat.local:3000",
            "nmea": null,
            "token": "jeton-signalk-tres-secret"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["tokenStored"], true);
    assert!(!view.to_string().contains("tres-secret"));

    let (status, body) = call(
        &app,
        Method::PUT,
        "/api/v1/sources",
        good(),
        Some(json!({
            "mode": "nmea0183",
            "signalkUrl": null,
            "nmea": {"kind": "replay", "path": "/etc/shadow"},
            "token": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let (status, body) = call(
        &app,
        Method::PUT,
        "/api/v1/simulator",
        good(),
        Some(json!({"speedMps": 1000})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn oversized_bodies_are_rejected() {
    let (app, _, _) = app().await;
    let huge = "x".repeat(MAX_BODY_BYTES + 1);
    let (status, _) = call(
        &app,
        Method::POST,
        "/api/v1/routes",
        good(),
        Some(json!({"name": huge, "positions": []})),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

// -------------------------------------------------------------- WebSocket

async fn start_server() -> (String, Arc<Service>, watch::Sender<bool>) {
    let (router, service, stop) = app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("port");
    let addr = listener.local_addr().expect("adresse");
    let mut shutdown = stop.subscribe();
    tokio::spawn(serve(listener, router, async move {
        let _ = shutdown.wait_for(|s| *s).await;
    }));
    (format!("ws://{addr}/api/v1/ws"), service, stop)
}

type Client =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn next_json(client: &mut Client) -> Value {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), client.next())
            .await
            .expect("message à temps")
            .expect("flux ouvert")
            .expect("message valide");
        match message {
            WsMessage::Text(text) => return serde_json::from_str(text.as_str()).expect("json"),
            WsMessage::Close(frame) => {
                return json!({"closed": frame.map(|f| u16::from(f.code))});
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn websocket_streams_state_after_first_message_auth() {
    let (url, service, stop) = start_server().await;
    let (mut client, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("connexion");
    client
        .send(WsMessage::Text(
            json!({"type": "auth", "token": good().expect("jeton")})
                .to_string()
                .into(),
        ))
        .await
        .expect("auth");
    let hello = next_json(&mut client).await;
    assert_eq!(hello["type"], "hello");
    assert_eq!(hello["data"]["version"], nav_core::VERSION);
    // État initial : alarmes, navigation, liaison, rejets.
    let mut initial = Vec::new();
    for _ in 0..4 {
        initial.push(next_json(&mut client).await["type"].clone());
    }
    assert_eq!(
        initial,
        [
            json!("alarms"),
            json!("navigation"),
            json!("sourceStatus"),
            json!("rejections")
        ]
    );
    // Puis le flux en continu.
    service.step(chrono::Utc::now()).await;
    let mut streamed = Vec::new();
    for _ in 0..3 {
        streamed.push(next_json(&mut client).await["type"].clone());
    }
    assert_eq!(
        streamed,
        [json!("alarms"), json!("instruments"), json!("ais")]
    );

    // Arrêt du service : la session se ferme proprement (1001).
    stop.send(true).expect("arrêt");
    assert_eq!(next_json(&mut client).await, json!({"closed": 1001}));
}

#[tokio::test]
async fn websocket_accepts_a_bearer_header() {
    let (url, _service, _stop) = start_server().await;
    let mut request = url.as_str().into_client_request().expect("requête");
    request.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {}", good().expect("jeton"))
            .parse()
            .expect("en-tête"),
    );
    let (mut client, _) = tokio_tungstenite::connect_async(request)
        .await
        .expect("connexion");
    assert_eq!(next_json(&mut client).await["type"], "hello");
}

#[tokio::test]
async fn websocket_rejects_bad_tokens() {
    let (url, _service, _stop) = start_server().await;
    // Mauvais jeton dans l'en-tête : refus avant l'ouverture.
    let mut request = url.as_str().into_client_request().expect("requête");
    request.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {}", "x".repeat(40))
            .parse()
            .expect("en-tête"),
    );
    assert!(tokio_tungstenite::connect_async(request).await.is_err());

    // Mauvais premier message : fermeture 4401, aucune donnée envoyée.
    let (mut client, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("connexion");
    client
        .send(WsMessage::Text(
            json!({"type": "auth", "token": "x".repeat(40)})
                .to_string()
                .into(),
        ))
        .await
        .expect("envoi");
    assert_eq!(
        next_json(&mut client).await,
        json!({"closed": ws::CLOSE_UNAUTHORIZED})
    );

    // Message illisible : même refus.
    let (mut client, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("connexion");
    client
        .send(WsMessage::Text("bonjour".into()))
        .await
        .expect("envoi");
    assert_eq!(
        next_json(&mut client).await,
        json!({"closed": ws::CLOSE_UNAUTHORIZED})
    );
}
