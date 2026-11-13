//! Tests de charge AIS : 100, 1 000 et 10 000 cibles simulées, décodées
//! depuis des phrases `!AIVDM` complètes (même chemin que les flux réels)
//! puis suivies.
//!
//! Les durées mesurées sont affichées (`cargo test --release -p nav-ais
//! --test load -- --nocapture`) ; les bornes vérifiées sont volontairement
//! larges pour rester fiables en mode debug et sur un exécuteur CI chargé.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::{Duration as StdDuration, Instant};

use chrono::{DateTime, Duration, Utc};
use nav_ais::sim::AisSimulator;
use nav_ais::{AisDecoder, AisTracker, Fragment, TargetState, TrackerConfig};
use nav_core::GeoPosition;

fn center() -> GeoPosition {
    GeoPosition::new(-14.968, -147.638).expect("pos")
}

fn t(seconds: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(1_800_000_000, 0).expect("t") + Duration::seconds(seconds)
}

/// Découpe `!AIVDM,…*CS` en type de phrase et champs (la somme de
/// contrôle est vérifiée par le connecteur NMEA, hors périmètre ici).
fn fragment(line: &str) -> Fragment {
    let body = line
        .strip_prefix('!')
        .and_then(|l| l.split_once('*'))
        .map(|(b, _)| b)
        .expect("phrase AIS");
    let mut fields = body.split(',');
    let address = fields.next().expect("adresse");
    let rest: Vec<&str> = fields.collect();
    Fragment::parse(&address[2..], &rest).expect("fragment valide")
}

/// Résultat d'une simulation de trafic.
struct Run {
    sentences: usize,
    decode: StdDuration,
    worst_snapshot: StdDuration,
    tracker: AisTracker,
}

/// `seconds` secondes de trafic d'une flotte de `count` navires, avec un
/// instantané par seconde (comme la boucle moteur).
fn simulate(count: u32, seconds: i64, config: TrackerConfig) -> Run {
    let mut fleet = AisSimulator::fleet(center(), count);
    let mut decoder = AisDecoder::new();
    let mut tracker = AisTracker::with_config(config);
    let mut sentences = 0;
    let mut decode = StdDuration::ZERO;
    let mut worst_snapshot = StdDuration::ZERO;
    for s in 0..seconds {
        let now = t(s);
        let lines = fleet.step(now);
        sentences += lines.len();
        let started = Instant::now();
        for line in &lines {
            if let Some(report) = decoder.push(fragment(line), now).expect("décodage") {
                tracker.update(&report, now);
            }
        }
        decode += started.elapsed();
        let started = Instant::now();
        let snapshot = tracker.snapshot(now);
        worst_snapshot = worst_snapshot.max(started.elapsed());
        assert_eq!(snapshot.len(), tracker.len());
    }
    Run {
        sentences,
        decode,
        worst_snapshot,
        tracker,
    }
}

fn report(label: &str, run: &Run, seconds: i64) {
    println!(
        "{label} : {} phrases en {seconds} s simulées ({:.0}/s), décodage + suivi {:?} \
         ({:.1} µs/phrase), instantané le plus lent {:?}",
        run.sentences,
        run.sentences as f64 / seconds as f64,
        run.decode,
        run.decode.as_secs_f64() * 1e6 / run.sentences.max(1) as f64,
        run.worst_snapshot,
    );
}

#[test]
fn hundred_targets_are_all_tracked() {
    let run = simulate(100, 120, TrackerConfig::default());
    report("100 cibles", &run, 120);
    let mut tracker = run.tracker;
    let snapshot = tracker.snapshot(t(120));
    assert_eq!(snapshot.len(), 100);
    assert!(snapshot
        .iter()
        .all(|v| v.state == TargetState::Active && v.position.is_some() && v.name.is_some()));
    assert!(run.worst_snapshot < StdDuration::from_millis(500));
}

#[test]
fn thousand_targets_stay_within_the_engine_tick() {
    let run = simulate(1_000, 120, TrackerConfig::default());
    report("1 000 cibles", &run, 120);
    // Débit nominal sous la limite du connecteur NMEA (200 phrases/s par
    // défaut) une fois la salve de départ passée.
    assert!(run.sentences / 120 < 200, "{} phrases", run.sentences);
    let mut tracker = run.tracker;
    let snapshot = tracker.snapshot(t(120));
    assert_eq!(snapshot.len(), 1_000);
    assert!(snapshot.iter().all(|v| v.state == TargetState::Active));
    // Historique borné par cible.
    assert!(snapshot
        .iter()
        .all(|v| v.history.len() <= TrackerConfig::default().history_len));
    // Un instantané doit tenir largement dans la période de la boucle
    // (1 s), même en debug.
    assert!(
        run.worst_snapshot < StdDuration::from_secs(1),
        "{:?}",
        run.worst_snapshot
    );
}

#[test]
fn ten_thousand_targets_are_capped_without_growing_memory() {
    let config = TrackerConfig::default();
    let run = simulate(10_000, 15, config);
    report("10 000 cibles", &run, 15);
    // Protection contre les flux excessifs : au plus `max_targets`.
    assert_eq!(run.tracker.len(), config.max_targets);
    assert!(
        run.worst_snapshot < StdDuration::from_secs(2),
        "{:?}",
        run.worst_snapshot
    );
}

#[test]
fn silent_fleet_goes_stale_then_lost_then_removed() {
    let config = TrackerConfig::default();
    let run = simulate(200, 30, config);
    let mut tracker = run.tracker;
    let last = t(29);
    // Classe A à 10 s : obsolète après 3 intervalles, perdue après 6.
    let stale = tracker.snapshot(last + Duration::seconds(45));
    assert!(stale.iter().any(|v| v.state == TargetState::Stale));
    // Navires lents ou au mouillage : 3 min, donc perdus après 18 min.
    let lost = tracker.snapshot(last + Duration::minutes(20));
    assert!(lost.iter().all(|v| v.state == TargetState::Lost));
    let removed = tracker.snapshot(last + config.removal_after + Duration::seconds(1));
    assert!(removed.is_empty(), "{} cibles restantes", removed.len());
}
