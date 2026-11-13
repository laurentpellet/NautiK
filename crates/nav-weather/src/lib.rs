//! # nav-weather
//!
//! Météo de NautiK : décodage GRIB2 (sans dépendance native), prévision
//! de vent à 10 m et téléchargement optionnel du modèle GFS.
//!
//! Les fichiers sont exploités hors ligne ; Internet n'est utilisé que
//! sur demande explicite. Les prévisions sont une aide : elles ne
//! remplacent pas les bulletins météorologiques officiels.

mod bits;
pub mod gfs;
pub mod grib2;
pub mod wind;

pub use gfs::{download_gfs, DownloadError, Downloaded, GfsRequest};
pub use grib2::{decode, Decoded, Field, GribError, LatLonGrid};
pub use wind::{Bounds, WindError, WindForecast, WindStep, WindVector};
