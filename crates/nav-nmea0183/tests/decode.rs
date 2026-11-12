//! Décodage des phrases (TEST_PLAN.md, § NMEA) : phrases valides,
//! checksum invalide, phrases incomplètes, champs absents, valeurs hors
//! limites, émetteurs différents.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use chrono::{DateTime, TimeZone, Utc};
use nav_core::{Measurement, PositionFix, Quantity, Sample, SourceKind};
use nav_nmea0183::decode::sentence_time;
use nav_nmea0183::sentence::checksum;
use nav_nmea0183::{decode, DecodeContext, DecodeError, Decoded};

const KNOT: f64 = 1852.0 / 3600.0;

fn at(h: u32, m: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 12, h, m, s)
        .single()
        .expect("date")
}

fn ctx(received_at: DateTime<Utc>) -> DecodeContext<'static> {
    DecodeContext {
        origin: "nmea0183",
        source: SourceKind::Nmea0183,
        received_at,
        use_sentence_time: true,
    }
}

fn line(body: &str) -> String {
    format!("${body}*{:02X}", checksum(body))
}

fn samples(body: &str) -> Vec<Sample> {
    samples_at(body, at(18, 30, 5))
}

fn samples_at(body: &str, received: DateTime<Utc>) -> Vec<Sample> {
    match decode(&line(body), ctx(received)).expect("décodage") {
        Decoded::Samples(s) => s,
        Decoded::Unsupported(kind) => panic!("non pris en charge : {kind}"),
    }
}

fn error(body: &str) -> DecodeError {
    decode(&line(body), ctx(at(18, 30, 5))).expect_err("erreur attendue")
}

fn position(samples: &[Sample]) -> &PositionFix {
    samples
        .iter()
        .find_map(|s| match s {
            Sample::Position(p) => Some(p),
            Sample::Measurement(_) => None,
        })
        .expect("position")
}

