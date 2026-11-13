//! Prévision de vent à 10 m tirée d'un fichier GRIB2.
//!
//! Les composantes U (vers l'est) et V (vers le nord) d'une même
//! échéance sont associées ; une échéance incomplète est ignorée. Aucune
//! valeur n'est interpolée : un point sans donnée n'est pas affiché.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::grib2::{Decoded, Field, LatLonGrid};

/// Discipline météorologique, catégorie « quantité de mouvement ».
const DISCIPLINE_METEO: u8 = 0;
const CATEGORY_MOMENTUM: u8 = 2;
const PARAM_U: u8 = 2;
const PARAM_V: u8 = 3;
/// Surface « hauteur au-dessus du sol ».
const SURFACE_HEIGHT: u8 = 103;
const WIND_HEIGHT_M: f64 = 10.0;
/// Vitesse aberrante (m/s) : le point est écarté.
const MAX_WIND_MPS: f32 = 120.0;

/// Erreur de construction d'une prévision.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WindError {
    /// Aucun couple U/V de vent à 10 m.
    #[error("aucun vent à 10 m dans le fichier{0}")]
    NoWind(String),
}

/// Zone géographique (degrés). `west > east` : la zone traverse
/// l'antiméridien.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    /// Ouest.
    pub west: f64,
    /// Sud.
    pub south: f64,
    /// Est.
    pub east: f64,
    /// Nord.
    pub north: f64,
}

impl Bounds {
    /// Zone valide ? (coordonnées finies et dans leurs domaines)
    #[must_use]
    pub fn is_valid(&self) -> bool {
        [self.west, self.south, self.east, self.north]
            .iter()
            .all(|v| v.is_finite())
            && (-90.0..=90.0).contains(&self.south)
            && (-90.0..=90.0).contains(&self.north)
            && self.south < self.north
            && (-180.0..=180.0).contains(&self.west)
            && (-180.0..=180.0).contains(&self.east)
            && self.west != self.east
    }

    /// Étendue en longitude (degrés), antiméridien compris.
    #[must_use]
    pub fn width(&self) -> f64 {
        (self.east - self.west).rem_euclid(360.0)
    }

    /// Le point (longitude dans [-180, 180[) est-il dans la zone ?
    #[must_use]
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        if lat < self.south || lat > self.north {
            return false;
        }
        if self.west <= self.east {
            lon >= self.west && lon <= self.east
        } else {
            lon >= self.west || lon <= self.east
        }
    }
}

/// Une échéance.
#[derive(Debug, Clone, PartialEq)]
pub struct WindStep {
    /// Heure de validité.
    pub valid_time: DateTime<Utc>,
    u: Vec<Option<f32>>,
    v: Vec<Option<f32>>,
}

/// Vent en un point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindVector {
    /// Latitude (degrés).
    pub lat: f64,
    /// Longitude (degrés, [-180, 180[).
    pub lon: f64,
    /// Vitesse (m/s).
    pub speed_mps: f64,
    /// Direction d'où vient le vent, vraie (rad, [0, 2π)).
    pub direction_rad: f64,
}

/// Prévision de vent.
#[derive(Debug, Clone, PartialEq)]
pub struct WindForecast {
    /// Heure de référence (analyse du modèle).
    pub reference_time: DateTime<Utc>,
    /// Grille commune à toutes les échéances.
    pub grid: LatLonGrid,
    /// Échéances, par heure de validité croissante.
    pub steps: Vec<WindStep>,
    /// Champs ignorés (non pris en charge ou incomplets).
    pub skipped: Vec<String>,
}

fn is_wind(f: &Field, number: u8) -> bool {
    f.discipline == DISCIPLINE_METEO
        && f.category == CATEGORY_MOMENTUM
        && f.number == number
        && f.surface_type == SURFACE_HEIGHT
        && (f.surface_value - WIND_HEIGHT_M).abs() < 1e-6
}

