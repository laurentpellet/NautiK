//! Téléchargement optionnel du vent GFS 0,25° (NOAA NOMADS).
//!
//! Le service de filtrage NOMADS extrait le vent à 10 m d'une zone pour
//! chaque échéance ; les messages GRIB2 reçus sont concaténés en un seul
//! fichier, conservé localement pour un usage hors ligne. Internet n'est
//! utilisé que sur demande explicite de l'utilisateur.

use chrono::{DateTime, Duration, DurationRound, Utc};
use thiserror::Error;

use crate::grib2::{decode, GribError};
use crate::wind::{Bounds, WindError, WindForecast};

/// Service de filtrage GFS 0,25°.
pub const NOMADS_FILTER_URL: &str = "https://nomads.ncep.noaa.gov/cgi-bin/filter_gfs_0p25.pl";
/// Délai de publication d'un run GFS complet (approximatif).
const PUBLICATION_DELAY_H: i64 = 5;
/// Horizon maximal proposé (h).
pub const MAX_HORIZON_H: u32 = 120;
/// Pas des échéances (h).
pub const STEP_H: u32 = 3;
/// Largeur maximale de la zone (degrés) : limite le téléchargement.
pub const MAX_AREA_WIDTH_DEG: f64 = 120.0;
/// Hauteur maximale de la zone (degrés).
pub const MAX_AREA_HEIGHT_DEG: f64 = 90.0;
/// Taille maximale d'une réponse (octets).
pub const MAX_RESPONSE_BYTES: usize = 20 * 1024 * 1024;
/// Taille maximale du fichier assemblé (octets).
pub const MAX_TOTAL_BYTES: usize = 150 * 1024 * 1024;
/// Délai maximal d'une requête.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Erreur de téléchargement.
#[derive(Debug, Error)]
pub enum DownloadError {
    /// Paramètres refusés.
    #[error("demande invalide : {0}")]
    InvalidRequest(String),
    /// Réseau indisponible ou erreur HTTP.
    #[error("téléchargement impossible : {0}")]
    Http(String),
    /// Réponse trop volumineuse.
    #[error("réponse trop volumineuse")]
    TooLarge,
    /// Données reçues invalides.
    #[error("données reçues invalides : {0}")]
    Grib(#[from] GribError),
    /// Pas de vent dans les données reçues.
    #[error(transparent)]
    Wind(#[from] WindError),
}

/// Demande de téléchargement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GfsRequest {
    /// Zone.
    pub area: Bounds,
    /// Horizon (h), multiple de [`STEP_H`].
    pub horizon_h: u32,
}

impl GfsRequest {
    /// Valide la demande.
    ///
    /// # Errors
    ///
    /// [`DownloadError::InvalidRequest`] si la zone ou l'horizon sont hors
    /// limites.
    pub fn validate(&self) -> Result<(), DownloadError> {
        if !self.area.is_valid() {
            return Err(DownloadError::InvalidRequest("zone invalide".to_owned()));
        }
        if self.area.width() > MAX_AREA_WIDTH_DEG
            || self.area.north - self.area.south > MAX_AREA_HEIGHT_DEG
        {
            return Err(DownloadError::InvalidRequest(format!(
                "zone trop étendue (au plus {MAX_AREA_WIDTH_DEG}° × {MAX_AREA_HEIGHT_DEG}°)"
            )));
        }
        if self.horizon_h == 0 || self.horizon_h > MAX_HORIZON_H || self.horizon_h % STEP_H != 0 {
            return Err(DownloadError::InvalidRequest(format!(
                "horizon {} h (multiple de {STEP_H} h, au plus {MAX_HORIZON_H} h)",
                self.horizon_h
            )));
        }
        Ok(())
    }

