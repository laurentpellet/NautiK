//! Régions du Pacifique proposées au téléchargement.
//!
//! Une région qui traverse l'antiméridien (Fidji) est décrite par deux
//! rectangles, de part et d'autre.

use serde::Serialize;

/// Rectangle en degrés WGS84 (`west <= east`, sans traversée de
/// l'antiméridien).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BBox {
    /// Latitude sud.
    pub south: f64,
    /// Longitude ouest.
    pub west: f64,
    /// Latitude nord.
    pub north: f64,
    /// Longitude est.
    pub east: f64,
}

impl BBox {
    const fn new(south: f64, west: f64, north: f64, east: f64) -> Self {
        Self {
            south,
            west,
            north,
            east,
        }
    }

    /// Rectangle bien formé.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        (-90.0..=90.0).contains(&self.south)
            && (-90.0..=90.0).contains(&self.north)
            && (-180.0..=180.0).contains(&self.west)
            && (-180.0..=180.0).contains(&self.east)
            && self.south < self.north
            && self.west < self.east
    }
}

/// Région proposée.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoiRegion {
    /// Identifiant stable (`[a-z-]`).
    pub id: &'static str,
    /// Nom affiché.
    pub name: &'static str,
    /// Emprise (un ou deux rectangles).
    pub boxes: &'static [BBox],
}

/// Régions proposées, d'est en ouest.
pub const REGIONS: &[PoiRegion] = &[
    PoiRegion {
        id: "pitcairn",
        name: "Îles Pitcairn",
        boxes: &[BBox::new(-25.5, -131.0, -23.5, -124.5)],
    },
    PoiRegion {
        id: "polynesie-francaise",
        name: "Polynésie française",
        boxes: &[BBox::new(-28.5, -155.5, -7.0, -134.0)],
    },
    PoiRegion {
        id: "hawaii",
        name: "Hawaï",
        boxes: &[BBox::new(18.5, -161.0, 22.5, -154.5)],
    },
    PoiRegion {
        id: "iles-cook",
        name: "Îles Cook",
        boxes: &[BBox::new(-22.5, -166.5, -8.5, -157.0)],
    },
    PoiRegion {
        id: "samoa",
        name: "Samoa et Samoa américaines",
        boxes: &[BBox::new(-14.9, -173.0, -11.0, -168.0)],
    },
    PoiRegion {
        id: "tonga",
        name: "Tonga",
        boxes: &[BBox::new(-23.0, -176.5, -15.3, -173.5)],
    },
    PoiRegion {
        id: "wallis-futuna",
        name: "Wallis-et-Futuna",
        boxes: &[BBox::new(-14.6, -178.4, -13.0, -175.9)],
    },
    PoiRegion {
        id: "fidji",
        name: "Fidji",
        boxes: &[
            BBox::new(-21.5, 176.5, -12.0, 180.0),
            BBox::new(-21.5, -180.0, -15.5, -178.0),
        ],
    },
    PoiRegion {
        id: "vanuatu",
        name: "Vanuatu",
        boxes: &[BBox::new(-20.5, 166.0, -13.0, 170.5)],
    },
    PoiRegion {
        id: "nouvelle-caledonie",
        name: "Nouvelle-Calédonie",
        boxes: &[BBox::new(-23.5, 158.0, -17.5, 172.5)],
    },
    PoiRegion {
        id: "iles-salomon",
        name: "Îles Salomon",
        boxes: &[BBox::new(-12.5, 155.5, -5.0, 168.0)],
    },
];

/// Région d'identifiant donné.
#[must_use]
pub fn region(id: &str) -> Option<&'static PoiRegion> {
    REGIONS.iter().find(|r| r.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn regions_are_well_formed_and_unique() {
        let mut ids = HashSet::new();
        for r in REGIONS {
            assert!(ids.insert(r.id), "identifiant en double : {}", r.id);
            assert!(r.id.chars().all(|c| c.is_ascii_lowercase() || c == '-'));
            assert!(!r.boxes.is_empty());
            for b in r.boxes {
                assert!(b.is_valid(), "{} : {b:?}", r.id);
            }
        }
    }

    #[test]
    fn finds_regions_by_id() {
        assert_eq!(
            region("polynesie-francaise").map(|r| r.name),
            Some("Polynésie française")
        );
        assert_eq!(region("fidji").map(|r| r.boxes.len()), Some(2));
        assert!(region("../etc").is_none());
        assert!(region("").is_none());
    }

    #[test]
    fn rejects_malformed_boxes() {
        assert!(!BBox::new(10.0, 0.0, 5.0, 1.0).is_valid());
        assert!(!BBox::new(0.0, 170.0, 1.0, -170.0).is_valid());
        assert!(!BBox::new(-91.0, 0.0, 0.0, 1.0).is_valid());
    }
}
