use super::testdata::{dictionary, encode, feature, CellBuilder, ATTRIBUTES, OBJECTS};
use super::*;
use crate::s57::display::{DisplayCategory, MapGeometry};

fn sample() -> Cm93Cell {
    decode_cell(&CellBuilder::sample().build(), &dictionary()).unwrap()
}

#[test]
fn dictionaries() {
    let dict = dictionary();
    assert_eq!(dict.class_name(44), Some("DEPARE"));
    assert_eq!(dict.class_name(250), None);
    assert_eq!(dict.attribute(116), Some(("VALSOU", AttrType::Word10)));
    assert_eq!(dict.attribute(99), Some(("SCAMIN", AttrType::Unknown)));
    for (objects, attributes) in [
        ("", ATTRIBUTES),
        (OBJECTS, ""),
        ("DEPARE|x|A", ATTRIBUTES),
        ("DEPARE|300|A", ATTRIBUTES),
        ("|44|A", ATTRIBUTES),
        ("TROPLONGNOM|44|A", ATTRIBUTES),
    ] {
        assert!(
            Cm93Dictionary::parse(objects, attributes).is_err(),
            "{objects:?}"
        );
    }
}

#[test]
fn loads_dictionaries_and_detects_a_base() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!is_cm93_root(dir.path()));
    assert!(Cm93Dictionary::load(dir.path()).is_err());
    std::fs::write(dir.path().join("cm93obj.dic"), OBJECTS).unwrap();
    std::fs::write(dir.path().join("CM93ATTR.DIC"), ATTRIBUTES).unwrap();
    assert!(is_cm93_root(dir.path()));
    let dict = Cm93Dictionary::load(dir.path()).unwrap();
    assert_eq!(dict.class_name(18), Some("BOYLAT"));
}

#[test]
fn decodes_sample_cell() {
    let cell = sample();
    assert_eq!(cell.features.len(), 6);
    assert_eq!(cell.skipped_features, 2);
    assert_eq!(cell.declared_offset_m, 40.0);
    assert_eq!(cell.data_date.as_deref(), Some("20140612"));
    let classes: Vec<_> = cell.features.iter().map(|f| f.class.as_str()).collect();
    assert_eq!(
        classes,
        ["BOYLAT", "COALNE", "DEPARE", "SOUNDG", "WRECKS", "_m_sor"]
    );

    let buoy = &cell.features[0];
    assert_eq!(buoy.attribute("COLMAR"), Some(&AttrValue::Int(1)));
    assert_eq!(
        buoy.attribute("OBJNAM"),
        Some(&AttrValue::Text("PA 3".into()))
    );
    let Cm93Geometry::Point([lon, lat]) = buoy.geometry else {
        panic!("point attendu");
    };
    // Milieu de la grille : milieu de la cellule (longitude ramenée à -180…180).
    assert!((lon - (-149.5)).abs() < 1e-3, "{lon}");
    assert!((lat - (-17.5)).abs() < 1e-3, "{lat}");

    let Cm93Geometry::Line(lines) = &cell.features[1].geometry else {
        panic!("ligne attendue");
    };
    assert_eq!(lines.len(), 1, "segments chaînés");
    assert_eq!(lines[0].len(), 3);

    let depth = &cell.features[2];
    assert_eq!(depth.attribute("DRVAL1"), Some(&AttrValue::Real(5.0)));
    let Cm93Geometry::Area(rings) = &depth.geometry else {
        panic!("surface attendue");
    };
    assert_eq!(rings.len(), 2);
    assert!(rings.iter().all(|r| r.len() == 5 && r.first() == r.last()));

    let Cm93Geometry::Soundings(soundings) = &cell.features[3].geometry else {
        panic!("sondes attendues");
    };
    assert_eq!(soundings.len(), 2);
    assert_eq!(soundings[0].1, 5.5);
    assert_eq!(soundings[1].1, 34.0, "z ≥ 12000 : mètres entiers");

    assert_eq!(
        cell.features[4].attribute("VALSOU"),
        Some(&AttrValue::Real(3.5))
    );
}

