//! Fragments VDM/VDO et réassemblage des messages fragmentés.
//!
//! Un message AIS long (type 5, par exemple) est transmis en plusieurs
//! phrases portant le même identifiant séquentiel. Les fragments
//! doivent arriver dans l'ordre et sans interruption ; un message
//! incomplet est abandonné après [`FRAGMENT_TIMEOUT`]. Le nombre de
//! messages en cours et la taille d'un message sont bornés
//! (SECURITY.md : messages incomplets, limites de taille).

use chrono::{DateTime, Duration, Utc};

use crate::armor::{is_payload_char, Bits};
use crate::AisError;

/// Nombre maximal de fragments d'un message (norme : 9).
pub const MAX_FRAGMENTS: u8 = 9;

/// Taille maximale de la charge utile réassemblée, en caractères
/// (5 créneaux AIS = 1 008 bits = 168 caractères ; marge comprise).
pub const MAX_PAYLOAD_CHARS: usize = 256;

/// Nombre maximal de messages fragmentés en cours de réassemblage.
pub const MAX_PENDING: usize = 16;

/// Délai au-delà duquel un message incomplet est abandonné.
pub const FRAGMENT_TIMEOUT: Duration = Duration::seconds(5);

/// Fragment d'un message AIS (phrase VDM ou VDO).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    /// Phrase VDO : message émis par le navire propre.
    pub own_vessel: bool,
    /// Nombre de fragments du message (1 à 9).
    pub count: u8,
    /// Numéro du fragment (1 à `count`).
    pub number: u8,
    /// Identifiant séquentiel (0 à 9), présent si `count` > 1.
    pub sequence: Option<u8>,
    /// Canal radio (`A`, `B`, `1`, `2`), s'il est indiqué.
    pub channel: Option<char>,
    /// Charge utile armurée.
    pub payload: String,
    /// Bits de remplissage (0 à 5), significatifs sur le dernier
    /// fragment seulement.
    pub fill_bits: u8,
}

fn field_error(field: &'static str, value: &str) -> AisError {
    AisError::Field {
        field,
        value: value.chars().take(16).collect(),
    }
}

fn digit(
    fields: &[&str],
    index: usize,
    field: &'static str,
    range: (u8, u8),
) -> Result<u8, AisError> {
    let raw = fields.get(index).map_or("", |v| v.trim());
    raw.parse::<u8>()
        .ok()
        .filter(|v| (range.0..=range.1).contains(v))
        .ok_or_else(|| field_error(field, raw))
}

impl Fragment {
    /// Analyse les champs d'une phrase `VDM` ou `VDO` (après l'adresse).
    ///
    /// # Errors
    ///
    /// [`AisError::Field`] si un champ est absent ou hors bornes,
    /// [`AisError::InvalidPayload`] si la charge utile est invalide.
    pub fn parse(kind: &str, fields: &[&str]) -> Result<Self, AisError> {
        let own_vessel = match kind {
            "VDM" => false,
            "VDO" => true,
            other => return Err(field_error("type de phrase", other)),
        };
        if fields.len() != 6 {
            return Err(field_error("nombre de champs", &fields.len().to_string()));
        }
        let count = digit(fields, 0, "nombre de fragments", (1, MAX_FRAGMENTS))?;
        let number = digit(fields, 1, "numéro de fragment", (1, count))?;
        let sequence = match fields[2].trim() {
            "" if count == 1 => None,
            "" => return Err(field_error("identifiant séquentiel", "")),
            _ => Some(digit(fields, 2, "identifiant séquentiel", (0, 9))?),
        };
        let channel = match fields[3].trim() {
            "" => None,
            c @ ("A" | "B" | "1" | "2") => c.chars().next(),
            other => return Err(field_error("canal", other)),
        };
        let payload = fields[4].trim();
        if payload.is_empty() {
            return Err(AisError::InvalidPayload("charge utile vide"));
        }
        if !payload.bytes().all(is_payload_char) {
            return Err(AisError::InvalidPayload("caractère hors alphabet"));
        }
        let fill_bits = digit(fields, 5, "bits de remplissage", (0, 5))?;
        if number < count && fill_bits != 0 {
            return Err(AisError::InvalidPayload(
                "bits de remplissage sur un fragment intermédiaire",
            ));
        }
        Ok(Self {
            own_vessel,
            count,
            number,
            sequence,
            channel,
            payload: payload.to_owned(),
            fill_bits,
        })
    }
}

/// Message réassemblé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assembled {
    /// Message émis par le navire propre (VDO).
    pub own_vessel: bool,
    /// Bits du message.
    pub bits: Bits,
}

#[derive(Debug, Clone)]
struct Pending {
    own_vessel: bool,
    sequence: Option<u8>,
    channel: Option<char>,
    count: u8,
    next: u8,
    payload: String,
    started_at: DateTime<Utc>,
}

