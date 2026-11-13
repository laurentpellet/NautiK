//! Mesures normalisées des instruments.
//!
//! Toute mesure porte sa valeur en unité SI, sa source et ses deux
//! horodatages (mesure et réception). Les valeurs sont validées et
//! normalisées à la construction : une valeur hors domaine est rejetée,
//! jamais corrigée en silence.

use std::f64::consts::{PI, TAU};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::SourceKind;

/// Grandeur mesurée (chemins Signal K prioritaires, SPEC SIG-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Quantity {
    /// Vitesse fond (m/s).
    SpeedOverGround,
    /// Route fond vraie (rad, [0, 2π)).
    CourseOverGroundTrue,
    /// Cap vrai (rad, [0, 2π)).
    HeadingTrue,
    /// Cap magnétique (rad, [0, 2π)).
    HeadingMagnetic,
    /// Vitesse surface (m/s).
    SpeedThroughWater,
    /// Profondeur sous le transducteur (m).
    DepthBelowTransducer,
    /// Vitesse du vent apparent (m/s).
    WindSpeedApparent,
    /// Angle du vent apparent (rad, ]-π, π], positif à tribord).
    WindAngleApparent,
    /// Vitesse du vent réel (m/s).
    WindSpeedTrue,
    /// Direction d'où vient le vent réel, vraie (rad, [0, 2π)).
    WindDirectionTrue,
    /// Température de l'eau (K).
    WaterTemperature,
    /// Température de l'air extérieur (K).
    AirTemperature,
    /// Pression atmosphérique (Pa).
    AtmosphericPressure,
    /// Régime moteur (Hz, tours par seconde).
    EngineSpeed,
    /// Température du liquide de refroidissement moteur (K).
    EngineCoolantTemperature,
    /// Pression d'huile moteur (Pa).
    EngineOilPressure,
    /// Niveau du réservoir de carburant (rapport, [0, 1]).
    FuelLevel,
    /// Niveau du réservoir d'eau douce (rapport, [0, 1]).
    FreshWaterLevel,
    /// Tension de la batterie de service (V).
    BatteryVoltage,
    /// Courant de la batterie de service (A, positif en charge).
    BatteryCurrent,
    /// État de charge de la batterie de service (rapport, [0, 1]).
    BatteryStateOfCharge,
    /// Puissance produite par les panneaux solaires (W).
    SolarPower,
}

impl Quantity {
    /// Toutes les grandeurs, dans l'ordre d'affichage.
    pub const ALL: [Self; 22] = [
        Self::SpeedOverGround,
        Self::CourseOverGroundTrue,
        Self::HeadingTrue,
        Self::HeadingMagnetic,
        Self::SpeedThroughWater,
        Self::DepthBelowTransducer,
        Self::WindSpeedApparent,
        Self::WindAngleApparent,
        Self::WindSpeedTrue,
        Self::WindDirectionTrue,
        Self::WaterTemperature,
        Self::AirTemperature,
        Self::AtmosphericPressure,
        Self::EngineSpeed,
        Self::EngineCoolantTemperature,
        Self::EngineOilPressure,
        Self::FuelLevel,
        Self::FreshWaterLevel,
        Self::BatteryVoltage,
        Self::BatteryCurrent,
        Self::BatteryStateOfCharge,
        Self::SolarPower,
    ];

    /// Âge au-delà duquel la mesure est obsolète.
    #[must_use]
    pub fn stale_after(self) -> Duration {
        match self {
            Self::DepthBelowTransducer
            | Self::WindSpeedApparent
            | Self::WindAngleApparent
            | Self::WindSpeedTrue
            | Self::WindDirectionTrue
            | Self::BatteryVoltage
            | Self::BatteryCurrent
            | Self::BatteryStateOfCharge => Duration::from_secs(10),
            Self::WaterTemperature
            | Self::AirTemperature
            | Self::AtmosphericPressure
            | Self::SolarPower => Duration::from_secs(30),
            Self::FuelLevel | Self::FreshWaterLevel => Duration::from_secs(60),
            _ => Duration::from_secs(5),
        }
    }

