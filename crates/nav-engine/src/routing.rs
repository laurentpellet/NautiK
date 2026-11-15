//! Routes, waypoints, navigation active et traces.
//!
//! Chaque opération valide ses entrées (identifiants, positions, noms) et
//! délègue à `nav-routing` / `nav-storage`.

use chrono::{DateTime, Utc};
use nav_core::GeoPosition;
use nav_routing::{bounds_of, GeoBounds, Route, RoutePlan, Track, Waypoint, WaypointEdit};
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
    /// Emprise de la route (pour zoomer dessus).
    pub bounds: Option<GeoBounds>,
}

impl RouteView {
    fn new(route: Route, active_id: Option<Uuid>) -> Self {
        Self {
            total_distance_m: route.total_distance_m(),
            active: active_id == Some(route.id),
            bounds: bounds_of(route.waypoints.iter().map(|w| w.position)),
            route,
        }
    }
}

/// Trace complète, longueur parcourue, durée et emprise.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    /// Trace.
    #[serde(flatten)]
    pub track: Track,
    /// Longueur parcourue, en mètres.
    pub length_m: f64,
    /// Durée, en secondes, de la première à la dernière position
    /// enregistrée (`None` sans au moins deux points).
    pub duration_s: Option<f64>,
    /// Emprise de la trace (`None` sans point).
    pub bounds: Option<GeoBounds>,
}

impl TrackView {
    fn new(track: Track) -> Self {
        let duration_s = match (track.points.first(), track.points.last()) {
            (Some(first), Some(last)) if track.points.len() >= 2 => {
                // Millisecondes d'une trace : bien en deçà de 2^53.
                #[allow(clippy::cast_precision_loss)]
                let ms = (last.timestamp - first.timestamp).num_milliseconds() as f64;
                Some(ms / 1_000.0)
            }
            _ => None,
        };
        Self {
            length_m: track.length_m(),
            duration_s,
            bounds: bounds_of(track.points.iter().map(|p| p.position)),
            track,
        }
    }
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

/// Enregistre une route éditée sur la carte : création si `id` est absent,
/// sinon remplacement de son nom et de ses waypoints (déplacés, renommés,
/// ajoutés en départ, en arrivée ou en intermédiaire, retirés). Refusé
/// pour la route active.
///
/// # Errors
///
/// Message lisible si l'identifiant, le nom, un waypoint ou la longueur
/// sont invalides, ou si la route est introuvable ou active.
pub async fn save_route(
    service: &Service,
    id: Option<&str>,
    name: &str,
    waypoints: &[WaypointEdit],
) -> CommandResult<RouteView> {
    let now = Utc::now();
    let route = match id {
        None => Route::from_edits(name, waypoints, now).map_err(text)?,
        Some(id) => {
            let id = parse_id(id)?;
            ensure_not_active(service, id)?;
            let mut route = service
                .db
                .get_route(id)
                .await
                .map_err(text)?
                .ok_or("route introuvable")?;
            route.apply_edit(name, waypoints, now).map_err(text)?;
            route
        }
    };
    service.db.save_route(&route).await.map_err(text)?;
    tracing::info!(route = %route.name, points = route.waypoints.len(), "route enregistrée");
    Ok(RouteView::new(route, None))
}

/// Planifie un parcours (route enregistrée ou en cours d'édition) à la
/// vitesse prévue : longueur, durée, heures de passage et emprise.
///
/// # Errors
///
/// Message lisible si la vitesse ou le nombre de points sont invalides.
pub fn plan_route(
    positions: &[GeoPosition],
    speed_mps: f64,
    departure: Option<DateTime<Utc>>,
) -> CommandResult<RoutePlan> {
    nav_routing::plan(positions, speed_mps, departure).map_err(text)
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
    Ok(TrackView::new(track))
}

/// Profil d'une trace pour sa relecture : statistiques et vitesse de
/// chaque point.
///
/// # Errors
///
/// Message lisible si la trace est introuvable.
pub async fn track_profile(
    service: &Service,
    id: &str,
) -> CommandResult<nav_routing::track_profile::TrackProfile> {
    let id = parse_id(id)?;
    let track = service
        .db
        .get_track(id)
        .await
        .map_err(text)?
        .ok_or("trace introuvable")?;
    Ok(nav_routing::track_profile::profile(&track))
}

/// Renomme une trace.
///
/// # Errors
///
/// Message lisible si le nom est invalide ou la trace introuvable.
pub async fn rename_track(service: &Service, id: &str, name: &str) -> CommandResult<()> {
    let id = parse_id(id)?;
    let name = nav_routing::model::validate_name(name).map_err(text)?;
    if !service.db.rename_track(id, &name).await.map_err(text)? {
        return Err("trace introuvable".to_owned());
    }
    Ok(())
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
        let profile = track_profile(&s, &id).await.expect("profil");
        assert_eq!(profile.points.len(), 1);
        assert!(track_profile(&s, "x").await.is_err());
        assert!(view.track.ended_at.is_some());
        assert_eq!(list_tracks(&s).await.expect("list").len(), 1);
        delete_track(&s, &id).await.expect("delete");
    }

