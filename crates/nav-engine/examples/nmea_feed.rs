//! Multiplexeur NMEA 0183 simulé pour les mesures de charge : serveur TCP
//! qui envoie à chaque client une position GPS (RMC, 1 Hz) et le trafic
//! AIS d'une flotte fictive (MMSI 971…, noms `SIM n`).
//!
//! ```text
//! cargo run --release -p nav-engine --example nmea_feed -- --port 10110 --targets 1000
//! ```
//!
//! Outil de développement uniquement : écoute sur la boucle locale.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::Duration;

use chrono::Utc;
use nav_ais::sim::{AisSimulator, MAX_FLEET};
use nav_core::GeoPosition;
use nav_nmea0183::sentence::checksum;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

/// Position émise (lagon de Rangiroa).
const LAT: f64 = -14.968;
const LON: f64 = -147.638;

struct Options {
    port: u16,
    targets: u32,
}

fn parse() -> Result<Options, String> {
    let mut options = Options {
        port: 10110,
        targets: 100,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("valeur manquante pour {arg}"))?;
        match arg.as_str() {
            "--port" => options.port = value.parse().map_err(|_| "port invalide")?,
            "--targets" => {
                options.targets = value
                    .parse::<u32>()
                    .map_err(|_| "nombre de cibles invalide")?
                    .min(MAX_FLEET);
            }
            other => return Err(format!("option inconnue : {other}")),
        }
    }
    Ok(options)
}

fn rmc() -> String {
    let now = Utc::now();
    let (lat, lon) = (LAT.abs(), LON.abs());
    let body = format!(
        "GPRMC,{},A,{:02}{:07.4},S,{:03}{:07.4},W,5.0,45.0,{},,,A",
        now.format("%H%M%S.00"),
        lat.trunc(),
        lat.fract() * 60.0,
        lon.trunc(),
        lon.fract() * 60.0,
        now.format("%d%m%y")
    );
    format!("${body}*{:02X}\r\n", checksum(&body))
}

async fn serve(mut socket: TcpStream, targets: u32) {
    let Some(center) = GeoPosition::new(LAT, LON).ok() else {
        return;
    };
    let mut fleet = AisSimulator::fleet(center, targets);
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    let mut ticks: u64 = 0;
    loop {
        interval.tick().await;
        let mut data = if ticks % 10 == 0 {
            rmc()
        } else {
            String::new()
        };
        ticks += 1;
        for line in fleet.step(Utc::now()) {
            data.push_str(&line);
            data.push_str("\r\n");
        }
        if socket.write_all(data.as_bytes()).await.is_err() {
            return;
        }
    }
}

#[tokio::main]
async fn main() {
    let options = match parse() {
        Ok(o) => o,
        Err(message) => {
            eprintln!("nmea_feed : {message}");
            std::process::exit(2);
        }
    };
    let listener = match TcpListener::bind(("127.0.0.1", options.port)).await {
        Ok(l) => l,
        Err(error) => {
            eprintln!(
                "nmea_feed : écoute impossible sur le port {} : {error}",
                options.port
            );
            std::process::exit(1);
        }
    };
    println!(
        "nmea_feed : 127.0.0.1:{} — GPS 1 Hz et {} cibles AIS",
        options.port, options.targets
    );
    loop {
        match listener.accept().await {
            Ok((socket, peer)) => {
                println!("nmea_feed : client {peer}");
                tokio::spawn(serve(socket, options.targets));
            }
            Err(error) => eprintln!("nmea_feed : {error}"),
        }
    }
}
