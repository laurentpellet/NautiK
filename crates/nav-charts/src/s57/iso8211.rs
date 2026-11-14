//! Lecture de l'enveloppe ISO/IEC 8211, format d'échange des cellules S-57.
//!
//! Un fichier est une suite d'enregistrements : un enregistrement
//! descriptif (DDR, guide `L`) qui décrit les champs, puis des
//! enregistrements de données (DR, guide `D`). Chaque enregistrement a un
//! guide de 24 octets, un répertoire (étiquette, longueur, position de
//! chaque champ) et une zone de champs.
//!
//! Toutes les longueurs et positions sont vérifiées : un fichier tronqué
//! ou corrompu produit une erreur, jamais de lecture hors limites.

use std::collections::HashMap;

use super::S57Error;

/// Fin de champ.
pub(crate) const FIELD_TERMINATOR: u8 = 0x1E;
/// Fin de sous-champ de longueur variable.
pub(crate) const UNIT_TERMINATOR: u8 = 0x1F;

/// Longueur du guide d'enregistrement.
const LEADER_LEN: usize = 24;

/// Nombre maximal de formats après développement des répétitions.
const MAX_FORMATS: usize = 256;

/// Champ d'un enregistrement de données.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Field<'a> {
    /// Étiquette (ex. `FRID`).
    pub tag: &'a str,
    /// Données, sans la fin de champ.
    pub data: &'a [u8],
}

/// Enregistrement de données.
#[derive(Debug, Clone)]
pub(crate) struct Record<'a> {
    /// Champs, dans l'ordre du répertoire.
    pub fields: Vec<Field<'a>>,
}

impl<'a> Record<'a> {
    /// Premier champ portant cette étiquette.
    pub fn field(&self, tag: &str) -> Option<&Field<'a>> {
        self.fields.iter().find(|f| f.tag == tag)
    }

    /// Tous les champs portant cette étiquette (un champ peut se répéter).
    pub fn fields<'s>(&'s self, tag: &'s str) -> impl Iterator<Item = &'s Field<'a>> + 's {
        self.fields.iter().filter(move |f| f.tag == tag)
    }
}

/// Description d'un champ dans le DDR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldDescription {
    /// Nom du champ.
    pub name: String,
    /// Descripteur de tableau (libellés des sous-champs, ex. `*YCOO!XCOO`).
    pub array: String,
    /// Formats des sous-champs, répétitions développées (ex. `b24`).
    pub formats: Vec<String>,
}

/// Fichier ISO 8211 lu : descriptions de champs et enregistrements.
#[derive(Debug, Clone)]
pub(crate) struct Iso8211File<'a> {
    /// Descriptions des champs, par étiquette.
    pub descriptions: HashMap<String, FieldDescription>,
    /// Enregistrements de données.
    pub records: Vec<Record<'a>>,
}

/// Entier ASCII à chiffres décimaux (espaces de tête tolérés).
fn digits(bytes: &[u8], what: &str) -> Result<usize, S57Error> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| S57Error::malformed(format!("{what} : caractères invalides")))?;
    let trimmed = text.trim_start();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(S57Error::malformed(format!(
            "{what} : « {text} » n'est pas un nombre"
        )));
    }
    trimmed
        .parse()
        .map_err(|_| S57Error::malformed(format!("{what} : nombre hors limites")))
}

/// Taille (un chiffre de 1 à 9) lue dans le guide.
fn size_digit(byte: u8, what: &str) -> Result<usize, S57Error> {
    match byte {
        b'1'..=b'9' => Ok(usize::from(byte - b'0')),
        _ => Err(S57Error::malformed(format!(
            "{what} invalide dans le guide"
        ))),
    }
}

/// Enregistrement brut : identifiant de guide, longueur de contrôle et champs.
struct RawRecord<'a> {
    leader_id: u8,
    field_control_len: usize,
    fields: Vec<Field<'a>>,
}

