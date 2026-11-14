//! Résumé de cellules CM93 réelles (essais).
//!
//! `cargo run -p nav-charts --example cm93_summary -- RACINE [CHEMIN…]`
//! - sans chemin : décode toutes les cellules de la base et compte les
//!   échecs ;
//! - avec chemins relatifs (`02100600/F/02170631.F`) : résumé de chacune ;
//!   `NAUTIK_CM93_GEOJSON=fichier` écrit les entités de la dernière.

use std::collections::BTreeMap;
use std::path::Path;

use nav_charts::cm93::{
    decode_cell_file_blocking, map_features, scan_cm93_cells, Cm93Dictionary, Cm93Geometry,
};

fn signed_area(ring: &[[f64; 2]]) -> f64 {
    ring.windows(2)
        .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
        .sum::<f64>()
        / 2.0
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(root) = args.first() else {
        anyhow::bail!("usage : cm93_summary RACINE [CHEMIN…]");
    };
    let root = Path::new(root);
    let dict = Cm93Dictionary::load(root)?;
    let started = std::time::Instant::now();
    if args.len() == 1 {
        let index = scan_cm93_cells(root)?;
        println!(
            "{} cellules ({} ignorés, tronqué : {}) en {} ms",
            index.cells.len(),
            index.ignored,
            index.truncated,
            started.elapsed().as_millis()
        );
        let mut errors: BTreeMap<String, usize> = BTreeMap::new();
        let (mut features, mut skipped, mut offsets) = (0usize, 0usize, 0usize);
        for cell in &index.cells {
            match decode_cell_file_blocking(&root.join(&cell.path), &dict) {
                Ok(c) => {
                    features += c.features.len();
                    skipped += c.skipped_features;
                    if c.declared_offset_m > 1.0 {
                        offsets += 1;
                    }
                }
                Err(e) => {
                    let key: String = e.to_string().chars().take(60).collect();
                    if !errors.contains_key(&key) {
                        println!("{} : {e}", cell.path);
                    }
                    *errors.entry(key).or_default() += 1;
                }
            }
        }
        println!(
            "{features} objets, {skipped} écartés, {offsets} cellules avec décalage déclaré, {} ms",
            started.elapsed().as_millis()
        );
        println!("erreurs : {errors:#?}");
        return Ok(());
    }
    for relative in &args[1..] {
        let cell = decode_cell_file_blocking(&root.join(relative), &dict)?;
        let mut classes: BTreeMap<&str, usize> = BTreeMap::new();
        let (mut cw, mut ccw, mut multi) = (0, 0, 0);
        for f in &cell.features {
            *classes.entry(f.class.as_str()).or_default() += 1;
            if let Cm93Geometry::Area(rings) = &f.geometry {
                if rings.len() > 1 {
                    multi += 1;
                }
                for r in rings {
                    if signed_area(r) < 0.0 {
                        cw += 1;
                    } else {
                        ccw += 1;
                    }
                }
            }
        }
        let name = relative.rsplit('/').next().unwrap_or(relative);
        let scale = name
            .rsplit('.')
            .next()
            .and_then(|e| e.chars().next())
            .and_then(nav_charts::cm93::Cm93Scale::from_letter)
            .map_or(1, |s| s.rank());
        let features = map_features(&cell, name, scale);
        let json = serde_json::to_string(&features)?;
        println!(
            "{relative} : emprise {:?}, {} objets ({} écartés), décalage {} m, date {:?}",
            cell.bounds,
            cell.features.len(),
            cell.skipped_features,
            cell.declared_offset_m,
            cell.data_date
        );
        println!(
            "  contours horaires {cw}, antihoraires {ccw}, surfaces à plusieurs contours {multi}"
        );
        println!(
            "  affichage : {} entités, {} Kio",
            features.len(),
            json.len() / 1024
        );
        println!("  classes : {classes:?}");
        if let Ok(out) = std::env::var("NAUTIK_CM93_GEOJSON") {
            std::fs::write(&out, &json)?;
        }
    }
    Ok(())
}
