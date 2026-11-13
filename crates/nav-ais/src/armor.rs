//! Charge utile AIS : armure 6 bits (ITU-R M.1371, IEC 61162-1) et
//! lecture/écriture de champs binaires.

use crate::AisError;

/// Valeur 6 bits d'un caractère de charge utile.
fn unarmor(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'W' => Some(c - b'0'),
        b'`'..=b'w' => Some(c - b'0' - 8),
        _ => None,
    }
}

/// Caractère de charge utile d'une valeur 6 bits (`value` < 64).
fn armor(value: u8) -> char {
    let v = value & 0x3f;
    char::from(if v < 40 { v + b'0' } else { v + b'0' + 8 })
}

/// Le caractère appartient-il à l'alphabet de la charge utile ?
#[must_use]
pub fn is_payload_char(c: u8) -> bool {
    unarmor(c).is_some()
}

/// Suite de bits d'un message AIS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bits {
    bits: Vec<bool>,
}

impl Bits {
    /// Décode une charge utile armurée.
    ///
    /// # Errors
    ///
    /// [`AisError::InvalidPayload`] si un caractère est hors alphabet ou
    /// si les bits de remplissage sont invalides.
    pub fn from_payload(payload: &str, fill_bits: u8) -> Result<Self, AisError> {
        if fill_bits > 5 {
            return Err(AisError::InvalidPayload("bits de remplissage > 5"));
        }
        let mut bits = Vec::with_capacity(payload.len() * 6);
        for c in payload.bytes() {
            let value = unarmor(c).ok_or(AisError::InvalidPayload("caractère hors alphabet"))?;
            bits.extend((0..6).rev().map(|i| value >> i & 1 == 1));
        }
        let len = bits
            .len()
            .checked_sub(usize::from(fill_bits))
            .ok_or(AisError::InvalidPayload("charge utile vide"))?;
        bits.truncate(len);
        Ok(Self { bits })
    }

    /// Nombre de bits utiles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bits.len()
    }

    /// Aucun bit ?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    /// Entier non signé de `len` bits (≤ 32) à partir de `start`.
    #[must_use]
    pub fn unsigned(&self, start: usize, len: usize) -> Option<u32> {
        if len > 32 {
            return None;
        }
        let slice = self.bits.get(start..start.checked_add(len)?)?;
        Some(
            slice
                .iter()
                .fold(0_u32, |acc, &bit| (acc << 1) | u32::from(bit)),
        )
    }

    /// Entier signé (complément à deux) de `len` bits (2 à 32).
    #[must_use]
    pub fn signed(&self, start: usize, len: usize) -> Option<i32> {
        if !(2..=32).contains(&len) {
            return None;
        }
        let raw = i64::from(self.unsigned(start, len)?);
        let value = if raw >> (len - 1) & 1 == 1 {
            raw - (1_i64 << len)
        } else {
            raw
        };
        i32::try_from(value).ok()
    }

    /// Texte 6 bits de `chars` caractères (alphabet AIS, `@` = vide).
    #[must_use]
    pub fn text(&self, start: usize, chars: usize) -> Option<String> {
        (0..chars)
            .map(|i| {
                let v = u8::try_from(self.unsigned(start + 6 * i, 6)?).ok()?;
                Some(char::from(if v < 32 { v + 64 } else { v }))
            })
            .collect()
    }
}

/// Construction d'une charge utile (simulation, tests).
#[derive(Debug, Clone, Default)]
pub struct BitWriter {
    bits: Vec<bool>,
}

impl BitWriter {
    /// Écrivain vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute les `len` bits de poids faible de `value` (zéros en tête
    /// au-delà de 32 bits).
    pub fn unsigned(&mut self, value: u32, len: usize) -> &mut Self {
        self.bits
            .extend((0..len).rev().map(|i| i < 32 && value >> i & 1 == 1));
        self
    }

    /// Ajoute un entier signé en complément à deux sur `len` bits.
    pub fn signed(&mut self, value: i32, len: usize) -> &mut Self {
        // Réinterprétation binaire volontaire (complément à deux).
        self.unsigned(u32::from_ne_bytes(value.to_ne_bytes()), len)
    }

    /// Ajoute un texte 6 bits de `chars` caractères, complété par `@`.
    pub fn text(&mut self, text: &str, chars: usize) -> &mut Self {
        let mut values = text.bytes().map(|c| {
            let c = c.to_ascii_uppercase();
            match c {
                64..=95 => c - 64,
                32..=63 => c,
                _ => 32,
            }
        });
        for _ in 0..chars {
            self.unsigned(u32::from(values.next().unwrap_or(0)), 6);
        }
        self
    }

    /// Charge utile armurée et nombre de bits de remplissage.
    #[must_use]
    pub fn payload(&self) -> (String, u8) {
        let fill = (6 - self.bits.len() % 6) % 6;
        let mut bits = self.bits.clone();
        bits.resize(bits.len() + fill, false);
        let payload = bits
            .chunks(6)
            .map(|c| armor(c.iter().fold(0_u8, |acc, &b| (acc << 1) | u8::from(b))))
            .collect();
        // `fill` < 6 : conversion sans perte.
        (payload, u8::try_from(fill).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabet_round_trip() {
        for v in 0..64 {
            assert_eq!(unarmor(armor(v) as u8), Some(v));
        }
        assert_eq!(unarmor(b'X'), None);
        assert_eq!(unarmor(b'_'), None);
        assert_eq!(unarmor(b'x'), None);
        assert!(is_payload_char(b'w'));
    }

    #[test]
    fn reads_fields() {
        let bits = Bits::from_payload("15M67FC000G?ufbE`FepT@3n00Sa", 0).expect("bits");
        assert_eq!(bits.len(), 168);
        assert_eq!(bits.unsigned(0, 6), Some(1));
        assert_eq!(bits.unsigned(8, 30), Some(366_053_209));
        assert_eq!(bits.unsigned(160, 32), None);
        assert_eq!(bits.unsigned(0, 33), None);
        assert_eq!(bits.signed(0, 1), None);
    }

    #[test]
    fn fill_bits_are_removed() {
        assert_eq!(Bits::from_payload("w", 2).expect("bits").len(), 4);
        assert!(Bits::from_payload("w", 6).is_err());
        assert!(Bits::from_payload("", 1).is_err());
        assert!(Bits::from_payload("1x", 0).is_err());
    }

    #[test]
    fn writer_round_trip() {
        let mut w = BitWriter::new();
        w.unsigned(5, 6)
            .signed(-1234, 27)
            .signed(4321, 28)
            .text("Tiputa 1", 10);
        let (payload, fill) = w.payload();
        let bits = Bits::from_payload(&payload, fill).expect("bits");
        assert_eq!(bits.len(), 6 + 27 + 28 + 60);
        assert_eq!(bits.unsigned(0, 6), Some(5));
        assert_eq!(bits.signed(6, 27), Some(-1234));
        assert_eq!(bits.signed(33, 28), Some(4321));
        assert_eq!(bits.text(61, 10).as_deref(), Some("TIPUTA 1@@"));
    }
}
