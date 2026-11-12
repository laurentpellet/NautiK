//! Commandes routes, waypoints, navigation active et traces.
//!
//! Chaque commande valide ses entrées (identifiants, positions, noms) et
//! délègue à `nav-routing` / `nav-storage`. Les fonctions `*_impl`
//! contiennent la logique d'adaptation et sont testées sans Tauri.

use chrono::Utc;
use nav_core::GeoPosition;
use nav_routing::{Route, Track, Waypoint};
use nav_storage::TrackSummary;
use serde::Serialize;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::runtime::{emit_navigation, AppState};

/// Résultat d'une commande : message d'erreur lisible côté interface.
pub type CommandResult<T> = Result<T, String>;

/// Préfixe des noms de traces enregistrées depuis le simulateur.
pub const SIMULATED_TRACK_PREFIX: &str = "SIMULATION — ";

fn parse_id(id: &str) -> CommandResult<Uuid> {
    Uuid::parse_str(id).map_err(|_| format!("identifiant invalide : « {id} »"))
}

fn text<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

/// Route et grandeurs dérivées, pour l'interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteView {
    /// Route.
    #[serde(flatten)]
    pub route: Route,
    /// Distance totale, en mètres.
    pub total_distance_m: f64,
    /// Route actuellement suivie.
    pub active: bool,
}

impl RouteView {
    fn new(route: Route, active_id: Option<Uuid>) -> Self {
        Self {
            total_distance_m: route.total_distance_m(),
            active: active_id == Some(route.id),
            route,
        }
    }
}

/// Trace complète et longueur parcourue.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    /// Trace.
    #[serde(flatten)]
    pub track: Track,
    /// Longueur parcourue, en mètres.
    pub length_m: f64,
}

/// État courant de la navigation et de l'enregistrement.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationStateView {
    /// Route active.
    pub active_route_id: Option<Uuid>,
    /// Trace en cours d'enregistrement.
    pub recording_track_id: Option<Uuid>,
}

// ---------------------------------------------------------------- routes

pub(crate) async fn list_routes_impl(state: &AppState) -> CommandResult<Vec<RouteView>> {
    let active = state.engine().active_route_id();
    let routes = state.db.list_routes().await.map_err(text)?;
    Ok(routes
        .into_iter()
        .map(|r| RouteView::new(r, active))
        .collect())
}

pub(crate) async fn create_route_impl(
    state: &AppState,
    name: &str,
    positions: &[GeoPosition],
) -> CommandResult<RouteView> {
    let route = Route::from_positions(name, positions, Utc::now()).map_err(text)?;
    state.db.save_route(&route).await.map_err(text)?;
    tracing::info!(route = %route.name, points = positions.len(), "route créée");
    Ok(RouteView::new(route, None))
}

fn ensure_not_active(state: &AppState, id: Uuid) -> CommandResult<()> {
    if state.engine().active_route_id() == Some(id) {
        return Err("route active : désactivez la navigation d'abord".to_owned());
    }
    Ok(())
}

pub(crate) async fn reverse_route_impl(state: &AppState, id: &str) -> CommandResult<RouteView> {
    let id = parse_id(id)?;
    ensure_not_active(state, id)?;
    let mut route = state
        .db
        .get_route(id)
        .await
        .map_err(text)?
        .ok_or("route introuvable")?;
    route.reverse(Utc::now());
    state.db.save_route(&route).await.map_err(text)?;
    Ok(RouteView::new(route, None))
}

pub(crate) async fn delete_route_impl(state: &AppState, id: &str) -> CommandResult<()> {
    let id = parse_id(id)?;
    ensure_not_active(state, id)?;
    if !state.db.delete_route(id).await.map_err(text)? {
        return Err("route introuvable".to_owned());
    }
    Ok(())
}

pub(crate) async fn activate_route_impl(state: &AppState, id: &str) -> CommandResult<()> {
    let id = parse_id(id)?;
    let route = state
        .db
        .get_route(id)
        .await
        .map_err(text)?
        .ok_or("route introuvable")?;
    tracing::info!(route = %route.name, "navigation activée");
    state.engine().activate(route).map_err(text)
}

/// Liste les routes.
#[tauri::command]
pub async fn list_routes(state: State<'_, AppState>) -> CommandResult<Vec<RouteView>> {
    list_routes_impl(&state).await
}

/// Crée une route à partir de positions (tracé sur la carte).
#[tauri::command]
pub async fn create_route(
    state: State<'_, AppState>,
    name: String,
    positions: Vec<GeoPosition>,
) -> CommandResult<RouteView> {
    create_route_impl(&state, &name, &positions).await
}

