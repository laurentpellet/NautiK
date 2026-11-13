//! Flux AIS simulé : quelques navires fictifs autour d'un point,
//! émis sous forme de phrases `!AIVDM` complètes (somme de contrôle,
//! fragmentation) afin d'emprunter exactement le chemin de décodage
//! des flux réels.
//!
//! Les MMSI simulés commencent par `9` (plage non attribuée aux
//! navires) et les noms par `SIM` : ils ne peuvent pas être confondus
//! avec des navires réels.

use std::f64::consts::TAU;

use chrono::{DateTime, Duration, Utc};
use nav_core::GeoPosition;
use nav_geodesy::units::{METERS_PER_NAUTICAL_MILE, MPS_PER_KNOT};

use crate::armor::BitWriter;
use crate::message::AisClass;

/// Longueur maximale de charge utile par phrase.
const CHUNK: usize = 60;

/// Navire simulé : cercle autour du centre, ou immobile au mouillage.
#[derive(Debug, Clone)]
struct SimVessel {
    mmsi: u32,
    name: String,
    callsign: String,
    class: AisClass,
    ship_type: u8,
    /// Rayon du cercle (m).
    radius_m: f64,
    /// Vitesse (nœuds) ; signe : sens de rotation.
    speed_kn: f64,
    /// Angle de départ (rad, depuis le nord).
    phase_rad: f64,
    position_period: Duration,
    next_position: Option<DateTime<Utc>>,
    next_static: Option<DateTime<Utc>>,
    /// Décalage de la première émission (étalement des comptes rendus,
    /// comme le partage du temps TDMA des équipements réels).
    first_offset: Duration,
}

impl SimVessel {
    #[allow(clippy::too_many_arguments)]
    fn new(
        mmsi: u32,
        name: String,
        callsign: String,
        class: AisClass,
        ship_type: u8,
        radius_nm: f64,
        speed_kn: f64,
        phase_deg: f64,
        period_s: i64,
    ) -> Self {
        Self {
            mmsi,
            name,
            callsign,
            class,
            ship_type,
            radius_m: radius_nm * METERS_PER_NAUTICAL_MILE,
            speed_kn,
            phase_rad: phase_deg.to_radians(),
            position_period: Duration::seconds(period_s),
            next_position: None,
            next_static: None,
            first_offset: Duration::zero(),
        }
    }
}

/// Nombre maximal de navires d'une flotte simulée ([`AisSimulator::fleet`]).
pub const MAX_FLEET: u32 = 20_000;

/// Premier MMSI d'une flotte simulée (plage non attribuée aux navires).
pub const FLEET_FIRST_MMSI: u32 = 971_000_001;

/// Générateur de flux AIS simulé.
#[derive(Debug, Clone)]
pub struct AisSimulator {
    center: GeoPosition,
    started_at: Option<DateTime<Utc>>,
    sequence: u8,
    vessels: Vec<SimVessel>,
}

/// Période d'émission des données statiques simulées.
const STATIC_PERIOD: Duration = Duration::seconds(60);

impl AisSimulator {
    /// Trois navires autour de `center` : un cargo (classe A), un
    /// voilier (classe B) et un pêcheur au mouillage (classe A).
    #[must_use]
    pub fn new(center: GeoPosition) -> Self {
        let vessel = |mmsi,
                      name: &str,
                      callsign: &str,
                      class,
                      ship_type,
                      radius_nm,
                      speed_kn,
                      phase_deg,
                      period| {
            SimVessel::new(
                mmsi,
                name.to_owned(),
                callsign.to_owned(),
                class,
                ship_type,
                radius_nm,
                speed_kn,
                phase_deg,
                period,
            )
        };
        Self {
            center,
            started_at: None,
            sequence: 0,
            vessels: vec![
                vessel(
                    970_000_001,
                    "SIM CARGO",
                    "SIM1",
                    AisClass::A,
                    70,
                    2.0,
                    12.0,
                    200.0,
                    10,
                ),
                vessel(
                    970_000_002,
                    "SIM VOILIER",
                    "SIM2",
                    AisClass::B,
                    37,
                    1.0,
                    -6.0,
                    90.0,
                    30,
                ),
                vessel(
                    970_000_003,
                    "SIM PECHEUR",
                    "SIM3",
                    AisClass::A,
                    30,
                    0.8,
                    0.0,
                    330.0,
                    60,
                ),
            ],
        }
    }

