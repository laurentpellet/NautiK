//! Machine à états commune à toutes les alarmes.
//!
//! ```text
//!            condition (≥ délai)             condition absente (≥ délai de retour)
//! Normal ──► EnAttente ──────────► Active ──acquit──► Acquittée ──────────────► Normal
//!   ▲            │ condition absente   │                    │
//!   └────────────┘                     └────────────────────┘
//! ```
//!
//! - **Temporisation** : la condition doit persister `delay` avant
//!   l'activation.
//! - **Retour** : la condition doit avoir disparu `clear_delay` avant la
//!   résolution (avec l'hystérésis propre à chaque règle, évite les
//!   oscillations).
//! - Une condition **inconnue** (donnée absente ou obsolète) ne résout
//!   jamais une alarme et n'en active jamais une : elle interrompt
//!   seulement une temporisation en cours. La perte de données est
//!   signalée par sa propre alarme.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Évaluation d'une règle à un instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    /// La situation est normale.
    Clear,
    /// La situation d'alarme est constatée.
    Triggered,
    /// Les données ne permettent pas de conclure.
    Unknown,
}

/// État d'une alarme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlarmState {
    /// Alarme désactivée par l'utilisateur.
    Off,
    /// Surveillance active, situation normale.
    Normal,
    /// Condition constatée, temporisation en cours.
    Pending,
    /// Alarme déclenchée, non acquittée (sonore).
    Active,
    /// Alarme déclenchée et acquittée (silencieuse, toujours affichée).
    Acknowledged,
}

/// Changement d'état notable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Passage à l'état actif.
    Activated,
    /// Retour à la normale.
    Resolved,
}

/// Machine à états d'une alarme.
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    state: AlarmState,
    pending_since: Option<DateTime<Utc>>,
    active_since: Option<DateTime<Utc>>,
    clear_since: Option<DateTime<Utc>>,
}