/// Inverse le sens d'une route (refusé si elle est active).
#[tauri::command]
pub async fn reverse_route(state: State<'_, AppState>, id: String) -> CommandResult<RouteView> {
    reverse_route_impl(&state, &id).await
}

/// Supprime une route (refusé si elle est active).
#[tauri::command]
pub async fn delete_route(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    delete_route_impl(&state, &id).await
}

/// Active la navigation sur une route.
#[tauri::command]
pub async fn activate_route(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    activate_route_impl(&state, &id).await
}

/// Désactive la navigation.
#[tauri::command]
pub fn deactivate_route(app: AppHandle, state: State<'_, AppState>) {
    if state.engine().deactivate() {
        tracing::info!("navigation désactivée");
    }
    emit_navigation(&app, None);
}

/// État de la navigation et de l'enregistrement.
#[tauri::command]
pub fn get_navigation_state(state: State<'_, AppState>) -> NavigationStateView {
    let engine = state.engine();
    NavigationStateView {
        active_route_id: engine.active_route_id(),
        recording_track_id: engine.recording_track_id(),
    }
}

// ------------------------------------------------------------- waypoints

/// Liste les waypoints isolés.
#[tauri::command]
pub async fn list_waypoints(state: State<'_, AppState>) -> CommandResult<Vec<Waypoint>> {
    state.db.list_waypoints().await.map_err(text)
}

/// Supprime un waypoint isolé.
#[tauri::command]
pub async fn delete_waypoint(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let id = parse_id(&id)?;
    if !state.db.delete_waypoint(id).await.map_err(text)? {
        return Err("waypoint introuvable".to_owned());
    }
    Ok(())
}

// ---------------------------------------------------------------- traces

pub(crate) async fn start_recording_impl(state: &AppState) -> CommandResult<TrackSummary> {
    let (busy, simulated) = {
        let engine = state.engine();
        (engine.recording_track_id().is_some(), engine.is_simulated())
    };
    if busy {
        return Err("un enregistrement est déjà en cours".to_owned());
    }
    let now = Utc::now();
    let prefix = if simulated {
        SIMULATED_TRACK_PREFIX
    } else {
        ""
    };
    let track = Track {
        id: Uuid::new_v4(),
        name: format!("{prefix}Trace du {}", now.format("%Y-%m-%d %H:%M UTC")),
        started_at: now,
        ended_at: None,
        points: Vec::new(),
    };
    state.db.create_track(&track, true).await.map_err(text)?;
    let started = state.engine().start_recording(track.id);
    if let Err(error) = started {
        // Démarrage concurrent : on retire la trace vide créée.
        let _ = state.db.delete_track(track.id).await;
        return Err(error.to_string());
    }
    tracing::info!(track = %track.name, "enregistrement de trace démarré");
    Ok(TrackSummary {
        id: track.id,
        name: track.name,
        started_at: now,
        ended_at: None,
        is_recording: true,
        point_count: 0,
    })
}

pub(crate) async fn stop_recording_impl(state: &AppState) -> CommandResult<()> {
    let id = state
        .engine()
        .stop_recording()
        .ok_or("aucun enregistrement en cours")?;
    state.db.finish_track(id, Utc::now()).await.map_err(text)?;
    tracing::info!(track = %id, "enregistrement de trace terminé");
    Ok(())
}

pub(crate) async fn delete_track_impl(state: &AppState, id: &str) -> CommandResult<()> {
    let id = parse_id(id)?;
    if state.engine().recording_track_id() == Some(id) {
        return Err("trace en cours d'enregistrement : arrêtez-la d'abord".to_owned());
    }
    if !state.db.delete_track(id).await.map_err(text)? {
        return Err("trace introuvable".to_owned());
    }
    Ok(())
}

pub(crate) async fn get_track_impl(state: &AppState, id: &str) -> CommandResult<TrackView> {
    let id = parse_id(id)?;
    let track = state
        .db
        .get_track(id)
        .await
        .map_err(text)?
        .ok_or("trace introuvable")?;
    Ok(TrackView {
        length_m: track.length_m(),
        track,
    })
}

/// Démarre l'enregistrement d'une trace.
#[tauri::command]
pub async fn start_track_recording(state: State<'_, AppState>) -> CommandResult<TrackSummary> {
    start_recording_impl(&state).await
}

/// Arrête l'enregistrement en cours.
#[tauri::command]
pub async fn stop_track_recording(state: State<'_, AppState>) -> CommandResult<()> {
    stop_recording_impl(&state).await
}

