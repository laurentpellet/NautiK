//! Client Signal K : découverte HTTP, flux WebSocket, reconnexion.
//!
//! Déroulement d'une session :
//! 1. `GET /signalk` (découverte) pour obtenir l'adresse du flux ;
//! 2. connexion WebSocket (`?subscribe=none`), jeton dans l'en-tête
//!    `Authorization: Bearer …` ;
//! 3. abonnement aux chemins prioritaires ;
//! 4. lecture des deltas, ping périodique, détection de silence.
//!
//! Toute perte de connexion est signalée ([`ConnectionState`]) puis
//! suivie d'une nouvelle tentative avec un délai exponentiel borné.
//! Le jeton n'est jamais journalisé.

use std::time::Duration;

use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use url::Url;

pub use nav_core::link::ConnectionState;

use crate::config::SignalKConfig;
use crate::delta::{decode, subscription_message, Decoded, Sample, MAX_MESSAGE_BYTES};

/// Taille maximale de la réponse de découverte.
const MAX_DISCOVERY_BYTES: usize = 64 * 1024;
/// Chemin du flux WebSocket v1.
const STREAM_PATH: &str = "/signalk/v1/stream";

/// Réglages temporels du client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientOptions {
    /// Délai maximal de découverte et de connexion.
    pub connect_timeout: Duration,
    /// Silence (aucun message, pong compris) au-delà duquel la
    /// connexion est considérée perdue.
    pub idle_timeout: Duration,
    /// Intervalle des pings.
    pub ping_interval: Duration,
    /// Premier délai de reconnexion.
    pub initial_backoff: Duration,
    /// Délai de reconnexion maximal.
    pub max_backoff: Duration,
    /// Période d'abonnement demandée au serveur (ms).
    pub subscription_period_ms: u32,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(30),
            ping_interval: Duration::from_secs(10),
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            subscription_period_ms: 1_000,
        }
    }
}

impl ClientOptions {
    /// Délai avant la tentative `attempt` (à partir de 1).
    #[must_use]
    pub fn backoff(&self, attempt: u32) -> Duration {
        let factor = 2_u32.saturating_pow(attempt.saturating_sub(1).min(16));
        self.initial_backoff
            .saturating_mul(factor)
            .min(self.max_backoff)
    }
}

/// Événement produit par le client.
#[derive(Debug, Clone, PartialEq)]
pub enum SignalKEvent {
    /// Changement d'état de connexion.
    State(ConnectionState),
    /// Donnée normalisée.
    Sample(Sample),
    /// Donnée ou message rejeté.
    Rejected(String),
}

enum SessionEnd {
    Lost(String),
    Unauthorized(String),
    Shutdown,
    ReceiverGone,
}

#[derive(Deserialize)]
struct Discovery {
    endpoints: Option<Endpoints>,
    server: Option<ServerInfo>,
}

#[derive(Deserialize)]
struct Endpoints {
    v1: Option<EndpointV1>,
}

#[derive(Deserialize)]
struct EndpointV1 {
    #[serde(rename = "signalk-ws")]
    ws: Option<String>,
}

#[derive(Deserialize, Clone)]
struct ServerInfo {
    id: Option<String>,
    version: Option<String>,
}

/// Installe le fournisseur cryptographique `ring` pour rustls (une fois
/// par processus ; sans effet s'il est déjà installé).
fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Adresse WebSocket dérivée de l'adresse du serveur. L'adresse annoncée
/// par la découverte n'est retenue que si elle désigne le même hôte et le
/// même port : le jeton n'est jamais envoyé à un autre serveur.
fn stream_url(base: &Url, advertised: Option<&str>) -> Result<Url, String> {
    let mut url = base.clone();
    let scheme = if base.scheme() == "https" {
        "wss"
    } else {
        "ws"
    };
    url.set_scheme(scheme)
        .map_err(|()| "schéma WebSocket impossible".to_owned())?;
    url.set_path(STREAM_PATH);
    if let Some(candidate) = advertised.and_then(|a| Url::parse(a).ok()) {
        let same_origin = candidate.host_str() == base.host_str()
            && candidate.port_or_known_default() == url.port_or_known_default()
            && candidate.scheme() == scheme;
        if same_origin {
            url.set_path(candidate.path());
        }
    }
    url.set_query(Some("subscribe=none"));
    Ok(url)
}

