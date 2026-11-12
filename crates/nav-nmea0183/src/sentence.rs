//! Découpage d'une phrase NMEA 0183 : délimiteur, adresse, champs et
//! somme de contrôle.
//!
//! Toute phrase sans somme de contrôle, ou dont la somme est fausse, est
//! rejetée (SECURITY.md §5). Un bloc d'étiquettes IEC 61162-450
//! (`\…\`) en tête de ligne est ignoré.

use thiserror::Error;

/// Longueur maximale d'une phrase (sans fin de ligne ni bloc
/// d'étiquettes). La norme fixe 82 caractères ; une marge tolère les
/// équipements non conformes.
pub const MAX_SENTENCE_LEN: usize = 100;

/// Phrase rejetée.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SentenceError {
    /// Ligne vide.
    #[error("ligne vide")]
    Empty,
    /// Phrase trop longue.
    #[error("phrase trop longue ({0} caractères)")]
    TooLong(usize),
    /// Caractère non ASCII imprimable.
    #[error("caractère invalide")]
    InvalidCharacter,
    /// Ni `$` ni `!` en tête.
    #[error("début de phrase absent")]
    BadStart,
    /// Bloc d'étiquettes non fermé.
    #[error("bloc d'étiquettes non fermé")]
    UnterminatedTagBlock,
    /// Somme de contrôle absente.
    #[error("somme de contrôle absente")]
    MissingChecksum,
    /// Somme de contrôle mal formée.
    #[error("somme de contrôle mal formée")]
    BadChecksumFormat,
    /// Somme de contrôle fausse.
    #[error("somme de contrôle fausse (calculée {computed:02X}, reçue {received:02X})")]
    ChecksumMismatch {
        /// Somme calculée.
        computed: u8,
        /// Somme reçue.
        received: u8,
    },
    /// Adresse (émetteur + type) invalide.
    #[error("adresse invalide : {0}")]
    BadAddress(String),
}

/// Phrase validée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sentence<'a> {
    /// Identifiant d'émetteur (`GP`, `GN`, `II`…), `P` pour une phrase
    /// propriétaire.
    pub talker: &'a str,
    /// Type de phrase (`RMC`, `GGA`…) ou code propriétaire.
    pub kind: &'a str,
    /// Champs de données (après l'adresse).
    pub fields: Vec<&'a str>,
    /// Phrase encapsulée (`!`, AIS).
    pub encapsulated: bool,
}

/// Somme de contrôle NMEA : OU exclusif des octets.
#[must_use]
pub fn checksum(body: &str) -> u8 {
    body.bytes().fold(0, |acc, b| acc ^ b)
}

fn strip_tag_block(line: &str) -> Result<&str, SentenceError> {
    match line.strip_prefix('\\') {
        Some(rest) => rest
            .find('\\')
            .map(|end| &rest[end + 1..])
            .ok_or(SentenceError::UnterminatedTagBlock),
        None => Ok(line),
    }
}

