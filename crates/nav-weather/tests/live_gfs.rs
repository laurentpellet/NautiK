//! Vérification sur données réelles (réseau requis) :
//! `cargo test -p nav-weather -- --ignored`.

use chrono::Utc;
use nav_weather::{download_gfs, Bounds, GfsRequest};

#[tokio::test]
#[ignore = "réseau : télécharge GFS depuis NOAA NOMADS"]
async fn downloads_and_decodes_real_gfs_wind() {
    let request = GfsRequest {
        area: Bounds {
            west: -152.0,
            south: -20.0,
            east: -144.0,
            north: -13.0,
        },
        horizon_h: 3,
    };
    let mut calls = Vec::new();
    let d = download_gfs(request, Utc::now(), |done, total| calls.push((done, total)))
        .await
        .expect("téléchargement");
    assert_eq!(calls, vec![(1, 2), (2, 2)]);
    assert_eq!(d.forecast.steps.len(), 2);
    assert!(d.forecast.skipped.is_empty(), "{:?}", d.forecast.skipped);
    let b = d.forecast.bounds();
    assert!(
        (b.west + 152.0).abs() < 0.3 && (b.north + 13.0).abs() < 0.3,
        "{b:?}"
    );
    let points = d.forecast.sample(0, None, 10_000);
    // Grille 0,25° : 33 × 29 points.
    assert!(points.len() > 900, "{}", points.len());
    let max = points.iter().map(|p| p.speed_mps).fold(0.0, f64::max);
    let mean = points.iter().map(|p| p.speed_mps).sum::<f64>() / points.len() as f64;
    assert!(
        max < 40.0 && mean > 0.5 && mean < 20.0,
        "moy {mean} max {max}"
    );
    println!(
        "run {} : {} points, vent moyen {mean:.1} m/s, max {max:.1} m/s, {} octets",
        d.run,
        points.len(),
        d.bytes.len()
    );
}
