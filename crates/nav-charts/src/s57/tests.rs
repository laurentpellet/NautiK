//! Tests du décodage S-57 sur des cellules fabriquées (voir `testdata`).

use super::catalogue::{attribute_code, object_class_code};
use super::iso8211::{Field, Record};
use super::testdata::{deg, CellBuilder};
use super::*;

fn close(a: Position, b: Position) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

fn by_class<'c>(cell: &'c S57Cell, acronym: &str) -> &'c S57Feature {
    let code = object_class_code(acronym).expect("classe connue");
    cell.features
        .iter()
        .find(|f| f.object_class == code)
        .expect("objet présent")
}

#[test]
fn decodes_sample_cell() {
    let cell = decode_cell(&CellBuilder::sample().build()).unwrap();
    assert_eq!(cell.dataset_name, "TEST0001.000");
    assert_eq!(cell.edition, "1");
    assert_eq!(cell.update_number, "0");
    assert_eq!(cell.issue_date, "20261102");
    assert_eq!(cell.intended_usage, 5);
    assert_eq!(cell.compilation_scale, 22_000);
    // 7 objets, l'obstacle sans nœud est écarté.
    assert_eq!(cell.features.len(), 6);
    assert_eq!(cell.skipped_features, 1);
    assert_eq!(cell.problems.len(), 1);
    assert!(cell.problems[0].contains("OBSTRN"), "{:?}", cell.problems);

    let depare = by_class(&cell, "DEPARE");
    assert_eq!(depare.acronym(), Some("DEPARE"));
    assert_eq!(
        depare.attribute(attribute_code("DRVAL1").unwrap()),
        Some("5")
    );
    assert_eq!(
        depare.attribute(attribute_code("DRVAL2").unwrap()),
        Some("10")
    );
    let S57Geometry::Area { rings } = &depare.geometry else {
        panic!("surface attendue");
    };
    assert_eq!(rings.len(), 2);
    let outer = &rings[0];
    assert_eq!(outer.len(), 5);
    assert!(close(outer[0], [-149.6, -17.5]));
    assert!(close(outer[1], [-149.6, -17.4]));
    assert!(close(outer[2], [-149.5, -17.4]));
    assert!(close(outer[3], [-149.5, -17.5]));
    assert_eq!(outer.first(), outer.last());
    assert!(close(rings[1][1], [-149.58, -17.46]));

    let S57Geometry::Line { lines } = &by_class(&cell, "COALNE").geometry else {
        panic!("ligne attendue");
    };
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].len(), 5);

    let S57Geometry::Soundings { soundings } = &by_class(&cell, "SOUNDG").geometry else {
        panic!("sondes attendues");
    };
    let depths: Vec<f64> = soundings.iter().map(|s| s.depth).collect();
    assert_eq!(depths, [5.5, 12.0, 0.3]);
    assert!(close(soundings[2].position, [-149.53, -17.43]));

    let wreck = by_class(&cell, "WRECKS");
    assert_eq!(
        wreck.geometry,
        S57Geometry::Point {
            position: [-149.52, -17.42]
        }
    );
    assert_eq!(wreck.attribute(179), Some("3.5"));
    assert_eq!(wreck.attribute(116), Some("Epave du port"));
    // Attribut national en UCS-2.
    assert_eq!(wreck.attribute(301), Some("Épave"));

    let collection = cell
        .features
        .iter()
        .find(|f| f.object_class == 400)
        .unwrap();
    assert_eq!(collection.geometry, S57Geometry::None);
}

#[test]
fn summary_counts_classes_soundings_and_bounds() {
    let summary = decode_cell(&CellBuilder::sample().build())
        .unwrap()
        .summary();
    assert_eq!(summary.features, 6);
    assert_eq!(summary.soundings, 3);
    assert_eq!(summary.classes.get("DEPARE"), Some(&1));
    assert_eq!(summary.classes.get("400"), Some(&1));
    assert_eq!(summary.classes.get("OBSTRN"), None);
    assert_eq!(summary.skipped_features, 1);
    let [w, s, e, n] = summary.bounds.unwrap();
    assert!(close([w, s], [-149.6, -17.5]));
    assert!(close([e, n], [-149.5, -17.4]));
    let json = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["datasetName"], "TEST0001.000");
    assert_eq!(json["skippedFeatures"], 1);
}

