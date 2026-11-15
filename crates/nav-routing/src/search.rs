//! Recherche : lecture de coordonnées saisies à la main et comparaison de
//! noms insensible à la casse et aux accents.
//!
//! Formats de coordonnées acceptés (latitude puis longitude, sauf si les
//! lettres d'hémisphère indiquent l'inverse) :
//! - degrés décimaux signés : `-17.5, -149.6`, `-17,5 ; -149,6` ;
//! - avec hémisphères : `17.5S 149.6W`, `S 17°32,1' O 149°34,2'` ;
//! - degrés, minutes (et secondes) : `17°32.100'S 149°34.200'W`,
//!   `17 32 06 S 149 34 12 W`.

use nav_core::GeoPosition;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Token {
    Number(f64),
    Minus,
    Hemisphere(char),
}

fn tokens(text: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if c.is_ascii_digit() {
            let mut s = String::new();
            let mut decimal = false;
            while let Some(&d) = chars.get(i) {
                if d.is_ascii_digit() {
                    s.push(d);
                } else if (d == '.' || d == ',')
                    && !decimal
                    && chars.get(i + 1).is_some_and(char::is_ascii_digit)
                {
                    // Une virgule suivie d'une espace sépare deux nombres ;
                    // collée à des chiffres, c'est une décimale.
                    decimal = true;
                    s.push('.');
                } else {
                    break;
                }
                i += 1;
            }
            out.push(Token::Number(s.parse().ok()?));
            continue;
        }
        match c.to_ascii_uppercase() {
            '-' | '\u{2212}' => out.push(Token::Minus),
            '+' => {}
            h @ ('N' | 'S' | 'E' | 'W' | 'O') => out.push(Token::Hemisphere(h)),
            _ if c.is_alphabetic() => return None,
            _ => {}
        }
        i += 1;
    }
    Some(out)
}

/// Degrés à partir de 1 à 3 composantes (degrés, minutes, secondes).
fn angle(parts: &[f64]) -> Option<f64> {
    let (deg, rest) = parts.split_first()?;
    if rest.len() > 2 || rest.iter().any(|v| *v >= 60.0) {
        return None;
    }
    // Seule la dernière composante peut être décimale.
    if parts[..parts.len() - 1].iter().any(|v| v.fract() != 0.0) {
        return None;
    }
    let min = rest.first().copied().unwrap_or(0.0);
    let sec = rest.get(1).copied().unwrap_or(0.0);
    Some(deg + min / 60.0 + sec / 3600.0)
}

fn signed_groups(tokens: &[Token]) -> Option<(f64, f64)> {
    let mut groups: Vec<(bool, Vec<f64>)> = Vec::new();
    let mut negative = false;
    let mut count = 0;
    for t in tokens {
        match t {
            Token::Minus => negative = true,
            Token::Number(v) => {
                count += 1;
                if negative || groups.is_empty() {
                    groups.push((negative, Vec::new()));
                    negative = false;
                }
                groups.last_mut()?.1.push(*v);
            }
            Token::Hemisphere(_) => return None,
        }
    }
    // Sans signe sur la longitude : partage en deux moitiés égales.
    if groups.len() == 1 && count % 2 == 0 {
        let (neg, values) = groups.pop()?;
        let (a, b) = values.split_at(count / 2);
        groups = vec![(neg, a.to_vec()), (false, b.to_vec())];
    }
    if groups.len() != 2 {
        return None;
    }
    let value = |(neg, v): &(bool, Vec<f64>)| angle(v).map(|a| if *neg { -a } else { a });
    Some((value(&groups[0])?, value(&groups[1])?))
}

fn hemisphere_groups(tokens: &[Token]) -> Option<(f64, f64)> {
    if tokens.contains(&Token::Minus) {
        return None;
    }
    let prefix = matches!(tokens.first(), Some(Token::Hemisphere(_)));
    let mut groups: Vec<(char, Vec<f64>)> = Vec::new();
    let mut pending: Vec<f64> = Vec::new();
    for t in tokens {
        match t {
            Token::Number(v) => {
                if prefix {
                    groups.last_mut()?.1.push(*v);
                } else {
                    pending.push(*v);
                }
            }
            Token::Hemisphere(h) => {
                groups.push((*h, std::mem::take(&mut pending)));
            }
            Token::Minus => return None,
        }
    }
    if !pending.is_empty() || groups.len() != 2 {
        return None;
    }
    let mut lat = None;
    let mut lon = None;
    for (h, values) in &groups {
        let a = angle(values)?;
        match h {
            'N' => lat = Some(a),
            'S' => lat = Some(-a),
            'E' => lon = Some(a),
            _ => lon = Some(-a),
        }
    }
    Some((lat?, lon?))
}