#[test]
fn corner_positions_match_the_bounds() {
    let mut builder = CellBuilder::sample();
    builder.points2d = vec![(0, 0), (65535, 65535)];
    let cell = decode_cell(&builder.build(), &dictionary()).unwrap();
    let Cm93Geometry::Point([lon, lat]) = cell.features[0].geometry else {
        panic!("point attendu");
    };
    assert!((lon - (210.333_333 - 360.0)).abs() < 1e-6);
    assert!((lat - (-17.666_667)).abs() < 1e-6);
    let Cm93Geometry::Point([lon, lat]) = cell.features[4].geometry else {
        panic!("point attendu");
    };
    assert!((lon - (210.666_667 - 360.0)).abs() < 1e-6);
    assert!((lat - (-17.333_333)).abs() < 1e-6);
}

#[test]
fn rejects_inconsistent_cells() {
    let dict = dictionary();
    let plain = CellBuilder::sample().plain();

    // Longueur annoncée fausse.
    let mut longer = plain.clone();
    longer.push(0);
    assert!(matches!(
        decode_cell(&encode(&longer), &dict),
        Err(Cm93Error::Corrupt(_))
    ));

    // Fichier tronqué, prologue ajusté : enregistrements manquants.
    let mut cut = plain[..plain.len() - 3].to_vec();
    let total = cut.len() - 138;
    cut[2..6].copy_from_slice(&(total as i32).to_le_bytes());
    cut[6..10].copy_from_slice(&0i32.to_le_bytes());
    assert!(decode_cell(&encode(&cut), &dict).is_err());

    // Emprise invalide.
    let mut builder = CellBuilder::sample();
    builder.bounds = [210.0, -95.0, 210.5, -17.0];
    assert!(decode_cell(&builder.build(), &dict).is_err());
    let mut builder = CellBuilder::sample();
    builder.bounds = [210.0, -17.0, 210.5, -18.0];
    assert!(decode_cell(&builder.build(), &dict).is_err());

    // Nombre d'objets impossible.
    let mut huge = plain.clone();
    huge[10 + 64 + 30..10 + 64 + 32].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(matches!(
        decode_cell(&encode(&huge), &dict),
        Err(Cm93Error::Truncated(_))
    ));

    // Trop court pour le prologue.
    assert!(decode_cell(&encode(&plain[..5]), &dict).is_err());
    assert!(decode_cell(&[], &dict).is_err());
}

#[test]
fn record_length_overrunning_the_file_is_refused() {
    let mut builder = CellBuilder::sample();
    let mut bad = feature(18, 0x01, &[0], &[]);
    bad[2..4].copy_from_slice(&1000u16.to_le_bytes());
    builder.features = vec![bad];
    assert!(decode_cell(&builder.build(), &dictionary()).is_err());
    let mut tiny = feature(18, 0x01, &[0], &[]);
    tiny[2..4].copy_from_slice(&2u16.to_le_bytes());
    builder.features = vec![tiny];
    assert!(decode_cell(&builder.build(), &dictionary()).is_err());
}

#[test]
fn malformed_records_are_skipped_not_invented() {
    let mut builder = CellBuilder::sample();
    // Point hors table, sondes hors table, contour ouvert, attribut
    // tronqué (texte sans fin) : objets écartés.
    let open_ring = feature(44, 0x04, &[1, 0], &[]);
    let mut unterminated = feature(18, 0x81, &[0], &[(80, b"PA".to_vec())]);
    unterminated.truncate(unterminated.len());
    builder.features = vec![
        feature(18, 0x01, &[7], &[]),
        feature(147, 0x08, &[3], &[]),
        open_ring,
        unterminated,
    ];
    let cell = decode_cell(&builder.build(), &dictionary()).unwrap();
    assert!(cell.features.is_empty());
    assert_eq!(cell.skipped_features, 4);
}

