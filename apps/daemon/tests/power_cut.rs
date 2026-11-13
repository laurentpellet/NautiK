//! Coupure d'alimentation simulée : le processus `nautikd` est tué
//! brutalement (SIGKILL / TerminateProcess, sans aucun nettoyage) pendant
//! l'enregistrement d'une trace, ancre mouillée, puis redémarré sur les
//! mêmes données.
//!
//! Attendu : base intègre, points enregistrés conservés, trace interrompue
//! clôturée à l'heure de son dernier point, mouillage toujours surveillé.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

/// Réponse HTTP simplifiée.
struct Response {
    status: u16,
    body: String,
}

impl Response {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("{e} : {}", self.body))
    }
}

fn request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: &str,
    body: Option<&str>,
) -> Option<Response> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).ok()?;
    let status = raw.get(9..12)?.parse().ok()?;
    let body = raw.split_once("\r\n\r\n").map(|(_, b)| b.to_owned())?;
    Some(Response { status, body })
}

/// Port libre sur la boucle locale.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("port")
        .local_addr()
        .expect("adresse")
        .port()
}

/// Processus tué à la fin du test, même en cas d'échec.
struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Daemon {
    fn start(data_dir: &Path, addr: SocketAddr) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_nautikd"))
            .arg("--data-dir")
            .arg(data_dir)
            .arg("--listen")
            .arg(addr.to_string())
            .env("RUST_LOG", "warn")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("lancement de nautikd");
        let daemon = Self(child);
        let ready = wait_until(Duration::from_secs(20), || {
            request(addr, "GET", "/health", "", None).is_some_and(|r| r.status == 200)
        });
        assert!(ready, "nautikd prêt à temps");
        daemon
    }

    /// Coupure brutale : aucun code d'arrêt n'est exécuté.
    fn power_cut(mut self) {
        self.0.kill().expect("arrêt brutal");
        self.0.wait().expect("fin du processus");
    }
}

fn wait_until(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

fn tracks(addr: SocketAddr, token: &str) -> Vec<Value> {
    let r = request(addr, "GET", "/api/v1/tracks", token, None).expect("traces");
    assert_eq!(r.status, 200, "{}", r.body);
    r.json().as_array().expect("liste").clone()
}

#[test]
fn power_cut_during_recording_loses_nothing_recorded() {
    let dir = tempfile::tempdir().expect("tmp");
    let data = dir.path().join("donnees");
    let addr: SocketAddr = format!("127.0.0.1:{}", free_port())
        .parse()
        .expect("adresse");

    // 1. Service en marche (simulateur) : mouillage et enregistrement.
    let daemon = Daemon::start(&data, addr);
    let token = std::fs::read_to_string(data.join("api-token")).expect("jeton");
    let token = token.trim();
    let anchored = wait_until(Duration::from_secs(10), || {
        request(
            addr,
            "POST",
            "/api/v1/alarms/anchor",
            token,
            Some(r#"{"radiusM": 60, "delayS": 10}"#),
        )
        .is_some_and(|r| r.status == 200)
    });
    assert!(anchored, "ancre mouillée (position simulée disponible)");
    let started = request(addr, "POST", "/api/v1/recording", token, None).expect("enregistrement");
    assert_eq!(started.status, 200, "{}", started.body);
    let track_id = started.json()["id"].as_str().expect("id").to_owned();
    let recorded = wait_until(Duration::from_secs(20), || {
        tracks(addr, token)
            .iter()
            .any(|t| t["id"] == track_id.as_str() && t["pointCount"].as_u64() >= Some(1))
    });
    assert!(
        recorded,
        "points enregistrés avant la coupure : {:?}",
        tracks(addr, token)
    );

    // 2. Coupure d'alimentation.
    daemon.power_cut();

    // 3. Base intègre, points présents, trace marquée en cours.
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let before_restart = runtime.block_on(async {
        let db = nav_storage::Db::open(&data.join("nautik.db"))
            .await
            .expect("base");
        assert_eq!(
            db.quick_check().await.expect("vérification"),
            Vec::<String>::new()
        );
        let list = db.list_tracks().await.expect("traces");
        db.close().await;
        list
    });
    let interrupted = before_restart
        .iter()
        .find(|t| t.id.to_string() == track_id)
        .expect("trace conservée");
    assert!(interrupted.is_recording, "coupure : trace non clôturée");
    assert!(interrupted.point_count >= 1);
    let points_before = interrupted.point_count;

    // 4. Redémarrage : trace clôturée sans heure inventée, ancre reprise.
    let daemon = Daemon::start(&data, addr);
    let list = tracks(addr, token);
    let track = list
        .iter()
        .find(|t| t["id"] == track_id.as_str())
        .expect("trace après redémarrage");
    assert_eq!(track["isRecording"], false);
    assert!(track["endedAt"].is_string(), "fin renseignée : {track}");
    assert_eq!(track["pointCount"].as_u64(), Some(points_before));
    let detail = request(
        addr,
        "GET",
        &format!("/api/v1/tracks/{track_id}"),
        token,
        None,
    )
    .expect("détail");
    let detail = detail.json();
    let last_point = detail["points"]
        .as_array()
        .and_then(|p| p.last())
        .and_then(|p| p["timestamp"].as_str())
        .map(str::to_owned);
    assert_eq!(
        detail["endedAt"].as_str().map(str::to_owned),
        last_point,
        "fin = dernier point enregistré"
    );
    let alarms = request(addr, "GET", "/api/v1/alarms", token, None).expect("alarmes");
    assert!(
        alarms.json()["anchor"].is_object(),
        "mouillage toujours surveillé : {}",
        alarms.body
    );
    drop(daemon);
}
