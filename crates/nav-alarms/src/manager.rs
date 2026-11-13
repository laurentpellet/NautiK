//! Gestionnaire des alarmes : évaluation périodique, commandes
//! (mouiller, lever, acquitter, activer), historique des événements.

use chrono::{DateTime, Duration, Utc};
use nav_core::InstrumentsSnapshot;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::machine::{AlarmState, Machine, Transition};
use crate::rules::{AnchorConfig, PositionLostConfig, CLEAR_DELAY_S, DEFAULT_ANCHOR_RADIUS_M};

/// Alarme gérée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlarmKind {
    /// Surveillance du mouillage (ALR-001).
    Anchor,
    /// Position perdue ou obsolète (ALR-002).
    PositionLost,
}

impl AlarmKind {
    /// Toutes les alarmes, dans l'ordre d'affichage.
    pub const ALL: [Self; 2] = [Self::Anchor, Self::PositionLost];

    /// Identifiant stable (table `alarms`).
    #[must_use]
    pub fn id(self) -> Uuid {
        match self {
            Self::Anchor => Uuid::from_u128(0x6e61_7574_696b_4000_8000_0000_0000_0001),
            Self::PositionLost => Uuid::from_u128(0x6e61_7574_696b_4000_8000_0000_0000_0002),
        }
    }

    /// Code stable (table `alarms`, colonne `alarm_type`).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Anchor => "anchor",
            Self::PositionLost => "positionLost",
        }
    }

    /// Alarme correspondant à un code.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.code() == code)
    }

    /// Libellé lisible.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Anchor => "Mouillage",
            Self::PositionLost => "Perte de position",
        }
    }
}

/// Nature d'un événement d'alarme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventKind {
    /// Alarme déclenchée.
    Activated,
    /// Alarme acquittée par l'utilisateur.
    Acknowledged,
    /// Retour à la normale.
    Resolved,
    /// Surveillance activée (mouillage : ancre posée).
    Enabled,
    /// Surveillance désactivée par l'utilisateur (mouillage : ancre levée).
    Disabled,
}

impl EventKind {
    /// Code stable (table `alarm_events`, colonne `event_type`).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Activated => "activated",
            Self::Acknowledged => "acknowledged",
            Self::Resolved => "resolved",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }

    /// Événement correspondant à un code.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        [
            Self::Activated,
            Self::Acknowledged,
            Self::Resolved,
            Self::Enabled,
            Self::Disabled,
        ]
        .into_iter()
        .find(|k| k.code() == code)
    }
}

/// Détails d'un événement.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventDetails {
    /// Message lisible.
    pub message: String,
    /// Distance à l'ancre (m), pour le mouillage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_m: Option<f64>,
    /// Données simulées ou rejouées au moment de l'événement.
    #[serde(default)]
    pub simulated: bool,
}

/// Événement d'alarme (historisé).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmEvent {
    /// Identifiant unique.
    pub id: Uuid,
    /// Alarme concernée.
    pub alarm: AlarmKind,
    /// Nature.
    pub kind: EventKind,
    /// Instant (UTC).
    pub at: DateTime<Utc>,
    /// Détails.
    pub details: EventDetails,
}

/// État d'une alarme, pour l'affichage.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmView {
    /// Alarme.
    pub kind: AlarmKind,
    /// Libellé.
    pub label: &'static str,
    /// État.
    pub state: AlarmState,
    /// Instant de déclenchement.
    pub since: Option<DateTime<Utc>>,
    /// Explication de l'état courant.
    pub message: String,
}

/// Mouillage surveillé, pour l'affichage.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorView {
    /// Configuration.
    #[serde(flatten)]
    pub config: AnchorConfig,
    /// Distance du bateau à l'ancre (m), si la position est fraîche.
    pub distance_m: Option<f64>,
    /// Relèvement de l'ancre vers le bateau (rad).
    pub bearing_rad: Option<f64>,
}

/// État de toutes les alarmes, diffusé périodiquement.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmsSnapshot {
    /// Instant de l'évaluation.
    pub computed_at: DateTime<Utc>,
    /// Alarmes, dans l'ordre de [`AlarmKind::ALL`].
    pub alarms: Vec<AlarmView>,
    /// Mouillage surveillé.
    pub anchor: Option<AnchorView>,
    /// Rayon proposé pour le prochain mouillage (dernier utilisé).
    pub anchor_radius_m: f64,
    /// Une alarme non acquittée doit être signalée par un son.
    pub sounding: bool,
    /// Données simulées ou rejouées.
    pub simulated: bool,
}