#[test]
fn geometry_serializes_with_type_tag() {
    let json = serde_json::to_value(S57Geometry::Point {
        position: [1.0, 2.0],
    })
    .unwrap();
    assert_eq!(json["type"], "point");
    assert_eq!(json["position"][1], 2.0);
}

#[test]
fn latin1_national_attributes() {
    let mut builder = CellBuilder::sample();
    builder.nall = 1;
    let cell = decode_cell(&builder.build()).unwrap();
    assert_eq!(by_class(&cell, "WRECKS").attribute(301), Some("Épave"));
}

#[test]
fn ucs2_field_ending_with_null_byte_is_not_an_attribute() {
    let data = [0x2D, 0x01, 0x41, 0x00, 0x1F, 0x00, 0x1E, 0x00];
    let natf = Field {
        tag: "NATF",
        data: &data,
    };
    let frid_data = [100, 1, 0, 0, 0, 255, 2, 144, 1, 1, 0, 1];
    let frid = Field {
        tag: "FRID",
        data: &frid_data,
    };
    let record = Record {
        fields: vec![frid, natf],
    };
    let feature = read_feature(&record, &frid, LEXICAL_UCS2).unwrap();
    assert_eq!(
        feature.attributes,
        [S57Attribute {
            code: 301,
            value: "A".into()
        }]
    );
}

#[test]
fn rejects_update_files() {
    let mut builder = CellBuilder::sample();
    builder.expp = 2;
    assert!(matches!(
        decode_cell(&builder.build()),
        Err(S57Error::Unsupported(_))
    ));
}

#[test]
fn rejects_other_datums_and_units() {
    let mut builder = CellBuilder::sample();
    builder.hdat = 75;
    let error = decode_cell(&builder.build()).unwrap_err();
    assert!(matches!(error, S57Error::Unsupported(_)), "{error}");
    assert!(error.to_string().contains("WGS 84"));

    let mut builder = CellBuilder::sample();
    builder.duni = 3;
    assert!(matches!(
        decode_cell(&builder.build()),
        Err(S57Error::Unsupported(_))
    ));
}

#[test]
fn rejects_null_scale_factors() {
    for (comf, somf) in [(0, 10), (10_000_000, 0)] {
        let mut builder = CellBuilder::sample();
        builder.comf = comf;
        builder.somf = somf;
        assert!(matches!(
            decode_cell(&builder.build()),
            Err(S57Error::Malformed(_))
        ));
    }
}

#[test]
fn rejects_ascii_implementation() {
    let mut builder = CellBuilder::sample();
    builder.sg2d_format = "(2R(10))";
    let error = decode_cell(&builder.build()).unwrap_err();
    assert!(matches!(error, S57Error::Unsupported(_)), "{error}");
}

#[test]
fn rejects_missing_parameters() {
    let mut builder = CellBuilder::sample();
    builder.with_dspm = false;
    assert!(matches!(
        decode_cell(&builder.build()),
        Err(S57Error::Malformed(_))
    ));
}

#[test]
fn detects_missing_records() {
    let mut builder = CellBuilder::sample();
    builder.declared_features = Some(8);
    let error = decode_cell(&builder.build()).unwrap_err();
    assert!(error.to_string().contains("incomplet"), "{error}");

    // Plus d'objets qu'annoncé : sans conséquence sur la lecture.
    builder.declared_features = Some(5);
    assert_eq!(decode_cell(&builder.build()).unwrap().features.len(), 6);
}

#[test]
fn update_instructions_in_base_cell_are_skipped() {
    let mut builder = CellBuilder::sample();
    builder.features[0].ruin = 2;
    builder.vectors[6].ruin = 3;
    let cell = decode_cell(&builder.build()).unwrap();
    // DEPARE écarté ; SOUNDG écarté car son nœud n'a pas été inséré.
    assert_eq!(cell.skipped_features, 3);
    assert!(cell.features.iter().all(|f| f.object_class != 42));
    assert!(cell.features.iter().all(|f| f.object_class != 129));
}

#[test]
fn rejects_out_of_range_coordinates() {
    let mut builder = CellBuilder::sample();
    builder.vectors[7].coords = vec![(deg(-149.5), deg(91.0))];
    let cell = decode_cell(&builder.build()).unwrap();
    assert!(cell.features.iter().all(|f| f.object_class != 159));
    assert_eq!(cell.skipped_features, 2);
}