async fn discover(
    http: &reqwest::Client,
    config: &SignalKConfig,
) -> Result<(Url, Option<ServerInfo>), SessionEnd> {
    let lost = |e: String| SessionEnd::Lost(format!("découverte HTTP : {e}"));
    let endpoint = config
        .base_url()
        .join("signalk")
        .map_err(|e| lost(e.to_string()))?;
    let response = http
        .get(endpoint)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| lost(e.without_url().to_string()))?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(SessionEnd::Unauthorized(format!(
            "découverte refusée ({status})"
        )));
    }
    if !status.is_success() {
        return Err(lost(format!("statut {status}")));
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_DISCOVERY_BYTES as u64)
    {
        return Err(lost("réponse trop volumineuse".to_owned()));
    }
    let body = response
        .bytes()
        .await
        .map_err(|e| lost(e.without_url().to_string()))?;
    if body.len() > MAX_DISCOVERY_BYTES {
        return Err(lost("réponse trop volumineuse".to_owned()));
    }
    let discovery: Discovery =
        serde_json::from_slice(&body).map_err(|e| lost(format!("réponse illisible : {e}")))?;
    let advertised = discovery.endpoints.and_then(|e| e.v1).and_then(|v1| v1.ws);
    let url = stream_url(config.base_url(), advertised.as_deref()).map_err(lost)?;
    Ok((url, discovery.server))
}

async fn emit(events: &mpsc::Sender<SignalKEvent>, event: SignalKEvent) -> Result<(), SessionEnd> {
    events
        .send(event)
        .await
        .map_err(|_| SessionEnd::ReceiverGone)
}

/// Une session complète ; retourne sa fin et si la connexion a abouti.
async fn session(
    config: &SignalKConfig,
    options: &ClientOptions,
    http: &reqwest::Client,
    events: &mpsc::Sender<SignalKEvent>,
    shutdown: &mut watch::Receiver<bool>,
) -> (bool, SessionEnd) {
    let (ws_url, server) = match discover(http, config).await {
        Ok(found) => found,
        Err(end) => return (false, end),
    };

    let mut request = match ws_url.as_str().into_client_request() {
        Ok(r) => r,
        Err(e) => return (false, SessionEnd::Lost(format!("requête WebSocket : {e}"))),
    };
    if let Some(token) = config.token() {
        match HeaderValue::from_str(&format!("Bearer {}", token.expose())) {
            Ok(value) => {
                request.headers_mut().insert(AUTHORIZATION, value);
            }
            Err(_) => {
                return (
                    false,
                    SessionEnd::Unauthorized("jeton inutilisable".to_owned()),
                )
            }
        }
    }
    let ws_config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));
    let connect = tokio_tungstenite::connect_async_with_config(request, Some(ws_config), false);
    let mut stream = match tokio::time::timeout(options.connect_timeout, connect).await {
        Err(_) => {
            return (
                false,
                SessionEnd::Lost("délai de connexion WebSocket dépassé".to_owned()),
            )
        }
        Ok(Err(WsError::Http(response)))
            if matches!(
                response.status(),
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
            ) =>
        {
            return (
                false,
                SessionEnd::Unauthorized(format!("accès refusé ({})", response.status())),
            );
        }
        Ok(Err(e)) => return (false, SessionEnd::Lost(format!("WebSocket : {e}"))),
        Ok(Ok((stream, _response))) => stream,
    };

    let subscribe = Message::text(subscription_message(options.subscription_period_ms));
    if let Err(e) = stream.send(subscribe).await {
        return (false, SessionEnd::Lost(format!("abonnement : {e}")));
    }
    let server = server.unwrap_or(ServerInfo {
        id: None,
        version: None,
    });
    let connected = ConnectionState::Connected {
        server_name: server.id,
        server_version: server.version,
    };
    if let Err(end) = emit(events, SignalKEvent::State(connected)).await {
        return (true, end);
    }
    tracing::info!(url = %config.display_url(), "Signal K connecté");

    let mut self_context: Option<String> = None;
    let mut ping = tokio::time::interval(options.ping_interval);
    ping.tick().await;
    // Le silence se mesure depuis le dernier message reçu (pong compris) ;
    // il ne doit pas être relancé par les autres branches du `select!`.
    let mut last_activity = tokio::time::Instant::now();
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                let _ = stream.close(None).await;
                return (true, SessionEnd::Shutdown);
            }
            _ = ping.tick() => {
                if let Err(e) = stream.send(Message::Ping(Vec::new().into())).await {
                    return (true, SessionEnd::Lost(format!("ping : {e}")));
                }
            }
            () = tokio::time::sleep_until(last_activity + options.idle_timeout) => {
                return (true, SessionEnd::Lost("aucune donnée reçue (délai dépassé)".to_owned()));
            }
            next = stream.next() => {
                last_activity = tokio::time::Instant::now();
                let message = match next {
                    None => return (true, SessionEnd::Lost("connexion fermée".to_owned())),
                    Some(Err(e)) => return (true, SessionEnd::Lost(format!("WebSocket : {e}"))),
                    Some(Ok(m)) => m,
                };
                let text = match message {
                    Message::Text(text) => text,
                    Message::Close(frame) => {
                        let reason = frame.map(|f| f.reason.to_string()).unwrap_or_default();
                        return (true, SessionEnd::Lost(format!("fermée par le serveur {reason}").trim().to_owned()));
                    }
                    Message::Binary(_) => {
                        if let Err(end) = emit(events, SignalKEvent::Rejected("message binaire ignoré".to_owned())).await {
                            return (true, end);
                        }
                        continue;
                    }
                    _ => continue,
                };
                let result = match decode(text.as_str(), self_context.as_deref(), Utc::now()) {
                    Err(e) => emit(events, SignalKEvent::Rejected(e.to_string())).await,
                    Ok(Decoded::Hello(hello)) => {
                        self_context = hello.self_context;
                        Ok(())
                    }
                    Ok(Decoded::Ignored) => Ok(()),
                    Ok(Decoded::Delta { samples, rejected }) => {
                        let mut r = Ok(());
                        for sample in samples {
                            r = emit(events, SignalKEvent::Sample(sample)).await;
                            if r.is_err() { break; }
                        }
                        for reason in rejected {
                            if r.is_err() { break; }
                            r = emit(events, SignalKEvent::Rejected(reason)).await;
                        }
                        r
                    }
                };
                if let Err(end) = result {
                    return (true, end);
                }
            }
        }
    }
}