    /// Flotte de `count` navires fictifs pour les tests de charge
    /// (au plus [`MAX_FLEET`]) : cercles de 0,5 à 10 milles autour de
    /// `center`, alternance classe A (compte rendu toutes les 10 s) et
    /// classe B (toutes les 30 s), vitesses de 0 à 10 nœuds. MMSI à
    /// partir de [`FLEET_FIRST_MMSI`], noms `SIM 1`, `SIM 2`, etc.
    ///
    /// Les comptes rendus sont étalés sur la période de chaque navire,
    /// comme dans un flux réel (partage du temps TDMA) : pas de salve.
    #[must_use]
    pub fn fleet(center: GeoPosition, count: u32) -> Self {
        let vessels = (0..count.min(MAX_FLEET))
            .map(|i| {
                let class = if i % 2 == 0 { AisClass::A } else { AisClass::B };
                let period = if class == AisClass::A { 10 } else { 30 };
                // Angle d'or : répartition régulière sans motif visible.
                let phase = (f64::from(i) * 137.508).rem_euclid(360.0);
                let radius = 0.5 + f64::from(i % 20) * 0.5;
                let speed = f64::from(i % 11) * if i % 3 == 0 { -1.0 } else { 1.0 };
                let mut vessel = SimVessel::new(
                    FLEET_FIRST_MMSI + i,
                    format!("SIM {}", i + 1),
                    format!("S{}", i + 1),
                    class,
                    if class == AisClass::A { 70 } else { 37 },
                    radius,
                    speed,
                    phase,
                    period,
                );
                // Émissions étalées sur la période (pas de salve).
                vessel.first_offset =
                    Duration::milliseconds(i64::from(i) * 7_919 % (period * 1_000));
                vessel
            })
            .collect();
        Self {
            center,
            started_at: None,
            sequence: 0,
            vessels,
        }
    }

    /// Nombre de navires simulés.
    #[must_use]
    pub fn len(&self) -> usize {
        self.vessels.len()
    }

    /// Aucun navire simulé.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vessels.is_empty()
    }

    /// Phrases à émettre à l'instant `now`.
    pub fn step(&mut self, now: DateTime<Utc>) -> Vec<String> {
        let started_at = *self.started_at.get_or_insert(now);
        let elapsed_s = (now - started_at).num_milliseconds() as f64 / 1_000.0;
        let mut payloads = Vec::new();
        for v in &mut self.vessels {
            let first = now + v.first_offset;
            if now >= *v.next_static.get_or_insert(first) {
                v.next_static = Some(now + STATIC_PERIOD);
                payloads.extend(static_payloads(v));
            }
            if now >= *v.next_position.get_or_insert(first) {
                v.next_position = Some(now + v.position_period);
                if let Some(payload) = position_payload(v, self.center, elapsed_s) {
                    payloads.push(payload);
                }
            }
        }
        let mut lines = Vec::new();
        for (payload, fill) in payloads {
            lines.extend(sentences(&payload, fill, &mut self.sequence));
        }
        lines
    }
}

