//! Résume des cellules ENC S-57 réelles (outil de vérification).
//!
//! ```text
//! cargo run -p nav-charts --example enc_summary -- CHEMIN/US5MA1AM.000 [...]
//! ```

use std::path::Path;

use nav_charts::s57::decode_cell_file_blocking;

fn main() -> anyhow::Result<()> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        anyhow::bail!("usage : enc_summary CELLULE.000 [...]");
    }
    for path in paths {
        let started = std::time::Instant::now();
        match decode_cell_file_blocking(Path::new(&path)) {
            Ok(cell) => {
                // NAUTIK_ENC_CLASS=42 : affiche les premiers objets de la classe.
                if let Some(class) = std::env::var("NAUTIK_ENC_CLASS")
                    .ok()
                    .and_then(|c| c.parse::<u16>().ok())
                {
                    for feature in cell
                        .features
                        .iter()
                        .filter(|f| f.object_class == class)
                        .take(5)
                    {
                        println!("{:?}", feature.attributes);
                    }
                }
                let display_started = std::time::Instant::now();
                let features = nav_charts::s57::display::map_features(&cell);
                let json = serde_json::to_string(&features)?;
                println!(
                    "affichage : {} entités, {} Kio de GeoJSON ({} ms)",
                    features.len(),
                    json.len() / 1024,
                    display_started.elapsed().as_millis()
                );
                // Entités d'affichage écrites dans un fichier (essais de rendu).
                if let Ok(out) = std::env::var("NAUTIK_ENC_GEOJSON") {
                    std::fs::write(&out, &json)?;
                    println!("entités écrites dans {out}");
                }
                let summary = cell.summary();
                println!(
                    "{path} ({} ms)\n{}",
                    started.elapsed().as_millis(),
                    serde_json::to_string_pretty(&summary)?
                );
            }
            Err(error) => println!("{path} : {error}"),
        }
    }
    Ok(())
}