impl WindForecast {
    /// Construit la prévision à partir des champs décodés.
    ///
    /// # Errors
    ///
    /// [`WindError::NoWind`] si aucune échéance complète n'est trouvée.
    pub fn from_decoded(decoded: Decoded) -> Result<Self, WindError> {
        let mut skipped = decoded.skipped;
        let (mut us, mut vs): (Vec<Field>, Vec<Field>) = (Vec::new(), Vec::new());
        for f in decoded.fields {
            if is_wind(&f, PARAM_U) {
                us.push(f);
            } else if is_wind(&f, PARAM_V) {
                vs.push(f);
            }
        }
        let Some(grid) = us.first().map(|f| f.grid.clone()) else {
            let detail = if skipped.is_empty() {
                String::new()
            } else {
                format!(" (ignorés : {})", skipped.join(", "))
            };
            return Err(WindError::NoWind(detail));
        };
        let reference_time = us
            .iter()
            .map(|f| f.reference_time)
            .min()
            .unwrap_or_default();
        let mut steps: Vec<WindStep> = Vec::new();
        for u in us {
            if u.grid != grid {
                skipped.push(format!("échéance {} sur une autre grille", u.valid_time));
                continue;
            }
            if steps.iter().any(|s| s.valid_time == u.valid_time) {
                continue;
            }
            let Some(pos) = vs
                .iter()
                .position(|v| v.valid_time == u.valid_time && v.grid == grid)
            else {
                skipped.push(format!("échéance {} sans composante V", u.valid_time));
                continue;
            };
            let v = vs.swap_remove(pos);
            steps.push(WindStep {
                valid_time: u.valid_time,
                u: u.values,
                v: v.values,
            });
        }
        if steps.is_empty() {
            return Err(WindError::NoWind(String::from(
                " (composantes incomplètes)",
            )));
        }
        steps.sort_by_key(|s| s.valid_time);
        Ok(Self {
            reference_time,
            grid,
            steps,
            skipped,
        })
    }

    /// Emprise de la grille.
    #[must_use]
    pub fn bounds(&self) -> Bounds {
        let g = &self.grid;
        let (lat_a, lon_a) = g.point(0, 0);
        let (lat_b, lon_b) = g.point(g.ni - 1, g.nj - 1);
        let (west, east) = if g.i_positive {
            (lon_a, lon_b)
        } else {
            (lon_b, lon_a)
        };
        let global = g.di * g.ni as f64 >= 359.0;
        Bounds {
            west: if global { -180.0 } else { west },
            south: lat_a.min(lat_b),
            east: if global { 180.0 } else { east },
            north: lat_a.max(lat_b),
        }
    }

