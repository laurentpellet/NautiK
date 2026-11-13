//! Flux d'état WebSocket (`/api/v1/ws`).
//!
//! Authentification : en-tête `Authorization: Bearer …` à l'ouverture,
//! ou, pour les navigateurs, premier message
//! `{"type": "auth", "token": "…"}` dans les [`AUTH_TIMEOUT`]. Sans
//! jeton valide, la session est fermée (code [`CLOSE_UNAUTHORIZED`]).
//!
//! Le serveur envoie ensuite `{"type": "hello", "data": EngineInfo}`,
//! l'état complet courant, puis chaque [`EngineEvent`] au fil de l'eau.
//! Les commandes passent par l'API REST : les messages du client sont
//! ignorés. Un client trop lent perd des événements (il en est averti par
//! `{"type": "lagged", "data": n}`) ; les instantanés suivants rétablissent
//! l'état complet. La perte de connexion n'est jamais masquée : la
//! session se ferme.

use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use nav_core::EngineInfo;
use nav_engine::EngineEvent;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::OwnedSemaphorePermit;

use crate::{auth, ApiError, ApiState, COMPONENT};

/// Délai accordé au client pour s'authentifier.
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(5);

/// Code de fermeture : authentification absente ou invalide.
pub const CLOSE_UNAUTHORIZED: u16 = 4401;

/// Taille maximale d'un message reçu d'un client.
const MAX_CLIENT_MESSAGE: usize = 4 * 1024;

/// Message d'authentification d'un client.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ClientMessage {
    Auth { token: String },
}

/// Messages propres à la session (en plus des [`EngineEvent`]).
#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
enum SessionMessage {
    Hello(EngineInfo),
    Lagged(u64),
}

pub(crate) async fn upgrade(
    State(state): State<ApiState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let presented = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(auth::bearer);
    let authenticated = match presented {
        Some(token) if state.token.matches(token) => true,
        Some(_) => {
            tracing::warn!(component = COMPONENT, "WebSocket refusé : jeton invalide");
            return ApiError::new(StatusCode::UNAUTHORIZED, "authentification requise")
                .into_response();
        }
        None => false,
    };
    let Ok(permit) = state.ws_slots.clone().try_acquire_owned() else {
        tracing::warn!(component = COMPONENT, "WebSocket refusé : trop de sessions");
        return ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "trop de sessions ouvertes")
            .into_response();
    };
    ws.max_message_size(MAX_CLIENT_MESSAGE)
        .on_upgrade(move |socket| session(socket, state, authenticated, permit))
}

async fn send_json<T: Serialize>(socket: &mut WebSocket, value: &T) -> bool {
    match serde_json::to_string(value) {
        Ok(text) => socket.send(Message::Text(text.into())).await.is_ok(),
        Err(error) => {
            tracing::error!(component = COMPONENT, %error, "sérialisation impossible");
            true
        }
    }
}

async fn close(socket: &mut WebSocket, code: u16, reason: &'static str) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}

/// Attend le message d'authentification.
async fn authenticate(socket: &mut WebSocket, state: &ApiState) -> bool {
    let Ok(Some(Ok(Message::Text(text)))) = tokio::time::timeout(AUTH_TIMEOUT, socket.recv()).await
    else {
        return false;
    };
    matches!(
        serde_json::from_str::<ClientMessage>(text.as_str()),
        Ok(ClientMessage::Auth { token }) if state.token.matches(&token)
    )
}

async fn session(
    mut socket: WebSocket,
    state: ApiState,
    authenticated: bool,
    _permit: OwnedSemaphorePermit,
) {
    if !authenticated && !authenticate(&mut socket, &state).await {
        tracing::warn!(
            component = COMPONENT,
            "WebSocket fermé : authentification échouée"
        );
        close(&mut socket, CLOSE_UNAUTHORIZED, "authentification requise").await;
        return;
    }
    tracing::info!(component = COMPONENT, "client WebSocket connecté");
    // Abonnement avant l'état initial : aucun événement n'est perdu (au
    // pire reçu deux fois).
    let mut events = state.service.subscribe();
    let mut stop = state.stop.clone();
    if !send_json(&mut socket, &SessionMessage::Hello(EngineInfo::current())).await {
        return;
    }
    for event in state.service.current_events() {
        if !send_json(&mut socket, &event).await {
            return;
        }
    }
    loop {
        if *stop.borrow() {
            close(&mut socket, 1001, "arrêt du service").await;
            break;
        }
        tokio::select! {
            event = events.recv() => match event {
                Ok(event) => {
                    if !send_json::<EngineEvent>(&mut socket, &event).await {
                        break;
                    }
                }
                Err(RecvError::Lagged(missed)) => {
                    tracing::warn!(component = COMPONENT, missed, "client WebSocket trop lent");
                    if !send_json(&mut socket, &SessionMessage::Lagged(missed)).await {
                        break;
                    }
                }
                Err(RecvError::Closed) => break,
            },
            incoming = socket.recv() => match incoming {
                // Les commandes passent par REST ; les autres messages
                // (ping/pong gérés par la bibliothèque) sont ignorés.
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            changed = stop.changed() => {
                if changed.is_err() {
                    break;
                }
            }
        }
    }
    tracing::info!(component = COMPONENT, "client WebSocket déconnecté");
}