    /// Échéances demandées (h).
    #[must_use]
    pub fn forecast_hours(&self) -> Vec<u32> {
        (0..=self.horizon_h).step_by(STEP_H as usize).collect()
    }
}

/// Dernier run GFS vraisemblablement publié à `now` (00, 06, 12, 18 UTC).
#[must_use]
pub fn latest_run(now: DateTime<Utc>) -> DateTime<Utc> {
    let t = now - Duration::hours(PUBLICATION_DELAY_H);
    t.duration_trunc(Duration::hours(6)).unwrap_or(t)
}

/// Adresse NOMADS du vent à 10 m pour un run, une échéance et une zone.
#[must_use]
pub fn gfs_url(run: DateTime<Utc>, hour: u32, area: &Bounds) -> String {
    let date = run.format("%Y%m%d");
    let cycle = run.format("%H");
    // Zone traversant l'antiméridien : longitudes est au-delà de 180°.
    let (left, right) = if area.west <= area.east {
        (area.west, area.east)
    } else {
        (area.west, area.east + 360.0)
    };
    format!(
        "{NOMADS_FILTER_URL}?dir=%2Fgfs.{date}%2F{cycle}%2Fatmos\
         &file=gfs.t{cycle}z.pgrb2.0p25.f{hour:03}\
         &var_UGRD=on&var_VGRD=on&lev_10_m_above_ground=on\
         &subregion=&leftlon={left:.2}&rightlon={right:.2}\
         &toplat={north:.2}&bottomlat={south:.2}",
        north = area.north,
        south = area.south,
    )
}

/// Fichier téléchargé.
#[derive(Debug)]
pub struct Downloaded {
    /// Run utilisé.
    pub run: DateTime<Utc>,
    /// Messages GRIB2 concaténés.
    pub bytes: Vec<u8>,
    /// Prévision décodée (vérification).
    pub forecast: WindForecast,
}

async fn fetch(client: &reqwest::Client, url: &str) -> Result<Option<Vec<u8>>, DownloadError> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| DownloadError::Http(e.without_url().to_string()))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(DownloadError::Http(format!("HTTP {}", response.status())));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
    {
        return Err(DownloadError::TooLarge);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| DownloadError::Http(e.without_url().to_string()))?
    {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(DownloadError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    // NOMADS répond parfois 200 avec une page d'erreur HTML.
    if !body.starts_with(b"GRIB") {
        return Err(DownloadError::Http(
            "réponse du service sans données GRIB".to_owned(),
        ));
    }
    Ok(Some(body))
}

/// Télécharge le vent GFS de la zone, échéance par échéance. Si le run
/// le plus récent n'est pas encore publié, le précédent est utilisé.
/// `progress(fait, total)` est appelé après chaque échéance.
///
/// # Errors
///
/// [`DownloadError`] en cas de demande invalide, d'erreur réseau, de
/// réponse trop volumineuse ou de données invalides.
pub async fn download_gfs(
    request: GfsRequest,
    now: DateTime<Utc>,
    mut progress: impl FnMut(usize, usize),
) -> Result<Downloaded, DownloadError> {
    request.validate()?;
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("NautiK/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| DownloadError::Http(e.to_string()))?;
    let hours = request.forecast_hours();
    let total = hours.len();

    let mut run = latest_run(now);
    let first = match fetch(&client, &gfs_url(run, 0, &request.area)).await? {
        Some(bytes) => bytes,
        None => {
            run -= Duration::hours(6);
            tracing::info!(%run, "run GFS récent non publié, run précédent");
            fetch(&client, &gfs_url(run, 0, &request.area))
                .await?
                .ok_or_else(|| DownloadError::Http("run GFS introuvable".to_owned()))?
        }
    };
    let mut bytes = first;
    progress(1, total);
    for (k, &hour) in hours.iter().enumerate().skip(1) {
        let part = fetch(&client, &gfs_url(run, hour, &request.area))
            .await?
            .ok_or_else(|| DownloadError::Http(format!("échéance +{hour} h introuvable")))?;
        if bytes.len() + part.len() > MAX_TOTAL_BYTES {
            return Err(DownloadError::TooLarge);
        }
        bytes.extend(part);
        progress(k + 1, total);
    }
    let forecast = WindForecast::from_decoded(decode(&bytes)?)?;
    Ok(Downloaded {
        run,
        bytes,
        forecast,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .expect("t")
            .with_timezone(&Utc)
    }

    fn area() -> Bounds {
        Bounds {
            west: -152.0,
            south: -20.0,
            east: -145.0,
            north: -14.0,
        }
    }

    #[test]
    fn picks_published_run() {
        assert_eq!(
            latest_run(t("2026-11-12T14:05:00Z")),
            t("2026-11-12T06:00:00Z")
        );
        assert_eq!(
            latest_run(t("2026-11-12T11:00:00Z")),
            t("2026-11-12T06:00:00Z")
        );
        assert_eq!(
            latest_run(t("2026-11-12T03:00:00Z")),
            t("2026-11-11T18:00:00Z")
        );
    }

    #[test]
    fn builds_filter_url() {
        let url = gfs_url(t("2026-11-12T06:00:00Z"), 3, &area());
        assert!(url.starts_with(NOMADS_FILTER_URL));
        assert!(url.contains("dir=%2Fgfs.20261112%2F06%2Fatmos"));
        assert!(url.contains("file=gfs.t06z.pgrb2.0p25.f003"));
        assert!(url.contains("lev_10_m_above_ground=on"));
        assert!(url.contains("leftlon=-152.00&rightlon=-145.00&toplat=-14.00&bottomlat=-20.00"));
        let across = Bounds {
            west: 170.0,
            east: -175.0,
            ..area()
        };
        assert!(gfs_url(t("2026-11-12T06:00:00Z"), 0, &across)
            .contains("leftlon=170.00&rightlon=185.00"));
    }

    #[test]
    fn validates_requests() {
        let ok = GfsRequest {
            area: area(),
            horizon_h: 48,
        };
        assert!(ok.validate().is_ok());
        assert_eq!(ok.forecast_hours().len(), 17);
        assert_eq!(ok.forecast_hours().last(), Some(&48));
        for horizon_h in [0, 47, 123, 240] {
            assert!(
                GfsRequest { horizon_h, ..ok }.validate().is_err(),
                "{horizon_h}"
            );
        }
        let huge = Bounds {
            west: -180.0,
            east: 179.0,
            ..area()
        };
        assert!(GfsRequest { area: huge, ..ok }.validate().is_err());
        let bad = Bounds {
            south: 10.0,
            ..area()
        };
        assert!(GfsRequest { area: bad, ..ok }.validate().is_err());
    }
}