#[test]
fn attributes_of_every_type_and_unknown_sizes() {
    let mut builder = CellBuilder::sample();
    builder.features = vec![
        feature(
            18,
            0x81,
            &[0],
            &[
                (59, vec![2, 3, 1]),
                (181, [&[1u8, 2, 3][..], b"note\0"].concat()),
                (182, (-7i32).to_le_bytes().to_vec()),
                // Taille inconnue : la lecture s'arrête, l'objet est gardé.
                (99, vec![1, 2]),
                (80, super::testdata::text("jamais lu")),
            ],
        ),
        // Attribut absent du dictionnaire : idem.
        feature(
            18,
            0x81,
            &[0],
            &[(77, vec![1]), (80, super::testdata::text("x"))],
        ),
        // Objets liés (0x10, 0x20) sautés.
        {
            let mut f = feature(18, 0x01, &[0], &[]);
            f.extend_from_slice(&[2, 0, 0, 1, 0]);
            f[1] |= 0x10;
            let len = f.len() as u16;
            f[2..4].copy_from_slice(&len.to_le_bytes());
            f
        },
    ];
    let cell = decode_cell(&builder.build(), &dictionary()).unwrap();
    assert_eq!(cell.features.len(), 3);
    let a = &cell.features[0];
    assert_eq!(a.attribute("COLOUR"), Some(&AttrValue::List(vec![3, 1])));
    assert_eq!(a.attribute("_cmplx"), Some(&AttrValue::Text("note".into())));
    assert_eq!(a.attribute("_long1"), Some(&AttrValue::Int(-7)));
    assert_eq!(a.attributes.len(), 3);
    assert!(cell.features[1].attributes.is_empty());
}

#[test]
fn every_truncation_fails_cleanly() {
    let dict = dictionary();
    let plain = CellBuilder::sample().plain();
    for len in 0..plain.len() {
        let mut cut = plain[..len].to_vec();
        if len >= 10 {
            let rest = (len as i32) - 138;
            cut[2..6].copy_from_slice(&rest.to_le_bytes());
            cut[6..10].copy_from_slice(&0i32.to_le_bytes());
        }
        // Ni panique ni objet inventé : erreur, ou cellule partielle
        // dont chaque objet est complet.
        if let Ok(cell) = decode_cell(&encode(&cut), &dict) {
            assert!(cell.features.len() <= 6);
        }
    }
    // Octets corrompus un par un.
    for i in 10..plain.len() {
        let mut bad = plain.clone();
        bad[i] ^= 0xA5;
        let _ = decode_cell(&encode(&bad), &dict);
    }
}

#[test]
fn map_features_reuse_the_s57_categories() {
    let cell = sample();
    let features = map_features(&cell, "02170631.F", Cm93Scale::F.rank());
    let cats: Vec<_> = features.iter().map(|f| f.properties.cat).collect();
    assert_eq!(
        cats,
        [
            DisplayCategory::Buoy,
            DisplayCategory::Coast,
            DisplayCategory::Depth,
            DisplayCategory::Sounding,
            DisplayCategory::Sounding,
            DisplayCategory::Danger,
        ]
    );
    let buoy = &features[0].properties;
    assert_eq!(buoy.colour, Some(4), "COLMAR 1 : vert");
    assert_eq!(buoy.name.as_deref(), Some("PA 3"));
    assert_eq!(buoy.usage, 7);
    assert_eq!(buoy.cell, "02170631.F");
    assert_eq!(features[2].properties.drval1, Some(5.0));
    assert!(
        matches!(&features[2].geometry, MapGeometry::Polygon { coordinates } if coordinates.len() == 2)
    );
    assert_eq!(features[3].properties.label.as_deref(), Some("5.5"));
    assert_eq!(features[4].properties.label.as_deref(), Some("34"));
    assert_eq!(features[5].properties.depth, Some(3.5));
    let json = serde_json::to_value(&features).unwrap();
    assert_eq!(json[2]["properties"]["class"], "DEPARE");
}

#[test]
fn intertidal_areas_are_drawn_as_shallow_depth_areas() {
    let mut builder = CellBuilder::sample();
    builder.features = vec![feature(190, 0x04, &[1, 2], &[])];
    let cell = decode_cell(&builder.build(), &dictionary()).unwrap();
    let features = map_features(&cell, "x", 1);
    assert_eq!(features.len(), 1);
    assert_eq!(features[0].properties.cat, DisplayCategory::Depth);
    assert_eq!(features[0].properties.drval1, None);
}

#[test]
fn scales() {
    assert_eq!(Cm93Scale::from_letter('f'), Some(Cm93Scale::F));
    assert_eq!(Cm93Scale::from_letter('H'), None);
    assert!(Cm93Scale::Z < Cm93Scale::G);
    assert_eq!(Cm93Scale::G.denominator(), 7_500);
    assert_eq!(Cm93Scale::Z.rank(), 1);
}