#[test]
fn rejects_garbage() {
    assert!(decode_cell(b"").is_err());
    assert!(decode_cell(b"ceci n'est pas une cellule S-57").is_err());
    assert!(decode_cell(&[0u8; 4096]).is_err());
    // Un DR en tête au lieu du DDR.
    let bytes = CellBuilder::sample().build();
    let ddr_len: usize = std::str::from_utf8(&bytes[..5]).unwrap().parse().unwrap();
    assert!(decode_cell(&bytes[ddr_len..]).is_err());
}

#[test]
fn every_truncation_is_an_error() {
    let bytes = CellBuilder::sample().build();
    for len in 0..bytes.len() {
        assert!(
            decode_cell(&bytes[..len]).is_err(),
            "cellule tronquée à {len} octets acceptée"
        );
    }
}

#[test]
fn corrupted_bytes_never_panic() {
    let bytes = CellBuilder::sample().build();
    for i in 0..bytes.len() {
        for mask in [0xFF, 0x01, 0x80] {
            let mut corrupted = bytes.clone();
            corrupted[i] ^= mask;
            let _ = decode_cell(&corrupted);
        }
    }
    // Corruptions multiples pseudo-aléatoires (générateur déterministe).
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..2_000 {
        let mut corrupted = bytes.clone();
        for _ in 0..4 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let index = (state % corrupted.len() as u64) as usize;
            corrupted[index] = (state >> 32) as u8;
        }
        let _ = decode_cell(&corrupted);
    }
}

#[test]
fn reads_cell_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TEST0001.000");
    std::fs::write(&path, CellBuilder::sample().build()).unwrap();
    assert_eq!(decode_cell_file_blocking(&path).unwrap().features.len(), 6);
    assert!(matches!(
        decode_cell_file_blocking(&dir.path().join("absent.000")),
        Err(S57Error::Io(_))
    ));
    let big = dir.path().join("BIG.000");
    std::fs::File::create(&big)
        .unwrap()
        .set_len(MAX_CELL_BYTES + 1)
        .unwrap();
    assert!(matches!(
        decode_cell_file_blocking(&big),
        Err(S57Error::TooLarge { .. })
    ));
}

#[tokio::test]
async fn reads_cell_files_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TEST0001.000");
    std::fs::write(&path, CellBuilder::sample().build()).unwrap();
    let cell = decode_cell_file(path).await.unwrap();
    assert_eq!(cell.dataset_name, "TEST0001.000");
}

// --- Mises à jour ---------------------------------------------------------

use super::testdata::{Feature, Vector};

fn point_node(rcid: u32, lon: f64, lat: f64) -> Vector {
    Vector {
        rcnm: 110,
        rcid,
        ruin: 1,
        coords: vec![(deg(lon), deg(lat))],
        depths: Vec::new(),
        nodes: Vec::new(),
        version: 1,
        pointer_update: None,
        coord_update: None,
    }
}

fn change(rcid: u32, objl: u16, ruin: u8, version: u16) -> Feature {
    Feature {
        rcid,
        prim: 1,
        objl,
        ruin,
        attributes: Vec::new(),
        national: Vec::new(),
        pointers: Vec::new(),
        version,
        pointer_update: None,
    }
}

/// Mise à jour 1 : épave renommée (profondeur supprimée), obstacle
/// incohérent supprimé, feu inséré sur un nouveau nœud.
fn update_one() -> CellBuilder {
    let mut u = CellBuilder::update(1);
    u.vectors = vec![point_node(52, -149.51, -17.41)];
    let mut wreck = change(104, 159, 3, 2);
    wreck.attributes = vec![(116, "Nouveau nom".into()), (179, "\u{7f}".into())];
    let mut light = change(107, 75, 1, 1);
    light.pointers = vec![(110, 52, 255, 255, 255)];
    u.features = vec![wreck, change(105, 86, 2, 2), light];
    u
}

/// Mise à jour 2 : nœud 2 déplacé (SGCC), trou retiré de la zone de
/// profondeur (FSPC).
fn update_two() -> CellBuilder {
    let mut u = CellBuilder::update(2);
    let mut moved = point_node(2, -149.45, -17.35);
    moved.rcnm = 120;
    moved.ruin = 3;
    moved.version = 2;
    moved.coord_update = Some((3, 1, 1));
    u.vectors = vec![moved];
    let mut depare = change(100, 42, 3, 2);
    depare.prim = 3;
    depare.pointer_update = Some((2, 3, 1));
    u.features = vec![depare];
    u
}