/// Configuration persistante des alarmes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlarmSettings {
    /// Mouillage surveillé (`None` : ancre levée).
    pub anchor: Option<AnchorConfig>,
    /// Dernier rayon utilisé (m).
    pub anchor_radius_m: f64,
    /// Alarme de perte de position active.
    pub position_lost_enabled: bool,
    /// Configuration de la perte de position.
    pub position_lost: PositionLostConfig,
}

impl Default for AlarmSettings {
    fn default() -> Self {
        Self {
            anchor: None,
            anchor_radius_m: DEFAULT_ANCHOR_RADIUS_M,
            position_lost_enabled: true,
            position_lost: PositionLostConfig::default(),
        }
    }
}

/// Gestionnaire des alarmes. Sans E/S : l'appelant persiste les
/// événements retournés.
#[derive(Debug, Clone)]
pub struct AlarmManager {
    settings: AlarmSettings,
    anchor: Machine,
    position_lost: Machine,
    /// Une position fraîche a été reçue depuis le choix de la source.
    armed: bool,
    distance_m: Option<f64>,
    bearing_rad: Option<f64>,
    simulated: bool,
}

impl AlarmManager {
    /// Gestionnaire restauré depuis une configuration.
    #[must_use]
    pub fn new(settings: AlarmSettings) -> Self {
        Self {
            anchor: Machine::new(settings.anchor.is_some()),
            position_lost: Machine::new(settings.position_lost_enabled),
            settings,
            armed: false,
            distance_m: None,
            bearing_rad: None,
            simulated: false,
        }
    }

    /// Configuration courante (à persister après chaque commande).
    #[must_use]
    pub fn settings(&self) -> AlarmSettings {
        self.settings
    }

    /// Les données viennent d'une autre source : l'alarme de perte de
    /// position attend une première position avant de surveiller (sauf
    /// au mouillage). Une alarme déjà déclenchée le reste jusqu'au
    /// retour d'une position fraîche.
    pub fn on_source_changed(&mut self, simulated: bool) {
        self.armed = false;
        self.simulated = simulated;
    }

    fn event(
        &self,
        alarm: AlarmKind,
        kind: EventKind,
        now: DateTime<Utc>,
        message: String,
    ) -> AlarmEvent {
        AlarmEvent {
            id: Uuid::new_v4(),
            alarm,
            kind,
            at: now,
            details: EventDetails {
                message,
                distance_m: match alarm {
                    AlarmKind::Anchor => self.distance_m,
                    AlarmKind::PositionLost => None,
                },
                simulated: self.simulated,
            },
        }
    }

    /// Pose l'ancre (remplace une surveillance en cours).
    pub fn drop_anchor(&mut self, config: AnchorConfig, now: DateTime<Utc>) -> AlarmEvent {
        self.settings.anchor = Some(config);
        self.settings.anchor_radius_m = config.radius_m;
        // Nouvelle surveillance : l'éventuelle alarme précédente est close.
        self.anchor = Machine::new(true);
        self.distance_m = None;
        self.bearing_rad = None;
        self.event(
            AlarmKind::Anchor,
            EventKind::Enabled,
            now,
            format!(
                "Ancre posée, rayon {:.0} m, temporisation {} s",
                config.radius_m, config.delay_s
            ),
        )
    }

    /// Lève l'ancre. `None` si aucun mouillage n'était surveillé.
    pub fn raise_anchor(&mut self, now: DateTime<Utc>) -> Option<AlarmEvent> {
        self.settings.anchor.take()?;
        let was_alerting = self.anchor.is_alerting();
        self.anchor.disable();
        let event = self.event(
            AlarmKind::Anchor,
            EventKind::Disabled,
            now,
            if was_alerting {
                "Ancre levée pendant une alarme de dérapage".to_owned()
            } else {
                "Ancre levée".to_owned()
            },
        );
        self.distance_m = None;
        self.bearing_rad = None;
        Some(event)
    }

