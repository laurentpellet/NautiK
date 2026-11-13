//! Instruments simulés (développement et démonstrations).
//!
//! Complète la position simulée par des mesures cohérentes entre elles :
//! cap, vitesse surface, sonde, vent réel et apparent (calculé à partir
//! du déplacement du bateau), environnement, moteur, réservoirs et
//! énergie. Les variations sont déterministes (fonctions du temps), sans
//! générateur aléatoire. Chaque mesure est marquée
//! [`SourceKind::Simulator`] et ne doit jamais être présentée comme réelle.

use std::f64::consts::{PI, TAU};

use chrono::{DateTime, Timelike, Utc};

use crate::measurement::{Measurement, Quantity};
use crate::position::{PositionFix, SourceKind};
use crate::simulation::SIMULATOR_SOURCE_ID;

/// Pas de temps maximal pris en compte (s).
const MAX_STEP_S: f64 = 10.0;
/// Zéro absolu (K).
const KELVIN: f64 = 273.15;
/// Déclinaison magnétique simulée (rad, positive à l'est).
const DECLINATION_RAD: f64 = 12.0 * PI / 180.0;
/// Capacités simulées.
const FUEL_TANK_L: f64 = 200.0;
const WATER_TANK_L: f64 = 300.0;
const BATTERY_AH: f64 = 400.0;
/// Vitesse fond au-delà de laquelle le moteur tourne (m/s).
const UNDERWAY_MPS: f64 = 0.2;
const KNOTS_PER_MPS: f64 = 3600.0 / 1852.0;

/// Simulateur d'instruments.
#[derive(Debug, Clone)]
pub struct InstrumentSimulator {
    start: Option<DateTime<Utc>>,
    last: Option<DateTime<Utc>>,
    heading_rad: f64,
    coolant_c: f64,
    fuel: f64,
    water: f64,
    soc: f64,
}

impl Default for InstrumentSimulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Angle ramené dans ]-π, π].
fn signed_angle(a: f64) -> f64 {
    let w = a.rem_euclid(TAU);
    if w > PI {
        w - TAU
    } else {
        w
    }
}

/// Vent apparent `(vitesse m/s, angle rad ]-π, π])` à partir du vent
/// réel (vitesse, direction d'où il vient), de la vitesse fond, de la
/// route fond et du cap du bateau.
#[must_use]
pub fn apparent_wind(tws: f64, twd: f64, sog: f64, cog: f64, heading: f64) -> (f64, f64) {
    // Vecteurs (est, nord) : vent « vers », bateau.
    let (wx, wy) = (-tws * twd.sin(), -tws * twd.cos());
    let (bx, by) = (sog * cog.sin(), sog * cog.cos());
    let (rx, ry) = (wx - bx, wy - by);
    let speed = rx.hypot(ry);
    if speed < 1e-9 {
        return (0.0, 0.0);
    }
    let from = (-rx).atan2(-ry);
    (speed, signed_angle(from - heading))
}

