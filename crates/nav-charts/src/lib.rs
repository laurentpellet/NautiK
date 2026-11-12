//! # nav-charts
//!
//! Catalogue et lecture des cartes MBTiles raster, validation des
//! métadonnées et service de tuiles locales.
//!
//! Les cartes sont lues en lecture seule, sans accès réseau. Le service
//! de tuiles ([`ChartCatalog::serve_tile`]) est indépendant du transport :
//! il est utilisé par le protocole local de l'application desktop et le
//! sera par l'API distante.
//!
//! Note : les fichiers MBTiles sont des bases SQLite en lecture seule
//! propres à chaque carte ; ils sont lus ici (voir `ARCHITECTURE.md` §4).
//! La base de l'application reste du ressort de `nav-storage`.

pub mod catalog;
pub mod error;
pub mod mbtiles;
pub mod metadata;
pub mod tile;

pub use catalog::{ChartCatalog, ChartInfo, ChartLoadFailure, TileReply};
pub use error::ChartError;
pub use mbtiles::{MbTiles, Tile};
pub use metadata::{Bounds, Center, ChartMetadata};
pub use tile::{ChartId, TileCoord, TileFormat};

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-charts";