/// Lit l'enregistrement qui commence à `offset` ; renvoie aussi sa longueur.
fn read_record(data: &[u8], offset: usize) -> Result<(RawRecord<'_>, usize), S57Error> {
    let rest = data.get(offset..).unwrap_or_default();
    let leader = rest
        .get(..LEADER_LEN)
        .ok_or_else(|| S57Error::truncated(offset))?;
    let record_len = digits(&leader[0..5], "longueur d'enregistrement")?;
    if record_len < LEADER_LEN + 1 {
        return Err(S57Error::malformed(format!(
            "enregistrement trop court ({record_len} octets) à l'octet {offset}"
        )));
    }
    let record = rest
        .get(..record_len)
        .ok_or_else(|| S57Error::truncated(offset))?;
    let leader_id = leader[6];
    let field_control_len = if leader_id == b'L' {
        digits(&leader[10..12], "longueur des contrôles de champ")?
    } else {
        0
    };
    let base = digits(&leader[12..17], "adresse de la zone de champs")?;
    let size_len = size_digit(leader[20], "taille des longueurs de champ")?;
    let size_pos = size_digit(leader[21], "taille des positions de champ")?;
    let size_tag = size_digit(leader[23], "taille des étiquettes")?;
    if base <= LEADER_LEN || base > record_len {
        return Err(S57Error::malformed(format!(
            "adresse de zone de champs {base} hors de l'enregistrement à l'octet {offset}"
        )));
    }
    // Le répertoire se termine par une fin de champ, juste avant la zone.
    if record[base - 1] != FIELD_TERMINATOR {
        return Err(S57Error::malformed(format!(
            "répertoire non terminé à l'octet {offset}"
        )));
    }
    let directory = &record[LEADER_LEN..base - 1];
    let entry_len = size_tag + size_len + size_pos;
    if directory.is_empty() || directory.len() % entry_len != 0 {
        return Err(S57Error::malformed(format!(
            "répertoire de taille incohérente à l'octet {offset}"
        )));
    }
    let area = &record[base..];
    let mut fields = Vec::with_capacity(directory.len() / entry_len);
    for entry in directory.chunks_exact(entry_len) {
        let tag = std::str::from_utf8(&entry[..size_tag])
            .ok()
            .filter(|t| t.bytes().all(|b| b.is_ascii_graphic()))
            .ok_or_else(|| S57Error::malformed(format!("étiquette invalide à l'octet {offset}")))?;
        let len = digits(&entry[size_tag..size_tag + size_len], "longueur de champ")?;
        let pos = digits(&entry[size_tag + size_len..], "position de champ")?;
        let mut field = pos
            .checked_add(len)
            .and_then(|end| area.get(pos..end))
            .ok_or_else(|| {
                S57Error::malformed(format!(
                    "champ {tag} hors de l'enregistrement à l'octet {offset}"
                ))
            })?;
        if let Some((&FIELD_TERMINATOR, head)) = field.split_last() {
            field = head;
        }
        fields.push(Field { tag, data: field });
    }
    Ok((
        RawRecord {
            leader_id,
            field_control_len,
            fields,
        },
        record_len,
    ))
}

/// Développe des contrôles de format : `(b11,2b12,A(8),3(b11,b14))` →
/// `b11 b12 b12 A(8) b11 b14 b11 b14 b11 b14`.
pub(crate) fn expand_formats(text: &str) -> Result<Vec<String>, S57Error> {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let bad = || S57Error::malformed(format!("contrôles de format invalides : « {text} »"));
    let inner = compact
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(bad)?;
    let mut out = Vec::new();
    expand_list(inner, &mut out, 0).map_err(|_| bad())?;
    Ok(out)
}

/// Développe une liste séparée par des virgules (au niveau 0).
fn expand_list(list: &str, out: &mut Vec<String>, depth: usize) -> Result<(), ()> {
    if depth > 4 || list.is_empty() {
        return Err(());
    }
    let mut level = 0i32;
    let mut start = 0usize;
    let bytes = list.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' => level += 1,
            b')' => {
                level -= 1;
                if level < 0 {
                    return Err(());
                }
            }
            b',' if level == 0 => {
                expand_item(&list[start..i], out, depth)?;
                start = i + 1;
            }
            _ => {}
        }
    }
    if level != 0 {
        return Err(());
    }
    expand_item(&list[start..], out, depth)
}

/// Développe un élément : répétition facultative puis groupe ou format.
fn expand_item(item: &str, out: &mut Vec<String>, depth: usize) -> Result<(), ()> {
    let count_len = item.bytes().take_while(u8::is_ascii_digit).count();
    let (count, body) = item.split_at(count_len);
    let count: usize = if count.is_empty() {
        1
    } else {
        count.parse().map_err(|_| ())?
    };
    if count == 0 || count > MAX_FORMATS {
        return Err(());
    }
    let mut one = Vec::new();
    if let Some(group) = body.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        expand_list(group, &mut one, depth + 1)?;
    } else if is_format(body) {
        one.push(body.to_owned());
    } else {
        return Err(());
    }
    for _ in 0..count {
        if out.len() + one.len() > MAX_FORMATS {
            return Err(());
        }
        out.extend(one.iter().cloned());
    }
    Ok(())
}