/// Position et route du navire après `elapsed_s` secondes.
fn kinematics(
    v: &SimVessel,
    center: GeoPosition,
    elapsed_s: f64,
) -> Option<(GeoPosition, f64, f64)> {
    let speed_mps = v.speed_kn.abs() * MPS_PER_KNOT;
    let angular = if v.radius_m > 0.0 {
        v.speed_kn.signum() * speed_mps / v.radius_m
    } else {
        0.0
    };
    let angle = (v.phase_rad + angular * elapsed_s).rem_euclid(TAU);
    let position = nav_geodesy::destination(center, angle, v.radius_m)?;
    let course = if v.speed_kn >= 0.0 {
        angle + TAU / 4.0
    } else {
        angle - TAU / 4.0
    };
    Some((position, speed_mps, course.rem_euclid(TAU)))
}

fn to_tenth(value: f64) -> u32 {
    // Valeurs bornées par construction (vitesse, angle) : conversion
    // sans dépassement.
    (value * 10.0).round().clamp(0.0, 4_095.0) as u32
}

fn position_payload(v: &SimVessel, center: GeoPosition, elapsed_s: f64) -> Option<(String, u8)> {
    let (position, speed_mps, course) = kinematics(v, center, elapsed_s)?;
    let lon = (position.longitude_deg() * 600_000.0).round() as i32;
    let lat = (position.latitude_deg() * 600_000.0).round() as i32;
    let sog = to_tenth(speed_mps / MPS_PER_KNOT).min(1022);
    let cog = to_tenth(course.to_degrees()) % 3600;
    let heading = (course.to_degrees().round() as u32) % 360;
    let mut w = BitWriter::new();
    match v.class {
        AisClass::A => {
            let status = if v.speed_kn == 0.0 { 1 } else { 0 };
            w.unsigned(1, 6)
                .unsigned(0, 2)
                .unsigned(v.mmsi, 30)
                .unsigned(status, 4)
                .signed(0, 8)
                .unsigned(sog, 10)
                .unsigned(1, 1)
                .signed(lon, 28)
                .signed(lat, 27)
                .unsigned(cog, 12)
                .unsigned(heading, 9)
                .unsigned(60, 6)
                .unsigned(0, 25);
        }
        AisClass::B => {
            w.unsigned(18, 6)
                .unsigned(0, 2)
                .unsigned(v.mmsi, 30)
                .unsigned(0, 8)
                .unsigned(sog, 10)
                .unsigned(0, 1)
                .signed(lon, 28)
                .signed(lat, 27)
                .unsigned(cog, 12)
                .unsigned(511, 9)
                .unsigned(60, 6)
                .unsigned(0, 29);
        }
    }
    Some(w.payload())
}

fn static_payloads(v: &SimVessel) -> Vec<(String, u8)> {
    match v.class {
        AisClass::A => {
            let mut w = BitWriter::new();
            w.unsigned(5, 6)
                .unsigned(0, 2)
                .unsigned(v.mmsi, 30)
                .unsigned(0, 2)
                .unsigned(0, 30)
                .text(&v.callsign, 7)
                .text(&v.name, 20)
                .unsigned(u32::from(v.ship_type), 8)
                .unsigned(0, 184);
            vec![w.payload()]
        }
        AisClass::B => {
            let mut a = BitWriter::new();
            a.unsigned(24, 6)
                .unsigned(0, 2)
                .unsigned(v.mmsi, 30)
                .unsigned(0, 2)
                .text(&v.name, 20);
            let mut b = BitWriter::new();
            b.unsigned(24, 6)
                .unsigned(0, 2)
                .unsigned(v.mmsi, 30)
                .unsigned(1, 2)
                .unsigned(u32::from(v.ship_type), 8)
                .unsigned(0, 42)
                .text(&v.callsign, 7)
                .unsigned(0, 36);
            vec![a.payload(), b.payload()]
        }
    }
}

