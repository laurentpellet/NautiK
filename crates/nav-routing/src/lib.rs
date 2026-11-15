//! # nav-routing
//!
//! Waypoints, routes, traces, import/export GPX, navigation active et
//! aides à la navigation (vecteurs, VMG, laylines).
//!
//! Toute la logique est pure (sans E/S ni dépendance à l'interface) :
//! la persistance relève de `nav-storage`, l'orchestration de
//! l'application hôte (desktop ou service).

pub mod gpx;
pub mod kml;
pub mod logbook;
pub mod mark;
pub mod model;
pub mod navigation;
pub mod plan;
pub mod recorder;
pub mod route;
pub mod sailing;
pub mod search;
pub mod simulation;
pub mod track_profile;

pub use logbook::{LogEntry, LogKind};
pub use mark::{Mark, MarkIcon};
pub use model::{Route, Track, TrackPoint, ValidationError, Waypoint};
pub use navigation::{ActiveNavigation, NavigationStatus};
pub use plan::{bounds_of, plan, GeoBounds, PlanError, PlannedLeg, RoutePlan};
pub use recorder::{RecorderConfig, TrackRecorder};
pub use route::{Leg, WaypointEdit};
pub use simulation::RouteSimulator;

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-routing";