    fn edit(id: Option<Uuid>, name: &str, position: GeoPosition) -> WaypointEdit {
        WaypointEdit {
            id,
            name: Some(name.to_owned()),
            position,
        }
    }

    #[tokio::test]
    async fn save_route_creates_then_edits_with_departure_and_arrival() {
        let s = state().await;
        let created = save_route(
            &s,
            None,
            "Moorea",
            &[
                edit(None, "Papeete", p(-17.53, -149.57)),
                edit(None, "Vaiare", p(-17.52, -149.77)),
            ],
        )
        .await
        .expect("création");
        let id = created.route.id.to_string();
        let first = created.route.waypoints[0].id;
        let bounds = created.bounds.expect("emprise");
        assert_eq!((bounds.west, bounds.east), (-149.77, -149.57));

        let edited = save_route(
            &s,
            Some(&id),
            "Moorea par la passe",
            &[
                edit(Some(first), "Port de Papeete", p(-17.53, -149.57)),
                edit(None, "Passe de Taapuna", p(-17.57, -149.63)),
                edit(
                    created.route.waypoints.get(1).map(|w| w.id),
                    "Vaiare",
                    p(-17.52, -149.77),
                ),
                edit(None, "Arrivée Opunohu", p(-17.49, -149.85)),
            ],
        )
        .await
        .expect("édition");
        let names: Vec<_> = edited
            .route
            .waypoints
            .iter()
            .map(|w| w.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "Port de Papeete",
                "Passe de Taapuna",
                "Vaiare",
                "Arrivée Opunohu"
            ]
        );
        assert_eq!(edited.route.waypoints[0].id, first);
        let listed = list_routes(&s).await.expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].route, edited.route);
    }

    #[tokio::test]
    async fn save_route_rejects_invalid_edits_and_active_route() {
        let s = state().await;
        let two = [edit(None, "A", p(0.0, 0.0)), edit(None, "B", p(0.0, 1.0))];
        assert!(save_route(&s, None, "R", &two[..1]).await.is_err());
        assert!(save_route(&s, Some("pas-un-id"), "R", &two).await.is_err());
        let unknown = Uuid::new_v4().to_string();
        let err = save_route(&s, Some(&unknown), "R", &two)
            .await
            .expect_err("inconnue");
        assert!(err.contains("introuvable"), "{err}");

        let r = save_route(&s, None, "R", &two).await.expect("création");
        let id = r.route.id.to_string();
        let stranger = [edit(Some(Uuid::new_v4()), "A", p(0.0, 0.0)), two[1].clone()];
        let err = save_route(&s, Some(&id), "R", &stranger)
            .await
            .expect_err("waypoint étranger");
        assert!(err.contains("waypoint inconnu"), "{err}");

        activate_route(&s, &id).await.expect("activate");
        let err = save_route(&s, Some(&id), "R2", &two)
            .await
            .expect_err("active");
        assert!(err.contains("route active"), "{err}");
        deactivate(&s);
    }

    #[test]
    fn plan_route_validates_and_computes() {
        let plan = plan_route(&[p(0.0, 0.0), p(0.0, 1.0)], 5.0, None).expect("plan");
        assert!((plan.total_duration_s - plan.total_distance_m / 5.0).abs() < 1e-9);
        assert!(plan_route(&[p(0.0, 0.0), p(0.0, 1.0)], 0.0, None).is_err());
        assert!(plan_route(&[p(0.0, 0.0)], 5.0, None).is_err());
    }

    #[tokio::test]
    async fn track_view_has_duration_and_bounds_and_can_be_renamed() {
        let s = state().await;
        let t0 = Utc::now();
        let point = |seconds: i64, lat: f64| nav_routing::TrackPoint {
            timestamp: t0 + chrono::Duration::seconds(seconds),
            position: p(lat, -149.6),
            sog_mps: None,
            cog_rad: None,
            depth_m: None,
        };
        let track = Track {
            id: Uuid::new_v4(),
            name: "Import".to_owned(),
            started_at: t0,
            ended_at: Some(t0 + chrono::Duration::seconds(3_600)),
            points: vec![point(0, -17.5), point(1_800, -17.4), point(3_600, -17.3)],
        };
        s.db.create_track(&track, false).await.expect("create");
        let id = track.id.to_string();
        let view = get_track(&s, &id).await.expect("get");
        assert_eq!(view.duration_s, Some(3_600.0));
        let b = view.bounds.expect("emprise");
        assert_eq!((b.south, b.north), (-17.5, -17.3));

        rename_track(&s, &id, "  Tour de Tahiti ")
            .await
            .expect("rename");
        assert_eq!(
            list_tracks(&s).await.expect("list")[0].name,
            "Tour de Tahiti"
        );
        assert!(rename_track(&s, &id, "   ").await.is_err());
        assert!(rename_track(&s, &Uuid::new_v4().to_string(), "x")
            .await
            .is_err());
        assert!(rename_track(&s, "../x", "x").await.is_err());

        let single = Track {
            id: Uuid::new_v4(),
            points: vec![point(0, -17.5)],
            ..track
        };
        s.db.create_track(&single, false).await.expect("create");
        let view = get_track(&s, &single.id.to_string()).await.expect("get");
        assert_eq!(view.duration_s, None);
        assert!(view.bounds.is_some());
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
