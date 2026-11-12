//! Découpage d'un flux d'octets en lignes, avec une taille bornée.
//!
//! Une ligne trop longue (flux corrompu, débit erroné sur un port série)
//! est écartée jusqu'à la fin de ligne suivante, sans accumuler de
//! mémoire.

use thiserror::Error;

/// Taille maximale d'une ligne (bloc d'étiquettes compris).
pub const MAX_LINE_BYTES: usize = 512;

/// Ligne écartée.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FrameError {
    /// Ligne trop longue.
    #[error("ligne trop longue (> {MAX_LINE_BYTES} octets)")]
    TooLong,
    /// Octets non UTF-8 (débit série incorrect, bruit).
    #[error("octets illisibles (débit incorrect ?)")]
    NotText,
}

/// Découpeur de lignes.
#[derive(Debug, Default)]
pub struct LineFramer {
    buffer: Vec<u8>,
    discarding: bool,
}

impl LineFramer {
    /// Découpeur vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn flush(&mut self, out: &mut Vec<Result<String, FrameError>>) {
        if self.discarding {
            self.discarding = false;
        } else if !self.buffer.is_empty() {
            let bytes = std::mem::take(&mut self.buffer);
            out.push(String::from_utf8(bytes).map_err(|_| FrameError::NotText));
        }
    }

    /// Ajoute des octets ; retourne les lignes complètes (sans fin de
    /// ligne) et les erreurs rencontrées.
    pub fn push(&mut self, data: &[u8]) -> Vec<Result<String, FrameError>> {
        let mut out = Vec::new();
        for &byte in data {
            match byte {
                b'\r' | b'\n' => self.flush(&mut out),
                _ if self.discarding => {}
                _ => {
                    self.buffer.push(byte);
                    if self.buffer.len() > MAX_LINE_BYTES {
                        self.buffer.clear();
                        self.discarding = true;
                        out.push(Err(FrameError::TooLong));
                    }
                }
            }
        }
        out
    }

    /// Termine le flux (fin d'un datagramme UDP) : la dernière ligne,
    /// même sans fin de ligne, est retournée.
    pub fn finish(&mut self) -> Vec<Result<String, FrameError>> {
        let mut out = Vec::new();
        self.flush(&mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lines_across_chunks() {
        let mut f = LineFramer::new();
        assert!(f.push(b"$GPHDT,1").is_empty());
        let lines = f.push(b",T*00\r\n$IIDPT,3*00\n\n");
        assert_eq!(
            lines,
            vec![Ok("$GPHDT,1,T*00".to_owned()), Ok("$IIDPT,3*00".to_owned())]
        );
    }

    #[test]
    fn discards_overlong_lines_until_next_line() {
        let mut f = LineFramer::new();
        let out = f.push(&vec![b'A'; MAX_LINE_BYTES + 10]);
        assert_eq!(out, vec![Err(FrameError::TooLong)]);
        // Suite de la ligne trop longue : ignorée, une seule erreur.
        assert!(f.push(&[b'A'; 100]).is_empty());
        assert_eq!(f.push(b"AAA\r\n$OK*00\r\n"), vec![Ok("$OK*00".to_owned())]);
    }

    #[test]
    fn reports_non_utf8_lines() {
        let mut f = LineFramer::new();
        assert_eq!(f.push(b"\xff\xfe\r\n"), vec![Err(FrameError::NotText)]);
    }

    #[test]
    fn finish_returns_unterminated_line() {
        let mut f = LineFramer::new();
        assert!(f.push(b"$GPHDT,1,T*00").is_empty());
        assert_eq!(f.finish(), vec![Ok("$GPHDT,1,T*00".to_owned())]);
        assert!(f.finish().is_empty());
    }
}