    /// Active ou désactive l'alarme de perte de position. `None` si rien
    /// ne change.
    pub fn set_position_lost_enabled(
        &mut self,
        enabled: bool,
        now: DateTime<Utc>,
    ) -> Option<AlarmEvent> {
        if self.settings.position_lost_enabled == enabled {
            return None;
        }
        self.settings.position_lost_enabled = enabled;
        let message = if enabled {
            self.position_lost.enable();
            "Surveillance de la position activée".to_owned()
        } else {
            let was_alerting = self.position_lost.is_alerting();
            self.position_lost.disable();
            if was_alerting {
                "Surveillance de la position désactivée pendant une alarme".to_owned()
            } else {
                "Surveillance de la position désactivée".to_owned()
            }
        };
        let kind = if enabled {
            EventKind::Enabled
        } else {
            EventKind::Disabled
        };
        Some(self.event(AlarmKind::PositionLost, kind, now, message))
    }

    /// Acquitte une alarme active. `None` si elle ne l'était pas.
    pub fn acknowledge(&mut self, alarm: AlarmKind, now: DateTime<Utc>) -> Option<AlarmEvent> {
        let acknowledged = match alarm {
            AlarmKind::Anchor => self.anchor.acknowledge(),
            AlarmKind::PositionLost => self.position_lost.acknowledge(),
        };
        acknowledged.then(|| {
            self.event(
                alarm,
                EventKind::Acknowledged,
                now,
                "Alarme acquittée".to_owned(),
            )
        })
    }

    /// Évalue les règles sur l'état du bateau ; retourne les événements
    /// (déclenchements, résolutions) à historiser.
    pub fn evaluate(
        &mut self,
        snapshot: &InstrumentsSnapshot,
        now: DateTime<Utc>,
    ) -> Vec<AlarmEvent> {
        let mut events = Vec::new();
        let position = snapshot.position.as_ref();
        if position.is_some_and(|p| !p.stale) {
            self.armed = true;
        }
        let clear_delay = Duration::seconds(CLEAR_DELAY_S);

        // Perte de position.
        let condition =
            PositionLostConfig::condition(position, self.armed || self.settings.anchor.is_some());
        let delay = self.settings.position_lost.delay();
        match self
            .position_lost
            .update(condition, now, delay, clear_delay)
        {
            Some(Transition::Activated) => events.push(self.event(
                AlarmKind::PositionLost,
                EventKind::Activated,
                now,
                match position {
                    Some(_) => "Position obsolète : plus de relevé GPS récent".to_owned(),
                    None => "Aucune position reçue".to_owned(),
                },
            )),
            Some(Transition::Resolved) => events.push(self.event(
                AlarmKind::PositionLost,
                EventKind::Resolved,
                now,
                "Position rétablie".to_owned(),
            )),
            None => {}
        }

        // Mouillage.
        if let Some(anchor) = self.settings.anchor {
            let offset = anchor.offset(position);
            self.distance_m = offset.map(|(d, _)| d);
            self.bearing_rad = offset.and_then(|(_, b)| b);
            let condition = anchor.condition(self.distance_m, self.anchor.is_alerting());
            match self
                .anchor
                .update(condition, now, anchor.delay(), clear_delay)
            {
                Some(Transition::Activated) => events.push(self.event(
                    AlarmKind::Anchor,
                    EventKind::Activated,
                    now,
                    format!(
                        "Dérapage : {:.0} m de l'ancre (rayon {:.0} m)",
                        self.distance_m.unwrap_or(f64::NAN),
                        anchor.radius_m
                    ),
                )),
                Some(Transition::Resolved) => events.push(self.event(
                    AlarmKind::Anchor,
                    EventKind::Resolved,
                    now,
                    "Bateau revenu dans le cercle d'évitage".to_owned(),
                )),
                None => {}
            }
        }
        events
    }