/// Format élémentaire : `A`, `I`, `R`, `B`, `@`, `C` avec largeur
/// facultative `(n)`, ou binaire `bXY` (X = 1 non signé, 2 signé).
fn is_format(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some('b') => {
            let rest = chars.as_str();
            rest.len() == 2
                && matches!(rest.as_bytes()[0], b'1' | b'2')
                && matches!(rest.as_bytes()[1], b'1' | b'2' | b'4' | b'8')
        }
        Some('A' | 'I' | 'R' | 'S' | 'C' | 'B' | '@') => {
            let rest = chars.as_str();
            rest.is_empty()
                || rest
                    .strip_prefix('(')
                    .and_then(|s| s.strip_suffix(')'))
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        }
        _ => false,
    }
}

/// Lit une description de champ du DDR.
fn read_description(data: &[u8], control_len: usize) -> Result<FieldDescription, S57Error> {
    let body = data
        .get(control_len..)
        .ok_or_else(|| S57Error::malformed("description de champ tronquée"))?;
    let mut parts = body.split(|&b| b == UNIT_TERMINATOR);
    let text = |part: Option<&[u8]>| String::from_utf8_lossy(part.unwrap_or_default()).into_owned();
    let name = text(parts.next());
    let array = text(parts.next());
    let format = text(parts.next());
    let formats = if format.trim().is_empty() {
        Vec::new()
    } else {
        expand_formats(&format)?
    };
    Ok(FieldDescription {
        name,
        array,
        formats,
    })
}

/// Lit un fichier ISO 8211 complet : un DDR puis des DR.
pub(crate) fn read_file(data: &[u8]) -> Result<Iso8211File<'_>, S57Error> {
    if data.is_empty() {
        return Err(S57Error::malformed("fichier vide"));
    }
    let (ddr, mut offset) = read_record(data, 0)?;
    if ddr.leader_id != b'L' {
        return Err(S57Error::malformed(
            "pas d'enregistrement descriptif (DDR) en tête : ce n'est pas un fichier ISO 8211",
        ));
    }
    let mut descriptions = HashMap::new();
    for field in &ddr.fields {
        let description = read_description(field.data, ddr.field_control_len).map_err(|e| {
            S57Error::malformed(format!("description du champ {} : {e}", field.tag))
        })?;
        descriptions.insert(field.tag.to_owned(), description);
    }
    let mut records = Vec::new();
    while offset < data.len() {
        let (record, len) = read_record(data, offset)?;
        if record.leader_id != b'D' {
            return Err(S57Error::unsupported(format!(
                "guide d'enregistrement « {} » non pris en charge à l'octet {offset}",
                char::from(record.leader_id)
            )));
        }
        for field in &record.fields {
            if !descriptions.contains_key(field.tag) {
                return Err(S57Error::malformed(format!(
                    "champ {} absent de l'enregistrement descriptif",
                    field.tag
                )));
            }
        }
        records.push(Record {
            fields: record.fields,
        });
        offset += len;
    }
    Ok(Iso8211File {
        descriptions,
        records,
    })
}

/// Lecture séquentielle des sous-champs d'un champ binaire S-57.
pub(crate) struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    tag: &'a str,
}

impl<'a> Cursor<'a> {
    /// Curseur au début du champ.
    pub fn new(field: &Field<'a>) -> Self {
        Self {
            data: field.data,
            pos: 0,
            tag: field.tag,
        }
    }