    /// Vecteurs de vent de l'échéance `step`, limités à `area` et
    /// éclaircis (un point sur `k` dans chaque direction) pour ne pas
    /// dépasser `max_points`.
    #[must_use]
    pub fn sample(&self, step: usize, area: Option<Bounds>, max_points: usize) -> Vec<WindVector> {
        let Some(step) = self.steps.get(step) else {
            return Vec::new();
        };
        let g = &self.grid;
        let inside = |i: usize, j: usize| {
            let (lat, lon) = g.point(i, j);
            area.map_or(true, |a| a.contains(lat, lon))
        };
        // Nombre de points dans la zone, pour choisir le pas.
        let probe = (g.len() / 20_000).max(1);
        let mut count = 0usize;
        for k in (0..g.len()).step_by(probe) {
            if inside(k % g.ni, k / g.ni) {
                count += probe;
            }
        }
        let stride = ((count as f64 / max_points.max(1) as f64).sqrt().ceil() as usize).max(1);
        let mut out = Vec::new();
        for j in (0..g.nj).step_by(stride) {
            for i in (0..g.ni).step_by(stride) {
                if out.len() >= max_points {
                    return out;
                }
                let k = j * g.ni + i;
                let (Some(Some(u)), Some(Some(v))) = (step.u.get(k), step.v.get(k)) else {
                    continue;
                };
                let (u, v) = (f64::from(*u), f64::from(*v));
                let speed = u.hypot(v);
                if !speed.is_finite() || speed > f64::from(MAX_WIND_MPS) || !inside(i, j) {
                    continue;
                }
                let (lat, lon) = g.point(i, j);
                out.push(WindVector {
                    lat,
                    lon,
                    speed_mps: speed,
                    // Vent venant de : opposé du vecteur (u, v).
                    direction_rad: (-u).atan2(-v).rem_euclid(std::f64::consts::TAU),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grib2::decode;
    use crate::grib2::tests::{simple_message, Spec};
    use std::f64::consts::PI;

    /// Valeur brute pour une vitesse (m/s) : (−50 + X) / 10.
    fn raw(mps: f64) -> u32 {
        (mps * 10.0 + 50.0).round() as u32
    }

    fn file(hours: &[u32], u: f64, v: f64) -> Vec<u8> {
        let mut out = Vec::new();
        for &h in hours {
            for (number, value) in [(2, u), (3, v)] {
                let spec = Spec {
                    number,
                    forecast_h: h,
                    ..Spec::default()
                };
                out.extend(simple_message(&spec, &[raw(value); 6], None));
            }
        }
        out
    }

    #[test]
    fn pairs_components_by_valid_time() {
        let decoded = decode(&file(&[6, 0, 3], 0.0, -5.0)).expect("decode");
        let forecast = WindForecast::from_decoded(decoded).expect("forecast");
        let hours: Vec<i64> = forecast
            .steps
            .iter()
            .map(|s| (s.valid_time - forecast.reference_time).num_hours())
            .collect();
        assert_eq!(hours, vec![0, 3, 6]);
        // V = −5 : le vent souffle vers le sud, il vient du nord.
        let points = forecast.sample(0, None, 100);
        assert_eq!(points.len(), 6);
        assert!((points[0].speed_mps - 5.0).abs() < 1e-6);
        assert!(points[0].direction_rad.abs() < 1e-9);
        let east = WindForecast::from_decoded(decode(&file(&[0], -3.0, 0.0)).expect("d"))
            .expect("f")
            .sample(0, None, 10);
        assert!(
            (east[0].direction_rad - PI / 2.0).abs() < 1e-9,
            "vent d'est"
        );
    }

    #[test]
    fn requires_both_components() {
        let only_u = simple_message(&Spec::default(), &[1; 6], None);
        assert!(matches!(
            WindForecast::from_decoded(decode(&only_u).expect("d")),
            Err(WindError::NoWind(_))
        ));
        let pressure = Spec {
            category: 3,
            number: 1,
            ..Spec::default()
        };
        let decoded = decode(&simple_message(&pressure, &[1; 6], None)).expect("d");
        assert!(WindForecast::from_decoded(decoded).is_err());
    }

    #[test]
    fn samples_within_area_and_point_budget() {
        let f = WindForecast::from_decoded(decode(&file(&[0], 2.0, 2.0)).expect("d")).expect("f");
        let b = f.bounds();
        assert_eq!(
            (b.west, b.east, b.south, b.north),
            (-150.0, -149.0, -14.5, -14.0)
        );
        let area = Bounds {
            west: -149.6,
            south: -15.0,
            east: -148.0,
            north: -13.0,
        };
        assert_eq!(f.sample(0, Some(area), 100).len(), 4);
        assert!(f.sample(0, None, 2).len() <= 2);
        assert!(f.sample(9, None, 10).is_empty(), "échéance inexistante");
    }

    #[test]
    fn bounds_handle_antimeridian() {
        let b = Bounds {
            west: 170.0,
            south: -20.0,
            east: -170.0,
            north: -10.0,
        };
        assert!(b.is_valid());
        assert!((b.width() - 20.0).abs() < 1e-9);
        assert!(b.contains(-15.0, 175.0));
        assert!(b.contains(-15.0, -175.0));
        assert!(!b.contains(-15.0, 0.0));
        let bad = Bounds { south: 10.0, ..b };
        assert!(!bad.is_valid());
        assert!(!Bounds {
            west: f64::NAN,
            ..b
        }
        .is_valid());
    }
}