fn measurement(samples: &[Sample], q: Quantity) -> &Measurement {
    samples
        .iter()
        .find_map(|s| match s {
            Sample::Measurement(m) if m.quantity == q => Some(m),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{q:?} absent"))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn rmc_gives_position_speed_and_course() {
    let s = samples("GPRMC,183000.00,A,1458.0800,S,14738.2800,W,6.5,315.0,121126,12.1,E,A");
    assert_eq!(s.len(), 3);
    let fix = position(&s);
    assert!(close(fix.position.latitude_deg(), -(14.0 + 58.08 / 60.0)));
    assert!(close(fix.position.longitude_deg(), -(147.0 + 38.28 / 60.0)));
    assert!(close(fix.sog_mps.expect("sog"), 6.5 * KNOT));
    assert!(close(fix.cog_rad.expect("cog"), 315_f64.to_radians()));
    assert_eq!(fix.measured_at, at(18, 30, 0));
    assert_eq!(fix.received_at, at(18, 30, 5));
    assert_eq!(fix.source, SourceKind::Nmea0183);
    assert_eq!(fix.source_id, "nmea0183.GP");
    let sog = measurement(&s, Quantity::SpeedOverGround);
    assert!(close(sog.value, 6.5 * KNOT));
}

#[test]
fn rmc_without_valid_fix_gives_nothing() {
    // Statut V, mode N, estimée, manuelle, simulée : jamais de position.
    for body in [
        "GPRMC,183000,V,1458.08,S,14738.28,W,6.5,315.0,121126,,",
        "GPRMC,183000,A,1458.08,S,14738.28,W,6.5,315.0,121126,,,N",
        "GPRMC,183000,A,1458.08,S,14738.28,W,6.5,315.0,121126,,,E",
        "GPRMC,183000,A,1458.08,S,14738.28,W,6.5,315.0,121126,,,M",
        "GPRMC,183000,A,1458.08,S,14738.28,W,6.5,315.0,121126,,,S",
        "GPRMC,,V,,,,,,,,,,N",
    ] {
        assert!(samples(body).is_empty(), "{body}");
    }
}

#[test]
fn rmc_with_empty_position_fields_gives_no_position() {
    let s = samples("GPRMC,183000,A,,,,,0.0,,121126,,,A");
    assert!(s.iter().all(|x| matches!(x, Sample::Measurement(_))));
    assert_eq!(s.len(), 1, "seule la SOG est présente");
}

#[test]
fn gga_quality_filters_positions() {
    for quality in ["1", "2", "4", "5"] {
        let body = format!("GPGGA,183000,1458.08,S,14738.28,W,{quality},09,0.9,2.1,M,,M,,");
        assert_eq!(samples(&body).len(), 1, "qualité {quality}");
    }
    for quality in ["0", "6", "7", "8", ""] {
        let body = format!("GPGGA,183000,1458.08,S,14738.28,W,{quality},09,0.9,2.1,M,,M,,");
        assert!(samples(&body).is_empty(), "qualité {quality}");
    }
}

#[test]
fn gga_time_uses_nearest_reception_day() {
    let body = "GNGGA,235959.50,1458.08,S,14738.28,W,1,09,0.9,2.1,M,,M,,";
    let received = Utc
        .with_ymd_and_hms(2026, 11, 13, 0, 0, 2)
        .single()
        .expect("d");
    let fix = samples_at(body, received);
    let fix = position(&fix);
    assert_eq!(
        fix.measured_at,
        Utc.with_ymd_and_hms(2026, 11, 12, 23, 59, 59)
            .single()
            .expect("d")
            + chrono::Duration::milliseconds(500)
    );
    assert_eq!(fix.source_id, "nmea0183.GN");
    let early = samples_at(
        "GPGGA,000001,1458.08,S,14738.28,W,1,09,0.9,,M,,M,,",
        at(23, 59, 58),
    );
    assert_eq!(
        position(&early).measured_at,
        Utc.with_ymd_and_hms(2026, 11, 13, 0, 0, 1)
            .single()
            .expect("d")
    );
}

#[test]
fn gll_status_and_mode() {
    let s = samples("GPGLL,1458.08,S,14738.28,W,183000,A,A");
    assert_eq!(position(&s).measured_at, at(18, 30, 0));
    assert_eq!(
        samples("GPGLL,1458.08,S,14738.28,W,183000").len(),
        1,
        "NMEA < 2.0"
    );
    assert!(samples("GPGLL,1458.08,S,14738.28,W,183000,V,N").is_empty());
    assert!(samples("GPGLL,1458.08,S,14738.28,W,183000,A,E").is_empty());
}

#[test]
fn reception_time_is_used_for_replays() {
    let mut c = ctx(at(18, 30, 5));
    c.use_sentence_time = false;
    c.source = SourceKind::Replay;
    let body = line("GPRMC,183000,A,1458.08,S,14738.28,W,6.5,315.0,010119,,,A");
    let Decoded::Samples(s) = decode(&body, c).expect("rejeu") else {
        panic!("échantillons attendus");
    };
    let fix = position(&s);
    assert_eq!(fix.measured_at, at(18, 30, 5));
    assert_eq!(fix.source, SourceKind::Replay);
}

#[test]
fn vtg_course_and_speed() {
    let s = samples("GPVTG,315.0,T,303.0,M,6.5,N,12.0,K,A");
    assert!(close(
        measurement(&s, Quantity::CourseOverGroundTrue).value,
        315_f64.to_radians()
    ));
    assert!(close(
        measurement(&s, Quantity::SpeedOverGround).value,
        6.5 * KNOT
    ));
    let kmh = samples("GPVTG,,T,,M,,N,36.0,K,A");
    assert_eq!(kmh.len(), 1);
    assert!(close(
        measurement(&kmh, Quantity::SpeedOverGround).value,
        10.0
    ));
    assert!(samples("GPVTG,315.0,T,303.0,M,6.5,N,12.0,K,N").is_empty());
}

#[test]
fn headings() {
    let s = samples("IIHDT,274.07,T");
    assert!(close(
        measurement(&s, Quantity::HeadingTrue).value,
        274.07_f64.to_radians()
    ));
    let s = samples("HCHDM,10.5,M");
    assert!(close(
        measurement(&s, Quantity::HeadingMagnetic).value,
        10.5_f64.to_radians()
    ));
    let s = samples("HCHDG,298.5,1.5,E,12.1,E");
    assert!(close(
        measurement(&s, Quantity::HeadingMagnetic).value,
        300_f64.to_radians()
    ));
    assert_eq!(s.len(), 1, "la variation n'est pas appliquée");
    let s = samples("HCHDG,1.0,2.0,W,,");
    let m = measurement(&s, Quantity::HeadingMagnetic).value;
    assert!(
        close(m, 359_f64.to_radians()),
        "cap ramené dans [0, 360[ : {m}"
    );
    let s = samples("HCHDG,90.0,,,,");
    assert!(close(
        measurement(&s, Quantity::HeadingMagnetic).value,
        90_f64.to_radians()
    ));
    assert!(matches!(
        error("HCHDG,90.0,2.0,,,"),
        DecodeError::Field { .. }
    ));
}

#[test]
fn speed_through_water() {
    let s = samples("IIVHW,,T,,M,6.1,N,11.3,K");
    assert_eq!(s.len(), 1);
    assert!(close(
        measurement(&s, Quantity::SpeedThroughWater).value,
        6.1 * KNOT
    ));
    let s = samples("IIVHW,312.0,T,300.0,M,6.1,N,11.3,K");
    assert_eq!(s.len(), 3);
}

#[test]
fn depths() {
    let s = samples("SDDPT,18.4,0.5,");
    assert!(close(
        measurement(&s, Quantity::DepthBelowTransducer).value,
        18.4
    ));
    let s = samples("SDDBT,32.8,f,10.0,M,5.5,F");
    assert!(close(
        measurement(&s, Quantity::DepthBelowTransducer).value,
        10.0
    ));
    let s = samples("SDDBT,10.0,f,,M,,F");
    assert!(close(
        measurement(&s, Quantity::DepthBelowTransducer).value,
        3.048
    ));
    let s = samples("SDDBT,,f,,M,2.0,F");
    assert!(close(
        measurement(&s, Quantity::DepthBelowTransducer).value,
        3.6576
    ));
    assert!(samples("SDDPT,,,").is_empty(), "sondeur sans fond");
    assert!(matches!(
        error("SDDPT,-3.0,0.5,"),
        DecodeError::Field { .. }
    ));
}

#[test]
fn apparent_wind() {
    let s = samples("IIMWV,045.0,R,14.2,N,A");
    assert!(close(
        measurement(&s, Quantity::WindAngleApparent).value,
        45_f64.to_radians()
    ));
    assert!(close(
        measurement(&s, Quantity::WindSpeedApparent).value,
        14.2 * KNOT
    ));
    let s = samples("IIMWV,270.0,R,10.0,M,A");
    assert!(close(
        measurement(&s, Quantity::WindAngleApparent).value,
        (-90_f64).to_radians()
    ));
    assert!(close(
        measurement(&s, Quantity::WindSpeedApparent).value,
        10.0
    ));
    let s = samples("IIMWV,180.0,R,36.0,K,A");
    assert!(close(
        measurement(&s, Quantity::WindSpeedApparent).value,
        10.0
    ));
    assert!(
        samples("IIMWV,045.0,T,14.2,N,A").is_empty(),
        "vent vrai ignoré"
    );
    assert!(
        samples("IIMWV,045.0,R,14.2,N,V").is_empty(),
        "donnée invalide"
    );
    assert!(matches!(
        error("IIMWV,045.0,R,14.2,X,A"),
        DecodeError::Field { .. }
    ));
}

#[test]
fn out_of_range_and_malformed_fields_are_rejected() {
    for body in [
        "GPRMC,183000,A,9100.00,N,14738.28,W,6.5,315.0,121126,,,A",
        "GPRMC,183000,A,1460.00,S,14738.28,W,6.5,315.0,121126,,,A",
        "GPRMC,183000,A,1458.08,S,18100.00,W,6.5,315.0,121126,,,A",
        "GPRMC,183000,A,1458.08,X,14738.28,W,6.5,315.0,121126,,,A",
        "GPRMC,183000,A,1458.08,,14738.28,W,6.5,315.0,121126,,,A",
        "GPRMC,183000,A,-1458.08,S,14738.28,W,6.5,315.0,121126,,,A",
        "GPRMC,183000,A,1458.08,S,14738.28,W,vite,315.0,121126,,,A",
        "GPRMC,183000,A,1458.08,S,14738.28,W,inf,315.0,121126,,,A",
        "GPRMC,183000,A,1458.08,S,14738.28,W,500.0,315.0,121126,,,A",
        "GPRMC,253000,A,1458.08,S,14738.28,W,6.5,315.0,121126,,,A",
        "GPRMC,1830,A,1458.08,S,14738.28,W,6.5,315.0,121126,,,A",
        "GPRMC,183000,A,1458.08,S,14738.28,W,6.5,315.0,321326,,,A",
        "GPRMC,183000,A,1458.08,S,14738.28,W,6.5,315.0,2026-11-12,,,A",
        "IIHDT,abc,T",
        "IIMWV,999.0,R,14.2,N,A",
    ] {
        assert!(
            matches!(error(body), DecodeError::Field { .. }),
            "{body} : erreur de champ attendue"
        );
    }
}

#[test]
fn checksum_and_framing_errors_are_reported() {
    let c = ctx(at(18, 30, 5));
    for raw in [
        "$GPRMC,183000,A,1458.08,S,14738.28,W,6.5,315.0,121126,,,A*00",
        "$IIHDT,312.3,T",
        "$GPGGA,183003.50,1458.08",
        "GPHDT,312.3,T*00",
        "",
    ] {
        assert!(
            matches!(decode(raw, c), Err(DecodeError::Sentence(_))),
            "{raw:?}"
        );
    }
}

#[test]
fn unsupported_and_ais_sentences_are_ignored() {
    let c = ctx(at(18, 30, 5));
    assert_eq!(
        decode(&line("GPZDA,183000.00,12,11,2026,00,00"), c),
        Ok(Decoded::Unsupported("ZDA".to_owned()))
    );
    let body = "AIVDM,1,1,,A,15M67FC000G?ufbE`FepT@3n00Sa,0";
    let ais = format!("!{body}*{:02X}", checksum(body));
    assert_eq!(decode(&ais, c), Ok(Decoded::Unsupported("VDM".to_owned())));
}

#[test]
fn sentence_time_is_extracted_for_pacing() {
    assert_eq!(
        sentence_time(&line("GPRMC,183000.25,A,,,,,,,121126,,,A")),
        chrono::NaiveTime::from_hms_milli_opt(18, 30, 0, 250)
    );
    assert!(sentence_time(&line("GPGLL,1458.08,S,14738.28,W,183001,A,A")).is_some());
    assert!(sentence_time(&line("IIHDT,1,T")).is_none());
    assert!(sentence_time("$GPRMC,183000,A*7F").is_none());
}