    /// Reste-t-il des données ?
    pub fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// Données restantes.
    pub fn rest(&self) -> &'a [u8] {
        self.data.get(self.pos..).unwrap_or_default()
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], S57Error> {
        let bytes = self
            .pos
            .checked_add(n)
            .and_then(|end| self.data.get(self.pos..end))
            .ok_or_else(|| S57Error::malformed(format!("champ {} tronqué", self.tag)))?;
        self.pos += n;
        Ok(bytes)
    }

    /// Entier non signé sur 1 octet (`b11`).
    pub fn u8(&mut self) -> Result<u8, S57Error> {
        Ok(self.take(1)?[0])
    }

    /// Entier non signé sur 2 octets, petit-boutiste (`b12`).
    pub fn u16(&mut self) -> Result<u16, S57Error> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    /// Entier non signé sur 4 octets, petit-boutiste (`b14`).
    pub fn u32(&mut self) -> Result<u32, S57Error> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Entier signé sur 4 octets, petit-boutiste (`b24`).
    pub fn i32(&mut self) -> Result<i32, S57Error> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Texte de longueur fixe (`A(n)`, `R(n)`), en Latin-1.
    pub fn fixed_text(&mut self, n: usize) -> Result<String, S57Error> {
        Ok(latin1(self.take(n)?))
    }

    /// Octets d'un sous-champ variable terminé par une fin de sous-champ
    /// (ou par la fin du champ).
    pub fn variable_bytes(&mut self) -> &'a [u8] {
        let rest = self.data.get(self.pos..).unwrap_or_default();
        let len = rest
            .iter()
            .position(|&b| b == UNIT_TERMINATOR)
            .unwrap_or(rest.len());
        self.pos += (len + 1).min(rest.len());
        &rest[..len]
    }

    /// Texte variable (`A`), en Latin-1.
    pub fn variable_text(&mut self) -> String {
        latin1(self.variable_bytes())
    }

    /// Texte variable en UCS-2 petit-boutiste (niveau lexical 2),
    /// terminé par `1F 00`.
    pub fn variable_ucs2(&mut self) -> String {
        let rest = self.data.get(self.pos..).unwrap_or_default();
        let mut units = Vec::new();
        let mut used = 0usize;
        for pair in rest.chunks(2) {
            used += pair.len();
            match pair {
                [lo, hi] => {
                    let unit = u16::from_le_bytes([*lo, *hi]);
                    if unit == u16::from(UNIT_TERMINATOR) || unit == u16::from(FIELD_TERMINATOR) {
                        break;
                    }
                    units.push(unit);
                }
                _ => break,
            }
        }
        self.pos += used;
        String::from_utf16_lossy(&units)
    }
}

/// Texte Latin-1 (niveaux lexicaux 0 et 1) ; chaque octet est un caractère.
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_repetitions_and_groups() {
        assert_eq!(
            expand_formats("(b11,b14,2b11,2b12,b11)").unwrap(),
            ["b11", "b14", "b11", "b11", "b12", "b12", "b11"]
        );
        assert_eq!(
            expand_formats("(A(2), 2(b11,R(4)))").unwrap(),
            ["A(2)", "b11", "R(4)", "b11", "R(4)"]
        );
        assert_eq!(expand_formats("(b12,A)").unwrap(), ["b12", "A"]);
    }

    #[test]
    fn rejects_malformed_formats() {
        for bad in [
            "",
            "b11",
            "(",
            "()",
            "(b1",
            "(b11",
            "((b11)",
            "(b11))",
            "(b13)",
            "(b31)",
            "(X)",
            "(A(x))",
            "(0b11)",
            "(999b11)",
            "(300b11)",
            "(,)",
            "(b11,)",
            "(((((((b11)))))))",
        ] {
            assert!(expand_formats(bad).is_err(), "« {bad} » accepté");
        }
    }

    #[test]
    fn rejects_short_or_inconsistent_records() {
        assert!(read_file(b"").is_err());
        assert!(read_file(b"00010").is_err());
        assert!(read_file(b"abcde3LE1 0600025 ! 3404").is_err());
        // Longueur annoncée plus grande que le fichier.
        assert!(read_file(b"099993LE1 0600025 ! 3404\x1e").is_err());
    }

    #[test]
    fn cursor_never_reads_past_the_field() {
        let field = Field {
            tag: "TEST",
            data: &[1, 2, 3],
        };
        let mut cursor = Cursor::new(&field);
        assert_eq!(cursor.u16().unwrap(), 0x0201);
        assert!(cursor.u16().is_err());
        assert_eq!(cursor.u8().unwrap(), 3);
        assert!(cursor.u8().is_err());
        assert!(cursor.is_empty());
        assert_eq!(cursor.variable_text(), "");
    }

    #[test]
    fn variable_text_stops_at_unit_terminator() {
        let field = Field {
            tag: "ATTF",
            data: b"12\x1fAB\x1f\xe9",
        };
        let mut cursor = Cursor::new(&field);
        assert_eq!(cursor.variable_text(), "12");
        assert_eq!(cursor.variable_text(), "AB");
        assert_eq!(cursor.variable_text(), "é");
        assert!(cursor.rest().is_empty());
    }

    #[test]
    fn ucs2_text_stops_at_terminator() {
        let field = Field {
            tag: "NATF",
            data: &[0xE9, 0x00, 0x41, 0x00, 0x1F, 0x00, 0x42, 0x00],
        };
        let mut cursor = Cursor::new(&field);
        assert_eq!(cursor.variable_ucs2(), "éA");
        assert_eq!(cursor.variable_ucs2(), "B");
        assert!(cursor.is_empty());
    }
}