/// Coordonnées lues dans un texte saisi ; `None` si ce n'en est pas.
#[must_use]
pub fn parse_coordinates(text: &str) -> Option<GeoPosition> {
    if text.len() > 80 {
        return None;
    }
    // « ; » sépare latitude et longitude : la virgule est alors décimale.
    let normalized = text.replace(';', " ");
    let tokens = tokens(&normalized)?;
    let has_hemisphere = tokens.iter().any(|t| matches!(t, Token::Hemisphere(_)));
    let (lat, lon) = if has_hemisphere {
        hemisphere_groups(&tokens)?
    } else {
        signed_groups(&tokens)?
    };
    GeoPosition::new(lat, lon).ok()
}

/// Texte replié pour la comparaison : minuscules, sans accents.
#[must_use]
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => out.push('a'),
            'ç' => out.push('c'),
            'è' | 'é' | 'ê' | 'ë' | 'ē' => out.push('e'),
            'ì' | 'í' | 'î' | 'ï' | 'ī' => out.push('i'),
            'ñ' => out.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' => out.push('o'),
            'ù' | 'ú' | 'û' | 'ü' | 'ū' => out.push('u'),
            'ý' | 'ÿ' => out.push('y'),
            'œ' => out.push_str("oe"),
            'æ' => out.push_str("ae"),
            '’' | 'ʻ' | '‘' | '`' => out.push('\''),
            c => out.push(c),
        }
    }
    out
}

/// Pertinence d'un nom pour une requête déjà repliée par [`fold`] :
/// 0 identique, 1 début du nom, 2 début d'un mot, 3 ailleurs ; `None` si
/// le nom ne contient pas la requête.
#[must_use]
pub fn match_rank(name: &str, folded_query: &str) -> Option<u8> {
    if folded_query.is_empty() {
        return None;
    }
    let name = fold(name);
    if name == folded_query {
        Some(0)
    } else if name.starts_with(folded_query) {
        Some(1)
    } else if name
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w.starts_with(folded_query))
    {
        Some(2)
    } else if name.contains(folded_query) {
        Some(3)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ll(text: &str) -> (f64, f64) {
        let p = parse_coordinates(text).unwrap_or_else(|| panic!("« {text} » non lu"));
        (p.latitude_deg(), p.longitude_deg())
    }

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    #[test]
    fn reads_decimal_degrees() {
        assert!(close(ll("-17.5, -149.6"), (-17.5, -149.6)));
        assert!(close(ll("-17.5 -149.6"), (-17.5, -149.6)));
        assert!(close(ll("−17,5 ; −149,6"), (-17.5, -149.6)));
        assert!(close(ll("17.5,149.6"), (17.5, 149.6)));
        assert!(close(ll("+48.1 -4.5"), (48.1, -4.5)));
    }

    #[test]
    fn reads_hemispheres_and_minutes() {
        let expected = (-(17.0 + 32.1 / 60.0), -(149.0 + 34.2 / 60.0));
        assert!(close(ll("17°32.100'S 149°34.200'W"), expected));
        assert!(close(ll("17°32,1' S, 149°34,2' O"), expected));
        assert!(close(ll("S 17 32.1 W 149 34.2"), expected));
        // Longitude d'abord : les lettres font foi.
        assert!(close(ll("149°34.2'W 17°32.1'S"), expected));
        assert!(close(ll("17 32 06 N 149 34 12 E"), (17.535, 149.57)));
        assert!(close(ll("-17 32.1 -149 34.2"), expected));
        assert!(close(ll("17 32.1 149 34.2"), (-expected.0, -expected.1)));
    }

    #[test]
    fn rejects_text_and_invalid_values() {
        for text in [
            "Moorea",
            "",
            "17.5",
            "95 10",
            "10 190",
            "17 75 S 149 W",
            "17.5 32 S 149 W",
            "17 S 149 S",
            "17 S -149 W",
            "1 2 3",
            "Passe 17 149",
        ] {
            assert_eq!(parse_coordinates(text), None, "« {text} »");
        }
        assert_eq!(parse_coordinates(&"1".repeat(200)), None);
    }

    #[test]
    fn names_match_without_case_or_accents() {
        let q = fold("TEAVA");
        assert_eq!(match_rank("Passe Teavapiti", &q), Some(2));
        assert_eq!(match_rank("Téava", &q), Some(0));
        assert_eq!(match_rank("teavaroa", &q), Some(1));
        assert_eq!(match_rank("Haapiti", &fold("piti")), Some(3));
        assert_eq!(match_rank("Moorea", &q), None);
        assert_eq!(match_rank("Moorea", ""), None);
        assert_eq!(fold("Œil ʻŌtea"), "oeil 'otea");
    }
}