impl Pending {
    fn matches(&self, f: &Fragment) -> bool {
        self.own_vessel == f.own_vessel && self.sequence == f.sequence && self.channel == f.channel
    }
}

/// Réassemblage des messages fragmentés.
#[derive(Debug, Clone, Default)]
pub struct Assembler {
    pending: Vec<Pending>,
}

impl Assembler {
    /// Réassembleur vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Nombre de messages en cours de réassemblage.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Oublie les messages en cours.
    pub fn clear(&mut self) {
        self.pending.clear();
    }

    /// Ajoute un fragment reçu à `now`. Retourne le message s'il est
    /// complet.
    ///
    /// # Errors
    ///
    /// - [`AisError::OutOfSequence`] : fragment sans début ou hors ordre
    ///   (le message en cours est abandonné) ;
    /// - [`AisError::Incomplete`] : un message incomplet a été abandonné
    ///   au profit de celui qui commence (qui est conservé) ;
    /// - [`AisError::TooLong`] : charge utile réassemblée trop longue ;
    /// - [`AisError::InvalidPayload`] : charge utile invalide.
    pub fn push(
        &mut self,
        fragment: Fragment,
        now: DateTime<Utc>,
    ) -> Result<Option<Assembled>, AisError> {
        self.pending
            .retain(|p| now.signed_duration_since(p.started_at) <= FRAGMENT_TIMEOUT);

        if fragment.count == 1 {
            return Self::finish(fragment.own_vessel, &fragment.payload, fragment.fill_bits)
                .map(Some);
        }

        if fragment.number == 1 {
            let mut dropped = false;
            if let Some(index) = self.pending.iter().position(|p| p.matches(&fragment)) {
                self.pending.remove(index);
                dropped = true;
            }
            if self.pending.len() >= MAX_PENDING {
                // Le plus ancien est en tête (ordre d'insertion).
                self.pending.remove(0);
                dropped = true;
            }
            self.pending.push(Pending {
                own_vessel: fragment.own_vessel,
                sequence: fragment.sequence,
                channel: fragment.channel,
                count: fragment.count,
                next: 2,
                payload: fragment.payload,
                started_at: now,
            });
            return if dropped {
                Err(AisError::Incomplete)
            } else {
                Ok(None)
            };
        }

        let index = self
            .pending
            .iter()
            .position(|p| p.matches(&fragment))
            .ok_or(AisError::OutOfSequence)?;
        let pending = &mut self.pending[index];
        if pending.count != fragment.count || pending.next != fragment.number {
            self.pending.remove(index);
            return Err(AisError::OutOfSequence);
        }
        if pending.payload.len() + fragment.payload.len() > MAX_PAYLOAD_CHARS {
            self.pending.remove(index);
            return Err(AisError::TooLong);
        }
        pending.payload.push_str(&fragment.payload);
        pending.next += 1;
        if fragment.number < fragment.count {
            return Ok(None);
        }
        let done = self.pending.remove(index);
        Self::finish(done.own_vessel, &done.payload, fragment.fill_bits).map(Some)
    }