#[test]
fn applies_updates_in_order() {
    let base = CellBuilder::sample().build();
    let (one, two) = (update_one().build(), update_two().build());
    let cell = decode_cell_with_updates(&base, &[&one, &two]).unwrap();
    assert_eq!(cell.updates_applied, 2);
    assert_eq!(cell.updates_pending, 0);
    assert_eq!(cell.update_error, None);
    assert_eq!(cell.cancelled, None);
    assert_eq!(cell.update_number, "2");
    assert_eq!(cell.update_issue_date.as_deref(), Some("20261102"));
    // L'obstacle incohérent a été supprimé : plus rien d'écarté.
    assert_eq!(cell.skipped_features, 0);

    let wreck = by_class(&cell, "WRECKS");
    assert_eq!(wreck.attribute(116), Some("Nouveau nom"));
    assert_eq!(wreck.attribute(179), None);
    assert_eq!(wreck.attribute(301), Some("Épave"));
    assert_eq!(
        by_class(&cell, "LIGHTS").geometry,
        S57Geometry::Point {
            position: [-149.51, -17.41]
        }
    );
    let S57Geometry::Area { rings } = &by_class(&cell, "DEPARE").geometry else {
        panic!("surface attendue");
    };
    assert_eq!(rings.len(), 1, "trou retiré");
    assert!(close(rings[0][2], [-149.45, -17.35]), "nœud déplacé");
}

#[test]
fn an_inconsistent_update_is_rejected_as_a_whole() {
    let base = CellBuilder::sample().build();
    let mut bad = update_one();
    // Version de l'épave inattendue (3 au lieu de 2).
    bad.features[0].version = 3;
    let cell = decode_cell_with_updates(&base, &[&bad.build(), &update_two().build()]).unwrap();
    assert_eq!(cell.updates_applied, 0);
    assert_eq!(cell.updates_pending, 2);
    assert_eq!(cell.update_number, "0");
    let error = cell.update_error.clone().unwrap();
    assert!(
        error.contains(".001") && error.contains("version"),
        "{error}"
    );
    // Rien de la mise à jour refusée : ni feu inséré, ni épave renommée.
    assert!(cell.features.iter().all(|f| f.object_class != 75));
    assert_eq!(
        by_class(&cell, "WRECKS").attribute(116),
        Some("Epave du port")
    );
}

type Edit = Box<dyn Fn(&mut CellBuilder)>;

#[test]
fn rejects_unknown_or_duplicate_targets() {
    let base = CellBuilder::sample().build();
    let cases: Vec<(&str, Edit)> = vec![
        (
            "introuvable",
            Box::new(|u| u.features = vec![change(999, 75, 3, 2)]),
        ),
        (
            "introuvable",
            Box::new(|u| u.features = vec![change(999, 75, 2, 2)]),
        ),
        (
            "déjà présent",
            Box::new(|u| u.features = vec![change(104, 159, 1, 1)]),
        ),
        (
            "déjà présent",
            Box::new(|u| u.vectors = vec![point_node(51, 0.0, 0.0)]),
        ),
        (
            "inconnue",
            Box::new(|u| u.features = vec![change(104, 159, 7, 2)]),
        ),
        (
            "FSPC",
            Box::new(|u| {
                let mut f = change(104, 159, 3, 2);
                f.pointers = vec![(110, 51, 255, 255, 255)];
                u.features = vec![f];
            }),
        ),
        (
            "incohérente",
            Box::new(|u| {
                let mut f = change(100, 42, 3, 2);
                f.pointer_update = Some((2, 4, 1));
                u.features = vec![f];
            }),
        ),
        (
            "SGCC",
            Box::new(|u| {
                let mut v = point_node(51, 0.0, 0.0);
                v.ruin = 3;
                v.version = 2;
                u.vectors = vec![v];
            }),
        ),
    ];
    for (expected, edit) in cases {
        let mut update = CellBuilder::update(1);
        edit(&mut update);
        let cell = decode_cell_with_updates(&base, &[&update.build()]).unwrap();
        let error = cell.update_error.unwrap_or_default();
        assert!(error.contains(expected), "« {expected} » attendu : {error}");
        assert_eq!(cell.updates_applied, 0);
    }
}

