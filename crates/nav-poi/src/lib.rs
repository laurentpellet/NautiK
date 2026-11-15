//! # nav-poi
//!
//! Îles et amers nautiques (passes, balises, bouées, feux, mouillages,
//! ports) issus d'OpenStreetMap, pour les régions du Pacifique.
//!
//! - [`regions`] : régions proposées et leurs emprises (dont celles qui
//!   traversent l'antiméridien) ;
//! - [`model`] : point d'intérêt validé ([`Poi`]) ;
//! - [`overpass`] : requête Overpass et analyse stricte de la réponse ;
//! - téléchargement (option `download`), déclenché uniquement à la demande
//!   de l'utilisateur ; les données sont ensuite exploitées hors ligne.
//!
//! Les données OpenStreetMap ne sont pas des données hydrographiques
//! officielles : elles complètent les cartes marines sans les remplacer.

pub mod model;
pub mod overpass;
pub mod regions;

#[cfg(feature = "download")]
mod download;

#[cfg(feature = "download")]
pub use download::{download_region, DEFAULT_ENDPOINT};
pub use model::{Poi, PoiError, PoiKind};
pub use overpass::{parse_overpass, query_for, ParsedPois, MAX_POIS};
pub use regions::{region, BBox, PoiRegion, REGIONS};

/// Attribution à afficher avec les données (licence ODbL).
pub const ATTRIBUTION: &str = "© contributeurs OpenStreetMap (ODbL)";

/// Nom du composant, utilisé comme identifiant de source dans les logs.
pub const COMPONENT: &str = "nav-poi";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_name_matches_crate() {
        assert_eq!(COMPONENT, env!("CARGO_PKG_NAME"));
    }
}