/// Valide et découpe une ligne.
///
/// # Errors
///
/// [`SentenceError`] si la ligne n'est pas une phrase NMEA 0183 valide.
pub fn parse(line: &str) -> Result<Sentence<'_>, SentenceError> {
    let line = strip_tag_block(line.trim_end_matches(['\r', '\n']))?;
    if line.is_empty() {
        return Err(SentenceError::Empty);
    }
    if line.len() > MAX_SENTENCE_LEN {
        return Err(SentenceError::TooLong(line.len()));
    }
    if !line.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
        return Err(SentenceError::InvalidCharacter);
    }
    let encapsulated = match line.as_bytes()[0] {
        b'$' => false,
        b'!' => true,
        _ => return Err(SentenceError::BadStart),
    };
    let (body, sum) = line[1..]
        .rsplit_once('*')
        .ok_or(SentenceError::MissingChecksum)?;
    let sum = sum.trim_end();
    if sum.len() != 2 {
        return Err(SentenceError::BadChecksumFormat);
    }
    let received = u8::from_str_radix(sum, 16).map_err(|_| SentenceError::BadChecksumFormat)?;
    let computed = checksum(body);
    if computed != received {
        return Err(SentenceError::ChecksumMismatch { computed, received });
    }

    let mut parts = body.split(',');
    let address = parts.next().unwrap_or_default();
    let valid_chars = address
        .bytes()
        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    let (talker, kind) = if !valid_chars || address.len() < 2 {
        return Err(SentenceError::BadAddress(address.to_owned()));
    } else if let Some(code) = address.strip_prefix('P') {
        ("P", code)
    } else if address.len() == 5 {
        address.split_at(2)
    } else {
        return Err(SentenceError::BadAddress(address.to_owned()));
    };
    Ok(Sentence {
        talker,
        kind,
        fields: parts.collect(),
        encapsulated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ajoute la somme de contrôle à un corps de phrase.
    fn framed(start: char, body: &str) -> String {
        format!("{start}{body}*{:02X}", checksum(body))
    }

    #[test]
    fn parses_valid_sentence() {
        let line = "$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W*6A\r\n";
        let s = parse(line).expect("valide");
        assert_eq!(s.talker, "GP");
        assert_eq!(s.kind, "RMC");
        assert_eq!(s.fields.len(), 11);
        assert_eq!(s.fields[0], "123519");
        assert!(!s.encapsulated);
    }

    #[test]
    fn accepts_other_talkers_and_lowercase_checksum() {
        let body = "IIHDT,274.07,T";
        let l = format!("${body}*{:02x}", checksum(body));
        let s = parse(&l).expect("valide");
        assert_eq!((s.talker, s.kind), ("II", "HDT"));
        let l = framed('$', "GNGGA,1,2");
        let s = parse(&l).expect("valide");
        assert_eq!(s.talker, "GN");
    }

    #[test]
    fn parses_encapsulated_and_proprietary() {
        let l = framed('!', "AIVDM,1,1,,A,15M67FC000G?ufbE`FepT@3n00Sa,0");
        let s = parse(&l).expect("ais");
        assert!(s.encapsulated);
        assert_eq!(s.kind, "VDM");
        let l = framed('$', "PGRME,15.0,M,45.0,M,25.0,M");
        let p = parse(&l).expect("prop");
        assert_eq!((p.talker, p.kind), ("P", "GRME"));
    }

    #[test]
    fn skips_tag_block() {
        let line = format!(
            "\\s:r003669,c:1241544035*4A\\{}",
            framed('!', "AIVDM,1,1,,B,x,0")
        );
        assert_eq!(parse(&line).expect("tag").kind, "VDM");
        assert_eq!(
            parse("\\s:r003669*4A!AIVDM"),
            Err(SentenceError::UnterminatedTagBlock)
        );
    }

    #[test]
    fn rejects_bad_checksum() {
        let err = parse("$GPHDT,274.07,T*00").expect_err("somme fausse");
        assert!(matches!(
            err,
            SentenceError::ChecksumMismatch { received: 0, .. }
        ));
    }

    #[test]
    fn rejects_missing_or_malformed_checksum() {
        assert_eq!(
            parse("$GPHDT,274.07,T"),
            Err(SentenceError::MissingChecksum)
        );
        assert_eq!(
            parse("$GPHDT,274.07,T*3"),
            Err(SentenceError::BadChecksumFormat)
        );
        assert_eq!(
            parse("$GPHDT,274.07,T*ZZ"),
            Err(SentenceError::BadChecksumFormat)
        );
    }

    #[test]
    fn rejects_malformed_lines() {
        assert_eq!(parse(""), Err(SentenceError::Empty));
        assert_eq!(parse("\r\n"), Err(SentenceError::Empty));
        assert_eq!(
            parse(&framed('#', "GPHDT,1,T")),
            Err(SentenceError::BadStart)
        );
        assert_eq!(
            parse("$GPHDT,1é,T*00"),
            Err(SentenceError::InvalidCharacter)
        );
        assert_eq!(
            parse("$GPHDT,1\0,T*00"),
            Err(SentenceError::InvalidCharacter)
        );
        let long = framed('$', &format!("GPTXT,{}", "A".repeat(120)));
        assert!(matches!(parse(&long), Err(SentenceError::TooLong(_))));
    }

    #[test]
    fn rejects_bad_addresses() {
        for body in ["G,1", "gprmc,1", "GPRMCX,1", "GP-MC,1", ",1"] {
            assert!(
                matches!(parse(&framed('$', body)), Err(SentenceError::BadAddress(_))),
                "{body}"
            );
        }
    }

    #[test]
    fn truncated_sentence_fails_checksum() {
        let full = framed(
            '$',
            "GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,,",
        );
        let (head, tail) = full.split_once(",E,").expect("split");
        let cut = format!("{head}{}", &tail[tail.len() - 3..]);
        assert!(parse(&cut).is_err());
    }
}
