//! Lecture de champs binaires (bits de poids fort en tête), bornée :
//! toute lecture au-delà des données est une erreur, jamais une panique.

use crate::grib2::GribError;

/// Lecteur de bits.
#[derive(Debug, Clone)]
pub(crate) struct Bits<'a> {
    data: &'a [u8],
    /// Position en bits.
    pos: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Lit `n` bits (0 à 32) non signés.
    pub(crate) fn read(&mut self, n: u32) -> Result<u32, GribError> {
        if n > 32 {
            return Err(GribError::Invalid(format!("largeur de champ {n} bits")));
        }
        let mut value: u64 = 0;
        let mut remaining = n;
        while remaining > 0 {
            let byte = *self
                .data
                .get(self.pos / 8)
                .ok_or(GribError::Truncated("données compactées"))?;
            let offset = (self.pos % 8) as u32;
            let available = 8 - offset;
            let take = available.min(remaining);
            let shift = available - take;
            let bits = (u64::from(byte) >> shift) & ((1u64 << take) - 1);
            value = (value << take) | bits;
            self.pos += take as usize;
            remaining -= take;
        }
        u32::try_from(value).map_err(|_| GribError::Invalid("valeur hors 32 bits".to_owned()))
    }

    /// Lit un entier signé « signe + valeur absolue » sur `n` bits (2 à 32).
    pub(crate) fn read_signed(&mut self, n: u32) -> Result<i64, GribError> {
        if n < 2 {
            return Err(GribError::Invalid(format!("entier signé sur {n} bits")));
        }
        let sign = self.read(1)?;
        let magnitude = i64::from(self.read(n - 1)?);
        Ok(if sign == 1 { -magnitude } else { magnitude })
    }

    /// Avance jusqu'à l'octet suivant.
    pub(crate) fn align(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_across_byte_boundaries() {
        let mut b = Bits::new(&[0b1010_1100, 0b0101_0000]);
        assert_eq!(b.read(3), Ok(0b101));
        assert_eq!(b.read(7), Ok(0b011_0001));
        assert_eq!(b.read(0), Ok(0));
        b.align();
        assert!(b.read(1).is_err(), "fin des données");
    }

    #[test]
    fn reads_sign_magnitude() {
        let mut b = Bits::new(&[0x80, 0x05, 0x00, 0x07]);
        assert_eq!(b.read_signed(16), Ok(-5));
        assert_eq!(b.read_signed(16), Ok(7));
        assert!(Bits::new(&[0]).read_signed(1).is_err());
    }

    #[test]
    fn rejects_oversized_fields() {
        assert!(Bits::new(&[0; 8]).read(33).is_err());
        assert_eq!(Bits::new(&[0xFF; 4]).read(32), Ok(u32::MAX));
    }
}