impl Machine {
    /// Machine à l'état `Normal` (ou `Off` si désactivée).
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            state: if enabled {
                AlarmState::Normal
            } else {
                AlarmState::Off
            },
            pending_since: None,
            active_since: None,
            clear_since: None,
        }
    }

    /// État courant.
    #[must_use]
    pub fn state(&self) -> AlarmState {
        self.state
    }

    /// Instant de déclenchement (alarme active ou acquittée).
    #[must_use]
    pub fn active_since(&self) -> Option<DateTime<Utc>> {
        self.active_since
    }

    /// L'alarme est déclenchée (acquittée ou non).
    #[must_use]
    pub fn is_alerting(&self) -> bool {
        matches!(self.state, AlarmState::Active | AlarmState::Acknowledged)
    }

    /// Active la surveillance (sans effet si elle l'est déjà).
    pub fn enable(&mut self) {
        if self.state == AlarmState::Off {
            *self = Self::new(true);
        }
    }

    /// Désactive la surveillance, quel que soit l'état.
    pub fn disable(&mut self) {
        *self = Self::new(false);
    }

    /// Acquitte une alarme active. Retourne `false` si elle ne l'était pas.
    pub fn acknowledge(&mut self) -> bool {
        if self.state == AlarmState::Active {
            self.state = AlarmState::Acknowledged;
            true
        } else {
            false
        }
    }

    /// Fait évoluer la machine selon la condition constatée à `now`.
    pub fn update(
        &mut self,
        condition: Condition,
        now: DateTime<Utc>,
        delay: Duration,
        clear_delay: Duration,
    ) -> Option<Transition> {
        match (self.state, condition) {
            (AlarmState::Off, _) => None,
            (AlarmState::Normal | AlarmState::Pending, Condition::Triggered) => {
                let since = *self.pending_since.get_or_insert(now);
                if now - since >= delay {
                    self.state = AlarmState::Active;
                    self.pending_since = None;
                    self.active_since = Some(now);
                    self.clear_since = None;
                    Some(Transition::Activated)
                } else {
                    self.state = AlarmState::Pending;
                    None
                }
            }
            (AlarmState::Normal | AlarmState::Pending, Condition::Clear | Condition::Unknown) => {
                self.state = AlarmState::Normal;
                self.pending_since = None;
                None
            }
            (AlarmState::Active | AlarmState::Acknowledged, Condition::Clear) => {
                let since = *self.clear_since.get_or_insert(now);
                if now - since >= clear_delay {
                    *self = Self::new(true);
                    Some(Transition::Resolved)
                } else {
                    None
                }
            }
            (AlarmState::Active | AlarmState::Acknowledged, _) => {
                self.clear_since = None;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000 + s, 0).expect("t")
    }

    const D: Duration = Duration::seconds(10);
    const C: Duration = Duration::seconds(3);

    #[test]
    fn activation_waits_for_the_delay() {
        let mut m = Machine::new(true);
        assert_eq!(m.update(Condition::Triggered, t(0), D, C), None);
        assert_eq!(m.state(), AlarmState::Pending);
        assert_eq!(m.update(Condition::Triggered, t(9), D, C), None);
        assert_eq!(
            m.update(Condition::Triggered, t(10), D, C),
            Some(Transition::Activated)
        );
        assert_eq!(m.state(), AlarmState::Active);
        assert_eq!(m.active_since(), Some(t(10)));
        // Une seule activation tant que la condition persiste.
        assert_eq!(m.update(Condition::Triggered, t(11), D, C), None);
    }

    #[test]
    fn interrupted_condition_restarts_the_delay() {
        let mut m = Machine::new(true);
        m.update(Condition::Triggered, t(0), D, C);
        m.update(Condition::Clear, t(5), D, C);
        assert_eq!(m.state(), AlarmState::Normal);
        m.update(Condition::Triggered, t(6), D, C);
        assert_eq!(m.update(Condition::Triggered, t(15), D, C), None);
        assert_eq!(
            m.update(Condition::Triggered, t(16), D, C),
            Some(Transition::Activated)
        );
    }

    #[test]
    fn zero_delay_activates_immediately() {
        let mut m = Machine::new(true);
        assert_eq!(
            m.update(Condition::Triggered, t(0), Duration::zero(), C),
            Some(Transition::Activated)
        );
    }

    #[test]
    fn acknowledgement_silences_but_keeps_the_alarm() {
        let mut m = Machine::new(true);
        assert!(!m.acknowledge(), "rien à acquitter");
        m.update(Condition::Triggered, t(0), Duration::zero(), C);
        assert!(m.acknowledge());
        assert_eq!(m.state(), AlarmState::Acknowledged);
        assert!(m.is_alerting());
        assert!(!m.acknowledge(), "déjà acquittée");
        assert_eq!(m.update(Condition::Triggered, t(5), D, C), None);
        assert_eq!(m.state(), AlarmState::Acknowledged);
    }

    #[test]
    fn resolution_needs_a_steady_return() {
        let mut m = Machine::new(true);
        m.update(Condition::Triggered, t(0), Duration::zero(), C);
        assert_eq!(m.update(Condition::Clear, t(1), D, C), None);
        // Rechute : le délai de retour repart de zéro.
        m.update(Condition::Triggered, t(2), D, C);
        assert_eq!(m.update(Condition::Clear, t(3), D, C), None);
        assert_eq!(m.update(Condition::Clear, t(5), D, C), None);
        assert_eq!(
            m.update(Condition::Clear, t(6), D, C),
            Some(Transition::Resolved)
        );
        assert_eq!(m.state(), AlarmState::Normal);
        assert_eq!(m.active_since(), None);
    }

    #[test]
    fn unknown_data_never_resolves_nor_activates() {
        let mut m = Machine::new(true);
        m.update(Condition::Triggered, t(0), D, C);
        assert_eq!(m.update(Condition::Unknown, t(20), D, C), None);
        assert_eq!(m.state(), AlarmState::Normal, "temporisation interrompue");

        m.update(Condition::Triggered, t(30), Duration::zero(), C);
        m.update(Condition::Clear, t(31), D, C);
        for s in 32..100 {
            assert_eq!(m.update(Condition::Unknown, t(s), D, C), None);
        }
        assert_eq!(m.state(), AlarmState::Active);
        // Le délai de retour a été interrompu par la donnée inconnue.
        assert_eq!(m.update(Condition::Clear, t(100), D, C), None);
    }

    #[test]
    fn disabled_machine_ignores_conditions() {
        let mut m = Machine::new(false);
        assert_eq!(
            m.update(Condition::Triggered, t(0), Duration::zero(), C),
            None
        );
        assert_eq!(m.state(), AlarmState::Off);
        m.enable();
        assert_eq!(m.state(), AlarmState::Normal);
        m.update(Condition::Triggered, t(1), Duration::zero(), C);
        m.enable();
        assert_eq!(m.state(), AlarmState::Active, "enable sans effet");
        m.disable();
        assert_eq!(m.state(), AlarmState::Off);
        assert!(!m.is_alerting());
    }
}
