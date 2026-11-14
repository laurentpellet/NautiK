//! Extraits du catalogue d'objets S-57 (IHO S-57 édition 3.1,
//! appendice A) : codes des classes d'objets et des attributs utiles à
//! l'affichage simplifié. Les autres codes restent disponibles en valeur
//! numérique.

/// Classes d'objets (code OBJL → acronyme).
const OBJECT_CLASSES: &[(u16, &str)] = &[
    (4, "ACHARE"),
    (5, "BCNCAR"),
    (6, "BCNISD"),
    (7, "BCNLAT"),
    (8, "BCNSAW"),
    (9, "BCNSPP"),
    (11, "BRIDGE"),
    (14, "BOYCAR"),
    (15, "BOYINB"),
    (16, "BOYISD"),
    (17, "BOYLAT"),
    (18, "BOYSAW"),
    (19, "BOYSPP"),
    (30, "COALNE"),
    (42, "DEPARE"),
    (43, "DEPCNT"),
    (46, "DRGARE"),
    (58, "FOGSIG"),
    (69, "LAKARE"),
    (71, "LNDARE"),
    (74, "LNDMRK"),
    (75, "LIGHTS"),
    (86, "OBSTRN"),
    (112, "RESARE"),
    (114, "RIVERS"),
    (121, "SBDARE"),
    (122, "SLCONS"),
    (129, "SOUNDG"),
    (144, "TOPMAR"),
    (153, "UWTROC"),
    (159, "WRECKS"),
    (302, "M_COVR"),
    (308, "M_QUAL"),
];

/// Attributs (code ATTL → acronyme).
const ATTRIBUTES: &[(u16, &str)] = &[
    (36, "CATLAM"),
    (75, "COLOUR"),
    (87, "DRVAL1"),
    (88, "DRVAL2"),
    (93, "EXPSOU"),
    (116, "OBJNAM"),
    (133, "SCAMIN"),
    (174, "VALDCO"),
    (179, "VALSOU"),
    (187, "WATLEV"),
    (301, "NOBJNM"),
];

/// Acronyme d'une classe d'objets connue.
pub fn object_class_acronym(code: u16) -> Option<&'static str> {
    OBJECT_CLASSES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, a)| *a)
}

/// Code d'une classe d'objets connue.
pub fn object_class_code(acronym: &str) -> Option<u16> {
    OBJECT_CLASSES
        .iter()
        .find(|(_, a)| *a == acronym)
        .map(|(c, _)| *c)
}

/// Acronyme d'un attribut connu.
pub fn attribute_acronym(code: u16) -> Option<&'static str> {
    ATTRIBUTES.iter().find(|(c, _)| *c == code).map(|(_, a)| *a)
}

/// Code d'un attribut connu.
pub fn attribute_code(acronym: &str) -> Option<u16> {
    ATTRIBUTES
        .iter()
        .find(|(_, a)| *a == acronym)
        .map(|(c, _)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_acronyms_round_trip() {
        for (code, acronym) in OBJECT_CLASSES {
            assert_eq!(object_class_acronym(*code), Some(*acronym));
            assert_eq!(object_class_code(acronym), Some(*code));
        }
        for (code, acronym) in ATTRIBUTES {
            assert_eq!(attribute_acronym(*code), Some(*acronym));
            assert_eq!(attribute_code(acronym), Some(*code));
        }
        assert_eq!(object_class_acronym(42), Some("DEPARE"));
        assert_eq!(object_class_acronym(9999), None);
        assert_eq!(attribute_code("VALSOU"), Some(179));
    }
}
