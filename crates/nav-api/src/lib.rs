//! # nav-api
//!
//! API distante du service `nautikd` (Axum) : REST pour les commandes,
//! WebSocket pour le flux d'état en continu, authentification par jeton.
//!
//! Toutes les opérations délèguent à `nav-engine`, qui valide les
//! entrées : l'API ne contient aucune logique métier. Toutes les routes
//! sous `/api/v1` exigent le jeton ; seul `/health` est public (il ne
//! révèle que l'état de la boucle moteur).
//!
//! Le protocole est versionné (`/api/v1`, `protocolVersion` dans
//! [`nav_core::EngineInfo`]).

pub mod auth;
mod rest;
mod ws;

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use nav_engine::Service;
use tokio::net::TcpListener;
use tokio::sync::{watch, Semaphore};

pub use auth::{ApiToken, InvalidToken};

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-api";

/// Taille maximale d'un corps de requête (routes de plusieurs centaines
/// de points comprises).
pub const MAX_BODY_BYTES: usize = 256 * 1024;

/// Nombre maximal de sessions WebSocket simultanées.
pub const MAX_WS_SESSIONS: usize = 16;

/// État partagé par les routes.
#[derive(Clone)]
pub(crate) struct ApiState {
    pub(crate) service: Arc<Service>,
    pub(crate) token: Arc<ApiToken>,
    pub(crate) ws_slots: Arc<Semaphore>,
    pub(crate) stop: watch::Receiver<bool>,
}

/// Erreur renvoyée au client : `{"error": "message"}`.
#[derive(Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

/// Les erreurs des opérations du moteur sont des refus de la demande.
impl From<String> for ApiError {
    fn from(message: String) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

fn remote(request: &Request) -> String {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or_else(|| "inconnu".to_owned(), |c| c.0.to_string())
}

/// Refuse toute requête sans jeton valide.
async fn require_token(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    let presented = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(auth::bearer);
    match presented {
        Some(token) if state.token.matches(token) => next.run(request).await,
        _ => {
            tracing::warn!(
                component = COMPONENT,
                remote = %remote(&request),
                path = %request.uri().path(),
                "requête refusée : jeton absent ou invalide"
            );
            let mut response =
                ApiError::new(StatusCode::UNAUTHORIZED, "authentification requise").into_response();
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                header::HeaderValue::from_static("Bearer"),
            );
            response
        }
    }
}

/// État de santé : la boucle moteur tourne-t-elle ? (public, sans
/// donnée de navigation).
async fn health(State(state): State<ApiState>) -> Response {
    let last = state.service.last_tick();
    let alive = last.is_some_and(|t| (now_ms() - t.timestamp_millis()) < STALL_THRESHOLD_MS);
    let status = if alive {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(serde_json::json!({
            "status": if alive { "ok" } else { "stalled" },
            "version": nav_core::VERSION,
        })),
    )
        .into_response()
}

/// Au-delà, la boucle moteur est considérée comme bloquée.
const STALL_THRESHOLD_MS: i64 = 5_000;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

async fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "ressource inconnue")
}

/// Construit le routeur de l'API.
///
/// `stop` passe à `true` à l'arrêt du service : les sessions WebSocket se
/// terminent alors proprement.
pub fn router(service: Arc<Service>, token: ApiToken, stop: watch::Receiver<bool>) -> Router {
    let state = ApiState {
        service,
        token: Arc::new(token),
        ws_slots: Arc::new(Semaphore::new(MAX_WS_SESSIONS)),
        stop,
    };
    let protected =
        rest::routes().layer(middleware::from_fn_with_state(state.clone(), require_token));
    Router::new()
        .route("/health", get(health))
        // Le WebSocket s'authentifie lui-même (en-tête ou premier
        // message), les navigateurs ne pouvant pas fixer d'en-tête.
        .route("/api/v1/ws", get(ws::upgrade))
        .nest("/api/v1", protected)
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

/// Sert l'API sur `listener` jusqu'à ce que `shutdown` se termine.
///
/// # Errors
///
/// Erreur d'entrée/sortie du serveur.
pub async fn serve<F>(listener: TcpListener, router: Router, shutdown: F) -> std::io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}

/// Corps vide (réponses 204).
pub(crate) fn no_content() -> Response {
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