    fn anchor_message(&self) -> String {
        let Some(anchor) = self.settings.anchor else {
            return "Ancre levée".to_owned();
        };
        let distance = match self.distance_m {
            Some(d) => format!("{d:.0} m de l'ancre"),
            None => "distance inconnue (position absente ou obsolète)".to_owned(),
        };
        match self.anchor.state() {
            AlarmState::Pending => format!("Hors du cercle, {distance} — confirmation en cours"),
            AlarmState::Active | AlarmState::Acknowledged => {
                format!("DÉRAPAGE — {distance}, rayon {:.0} m", anchor.radius_m)
            }
            AlarmState::Normal | AlarmState::Off => {
                format!("Surveillance : {distance}, rayon {:.0} m", anchor.radius_m)
            }
        }
    }

    fn position_message(&self) -> String {
        match self.position_lost.state() {
            AlarmState::Off => "Surveillance désactivée".to_owned(),
            AlarmState::Pending => "Position obsolète — confirmation en cours".to_owned(),
            AlarmState::Active | AlarmState::Acknowledged => {
                "POSITION PERDUE — ne pas se fier à la position affichée".to_owned()
            }
            AlarmState::Normal if self.armed || self.settings.anchor.is_some() => {
                "Position reçue".to_owned()
            }
            AlarmState::Normal => "En attente d'une première position".to_owned(),
        }
    }

