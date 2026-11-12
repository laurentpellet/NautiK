//! Tests d'intégration du catalogue MBTiles sur des fichiers réels.

// Les fonctions utilitaires de test peuvent échouer bruyamment.
#![allow(clippy::expect_used)]

use std::path::Path;

use nav_charts::{ChartCatalog, ChartError, TileFormat};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake-png-tile";
const JPG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];

/// Tuile à écrire : zoom, colonne, ligne TMS, données.
type TileRow<'a> = (u8, u32, u32, &'a [u8]);

async fn write_mbtiles(path: &Path, metadata: &[(&str, &str)], tiles: &[TileRow<'_>]) {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("création MBTiles");
    sqlx::query("CREATE TABLE metadata (name TEXT, value TEXT)")
        .execute(&pool)
        .await
        .expect("table metadata");
    sqlx::query(
        "CREATE TABLE tiles (zoom_level INTEGER, tile_column INTEGER, tile_row INTEGER, tile_data BLOB)",
    )
    .execute(&pool)
    .await
    .expect("table tiles");
    for (name, value) in metadata {
        sqlx::query("INSERT INTO metadata VALUES (?1, ?2)")
            .bind(name)
            .bind(value)
            .execute(&pool)
            .await
            .expect("métadonnée");
    }
    for (z, x, y, data) in tiles {
        sqlx::query("INSERT INTO tiles VALUES (?1, ?2, ?3, ?4)")
            .bind(i64::from(*z))
            .bind(i64::from(*x))
            .bind(i64::from(*y))
            .bind(*data)
            .execute(&pool)
            .await
            .expect("tuile");
    }
    pool.close().await;
}

async fn simple_chart(dir: &Path, file: &str, name: &str, max_zoom: u8) {
    write_mbtiles(
        &dir.join(file),
        &[
            ("name", name),
            ("format", "png"),
            ("minzoom", "0"),
            ("maxzoom", &max_zoom.to_string()),
        ],
        &[(0, 0, 0, PNG)],
    )
    .await;
}

#[tokio::test]
async fn loads_valid_chart_with_metadata() {
    let dir = TempDir::new().expect("tempdir");
    write_mbtiles(
        &dir.path().join("Tahiti Nord.mbtiles"),
        &[
            ("name", "Tahiti Nord"),
            ("format", "png"),
            ("minzoom", "1"),
            ("maxzoom", "2"),
            ("bounds", "-150,-18,-149,-17"),
            ("attribution", "<b>Test</b>"),
        ],
        &[(1, 0, 1, PNG), (2, 1, 3, PNG)],
    )
    .await;

    let catalog = ChartCatalog::load_dir(dir.path()).await.expect("catalogue");
    let charts: Vec<_> = catalog.charts().collect();
    assert_eq!(charts.len(), 1);
    let chart = charts[0];
    assert_eq!(chart.id.as_str(), "tahiti-nord");
    assert_eq!(chart.name, "Tahiti Nord");
    assert_eq!(chart.file_name, "Tahiti Nord.mbtiles");
    assert_eq!(chart.format, TileFormat::Png);
    assert_eq!((chart.min_zoom, chart.max_zoom), (1, 2));
    assert!(chart.bounds.is_some());
    assert_eq!(chart.attribution.as_deref(), Some("<b>Test</b>"));
    assert!(catalog.failures().is_empty());
    catalog.close().await;
}

#[tokio::test]
async fn serves_tiles_with_xyz_to_tms_conversion() {
    let dir = TempDir::new().expect("tempdir");
    // Zoom 2 : ligne XYZ 0 == ligne TMS 3.
    write_mbtiles(
        &dir.path().join("chart.mbtiles"),
        &[("format", "png")],
        &[(2, 1, 3, PNG), (2, 1, 0, JPG)],
    )
    .await;
    let catalog = ChartCatalog::load_dir(dir.path()).await.expect("catalogue");

    let top = catalog.serve_tile("/chart/2/1/0").await;
    assert_eq!(top.status, 200);
    assert_eq!(top.content_type, Some("image/png"));
    assert_eq!(top.body, PNG);

    let bottom = catalog.serve_tile("/chart/2/1/3").await;
    assert_eq!(bottom.status, 200);
    assert_eq!(
        bottom.content_type,
        Some("image/jpeg"),
        "format détecté par tuile"
    );
    catalog.close().await;
}

#[tokio::test]
async fn tile_reply_statuses() {
    let dir = TempDir::new().expect("tempdir");
    simple_chart(dir.path(), "chart.mbtiles", "Chart", 3).await;
    let catalog = ChartCatalog::load_dir(dir.path()).await.expect("catalogue");

    assert_eq!(catalog.serve_tile("/chart/0/0/0").await.status, 200);
    let missing = catalog.serve_tile("/chart/3/2/2").await;
    assert_eq!(missing.status, 204, "tuile absente");
    assert!(missing.body.is_empty());
    assert_eq!(catalog.serve_tile("/other/0/0/0").await.status, 404);
    assert_eq!(catalog.serve_tile("/../0/0/0").await.status, 404);
    assert_eq!(catalog.serve_tile("/chart/0/0").await.status, 400);
    assert_eq!(catalog.serve_tile("/chart/1/5/0").await.status, 400);
    assert_eq!(catalog.serve_tile("/chart/a/b/c").await.status, 400);
    catalog.close().await;
}

#[tokio::test]
async fn invalid_files_are_reported_without_blocking_others() {
    let dir = TempDir::new().expect("tempdir");
    simple_chart(dir.path(), "good.mbtiles", "Good", 5).await;
    std::fs::write(
        dir.path().join("corrupted.mbtiles"),
        b"this is not sqlite at all",
    )
    .expect("fichier corrompu");
    write_mbtiles(&dir.path().join("empty.mbtiles"), &[("format", "png")], &[]).await;
    write_mbtiles(
        &dir.path().join("vector.mbtiles"),
        &[("format", "pbf")],
        &[(0, 0, 0, b"\x1f\x8b")],
    )
    .await;
    // Base SQLite valide mais sans tables MBTiles.
    {
        let options = SqliteConnectOptions::new()
            .filename(dir.path().join("notables.mbtiles"))
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .expect("base vide");
        sqlx::query("CREATE TABLE other (id INTEGER)")
            .execute(&pool)
            .await
            .expect("table");
        pool.close().await;
    }

    let catalog = ChartCatalog::load_dir(dir.path()).await.expect("catalogue");
    let ids: Vec<_> = catalog.charts().map(|c| c.id.as_str().to_owned()).collect();
    assert_eq!(ids, ["good"]);

    let mut failed: Vec<_> = catalog
        .failures()
        .iter()
        .map(|f| f.file_name.as_str())
        .collect();
    failed.sort_unstable();
    assert_eq!(
        failed,
        [
            "corrupted.mbtiles",
            "empty.mbtiles",
            "notables.mbtiles",
            "vector.mbtiles"
        ]
    );
    let vector = catalog
        .failures()
        .iter()
        .find(|f| f.file_name == "vector.mbtiles")
        .expect("échec vectoriel");
    assert!(vector.reason.contains("pbf"), "{}", vector.reason);
    catalog.close().await;
}

#[tokio::test]
async fn loads_multiple_charts_in_drawing_order_with_unique_ids() {
    let dir = TempDir::new().expect("tempdir");
    simple_chart(dir.path(), "detail.mbtiles", "Détail", 16).await;
    simple_chart(dir.path(), "overview.mbtiles", "Aperçu", 8).await;
    simple_chart(dir.path(), "tahiti nord.mbtiles", "Tahiti A", 12).await;
    simple_chart(dir.path(), "tahiti-nord.mbtiles", "Tahiti B", 12).await;
    std::fs::write(dir.path().join("readme.txt"), b"ignored").expect("fichier annexe");
    std::fs::create_dir(dir.path().join("subdir.mbtiles")).expect("répertoire");

    let catalog = ChartCatalog::load_dir(dir.path()).await.expect("catalogue");
    let names: Vec<_> = catalog.charts().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Aperçu", "Tahiti A", "Tahiti B", "Détail"]);

    let mut ids: Vec<_> = catalog.charts().map(|c| c.id.as_str().to_owned()).collect();
    ids.sort();
    assert_eq!(ids, ["detail", "overview", "tahiti-nord", "tahiti-nord-2"]);
    assert!(catalog.failures().is_empty());
    catalog.close().await;
}

#[tokio::test]
async fn empty_directory_gives_empty_catalog() {
    let dir = TempDir::new().expect("tempdir");
    let catalog = ChartCatalog::load_dir(dir.path()).await.expect("catalogue");
    assert_eq!(catalog.charts().count(), 0);
    assert_eq!(catalog.serve_tile("/any/0/0/0").await.status, 404);
}

#[tokio::test]
async fn missing_directory_is_an_error() {
    let dir = TempDir::new().expect("tempdir");
    let result = ChartCatalog::load_dir(&dir.path().join("absent")).await;
    assert!(matches!(
        result,
        Err(ChartError::DirectoryUnavailable { .. })
    ));
}

#[tokio::test]
async fn charts_are_opened_read_only() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("chart.mbtiles");
    simple_chart(dir.path(), "chart.mbtiles", "Chart", 3).await;
    let before = std::fs::read(&path).expect("lecture");

    let catalog = ChartCatalog::load_dir(dir.path()).await.expect("catalogue");
    assert_eq!(catalog.serve_tile("/chart/0/0/0").await.status, 200);
    catalog.close().await;

    assert_eq!(
        std::fs::read(&path).expect("lecture"),
        before,
        "fichier inchangé"
    );
}