    /// Libellé français (journaux, messages d'erreur).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::SpeedOverGround => "vitesse fond",
            Self::CourseOverGroundTrue => "route fond",
            Self::HeadingTrue => "cap vrai",
            Self::HeadingMagnetic => "cap magnétique",
            Self::SpeedThroughWater => "vitesse surface",
            Self::DepthBelowTransducer => "profondeur",
            Self::WindSpeedApparent => "vitesse du vent apparent",
            Self::WindAngleApparent => "angle du vent apparent",
            Self::WindSpeedTrue => "vitesse du vent réel",
            Self::WindDirectionTrue => "direction du vent réel",
            Self::WaterTemperature => "température de l'eau",
            Self::AirTemperature => "température de l'air",
            Self::AtmosphericPressure => "pression atmosphérique",
            Self::EngineSpeed => "régime moteur",
            Self::EngineCoolantTemperature => "température moteur",
            Self::EngineOilPressure => "pression d'huile",
            Self::FuelLevel => "niveau de carburant",
            Self::FreshWaterLevel => "niveau d'eau douce",
            Self::BatteryVoltage => "tension batterie",
            Self::BatteryCurrent => "courant batterie",
            Self::BatteryStateOfCharge => "charge batterie",
            Self::SolarPower => "puissance solaire",
        }
    }

    /// Domaine physique admis pour les grandeurs scalaires (non angulaires).
    fn range(self) -> Option<(f64, f64)> {
        Some(match self {
            Self::SpeedOverGround
            | Self::SpeedThroughWater
            | Self::WindSpeedApparent
            | Self::WindSpeedTrue => (0.0, 200.0),
            Self::DepthBelowTransducer => (0.0, 12_000.0),
            // −73 °C à +77 °C.
            Self::WaterTemperature | Self::AirTemperature => (200.0, 350.0),
            Self::AtmosphericPressure => (80_000.0, 110_000.0),
            // 12 000 tr/min au plus.
            Self::EngineSpeed => (0.0, 200.0),
            Self::EngineCoolantTemperature => (200.0, 450.0),
            Self::EngineOilPressure => (0.0, 2_000_000.0),
            Self::FuelLevel | Self::FreshWaterLevel | Self::BatteryStateOfCharge => (0.0, 1.0),
            Self::BatteryVoltage => (0.0, 100.0),
            Self::BatteryCurrent => (-2_000.0, 2_000.0),
            Self::SolarPower => (0.0, 20_000.0),
            Self::CourseOverGroundTrue
            | Self::HeadingTrue
            | Self::HeadingMagnetic
            | Self::WindDirectionTrue
            | Self::WindAngleApparent => return None,
        })
    }

    /// Valide et normalise une valeur SI pour cette grandeur.
    ///
    /// # Errors
    ///
    /// [`MeasurementError`] si la valeur est non finie ou hors domaine.
    pub fn normalize(self, value: f64) -> Result<f64, MeasurementError> {
        if !value.is_finite() {
            return Err(MeasurementError::NotFinite(self));
        }
        if let Some((min, max)) = self.range() {
            // Ex. vitesse négative ou > 200 m/s : aberrante, rejetée.
            return if (min..=max).contains(&value) {
                Ok(value)
            } else {
                Err(MeasurementError::OutOfRange(self, value))
            };
        }
        match self {
            Self::CourseOverGroundTrue
            | Self::HeadingTrue
            | Self::HeadingMagnetic
            | Self::WindDirectionTrue => {
                if value.abs() > 4.0 * PI {
                    return Err(MeasurementError::OutOfRange(self, value));
                }
                Ok(value.rem_euclid(TAU))
            }
            Self::WindAngleApparent => {
                if value.abs() > 4.0 * PI {
                    return Err(MeasurementError::OutOfRange(self, value));
                }
                let wrapped = value.rem_euclid(TAU);
                Ok(if wrapped > PI { wrapped - TAU } else { wrapped })
            }
            // Grandeurs scalaires : traitées par `range` ci-dessus.
            _ => Err(MeasurementError::OutOfRange(self, value)),
        }
    }
}

/// Valeur rejetée.
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum MeasurementError {
    /// NaN ou infini.
    #[error("{0:?} : valeur non finie")]
    NotFinite(Quantity),
    /// Hors du domaine physique attendu.
    #[error("{0:?} : valeur hors domaine ({1})")]
    OutOfRange(Quantity, f64),
}

/// Mesure normalisée.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    /// Grandeur.
    pub quantity: Quantity,
    /// Valeur SI normalisée.
    pub value: f64,
    /// Nature de la source.
    pub source: SourceKind,
    /// Identifiant de la source (ex. `nmea0183.GP` côté Signal K).
    pub source_id: String,
    /// Horodatage de la mesure (UTC), tel que fourni par la source.
    pub measured_at: DateTime<Utc>,
    /// Horodatage de réception par NautiK (UTC, horloge locale).
    pub received_at: DateTime<Utc>,
}

impl Measurement {
    /// Construit une mesure en validant la valeur.
    ///
    /// # Errors
    ///
    /// [`MeasurementError`] si la valeur est invalide.
    pub fn new(
        quantity: Quantity,
        value: f64,
        source: SourceKind,
        source_id: impl Into<String>,
        measured_at: DateTime<Utc>,
        received_at: DateTime<Utc>,
    ) -> Result<Self, MeasurementError> {
        Ok(Self {
            quantity,
            value: quantity.normalize(value)?,
            source,
            source_id: source_id.into(),
            measured_at,
            received_at,
        })
    }
}