    /// État des alarmes.
    #[must_use]
    pub fn snapshot(&self, now: DateTime<Utc>) -> AlarmsSnapshot {
        let alarms = vec![
            AlarmView {
                kind: AlarmKind::Anchor,
                label: AlarmKind::Anchor.label(),
                state: self.anchor.state(),
                since: self.anchor.active_since(),
                message: self.anchor_message(),
            },
            AlarmView {
                kind: AlarmKind::PositionLost,
                label: AlarmKind::PositionLost.label(),
                state: self.position_lost.state(),
                since: self.position_lost.active_since(),
                message: self.position_message(),
            },
        ];
        AlarmsSnapshot {
            computed_at: now,
            sounding: alarms.iter().any(|a| a.state == AlarmState::Active),
            alarms,
            anchor: self.settings.anchor.map(|config| AnchorView {
                config,
                distance_m: self.distance_m,
                bearing_rad: self.bearing_rad,
            }),
            anchor_radius_m: self.settings.anchor_radius_m,
            simulated: self.simulated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nav_core::{GeoPosition, PositionFix, PositionStatus, SourceKind};

    fn t(s: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000 + s, 0).expect("t")
    }

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    /// ~1,1 m par 1e-5° de latitude.
    fn snapshot(s: i64, north_m: Option<f64>, stale: bool) -> InstrumentsSnapshot {
        InstrumentsSnapshot {
            computed_at: t(s),
            position: north_m.map(|m| PositionStatus {
                fix: PositionFix {
                    position: p(m / 110_574.0, 0.0),
                    cog_rad: None,
                    sog_mps: None,
                    source: SourceKind::SignalK,
                    source_id: "gps".to_owned(),
                    measured_at: t(s),
                    received_at: t(s),
                },
                age_ms: 0,
                stale,
                fresh_sources: 1,
            }),
            readings: Vec::new(),
        }
    }

    fn kinds(events: &[AlarmEvent]) -> Vec<(AlarmKind, EventKind)> {
        events.iter().map(|e| (e.alarm, e.kind)).collect()
    }

    fn anchored(radius: f64, delay_s: u32) -> AlarmManager {
        let mut m = AlarmManager::new(AlarmSettings::default());
        let e = m.drop_anchor(
            AnchorConfig::new(p(0.0, 0.0), radius, delay_s).expect("cfg"),
            t(0),
        );
        assert_eq!((e.alarm, e.kind), (AlarmKind::Anchor, EventKind::Enabled));
        m
    }

    fn state(m: &AlarmManager, kind: AlarmKind) -> AlarmState {
        m.snapshot(t(0))
            .alarms
            .into_iter()
            .find(|a| a.kind == kind)
            .expect("alarme")
            .state
    }

    #[test]
    fn anchor_drag_triggers_after_delay_and_resolves_with_hysteresis() {
        let mut m = anchored(50.0, 10);
        assert!(m.evaluate(&snapshot(1, Some(20.0), false), t(1)).is_empty());
        assert!(m.evaluate(&snapshot(2, Some(60.0), false), t(2)).is_empty());
        assert_eq!(state(&m, AlarmKind::Anchor), AlarmState::Pending);
        let events = m.evaluate(&snapshot(12, Some(62.0), false), t(12));
        assert_eq!(kinds(&events), [(AlarmKind::Anchor, EventKind::Activated)]);
        assert!(events[0].details.message.contains("62 m"));
        assert_eq!(events[0].details.distance_m.map(f64::round), Some(62.0));
        let snap = m.snapshot(t(12));
        assert!(snap.sounding);
        assert_eq!(
            snap.anchor.and_then(|a| a.distance_m).map(f64::round),
            Some(62.0)
        );

        // 48 m : dans le cercle mais pas sous l'hystérésis (45 m).
        for s in 13..30 {
            assert!(m.evaluate(&snapshot(s, Some(48.0), false), t(s)).is_empty());
        }
        assert!(m
            .evaluate(&snapshot(30, Some(40.0), false), t(30))
            .is_empty());
        let events = m.evaluate(&snapshot(35, Some(40.0), false), t(35));
        assert_eq!(kinds(&events), [(AlarmKind::Anchor, EventKind::Resolved)]);
        assert!(!m.snapshot(t(35)).sounding);
    }

    #[test]
    fn short_excursion_does_not_trigger() {
        let mut m = anchored(50.0, 10);
        m.evaluate(&snapshot(1, Some(70.0), false), t(1));
        m.evaluate(&snapshot(5, Some(30.0), false), t(5));
        assert!(m
            .evaluate(&snapshot(20, Some(30.0), false), t(20))
            .is_empty());
        assert_eq!(state(&m, AlarmKind::Anchor), AlarmState::Normal);
    }

    #[test]
    fn acknowledged_alarm_is_silent_but_visible() {
        let mut m = anchored(50.0, 0);
        m.evaluate(&snapshot(1, Some(80.0), false), t(1));
        assert!(m.acknowledge(AlarmKind::PositionLost, t(2)).is_none());
        let e = m.acknowledge(AlarmKind::Anchor, t(2)).expect("acquit");
        assert_eq!(e.kind, EventKind::Acknowledged);
        let snap = m.snapshot(t(2));
        assert!(!snap.sounding);
        assert_eq!(snap.alarms[0].state, AlarmState::Acknowledged);
        assert!(snap.alarms[0].message.contains("DÉRAPAGE"));
    }

    #[test]
    fn lost_position_while_anchored_raises_position_alarm_not_resolution() {
        let mut m = anchored(50.0, 0);
        m.evaluate(&snapshot(1, Some(80.0), false), t(1));
        // Position obsolète : le mouillage reste en alarme, la perte de
        // position se déclenche après sa temporisation.
        let mut events = Vec::new();
        for s in 2..20 {
            events.extend(m.evaluate(&snapshot(s, Some(10.0), true), t(s)));
        }
        assert_eq!(
            kinds(&events),
            [(AlarmKind::PositionLost, EventKind::Activated)]
        );
        assert_eq!(state(&m, AlarmKind::Anchor), AlarmState::Active);
        let snap = m.snapshot(t(20));
        assert_eq!(snap.anchor.and_then(|a| a.distance_m), None);
        assert!(snap.alarms[0].message.contains("distance inconnue"));

        // Retour de la position, de nouveau au mouillage.
        let mut events = Vec::new();
        for s in 20..30 {
            events.extend(m.evaluate(&snapshot(s, Some(10.0), false), t(s)));
        }
        assert_eq!(
            kinds(&events),
            [
                (AlarmKind::PositionLost, EventKind::Resolved),
                (AlarmKind::Anchor, EventKind::Resolved)
            ]
        );
    }

    #[test]
    fn position_alarm_waits_for_a_first_fix_unless_anchored() {
        let mut m = AlarmManager::new(AlarmSettings::default());
        for s in 0..60 {
            assert!(m.evaluate(&snapshot(s, None, false), t(s)).is_empty());
        }
        assert!(m.snapshot(t(60)).alarms[1].message.contains("En attente"));
        m.evaluate(&snapshot(60, Some(0.0), false), t(60));
        let mut events = Vec::new();
        for s in 61..70 {
            events.extend(m.evaluate(&snapshot(s, Some(0.0), true), t(s)));
        }
        assert_eq!(
            kinds(&events),
            [(AlarmKind::PositionLost, EventKind::Activated)]
        );

        // Au mouillage, l'absence totale de position est une alarme.
        let mut m = anchored(50.0, 10);
        let mut events = Vec::new();
        for s in 0..10 {
            events.extend(m.evaluate(&snapshot(s, None, false), t(s)));
        }
        assert_eq!(
            kinds(&events),
            [(AlarmKind::PositionLost, EventKind::Activated)]
        );
    }

    #[test]
    fn source_change_keeps_an_active_alarm_until_fresh_data() {
        let mut m = AlarmManager::new(AlarmSettings::default());
        m.evaluate(&snapshot(0, Some(0.0), false), t(0));
        for s in 1..10 {
            m.evaluate(&snapshot(s, Some(0.0), true), t(s));
        }
        assert_eq!(state(&m, AlarmKind::PositionLost), AlarmState::Active);
        m.on_source_changed(false);
        for s in 10..100 {
            assert!(m.evaluate(&snapshot(s, None, false), t(s)).is_empty());
        }
        assert_eq!(state(&m, AlarmKind::PositionLost), AlarmState::Active);
    }

    #[test]
    fn disabling_is_explicit_and_logged() {
        let mut m = AlarmManager::new(AlarmSettings::default());
        assert!(m.set_position_lost_enabled(true, t(0)).is_none());
        let e = m.set_position_lost_enabled(false, t(0)).expect("événement");
        assert_eq!(e.kind, EventKind::Disabled);
        assert!(!m.settings().position_lost_enabled);
        assert_eq!(state(&m, AlarmKind::PositionLost), AlarmState::Off);
        m.evaluate(&snapshot(0, Some(0.0), false), t(0));
        for s in 1..30 {
            assert!(m.evaluate(&snapshot(s, None, false), t(s)).is_empty());
        }
        let e = m.set_position_lost_enabled(true, t(30)).expect("événement");
        assert_eq!(e.kind, EventKind::Enabled);
    }

    #[test]
    fn raising_the_anchor_is_logged_and_remembers_the_radius() {
        let mut m = anchored(80.0, 0);
        m.evaluate(&snapshot(1, Some(100.0), false), t(1));
        let e = m.raise_anchor(t(2)).expect("événement");
        assert_eq!(e.kind, EventKind::Disabled);
        assert!(e.details.message.contains("pendant une alarme"));
        assert!(m.raise_anchor(t(3)).is_none());
        let snap = m.snapshot(t(3));
        assert!(snap.anchor.is_none());
        assert!(!snap.sounding);
        assert_eq!(snap.anchor_radius_m, 80.0);
        assert_eq!(snap.alarms[0].state, AlarmState::Off);
    }

    #[test]
    fn restart_restores_the_anchor_watch() {
        let mut m = anchored(50.0, 0);
        let settings = m.settings();
        m.evaluate(&snapshot(1, Some(80.0), false), t(1));
        // Redémarrage : la surveillance reprend, l'alarme est réévaluée.
        let mut restarted = AlarmManager::new(settings);
        let events = restarted.evaluate(&snapshot(100, Some(80.0), false), t(100));
        assert_eq!(kinds(&events), [(AlarmKind::Anchor, EventKind::Activated)]);
    }

    #[test]
    fn codes_round_trip() {
        for k in AlarmKind::ALL {
            assert_eq!(AlarmKind::from_code(k.code()), Some(k));
        }
        assert_ne!(AlarmKind::Anchor.id(), AlarmKind::PositionLost.id());
        for k in [
            EventKind::Activated,
            EventKind::Acknowledged,
            EventKind::Resolved,
            EventKind::Enabled,
            EventKind::Disabled,
        ] {
            assert_eq!(EventKind::from_code(k.code()), Some(k));
        }
        assert_eq!(AlarmKind::from_code("x"), None);
        assert_eq!(EventKind::from_code(""), None);
    }

    #[test]
    fn simulated_flag_follows_the_source() {
        let mut m = anchored(50.0, 0);
        m.on_source_changed(true);
        m.evaluate(&snapshot(1, Some(0.0), false), t(1));
        let events = m.evaluate(&snapshot(2, Some(90.0), false), t(2));
        assert!(events[0].details.simulated);
        assert!(m.snapshot(t(2)).simulated);
    }
}