/// Exécute le client jusqu'à l'arrêt (`shutdown` à `true`) ou la
/// fermeture du canal d'événements, en se reconnectant indéfiniment.
pub async fn run(
    config: SignalKConfig,
    options: ClientOptions,
    events: mpsc::Sender<SignalKEvent>,
    mut shutdown: watch::Receiver<bool>,
) {
    ensure_crypto_provider();
    let http = match reqwest::Client::builder()
        .connect_timeout(options.connect_timeout)
        .timeout(options.connect_timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            tracing::error!(error = %e, "client HTTP Signal K indisponible");
            let _ = events
                .send(SignalKEvent::State(ConnectionState::Disconnected {
                    reason: format!("client HTTP indisponible : {e}"),
                    retry_in_ms: 0,
                }))
                .await;
            return;
        }
    };

    let mut attempt: u32 = 0;
    loop {
        if *shutdown.borrow() {
            break;
        }
        attempt = attempt.saturating_add(1);
        if events
            .send(SignalKEvent::State(ConnectionState::Connecting { attempt }))
            .await
            .is_err()
        {
            return;
        }
        let (was_connected, end) = session(&config, &options, &http, &events, &mut shutdown).await;
        if was_connected {
            attempt = 1;
        }
        let delay = options.backoff(attempt);
        let retry_in_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX);
        let state = match end {
            SessionEnd::Shutdown => break,
            SessionEnd::ReceiverGone => return,
            SessionEnd::Lost(reason) => {
                tracing::warn!(url = %config.display_url(), %reason, "Signal K déconnecté");
                ConnectionState::Disconnected {
                    reason,
                    retry_in_ms,
                }
            }
            SessionEnd::Unauthorized(reason) => {
                tracing::warn!(url = %config.display_url(), %reason, "Signal K : authentification refusée");
                ConnectionState::Unauthorized {
                    reason,
                    retry_in_ms,
                }
            }
        };
        if events.send(SignalKEvent::State(state)).await.is_err() {
            return;
        }
        tokio::select! {
            () = tokio::time::sleep(delay) => {}
            _ = shutdown.changed() => break,
        }
    }
    let _ = events
        .send(SignalKEvent::State(ConnectionState::Stopped))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_is_capped() {
        let o = ClientOptions::default();
        let delays: Vec<u64> = (1..=8).map(|a| o.backoff(a).as_secs()).collect();
        assert_eq!(delays, [1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(o.backoff(u32::MAX), o.max_backoff);
        assert_eq!(o.backoff(0), o.initial_backoff);
    }

    #[test]
    fn stream_url_keeps_token_on_configured_host() {
        let base = Url::parse("http://boat.local:3000/").expect("url");
        let same = stream_url(&base, Some("ws://boat.local:3000/signalk/v1/stream")).expect("ok");
        assert_eq!(
            same.as_str(),
            "ws://boat.local:3000/signalk/v1/stream?subscribe=none"
        );
        let other = stream_url(&base, Some("ws://evil.example:3000/steal")).expect("ok");
        assert_eq!(other.host_str(), Some("boat.local"));
        assert_eq!(other.path(), STREAM_PATH);
        let tls = stream_url(&Url::parse("https://boat.local/").expect("url"), None).expect("ok");
        assert_eq!(
            tls.as_str(),
            "wss://boat.local/signalk/v1/stream?subscribe=none"
        );
    }

    #[test]
    fn states_serialize_with_tag() {
        let json = serde_json::to_value(ConnectionState::Disconnected {
            reason: "x".to_owned(),
            retry_in_ms: 2000,
        })
        .expect("json");
        assert_eq!(json["state"], "disconnected");
        assert_eq!(json["retryInMs"], 2000);
    }
}
