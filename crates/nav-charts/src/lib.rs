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
pub mod cm93;
pub mod cm93_map;
pub mod enc;
pub mod enc_map;
pub mod error;
pub mod mbtiles;
pub mod metadata;
pub mod s57;
pub mod tile;

pub use catalog::{ChartCatalog, ChartInfo, ChartLoadFailure, TileReply};
pub use cm93_map::{Cm93Library, Cm93View};
pub use enc::{find_cm93_root, resolve_cell_path, scan_enc_dir, EncCell, EncInventory};
pub use enc_map::{EncLibrary, EncMapCell, EncMapIndex};
pub use error::ChartError;
pub use mbtiles::{MbTiles, Tile};
pub use metadata::{Bounds, Center, ChartMetadata};
pub use s57::{
    decode_cell, decode_cell_file, S57Cell, S57CellSummary, S57Error, S57Feature, S57Geometry,
};
pub use tile::{ChartId, TileCoord, TileFormat};

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-charts";
