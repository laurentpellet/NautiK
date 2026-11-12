//! Génère une carte MBTiles raster synthétique pour les tests et les
//! démonstrations hors ligne.
//!
//! Les tuiles représentent une grille colorée (couleur par niveau de
//! zoom) : ce n'est PAS une carte marine et elle ne doit jamais servir à
//! la navigation.
//!
//! Usage :
//!
//! ```text
//! cargo run -p nav-charts --example make_test_chart -- \
//!     <sortie.mbtiles> <nom> <zoom_min> <zoom_max> <ouest,sud,est,nord>
//! ```
//!
//! Exemple (Tahiti) :
//!
//! ```text
//! cargo run -p nav-charts --example make_test_chart -- \
//!     tahiti-test.mbtiles "Tahiti (test)" 5 11 -149.95,-17.9,-149.1,-17.45
//! ```

use std::f64::consts::PI;
use std::path::Path;

use anyhow::{bail, Context};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const TILE_SIZE: u32 = 256;
const MAX_TILES: u64 = 50_000;

/// Couleurs de fond par niveau de zoom (cycle).
const PALETTE: [[u8; 3]; 6] = [
    [170, 211, 223],
    [185, 220, 200],
    [225, 215, 175],
    [215, 190, 210],
    [200, 200, 230],
    [230, 200, 185],
];

fn lon_to_x(lon: f64, z: u8) -> u32 {
    let n = f64::from(1_u32 << z);
    (((lon + 180.0) / 360.0 * n).floor()).clamp(0.0, n - 1.0) as u32
}

fn lat_to_y(lat: f64, z: u8) -> u32 {
    let n = f64::from(1_u32 << z);
    let lat = lat.clamp(-85.0511, 85.0511).to_radians();
    let y = (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / PI) / 2.0 * n;
    y.floor().clamp(0.0, n - 1.0) as u32
}

fn render_tile(z: u8, x: u32, y: u32) -> anyhow::Result<Vec<u8>> {
    let [r, g, b] = PALETTE[usize::from(z) % PALETTE.len()];
    let checker = (x + y) % 2 == 0;
    let mut pixels = Vec::with_capacity((TILE_SIZE * TILE_SIZE * 3) as usize);
    for py in 0..TILE_SIZE {
        for px in 0..TILE_SIZE {
            let border = px < 2 || py < 2;
            let cross = px.abs_diff(py) < 2 || (px + py).abs_diff(TILE_SIZE) < 2;
            let rgb = if border {
                [40, 60, 90]
            } else if cross {
                [90, 120, 150]
            } else if checker {
                [r, g, b]
            } else {
                [
                    r.saturating_sub(15),
                    g.saturating_sub(15),
                    b.saturating_sub(15),
                ]
            };
            pixels.extend_from_slice(&rgb);
        }
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, TILE_SIZE, TILE_SIZE);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().context("en-tête PNG")?;
    writer.write_image_data(&pixels).context("données PNG")?;
    writer.finish().context("fin PNG")?;
    Ok(out)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, name, min_zoom, max_zoom, bounds] = args.as_slice() else {
        bail!("usage : make_test_chart <sortie.mbtiles> <nom> <zoom_min> <zoom_max> <ouest,sud,est,nord>");
    };
    let min_zoom: u8 = min_zoom.parse().context("zoom_min")?;
    let max_zoom: u8 = max_zoom.parse().context("zoom_max")?;
    if min_zoom > max_zoom || max_zoom > 18 {
        bail!("plage de zoom invalide (0 ≤ min ≤ max ≤ 18)");
    }
    let parts: Vec<f64> = bounds
        .split(',')
        .map(|p| p.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .context("emprise")?;
    let [west, south, east, north] = parts.as_slice() else {
        bail!("emprise attendue : ouest,sud,est,nord");
    };
    let (west, south, east, north) = (*west, *south, *east, *north);
    if !(west < east && south < north) {
        bail!("emprise invalide");
    }

    let ranges: Vec<(u8, u32, u32, u32, u32)> = (min_zoom..=max_zoom)
        .map(|z| {
            (
                z,
                lon_to_x(west, z),
                lon_to_x(east, z),
                lat_to_y(north, z),
                lat_to_y(south, z),
            )
        })
        .collect();
    let total: u64 = ranges
        .iter()
        .map(|(_, x0, x1, y0, y1)| u64::from(x1 - x0 + 1) * u64::from(y1 - y0 + 1))
        .sum();
    if total > MAX_TILES {
        bail!("{total} tuiles demandées (maximum {MAX_TILES}) : réduire l'emprise ou le zoom");
    }

    let path = Path::new(out);
    if path.exists() {
        bail!("{out} existe déjà");
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    sqlx::query("CREATE TABLE metadata (name TEXT, value TEXT)")
        .execute(&pool)
        .await?;
    sqlx::query(
        "CREATE TABLE tiles (zoom_level INTEGER, tile_column INTEGER, tile_row INTEGER, tile_data BLOB)",
    )
    .execute(&pool)
    .await?;
    sqlx::query("CREATE UNIQUE INDEX tile_index ON tiles (zoom_level, tile_column, tile_row)")
        .execute(&pool)
        .await?;

    let center_lon = (west + east) / 2.0;
    let center_lat = (south + north) / 2.0;
    let metadata = [
        ("name", name.clone()),
        ("format", "png".to_owned()),
        ("type", "baselayer".to_owned()),
        ("minzoom", min_zoom.to_string()),
        ("maxzoom", max_zoom.to_string()),
        ("bounds", format!("{west},{south},{east},{north}")),
        ("center", format!("{center_lon},{center_lat},{min_zoom}")),
        (
            "description",
            "Carte synthétique de test NautiK — ne pas utiliser pour la navigation".to_owned(),
        ),
        ("attribution", "NautiK — carte de test".to_owned()),
    ];
    for (key, value) in metadata {
        sqlx::query("INSERT INTO metadata VALUES (?1, ?2)")
            .bind(key)
            .bind(value)
            .execute(&pool)
            .await?;
    }

    let mut tx = pool.begin().await?;
    for (z, x0, x1, y0, y1) in ranges {
        for x in x0..=x1 {
            for y in y0..=y1 {
                let tms_row = (1_u32 << z) - 1 - y;
                sqlx::query("INSERT INTO tiles VALUES (?1, ?2, ?3, ?4)")
                    .bind(i64::from(z))
                    .bind(i64::from(x))
                    .bind(i64::from(tms_row))
                    .bind(render_tile(z, x, y)?)
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }
    tx.commit().await?;
    pool.close().await;
    println!("{out} : {total} tuiles, zooms {min_zoom}..{max_zoom}");
    Ok(())
}