/// Âge d'une donnée à `now`, mesuré sur l'horloge locale (réception) :
/// une horloge de source décalée ne peut pas masquer une perte de flux.
#[must_use]
pub fn age(received_at: DateTime<Utc>, now: DateTime<Utc>) -> Duration {
    (now - received_at).to_std().unwrap_or(Duration::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_are_wrapped() {
        let cog = Quantity::CourseOverGroundTrue;
        assert!((cog.normalize(-FRAC).expect("ok") - (TAU - FRAC)).abs() < 1e-12);
        assert!((cog.normalize(TAU + 0.5).expect("ok") - 0.5).abs() < 1e-12);
        assert_eq!(cog.normalize(TAU).expect("ok"), 0.0);
        let awa = Quantity::WindAngleApparent;
        assert!((awa.normalize(3.0 * PI / 2.0).expect("ok") + PI / 2.0).abs() < 1e-12);
        assert!((awa.normalize(-PI / 4.0).expect("ok") + PI / 4.0).abs() < 1e-12);
        assert!((awa.normalize(PI).expect("ok") - PI).abs() < 1e-12);
    }

    const FRAC: f64 = 0.25;

    #[test]
    fn rejects_invalid_values() {
        for q in Quantity::ALL {
            assert_eq!(q.normalize(f64::NAN), Err(MeasurementError::NotFinite(q)));
            assert_eq!(
                q.normalize(f64::INFINITY),
                Err(MeasurementError::NotFinite(q))
            );
        }
        assert!(Quantity::SpeedOverGround.normalize(-0.1).is_err());
        assert!(Quantity::SpeedOverGround.normalize(250.0).is_err());
        assert!(Quantity::DepthBelowTransducer.normalize(-1.0).is_err());
        assert!(
            Quantity::HeadingTrue.normalize(100.0).is_err(),
            "degrés envoyés pour des radians"
        );
        assert_eq!(Quantity::DepthBelowTransducer.normalize(0.0), Ok(0.0));
    }

    #[test]
    fn validates_environment_engine_and_energy_values() {
        // Celsius envoyés pour des kelvins : rejetés.
        assert!(Quantity::WaterTemperature.normalize(27.0).is_err());
        assert_eq!(Quantity::WaterTemperature.normalize(300.15), Ok(300.15));
        // hPa envoyés pour des Pa : rejetés.
        assert!(Quantity::AtmosphericPressure.normalize(1013.0).is_err());
        assert!(Quantity::AtmosphericPressure.normalize(101_300.0).is_ok());
        // tr/min envoyés pour des Hz : rejetés au-delà de 12 000 tr/min.
        assert!(Quantity::EngineSpeed.normalize(2_400.0).is_err());
        assert_eq!(Quantity::EngineSpeed.normalize(40.0), Ok(40.0));
        // Pourcentages envoyés pour des rapports : rejetés.
        assert!(Quantity::FuelLevel.normalize(84.0).is_err());
        assert_eq!(Quantity::FuelLevel.normalize(1.0), Ok(1.0));
        assert!(Quantity::BatteryStateOfCharge.normalize(-0.01).is_err());
        assert_eq!(Quantity::BatteryCurrent.normalize(-35.0), Ok(-35.0));
        assert!(Quantity::BatteryVoltage.normalize(-1.0).is_err());
        assert!(Quantity::SolarPower.normalize(-5.0).is_err());
        let twd = Quantity::WindDirectionTrue.normalize(-FRAC).expect("ok");
        assert!((twd - (TAU - FRAC)).abs() < 1e-12);
    }

    #[test]
    fn every_quantity_has_a_freshness_threshold() {
        for q in Quantity::ALL {
            assert!(q.stale_after() >= Duration::from_secs(5));
        }
        assert_eq!(Quantity::FuelLevel.stale_after(), Duration::from_secs(60));
    }

    #[test]
    fn age_uses_local_reception_clock() {
        let t0 = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t");
        assert_eq!(
            age(t0, t0 + chrono::Duration::seconds(7)),
            Duration::from_secs(7)
        );
        assert_eq!(age(t0, t0 - chrono::Duration::seconds(7)), Duration::ZERO);
    }

    #[test]
    fn serializes_camel_case() {
        let t = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t");
        let m = Measurement::new(
            Quantity::DepthBelowTransducer,
            12.5,
            SourceKind::SignalK,
            "n2k.115",
            t,
            t,
        )
        .expect("m");
        let json = serde_json::to_value(&m).expect("json");
        assert_eq!(json["quantity"], "depthBelowTransducer");
        assert_eq!(json["sourceId"], "n2k.115");
    }
}
