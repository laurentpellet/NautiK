//! Commandes routes, waypoints, navigation active et traces : délèguent
//! à `nav_engine::routing`, qui valide les entrées.

use chrono::{DateTime, Utc};
use nav_core::GeoPosition;
use nav_engine::routing::{self, NavigationStateView, RouteView, TrackView};
use nav_engine::CommandResult;
use nav_routing::{RoutePlan, Waypoint, WaypointEdit};
use nav_storage::TrackSummary;
use tauri::State;

use crate::AppState;

/// Liste les routes.
#[tauri::command]
pub async fn list_routes(state: State<'_, AppState>) -> CommandResult<Vec<RouteView>> {
    routing::list_routes(&state).await
}

/// Crée une route à partir de positions (tracé sur la carte).
#[tauri::command]
pub async fn create_route(
    state: State<'_, AppState>,
    name: String,
    positions: Vec<GeoPosition>,
) -> CommandResult<RouteView> {
    routing::create_route(&state, &name, &positions).await
}

/// Inverse le sens d'une route (refusé si elle est active).
#[tauri::command]
pub async fn reverse_route(state: State<'_, AppState>, id: String) -> CommandResult<RouteView> {
    routing::reverse_route(&state, &id).await
}

/// Supprime une route (refusé si elle est active).
#[tauri::command]
pub async fn delete_route(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    routing::delete_route(&state, &id).await
}

/// Active la navigation sur une route.
#[tauri::command]
pub async fn activate_route(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    routing::activate_route(&state, &id).await
}

/// Désactive la navigation.
#[tauri::command]
pub fn deactivate_route(state: State<'_, AppState>) {
    routing::deactivate(&state);
}

/// État de la navigation et de l'enregistrement.
#[tauri::command]
pub fn get_navigation_state(state: State<'_, AppState>) -> NavigationStateView {
    routing::navigation_state(&state)
}

/// Liste les waypoints isolés.
#[tauri::command]
pub async fn list_waypoints(state: State<'_, AppState>) -> CommandResult<Vec<Waypoint>> {
    routing::list_waypoints(&state).await
}

/// Supprime un waypoint isolé.
#[tauri::command]
pub async fn delete_waypoint(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    routing::delete_waypoint(&state, &id).await
}

/// Démarre l'enregistrement d'une trace.
#[tauri::command]
pub async fn start_track_recording(state: State<'_, AppState>) -> CommandResult<TrackSummary> {
    routing::start_recording(&state).await
}

/// Arrête l'enregistrement en cours.
#[tauri::command]
pub async fn stop_track_recording(state: State<'_, AppState>) -> CommandResult<()> {
    routing::stop_recording(&state).await
}

/// Liste les traces (sans leurs points).
#[tauri::command]
pub async fn list_tracks(state: State<'_, AppState>) -> CommandResult<Vec<TrackSummary>> {
    routing::list_tracks(&state).await
}

/// Charge une trace avec ses points.
#[tauri::command]
pub async fn get_track(state: State<'_, AppState>, id: String) -> CommandResult<TrackView> {
    routing::get_track(&state, &id).await
}

/// Supprime une trace (refusé pendant son enregistrement).
#[tauri::command]
pub async fn delete_track(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    routing::delete_track(&state, &id).await
}

/// Enregistre une route éditée sur la carte (création si `id` est absent).
#[tauri::command]
pub async fn save_route(
    state: State<'_, AppState>,
    id: Option<String>,
    name: String,
    waypoints: Vec<WaypointEdit>,
) -> CommandResult<RouteView> {
    routing::save_route(&state, id.as_deref(), &name, &waypoints).await
}

/// Planifie un parcours à la vitesse prévue (durée, heures de passage, emprise).
#[tauri::command]
pub fn plan_route(
    positions: Vec<GeoPosition>,
    speed_mps: f64,
    departure: Option<DateTime<Utc>>,
) -> CommandResult<RoutePlan> {
    routing::plan_route(&positions, speed_mps, departure)
}

/// Renomme une trace.
#[tauri::command]
pub async fn rename_track(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> CommandResult<()> {
    routing::rename_track(&state, &id, &name).await
}

/// Profil d'une trace (statistiques, vitesse de chaque point) pour sa
/// relecture.
#[tauri::command]
pub async fn get_track_profile(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<nav_routing::track_profile::TrackProfile> {
    routing::track_profile(&state, &id).await
}

/// Recherche un lieu (coordonnées, marques, waypoints, îles et amers).
#[tauri::command]
pub async fn search_places(
    state: State<'_, AppState>,
    query: String,
) -> CommandResult<Vec<nav_engine::search::Place>> {
    nav_engine::search::search(&state, &query).await
}
