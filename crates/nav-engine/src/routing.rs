//! Routes, waypoints, navigation active et traces.
//!
//! Chaque opération valide ses entrées (identifiants, positions, noms) et
//! délègue à `nav-routing` / `nav-storage`.

use chrono::Utc;
use nav_core::GeoPosition;
use nav_routing::{Route, Track, Waypoint};
use nav_storage::TrackSummary;
use serde::Serialize;
use uuid::Uuid;

use crate::service::{EngineEvent, Service};
use crate::CommandResult;

/// Préfixe des noms de traces enregistrées depuis le simulateur.
pub const SIMULATED_TRACK_PREFIX: &str = "SIMULATION — ";

/// Analyse un identifiant transmis par une interface.
///
/// # Errors
///
/// Message lisible si l'identifiant n'est pas un UUID.
pub fn parse_id(id: &str) -> CommandResult<Uuid> {
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

/// Liste les routes.
///
/// # Errors
///
/// Message lisible en cas d'erreur de la base.
pub async fn list_routes(service: &Service) -> CommandResult<Vec<RouteView>> {
    let active = service.engine().active_route_id();
    let routes = service.db.list_routes().await.map_err(text)?;
    Ok(routes
        .into_iter()
        .map(|r| RouteView::new(r, active))
        .collect())
}

/// Crée une route à partir de positions (tracé sur la carte).
///
/// # Errors
///
/// Message lisible si le nom ou les positions sont invalides.
pub async fn create_route(
    service: &Service,
    name: &str,
    positions: &[GeoPosition],
) -> CommandResult<RouteView> {
    let route = Route::from_positions(name, positions, Utc::now()).map_err(text)?;
    service.db.save_route(&route).await.map_err(text)?;
    tracing::info!(route = %route.name, points = positions.len(), "route créée");
    Ok(RouteView::new(route, None))
}

fn ensure_not_active(service: &Service, id: Uuid) -> CommandResult<()> {
    if service.engine().active_route_id() == Some(id) {
        return Err("route active : désactivez la navigation d'abord".to_owned());
    }
    Ok(())
}

/// Inverse le sens d'une route (refusé si elle est active).
///
/// # Errors
///
/// Message lisible si la route est introuvable ou active.
pub async fn reverse_route(service: &Service, id: &str) -> CommandResult<RouteView> {
    let id = parse_id(id)?;
    ensure_not_active(service, id)?;
    let mut route = service
        .db
        .get_route(id)
        .await
        .map_err(text)?
        .ok_or("route introuvable")?;
    route.reverse(Utc::now());
    service.db.save_route(&route).await.map_err(text)?;
    Ok(RouteView::new(route, None))
}

/// Supprime une route (refusé si elle est active).
///
/// # Errors
///
/// Message lisible si la route est introuvable ou active.
pub async fn delete_route(service: &Service, id: &str) -> CommandResult<()> {
    let id = parse_id(id)?;
    ensure_not_active(service, id)?;
    if !service.db.delete_route(id).await.map_err(text)? {
        return Err("route introuvable".to_owned());
    }
    Ok(())
}

/// Active la navigation sur une route.
///
/// # Errors
///
/// Message lisible si la route est introuvable ou invalide.
pub async fn activate_route(service: &Service, id: &str) -> CommandResult<()> {
    let id = parse_id(id)?;
    let route = service
        .db
        .get_route(id)
        .await
        .map_err(text)?
        .ok_or("route introuvable")?;
    tracing::info!(route = %route.name, "navigation activée");
    service.engine().activate(route).map_err(text)
}

/// Désactive la navigation (diffuse un état de navigation vide).
pub fn deactivate(service: &Service) {
    if service.engine().deactivate() {
        tracing::info!("navigation désactivée");
    }
    service.publish(EngineEvent::Navigation(None));
}

/// État de la navigation et de l'enregistrement.
#[must_use]
pub fn navigation_state(service: &Service) -> NavigationStateView {
    let engine = service.engine();
    NavigationStateView {
        active_route_id: engine.active_route_id(),
        recording_track_id: engine.recording_track_id(),
    }
}

// ------------------------------------------------------------- waypoints

/// Liste les waypoints isolés.
///
/// # Errors
///
/// Message lisible en cas d'erreur de la base.
pub async fn list_waypoints(service: &Service) -> CommandResult<Vec<Waypoint>> {
    service.db.list_waypoints().await.map_err(text)
}

/// Supprime un waypoint isolé.
///
/// # Errors
///
/// Message lisible si le waypoint est introuvable.
pub async fn delete_waypoint(service: &Service, id: &str) -> CommandResult<()> {
    let id = parse_id(id)?;
    if !service.db.delete_waypoint(id).await.map_err(text)? {
        return Err("waypoint introuvable".to_owned());
    }
    Ok(())
}

// ---------------------------------------------------------------- traces

/// Démarre l'enregistrement d'une trace.
///
/// # Errors
///
/// Message lisible si un enregistrement est déjà en cours.
pub async fn start_recording(service: &Service) -> CommandResult<TrackSummary> {
    let (busy, simulated) = {
        let engine = service.engine();
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
    service.db.create_track(&track, true).await.map_err(text)?;
    let started = service.engine().start_recording(track.id);
    if let Err(error) = started {
        // Démarrage concurrent : on retire la trace vide créée.
        let _ = service.db.delete_track(track.id).await;
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

/// Arrête l'enregistrement en cours.
///
/// # Errors
///
/// Message lisible si aucun enregistrement n'est en cours.
pub async fn stop_recording(service: &Service) -> CommandResult<()> {
    let id = service
        .engine()
        .stop_recording()
        .ok_or("aucun enregistrement en cours")?;
    service
        .db
        .finish_track(id, Utc::now())
        .await
        .map_err(text)?;
    tracing::info!(track = %id, "enregistrement de trace terminé");
    Ok(())
}

/// Liste les traces (sans leurs points).
///
/// # Errors
///
/// Message lisible en cas d'erreur de la base.
pub async fn list_tracks(service: &Service) -> CommandResult<Vec<TrackSummary>> {
    service.db.list_tracks().await.map_err(text)
}

/// Supprime une trace (refusé pendant son enregistrement).
///
/// # Errors
///
/// Message lisible si la trace est introuvable ou en cours.
pub async fn delete_track(service: &Service, id: &str) -> CommandResult<()> {
    let id = parse_id(id)?;
    if service.engine().recording_track_id() == Some(id) {
        return Err("trace en cours d'enregistrement : arrêtez-la d'abord".to_owned());
    }
    if !service.db.delete_track(id).await.map_err(text)? {
        return Err("trace introuvable".to_owned());
    }
    Ok(())
}

/// Charge une trace avec ses points.
///
/// # Errors
///
/// Message lisible si la trace est introuvable.
pub async fn get_track(service: &Service, id: &str) -> CommandResult<TrackView> {
    let id = parse_id(id)?;
    let track = service
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_service as state;

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    #[tokio::test]
    async fn create_list_reverse_delete_route() {
        let s = state().await;
        let created = create_route(&s, " Passe ", &[p(-14.968, -147.638), p(-14.9, -147.6)])
            .await
            .expect("create");
        assert_eq!(created.route.name, "Passe");
        assert!(created.total_distance_m > 8_000.0);
        let id = created.route.id.to_string();

        let listed = list_routes(&s).await.expect("list");
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].active);

        let reversed = reverse_route(&s, &id).await.expect("reverse");
        assert_eq!(reversed.route.waypoints[0].position, p(-14.9, -147.6));

        delete_route(&s, &id).await.expect("delete");
        assert!(delete_route(&s, &id).await.is_err(), "déjà supprimée");
    }

    #[tokio::test]
    async fn invalid_inputs_are_rejected() {
        let s = state().await;
        assert!(create_route(&s, "x", &[p(0.0, 0.0)]).await.is_err());
        assert!(create_route(&s, "  ", &[p(0.0, 0.0), p(1.0, 1.0)])
            .await
            .is_err());
        let err = activate_route(&s, "../../etc").await.expect_err("id");
        assert!(err.contains("identifiant invalide"), "{err}");
        let unknown = Uuid::new_v4().to_string();
        assert!(activate_route(&s, &unknown).await.is_err());
        assert!(get_track(&s, &unknown).await.is_err());
        assert!(delete_track(&s, "").await.is_err());
        assert!(delete_waypoint(&s, &unknown).await.is_err());
    }

    #[tokio::test]
    async fn active_route_is_protected_and_deactivation_is_published() {
        let s = state().await;
        let r = create_route(&s, "R", &[p(-14.968, -147.638), p(-14.9, -147.6)])
            .await
            .expect("create");
        let id = r.route.id.to_string();
        activate_route(&s, &id).await.expect("activate");
        assert!(list_routes(&s).await.expect("list")[0].active);
        assert_eq!(navigation_state(&s).active_route_id, Some(r.route.id));
        assert!(delete_route(&s, &id).await.is_err());
        assert!(reverse_route(&s, &id).await.is_err());
        let mut rx = s.subscribe();
        deactivate(&s);
        assert_eq!(rx.try_recv().ok(), Some(EngineEvent::Navigation(None)));
        delete_route(&s, &id).await.expect("delete");
    }

    #[tokio::test]
    async fn recording_lifecycle_names_simulated_tracks() {
        let s = state().await;
        let summary = start_recording(&s).await.expect("start");
        assert!(summary.name.starts_with(SIMULATED_TRACK_PREFIX));
        assert!(start_recording(&s).await.is_err(), "un seul à la fois");
        let id = summary.id.to_string();
        assert!(delete_track(&s, &id).await.is_err(), "en cours");

        // Un pas de boucle persiste le premier point.
        s.step(Utc::now()).await;

        stop_recording(&s).await.expect("stop");
        assert!(stop_recording(&s).await.is_err());
        let view = get_track(&s, &id).await.expect("get");
        assert_eq!(view.track.points.len(), 1);
        assert!(view.track.ended_at.is_some());
        assert_eq!(list_tracks(&s).await.expect("list").len(), 1);
        delete_track(&s, &id).await.expect("delete");
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
