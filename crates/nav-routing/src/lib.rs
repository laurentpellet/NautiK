//! # nav-routing
//!
//! Waypoints, routes, traces, import/export GPX et navigation active.
//!
//! Toute la logique est pure (sans E/S ni dépendance à l'interface) :
//! la persistance relève de `nav-storage`, l'orchestration de
//! l'application hôte (desktop ou service).

pub mod gpx;
pub mod model;
pub mod navigation;
pub mod recorder;
pub mod route;
pub mod simulation;

pub use model::{Route, Track, TrackPoint, ValidationError, Waypoint};
pub use navigation::{ActiveNavigation, NavigationStatus};
pub use recorder::{RecorderConfig, TrackRecorder};
pub use route::Leg;
pub use simulation::RouteSimulator;

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-routing";