    fn finish(own_vessel: bool, payload: &str, fill_bits: u8) -> Result<Assembled, AisError> {
        if payload.len() > MAX_PAYLOAD_CHARS {
            return Err(AisError::TooLong);
        }
        Ok(Assembled {
            own_vessel,
            bits: Bits::from_payload(payload, fill_bits)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(seconds: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t") + Duration::seconds(seconds)
    }

    fn frag(count: u8, number: u8, seq: &str, payload: &str, fill: u8) -> Fragment {
        let c = count.to_string();
        let n = number.to_string();
        let f = fill.to_string();
        Fragment::parse("VDM", &[&c, &n, seq, "A", payload, &f]).expect("fragment")
    }

    #[test]
    fn parses_fields() {
        let f = Fragment::parse(
            "VDO",
            &["1", "1", "", "B", "15M67FC000G?ufbE`FepT@3n00Sa", "0"],
        )
        .expect("vdo");
        assert!(f.own_vessel);
        assert_eq!(
            (f.count, f.number, f.sequence, f.channel),
            (1, 1, None, Some('B'))
        );
        let f = Fragment::parse("VDM", &["2", "1", "3", "", "55", "0"]).expect("vdm");
        assert_eq!((f.sequence, f.channel), (Some(3), None));
    }

    #[test]
    fn rejects_malformed_fields() {
        let cases: [(&str, [&str; 6]); 10] = [
            ("VDX", ["1", "1", "", "A", "1", "0"]),
            ("VDM", ["0", "1", "", "A", "1", "0"]),
            ("VDM", ["10", "1", "", "A", "1", "0"]),
            ("VDM", ["2", "3", "1", "A", "1", "0"]),
            ("VDM", ["2", "1", "", "A", "1", "0"]),
            ("VDM", ["1", "1", "", "C", "1", "0"]),
            ("VDM", ["1", "1", "", "A", "", "0"]),
            ("VDM", ["1", "1", "", "A", "1x", "0"]),
            ("VDM", ["1", "1", "", "A", "1", "6"]),
            ("VDM", ["2", "1", "1", "A", "1", "2"]),
        ];
        for (kind, fields) in cases {
            assert!(Fragment::parse(kind, &fields).is_err(), "{kind} {fields:?}");
        }
        assert!(Fragment::parse("VDM", &["1", "1", "", "A", "1"]).is_err());
    }

    #[test]
    fn single_fragment_is_complete() {
        let mut a = Assembler::new();
        let m = a
            .push(frag(1, 1, "", "15M67F", 0), t(0))
            .expect("ok")
            .expect("complete");
        assert_eq!(m.bits.len(), 36);
        assert!(!m.own_vessel);
    }

    #[test]
    fn reassembles_in_order() {
        let mut a = Assembler::new();
        assert_eq!(a.push(frag(3, 1, "7", "1111", 0), t(0)), Ok(None));
        assert_eq!(a.push(frag(3, 2, "7", "2222", 0), t(1)), Ok(None));
        let m = a
            .push(frag(3, 3, "7", "33", 2), t(2))
            .expect("ok")
            .expect("complete");
        assert_eq!(m.bits.len(), 10 * 6 - 2);
        assert_eq!(a.pending(), 0);
    }

    #[test]
    fn interleaved_sequences_are_kept_apart() {
        let mut a = Assembler::new();
        a.push(frag(2, 1, "1", "11", 0), t(0)).expect("ok");
        a.push(frag(2, 1, "2", "22", 0), t(0)).expect("ok");
        let m2 = a
            .push(frag(2, 2, "2", "2", 0), t(0))
            .expect("ok")
            .expect("2");
        let m1 = a
            .push(frag(2, 2, "1", "1", 0), t(0))
            .expect("ok")
            .expect("1");
        assert_eq!(m1.bits.len(), 18);
        assert_eq!(m2.bits.len(), 18);
    }

    #[test]
    fn rejects_orphans_and_gaps() {
        let mut a = Assembler::new();
        assert_eq!(
            a.push(frag(2, 2, "1", "11", 0), t(0)),
            Err(AisError::OutOfSequence)
        );
        a.push(frag(3, 1, "1", "11", 0), t(0)).expect("ok");
        assert_eq!(
            a.push(frag(3, 3, "1", "11", 0), t(0)),
            Err(AisError::OutOfSequence)
        );
        assert_eq!(a.pending(), 0);
    }

    #[test]
    fn restart_drops_incomplete_message() {
        let mut a = Assembler::new();
        a.push(frag(2, 1, "1", "11", 0), t(0)).expect("ok");
        assert_eq!(
            a.push(frag(2, 1, "1", "33", 0), t(0)),
            Err(AisError::Incomplete)
        );
        let m = a
            .push(frag(2, 2, "1", "4", 0), t(0))
            .expect("ok")
            .expect("complete");
        assert_eq!(m.bits.len(), 18);
    }

    #[test]
    fn incomplete_messages_expire() {
        let mut a = Assembler::new();
        a.push(frag(2, 1, "1", "11", 0), t(0)).expect("ok");
        assert_eq!(
            a.push(frag(2, 2, "1", "11", 0), t(6)),
            Err(AisError::OutOfSequence)
        );
    }

    #[test]
    fn pending_messages_are_bounded() {
        let mut a = Assembler::new();
        for i in 0..MAX_PENDING {
            let seq = (i % 10).to_string();
            let channel = if i < 10 { "A" } else { "B" };
            let f = Fragment::parse("VDM", &["2", "1", &seq, channel, "11", "0"]).expect("f");
            assert_eq!(a.push(f, t(0)), Ok(None));
        }
        let f = Fragment::parse("VDO", &["2", "1", "0", "A", "11", "0"]).expect("f");
        assert_eq!(a.push(f, t(0)), Err(AisError::Incomplete));
        assert_eq!(a.pending(), MAX_PENDING);
    }

    #[test]
    fn oversized_message_is_rejected() {
        let mut a = Assembler::new();
        let chunk = "1".repeat(100);
        a.push(frag(3, 1, "1", &chunk, 0), t(0)).expect("ok");
        a.push(frag(3, 2, "1", &chunk, 0), t(0)).expect("ok");
        assert_eq!(
            a.push(frag(3, 3, "1", &chunk, 0), t(0)),
            Err(AisError::TooLong)
        );
        assert_eq!(a.pending(), 0);
    }
}
