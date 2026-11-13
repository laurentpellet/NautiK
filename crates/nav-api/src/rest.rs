//! Routes REST (`/api/v1/…`), protégées par jeton.
//!
//! Chaque route délègue à une opération de `nav-engine`, qui valide les
//! entrées ; les erreurs sont renvoyées en `400 {"error": "…"}`.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use nav_alarms::{AlarmEvent, AlarmsSnapshot};
use nav_core::{EngineInfo, GeoPosition};
use nav_engine::routing::{NavigationStateView, RouteView, TrackView};
use nav_engine::simulator::SimulatorSettings;
use nav_engine::sources::{SourcesRequest, SourcesView};
use nav_engine::{alarms, routing, simulator, sources, EngineEvent};
use nav_routing::Waypoint;
use nav_storage::TrackSummary;
use serde::Deserialize;

use crate::{no_content, ApiError, ApiState};

type ApiResult<T> = Result<Json<T>, ApiError>;

/// Nombre d'événements d'alarme renvoyés par défaut.
const DEFAULT_EVENTS: u32 = 50;

pub(crate) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/info", get(info))
        .route("/state", get(state))
        .route("/alarms", get(get_alarms))
        .route("/alarms/anchor", post(drop_anchor).delete(raise_anchor))
        .route("/alarms/events", get(alarm_events))
        .route("/alarms/{kind}/acknowledge", post(acknowledge))
        .route("/alarms/{kind}/enabled", put(set_alarm_enabled))
        .route(
            "/navigation",
            get(navigation).put(activate).delete(deactivate),
        )
        .route("/routes", get(list_routes).post(create_route))
        .route("/routes/{id}", delete(delete_route))
        .route("/routes/{id}/reverse", post(reverse_route))
        .route("/waypoints", get(list_waypoints))
        .route("/waypoints/{id}", delete(delete_waypoint))
        .route("/tracks", get(list_tracks))
        .route("/tracks/{id}", get(get_track).delete(delete_track))
        .route("/recording", post(start_recording).delete(stop_recording))
        .route("/sources", get(get_sources).put(set_sources))
        .route("/serial-ports", get(serial_ports))
        .route("/simulator", get(get_simulator).put(set_simulator))
}

async fn info() -> Json<EngineInfo> {
    Json(EngineInfo::current())
}

/// État complet courant (mêmes événements qu'à l'ouverture d'un
/// WebSocket).
async fn state(State(s): State<ApiState>) -> Json<Vec<EngineEvent>> {
    Json(s.service.current_events())
}

// ---------------------------------------------------------------- alarmes