#[test]
fn missing_or_foreign_updates_stop_the_sequence() {
    let base = CellBuilder::sample().build();
    let three = CellBuilder::update(3).build();
    let cell = decode_cell_with_updates(&base, &[&update_one().build(), &three]).unwrap();
    assert_eq!(cell.updates_applied, 1);
    assert_eq!(cell.updates_pending, 1);
    assert!(cell.update_error.unwrap().contains("n° 2 manquante"));

    let mut other_edition = CellBuilder::update(1);
    other_edition.edition = "2";
    let cell = decode_cell_with_updates(&base, &[&other_edition.build()]).unwrap();
    assert!(cell.update_error.unwrap().contains("édition 2"));

    // Une cellule de base n'est pas une mise à jour.
    let cell = decode_cell_with_updates(&base, &[&base]).unwrap();
    assert!(cell
        .update_error
        .unwrap()
        .contains("pas un fichier de mise à jour"));
}

#[test]
fn updates_already_in_a_reissue_are_skipped() {
    let mut reissue = CellBuilder::sample();
    reissue.update = "1".into();
    let cell = decode_cell_with_updates(
        &reissue.build(),
        &[&update_one().build(), &update_two().build()],
    )
    .unwrap();
    // .001 déjà intégrée ; .002 appliquée.
    assert_eq!(cell.updates_applied, 1);
    assert_eq!(cell.update_number, "2");
    assert_eq!(cell.update_error, None);
    assert!(cell.features.iter().all(|f| f.object_class != 75));
}

#[test]
fn cancellation_is_reported() {
    let base = CellBuilder::sample().build();
    let mut cancel = CellBuilder::update(2);
    cancel.edition = "0";
    let cell = decode_cell_with_updates(
        &base,
        &[
            &update_one().build(),
            &cancel.build(),
            &CellBuilder::update(3).build(),
        ],
    )
    .unwrap();
    assert_eq!(cell.cancelled.as_deref(), Some("20261102"));
    assert_eq!(cell.updates_applied, 1);
    assert_eq!(cell.updates_pending, 1);
    assert_eq!(cell.update_error, None);
    assert_eq!(cell.summary().cancelled.as_deref(), Some("20261102"));
}

#[test]
fn truncated_or_corrupted_updates_never_apply_partially() {
    let base = CellBuilder::sample().build();
    let update = update_one().build();
    for len in 0..update.len() {
        let cell = decode_cell_with_updates(&base, &[&update[..len]]).unwrap();
        assert_eq!(
            cell.updates_applied, 0,
            "mise à jour tronquée à {len} octets appliquée"
        );
        assert!(cell.update_error.is_some());
        assert_eq!(
            by_class(&cell, "WRECKS").attribute(116),
            Some("Epave du port")
        );
    }
    for i in 0..update.len() {
        let mut corrupted = update.clone();
        corrupted[i] ^= 0xFF;
        let _ = decode_cell_with_updates(&base, &[&corrupted]).unwrap();
    }
}

#[test]
fn finds_update_files_next_to_the_cell() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TEST0001.000");
    std::fs::write(&path, CellBuilder::sample().build()).unwrap();
    std::fs::write(dir.path().join("test0001.002"), update_two().build()).unwrap();
    std::fs::write(dir.path().join("TEST0001.001"), update_one().build()).unwrap();
    // Ni d'une autre cellule, ni hors numérotation.
    std::fs::write(dir.path().join("OTHER001.001"), b"x").unwrap();
    std::fs::write(dir.path().join("TEST0001.TXT"), b"x").unwrap();
    std::fs::write(dir.path().join("TEST0001.01"), b"x").unwrap();
    let cell = decode_cell_file_blocking(&path).unwrap();
    assert_eq!(cell.updates_applied, 2);
    assert_eq!(cell.update_error, None);

    // Mise à jour manquante dans la séquence.
    std::fs::remove_file(dir.path().join("TEST0001.001")).unwrap();
    let cell = decode_cell_file_blocking(&path).unwrap();
    assert_eq!(cell.updates_applied, 0);
    assert_eq!(cell.updates_pending, 1);
    assert!(cell.update_error.unwrap().contains("n° 1 manquante"));
}