impl InstrumentSimulator {
    /// Crée le simulateur (réservoirs et batterie partiellement remplis).
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: None,
            last: None,
            heading_rad: 0.8,
            coolant_c: 28.0,
            fuel: 0.78,
            water: 0.64,
            soc: 0.84,
        }
    }

    /// Mesures simulées à l'instant du relevé `fix` (position simulée).
    pub fn step(&mut self, fix: &PositionFix) -> Vec<Measurement> {
        let now = fix.measured_at;
        let start = *self.start.get_or_insert(now);
        let s = (now - start).num_milliseconds() as f64 / 1000.0;
        let dt = self
            .last
            .map(|last| ((now - last).num_milliseconds() as f64 / 1000.0).clamp(0.0, MAX_STEP_S))
            .unwrap_or(0.0);
        self.last = Some(now);

        let sog = fix.sog_mps.unwrap_or(0.0).max(0.0);
        let underway = sog > UNDERWAY_MPS;

        // Cap : route fond + dérive en route ; léger évitage au mouillage.
        self.heading_rad = match fix.cog_rad {
            Some(cog) if underway => (cog + 0.05 * (s / 50.0).sin()).rem_euclid(TAU),
            _ => (self.heading_rad + 0.002 * (s / 20.0).sin()).rem_euclid(TAU),
        };
        let heading = self.heading_rad;
        let cog = fix.cog_rad.unwrap_or(heading);

        // Vent réel : alizé d'est-sud-est, rafales.
        let tws = (7.5 + 1.5 * (s / 600.0).sin() + 0.8 * (s / 17.0).sin()).max(0.5);
        let twd = (110.0_f64.to_radians()
            + 8.0_f64.to_radians() * (s / 900.0).sin()
            + 3.0_f64.to_radians() * (s / 43.0).sin())
        .rem_euclid(TAU);
        let (aws, awa) = apparent_wind(tws, twd, sog, cog, heading);

        // Soleil : hauteur approchée par l'heure solaire locale.
        let utc_hours = f64::from(now.num_seconds_from_midnight()) / 3600.0;
        let solar_hour = (utc_hours + fix.position.longitude_deg() / 15.0).rem_euclid(24.0);
        let sun = (PI * (solar_hour - 6.0) / 12.0).sin().max(0.0);
        let solar_w = 420.0 * sun * (0.85 + 0.15 * (s / 90.0).sin());

        // Moteur.
        let rpm = if underway {
            (900.0 + sog * KNOTS_PER_MPS * 230.0).min(3200.0)
        } else {
            0.0
        };
        let coolant_target = if underway { 82.0 } else { 28.0 };
        self.coolant_c += (coolant_target - self.coolant_c) * (dt / 120.0).min(1.0);
        let oil_pa = if underway {
            320_000.0 + 30_000.0 * (rpm - 900.0) / 2000.0
        } else {
            0.0
        };

        // Réservoirs.
        let fuel_lph = if underway {
            0.5 + 3.5 * (rpm / 3000.0).powi(3)
        } else {
            0.0
        };
        self.fuel = (self.fuel - fuel_lph * dt / 3600.0 / FUEL_TANK_L).max(0.0);
        self.water = (self.water - 2.0 * dt / 3600.0 / WATER_TANK_L).max(0.0);

        // Batterie : solaire + alternateur − consommation du bord.
        let alternator_a = if underway { 25.0 } else { 0.0 };
        let load_a = 6.0 + 0.3 * (s / 13.0).sin();
        let current = solar_w / 13.2 + alternator_a - load_a;
        self.soc = (self.soc + current * dt / 3600.0 / BATTERY_AH).clamp(0.05, 1.0);
        let voltage = (12.0 + 1.2 * self.soc + 0.012 * current).clamp(11.0, 14.6);

        let values = [
            (Quantity::HeadingTrue, heading),
            (Quantity::HeadingMagnetic, heading - DECLINATION_RAD),
            (
                Quantity::SpeedThroughWater,
                sog * (1.0 + 0.04 * (s / 120.0).sin()),
            ),
            (
                Quantity::DepthBelowTransducer,
                30.0 + 18.0 * (s / 240.0).sin() + 4.0 * (s / 41.0).sin(),
            ),
            (Quantity::WindSpeedApparent, aws),
            (Quantity::WindAngleApparent, awa),
            (Quantity::WindSpeedTrue, tws),
            (Quantity::WindDirectionTrue, twd),
            (
                Quantity::WaterTemperature,
                KELVIN + 27.2 + 0.3 * (s / 1800.0).sin(),
            ),
            (
                Quantity::AirTemperature,
                KELVIN + 25.5 + 3.0 * sun + 0.3 * (s / 300.0).sin(),
            ),
            (
                Quantity::AtmosphericPressure,
                101_300.0 + 120.0 * (TAU * solar_hour / 12.0).cos() + 30.0 * (s / 600.0).sin(),
            ),
            (Quantity::EngineSpeed, rpm / 60.0),
            (Quantity::EngineCoolantTemperature, KELVIN + self.coolant_c),
            (Quantity::EngineOilPressure, oil_pa),
            (Quantity::FuelLevel, self.fuel),
            (Quantity::FreshWaterLevel, self.water),
            (Quantity::BatteryVoltage, voltage),
            (Quantity::BatteryCurrent, current),
            (Quantity::BatteryStateOfCharge, self.soc),
            (Quantity::SolarPower, solar_w),
        ];
        values
            .into_iter()
            .filter_map(|(quantity, value)| {
                Measurement::new(
                    quantity,
                    value,
                    SourceKind::Simulator,
                    SIMULATOR_SOURCE_ID,
                    now,
                    now,
                )
                .ok()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GeoPosition;
    use chrono::Duration;

    fn fix(seconds: i64, sog: f64, cog: Option<f64>) -> PositionFix {
        let t = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t")
            + Duration::seconds(seconds);
        PositionFix {
            position: GeoPosition::new(-14.97, -147.64).expect("pos"),
            cog_rad: cog,
            sog_mps: Some(sog),
            source: SourceKind::Simulator,
            source_id: SIMULATOR_SOURCE_ID.to_owned(),
            measured_at: t,
            received_at: t,
        }
    }

    fn value(ms: &[Measurement], q: Quantity) -> f64 {
        ms.iter().find(|m| m.quantity == q).expect("mesure").value
    }

    #[test]
    fn produces_every_instrument_quantity_marked_simulated() {
        let ms = InstrumentSimulator::new().step(&fix(0, 4.0, Some(1.0)));
        assert_eq!(ms.len(), 20, "toutes les mesures sont valides");
        assert!(ms.iter().all(|m| m.source.is_simulated()));
        // La position, la vitesse et la route fond viennent du relevé.
        assert!(ms.iter().all(|m| m.quantity != Quantity::SpeedOverGround
            && m.quantity != Quantity::CourseOverGroundTrue));
    }

    #[test]
    fn apparent_wind_follows_boat_motion() {
        // Sans vent réel, le vent apparent vient de l'avant à la vitesse du bateau.
        let (aws, awa) = apparent_wind(0.0, 0.0, 5.0, 0.0, 0.0);
        assert!((aws - 5.0).abs() < 1e-9);
        assert!(awa.abs() < 1e-9);
        // Vent de travers tribord, bateau arrêté.
        let (aws, awa) = apparent_wind(6.0, PI / 2.0, 0.0, 0.0, 0.0);
        assert!((aws - 6.0).abs() < 1e-9);
        assert!((awa - PI / 2.0).abs() < 1e-9);
        // Au près, le vent apparent forcit et refuse.
        let (aws, awa) = apparent_wind(6.0, PI / 3.0, 3.0, 0.0, 0.0);
        assert!(aws > 6.0);
        assert!(awa > 0.0 && awa < PI / 3.0);
        assert_eq!(apparent_wind(0.0, 0.0, 0.0, 0.0, 0.0), (0.0, 0.0));
    }

    #[test]
    fn engine_runs_only_underway_and_consumes_fuel() {
        let mut sim = InstrumentSimulator::new();
        let at_anchor = sim.step(&fix(0, 0.0, None));
        assert_eq!(value(&at_anchor, Quantity::EngineSpeed), 0.0);
        assert_eq!(value(&at_anchor, Quantity::EngineOilPressure), 0.0);
        let fuel0 = value(&at_anchor, Quantity::FuelLevel);
        let mut last = Vec::new();
        for s in (1..=600).step_by(5) {
            last = sim.step(&fix(s, 4.0, Some(1.0)));
        }
        let rpm = value(&last, Quantity::EngineSpeed) * 60.0;
        assert!((2_000.0..=3_200.0).contains(&rpm), "{rpm}");
        assert!(value(&last, Quantity::FuelLevel) < fuel0);
        let coolant_c = value(&last, Quantity::EngineCoolantTemperature) - KELVIN;
        assert!(coolant_c > 70.0, "moteur chaud : {coolant_c}");
        // Alternateur : la batterie charge.
        assert!(value(&last, Quantity::BatteryCurrent) > 0.0);
    }

    #[test]
    fn heading_follows_course_underway() {
        let mut sim = InstrumentSimulator::new();
        let ms = sim.step(&fix(0, 4.0, Some(2.0)));
        assert!((value(&ms, Quantity::HeadingTrue) - 2.0).abs() < 0.06);
        let magnetic = value(&ms, Quantity::HeadingMagnetic);
        assert!((value(&ms, Quantity::HeadingTrue) - magnetic - DECLINATION_RAD).abs() < 1e-9);
    }

    #[test]
    fn long_pause_does_not_jump_levels() {
        let mut sim = InstrumentSimulator::new();
        let a = sim.step(&fix(0, 4.0, Some(1.0)));
        let b = sim.step(&fix(86_400, 4.0, Some(1.0)));
        let drop = value(&a, Quantity::FuelLevel) - value(&b, Quantity::FuelLevel);
        assert!(drop < 0.001, "pas de saut après une suspension : {drop}");
    }
}