/// Liste les traces (sans leurs points).
#[tauri::command]
pub async fn list_tracks(state: State<'_, AppState>) -> CommandResult<Vec<TrackSummary>> {
    state.db.list_tracks().await.map_err(text)
}

/// Charge une trace avec ses points.
#[tauri::command]
pub async fn get_track(state: State<'_, AppState>, id: String) -> CommandResult<TrackView> {
    get_track_impl(&state, &id).await
}

/// Supprime une trace (refusé pendant son enregistrement).
#[tauri::command]
pub async fn delete_track(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    delete_track_impl(&state, &id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use nav_storage::Db;

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    async fn state() -> AppState {
        let db = Db::open_in_memory().await.expect("db");
        AppState::new(
            db,
            Engine::new(p(-14.968, -147.638), 4.0),
            std::sync::Arc::new(crate::secrets::MemoryStore::default()),
        )
    }

    #[tokio::test]
    async fn create_list_reverse_delete_route() {
        let s = state().await;
        let created = create_route_impl(&s, " Passe ", &[p(-14.968, -147.638), p(-14.9, -147.6)])
            .await
            .expect("create");
        assert_eq!(created.route.name, "Passe");
        assert!(created.total_distance_m > 8_000.0);
        let id = created.route.id.to_string();

        let listed = list_routes_impl(&s).await.expect("list");
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].active);

        let reversed = reverse_route_impl(&s, &id).await.expect("reverse");
        assert_eq!(reversed.route.waypoints[0].position, p(-14.9, -147.6));

        delete_route_impl(&s, &id).await.expect("delete");
        assert!(delete_route_impl(&s, &id).await.is_err(), "déjà supprimée");
    }

    #[tokio::test]
    async fn invalid_inputs_are_rejected() {
        let s = state().await;
        assert!(create_route_impl(&s, "x", &[p(0.0, 0.0)]).await.is_err());
        assert!(create_route_impl(&s, "  ", &[p(0.0, 0.0), p(1.0, 1.0)])
            .await
            .is_err());
        let err = activate_route_impl(&s, "../../etc").await.expect_err("id");
        assert!(err.contains("identifiant invalide"), "{err}");
        let unknown = Uuid::new_v4().to_string();
        assert!(activate_route_impl(&s, &unknown).await.is_err());
        assert!(get_track_impl(&s, &unknown).await.is_err());
        assert!(delete_track_impl(&s, "").await.is_err());
    }

    #[tokio::test]
    async fn active_route_is_protected() {
        let s = state().await;
        let r = create_route_impl(&s, "R", &[p(-14.968, -147.638), p(-14.9, -147.6)])
            .await
            .expect("create");
        let id = r.route.id.to_string();
        activate_route_impl(&s, &id).await.expect("activate");
        assert!(list_routes_impl(&s).await.expect("list")[0].active);
        assert!(delete_route_impl(&s, &id).await.is_err());
        assert!(reverse_route_impl(&s, &id).await.is_err());
        assert!(s.engine().deactivate());
        delete_route_impl(&s, &id).await.expect("delete");
    }

    #[tokio::test]
    async fn recording_lifecycle_names_simulated_tracks() {
        let s = state().await;
        let summary = start_recording_impl(&s).await.expect("start");
        assert!(summary.name.starts_with(SIMULATED_TRACK_PREFIX));
        assert!(start_recording_impl(&s).await.is_err(), "un seul à la fois");
        let id = summary.id.to_string();
        assert!(delete_track_impl(&s, &id).await.is_err(), "en cours");

        let tick = s.engine().tick(Utc::now());
        let (track_id, point) = tick
            .update
            .and_then(|u| u.track_point)
            .expect("premier point");
        s.db.append_track_points(track_id, &[point])
            .await
            .expect("append");

        stop_recording_impl(&s).await.expect("stop");
        assert!(stop_recording_impl(&s).await.is_err());
        let view = get_track_impl(&s, &id).await.expect("get");
        assert_eq!(view.track.points.len(), 1);
        assert!(view.track.ended_at.is_some());
        delete_track_impl(&s, &id).await.expect("delete");
    }

    #[test]
    fn route_view_serializes_flat_camel_case() {
        let route =
            Route::from_positions("R", &[p(0.0, 0.0), p(0.0, 1.0)], Utc::now()).expect("route");
        let json = serde_json::to_value(RouteView::new(route, None)).expect("json");
        assert!(json.get("waypoints").is_some());
        assert!(json.get("totalDistanceM").is_some());
        assert_eq!(json["active"], false);
    }
}