async fn get_alarms(State(s): State<ApiState>) -> Json<AlarmsSnapshot> {
    Json(alarms::current(&s.service))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AnchorRequest {
    radius_m: f64,
    delay_s: u32,
}

async fn drop_anchor(
    State(s): State<ApiState>,
    Json(r): Json<AnchorRequest>,
) -> ApiResult<AlarmsSnapshot> {
    Ok(Json(
        alarms::drop_anchor(&s.service, r.radius_m, r.delay_s).await?,
    ))
}

async fn raise_anchor(State(s): State<ApiState>) -> ApiResult<AlarmsSnapshot> {
    Ok(Json(alarms::raise_anchor(&s.service).await?))
}

async fn acknowledge(
    State(s): State<ApiState>,
    Path(kind): Path<String>,
) -> ApiResult<AlarmsSnapshot> {
    Ok(Json(alarms::acknowledge(&s.service, &kind).await?))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnabledRequest {
    enabled: bool,
}

async fn set_alarm_enabled(
    State(s): State<ApiState>,
    Path(kind): Path<String>,
    Json(r): Json<EnabledRequest>,
) -> ApiResult<AlarmsSnapshot> {
    Ok(Json(
        alarms::set_enabled(&s.service, &kind, r.enabled).await?,
    ))
}

#[derive(Debug, Deserialize)]
struct EventsQuery {
    limit: Option<u32>,
}

async fn alarm_events(
    State(s): State<ApiState>,
    Query(q): Query<EventsQuery>,
) -> ApiResult<Vec<AlarmEvent>> {
    let limit = q.limit.unwrap_or(DEFAULT_EVENTS);
    Ok(Json(alarms::events(&s.service, limit).await?))
}

// ------------------------------------------------------------- navigation

async fn navigation(State(s): State<ApiState>) -> Json<NavigationStateView> {
    Json(routing::navigation_state(&s.service))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActivateRequest {
    route_id: String,
}

async fn activate(
    State(s): State<ApiState>,
    Json(r): Json<ActivateRequest>,
) -> Result<Response, ApiError> {
    routing::activate_route(&s.service, &r.route_id).await?;
    Ok(no_content())
}

async fn deactivate(State(s): State<ApiState>) -> Response {
    routing::deactivate(&s.service);
    no_content()
}

// ----------------------------------------------------------------- routes

async fn list_routes(State(s): State<ApiState>) -> ApiResult<Vec<RouteView>> {
    Ok(Json(routing::list_routes(&s.service).await?))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteRequest {
    name: String,
    positions: Vec<GeoPosition>,
}

async fn create_route(
    State(s): State<ApiState>,
    Json(r): Json<RouteRequest>,
) -> ApiResult<RouteView> {
    Ok(Json(
        routing::create_route(&s.service, &r.name, &r.positions).await?,
    ))
}

async fn reverse_route(State(s): State<ApiState>, Path(id): Path<String>) -> ApiResult<RouteView> {
    Ok(Json(routing::reverse_route(&s.service, &id).await?))
}

async fn delete_route(
    State(s): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    routing::delete_route(&s.service, &id).await?;
    Ok(no_content())
}

// -------------------------------------------------------------- waypoints

async fn list_waypoints(State(s): State<ApiState>) -> ApiResult<Vec<Waypoint>> {
    Ok(Json(routing::list_waypoints(&s.service).await?))
}

async fn delete_waypoint(
    State(s): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    routing::delete_waypoint(&s.service, &id).await?;
    Ok(no_content())
}

// ----------------------------------------------------------------- traces

async fn list_tracks(State(s): State<ApiState>) -> ApiResult<Vec<TrackSummary>> {
    Ok(Json(routing::list_tracks(&s.service).await?))
}

async fn get_track(State(s): State<ApiState>, Path(id): Path<String>) -> ApiResult<TrackView> {
    Ok(Json(routing::get_track(&s.service, &id).await?))
}

async fn delete_track(
    State(s): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    routing::delete_track(&s.service, &id).await?;
    Ok(no_content())
}

async fn start_recording(State(s): State<ApiState>) -> ApiResult<TrackSummary> {
    Ok(Json(routing::start_recording(&s.service).await?))
}

async fn stop_recording(State(s): State<ApiState>) -> Result<Response, ApiError> {
    routing::stop_recording(&s.service).await?;
    Ok(no_content())
}

// ---------------------------------------------------------------- sources

async fn get_sources(State(s): State<ApiState>) -> ApiResult<SourcesView> {
    Ok(Json(sources::get(&s.service).await?))
}

/// Modifie la source. Le rejeu d'un fichier est refusé à distance : un
/// chemin de fichier ne doit jamais venir d'une saisie libre d'un client
/// réseau.
async fn set_sources(
    State(s): State<ApiState>,
    Json(request): Json<SourcesRequest>,
) -> ApiResult<SourcesView> {
    if request.is_replay() {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "le rejeu d'un enregistrement n'est pas disponible à distance",
        ));
    }
    Ok(Json(sources::set(&s.service, request).await?))
}

async fn serial_ports() -> ApiResult<Vec<String>> {
    Ok(Json(sources::serial_ports().await?))
}

// -------------------------------------------------------------- simulateur

async fn get_simulator(State(s): State<ApiState>) -> Json<SimulatorSettings> {
    Json(simulator::current(&s.service))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SimulatorRequest {
    speed_mps: f64,
}

async fn set_simulator(
    State(s): State<ApiState>,
    Json(r): Json<SimulatorRequest>,
) -> ApiResult<SimulatorSettings> {
    Ok(Json(simulator::set(&s.service, r.speed_mps).await?))
}