/// Découpe une charge utile en phrases `!AIVDM` (canal A).
#[must_use]
pub fn sentences(payload: &str, fill_bits: u8, sequence: &mut u8) -> Vec<String> {
    let chunks: Vec<&str> = payload
        .as_bytes()
        .chunks(CHUNK)
        .filter_map(|c| std::str::from_utf8(c).ok())
        .collect();
    let count = chunks.len();
    let seq = if count > 1 {
        let s = *sequence;
        *sequence = (s + 1) % 10;
        s.to_string()
    } else {
        String::new()
    };
    chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let fill = if i + 1 == count { fill_bits } else { 0 };
            let body = format!("AIVDM,{count},{},{seq},A,{chunk},{fill}", i + 1);
            let sum = body.bytes().fold(0_u8, |acc, b| acc ^ b);
            format!("!{body}*{sum:02X}")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(seconds: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t") + Duration::seconds(seconds)
    }

    #[test]
    fn long_payloads_are_fragmented() {
        let mut seq = 9;
        let lines = sentences(&"1".repeat(71), 2, &mut seq);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("!AIVDM,2,1,9,A,"));
        assert!(lines[0].contains(",0*"));
        assert!(lines[1].starts_with("!AIVDM,2,2,9,A,11111111111,2*"));
        assert_eq!(seq, 0);
        assert_eq!(
            sentences("15M67F", 0, &mut seq)[0],
            "!AIVDM,1,1,,A,15M67F,0*28"
        );
    }

    #[test]
    fn emits_positions_on_schedule() {
        let mut sim = AisSimulator::new(GeoPosition::new(-14.968, -147.638).expect("pos"));
        // Départ : statiques (5 en 2 phrases, 24 A et B) et positions.
        assert_eq!(sim.step(t(0)).len(), 2 + 1 + 2 + 1 + 2 + 1);
        assert!(sim.step(t(1)).is_empty());
        assert_eq!(sim.step(t(10)).len(), 1);
        assert_eq!(sim.step(t(30)).len(), 2);
    }

    #[test]
    fn circles_around_center() {
        let center = GeoPosition::new(-14.968, -147.638).expect("pos");
        let v = &AisSimulator::new(center).vessels[0];
        for s in [0.0, 600.0, 3_600.0] {
            let (p, sog, _) = kinematics(v, center, s).expect("kinematics");
            let d = nav_geodesy::distance_m(center, p);
            assert!((d - 2.0 * METERS_PER_NAUTICAL_MILE).abs() < 1.0, "{d}");
            assert!((sog - 12.0 * MPS_PER_KNOT).abs() < 1e-9);
        }
    }

    #[test]
    fn fleet_is_capped_and_uses_distinct_unassigned_mmsi() {
        let center = GeoPosition::new(-14.968, -147.638).expect("pos");
        assert!(AisSimulator::fleet(center, 0).is_empty());
        let huge = AisSimulator::fleet(center, u32::MAX);
        assert_eq!(huge.len(), MAX_FLEET as usize);
        let mut mmsi: Vec<u32> = huge.vessels.iter().map(|v| v.mmsi).collect();
        mmsi.dedup();
        assert_eq!(mmsi.len(), huge.len());
        assert!(mmsi.iter().all(|m| (970_000_000..=999_999_999).contains(m)));
    }

    #[test]
    fn fleet_reports_on_schedule_and_stays_within_ten_miles() {
        let center = GeoPosition::new(-14.968, -147.638).expect("pos");
        let mut fleet = AisSimulator::fleet(center, 10);
        let per_step: Vec<usize> = (0..60).map(|s| fleet.step(t(s)).len()).collect();
        // Sur une minute : 5 A (6 positions + statique en 2 phrases) et
        // 5 B (2 positions + statiques A et B).
        assert_eq!(per_step.iter().sum::<usize>(), 5 * 8 + 5 * 4);
        // Émissions étalées : jamais toute la flotte au même instant.
        assert!(per_step.iter().all(|&n| n <= 6), "{per_step:?}");
        for v in &fleet.vessels {
            let (p, _, _) = kinematics(v, center, 120.0).expect("kinematics");
            assert!(nav_geodesy::distance_m(center, p) <= 10.0 * METERS_PER_NAUTICAL_MILE + 1.0);
        }
    }
}
