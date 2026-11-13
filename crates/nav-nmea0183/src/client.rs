//! Acquisition NMEA 0183 : TCP, UDP, série et rejeu de fichier.
//!
//! Chaque session ouvre la liaison, découpe le flux en lignes, valide et
//! décode les phrases. Une liaison fermée, en erreur ou muette (aucune
//! phrase valide pendant [`ClientOptions::idle_timeout`]) est signalée
//! puis rouverte avec un délai exponentiel borné. Un débit excessif est
//! écrêté et signalé.

use std::time::Duration;

use chrono::{DateTime, NaiveTime, Utc};
use nav_ais::Fragment;
use nav_core::{ConnectionState, Sample, SourceKind};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::{mpsc, watch};
use tokio::time::{sleep, sleep_until, timeout, Instant};

use crate::config::Transport;
use crate::decode::{decode, sentence_time, DecodeContext, Decoded};
use crate::framer::{FrameError, LineFramer, MAX_LINE_BYTES};

/// Taille maximale d'un fichier rejoué.
pub const MAX_REPLAY_BYTES: u64 = 64 * 1024 * 1024;

/// Taille du tampon de réception UDP (datagramme maximal).
const UDP_BUFFER: usize = 65_536;

/// Réglages du client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientOptions {
    /// Délai maximal d'ouverture de la liaison.
    pub connect_timeout: Duration,
    /// Silence (aucune phrase valide) au-delà duquel la liaison est
    /// considérée perdue.
    pub idle_timeout: Duration,
    /// Premier délai de reconnexion.
    pub initial_backoff: Duration,
    /// Délai de reconnexion maximal.
    pub max_backoff: Duration,
    /// Débit maximal traité ; l'excédent est ignoré et signalé.
    pub max_sentences_per_second: u32,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(15),
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            max_sentences_per_second: 200,
        }
    }
}

impl ClientOptions {
    /// Délai avant la tentative `attempt` (à partir de 1).
    #[must_use]
    pub fn backoff(&self, attempt: u32) -> Duration {
        let factor = 2_u32.saturating_pow(attempt.saturating_sub(1).min(16));
        self.initial_backoff
            .saturating_mul(factor)
            .min(self.max_backoff)
    }
}

/// Événement produit par le client.
#[derive(Debug, Clone, PartialEq)]
pub enum NmeaEvent {
    /// Changement d'état de la liaison.
    State(ConnectionState),
    /// Donnée normalisée.
    Sample(Sample),
    /// Fragment de message AIS (VDM/VDO), à réassembler par le
    /// destinataire (`nav_ais::AisDecoder`).
    Ais {
        /// Fragment validé.
        fragment: Fragment,
        /// Horodatage de réception.
        received_at: DateTime<Utc>,
    },
    /// Phrase ou ligne rejetée (cause).
    Rejected(String),
}

/// Le destinataire des événements a disparu : arrêt.
struct Gone;

/// Fin d'une session.
enum End {
    /// Liaison perdue (cause).
    Lost(String),
    /// Destinataire disparu.
    Gone,
}

impl From<Gone> for End {
    fn from(_: Gone) -> Self {
        Self::Gone
    }
}

/// Limiteur de débit par fenêtre d'une seconde.
struct RateLimiter {
    max: u32,
    window: Instant,
    count: u32,
}

impl RateLimiter {
    fn new(max: u32) -> Self {
        Self {
            max,
            window: Instant::now(),
            count: 0,
        }
    }

    /// `Some(premier_rejet)` si la phrase doit être ignorée.
    fn check(&mut self) -> Option<bool> {
        let now = Instant::now();
        if now.duration_since(self.window) >= Duration::from_secs(1) {
            self.window = now;
            self.count = 0;
        }
        self.count = self.count.saturating_add(1);
        (self.count > self.max).then_some(self.count == self.max.saturating_add(1))
    }
}

struct Session<'a> {
    options: ClientOptions,
    events: &'a mpsc::Sender<NmeaEvent>,
    origin: &'static str,
    source: SourceKind,
    replay: bool,
    limiter: RateLimiter,
    last_valid: Instant,
    /// Une phrase valide a été reçue pendant la session.
    receiving: bool,
}

impl Session<'_> {
    async fn emit(&self, event: NmeaEvent) -> Result<(), Gone> {
        self.events.send(event).await.map_err(|_| Gone)
    }

    async fn line(&mut self, line: Result<String, FrameError>) -> Result<(), Gone> {
        match line {
            Ok(line) => self.sentence(&line).await,
            Err(error) => self.emit(NmeaEvent::Rejected(error.to_string())).await,
        }
    }

    async fn sentence(&mut self, line: &str) -> Result<(), Gone> {
        if line.trim().is_empty() {
            return Ok(());
        }
        if let Some(first) = self.limiter.check() {
            if first {
                let reason = format!(
                    "flux excessif (> {} phrases/s) : phrases ignorées",
                    self.options.max_sentences_per_second
                );
                self.emit(NmeaEvent::Rejected(reason)).await?;
            }
            return Ok(());
        }
        let ctx = DecodeContext {
            origin: self.origin,
            source: self.source,
            received_at: Utc::now(),
            use_sentence_time: !self.replay,
        };
        match decode(line, ctx) {
            Ok(decoded) => {
                self.last_valid = Instant::now();
                self.receiving = true;
                match decoded {
                    Decoded::Samples(samples) => {
                        for sample in samples {
                            self.emit(NmeaEvent::Sample(sample)).await?;
                        }
                    }
                    Decoded::Ais(fragment) => {
                        let received_at = ctx.received_at;
                        self.emit(NmeaEvent::Ais {
                            fragment,
                            received_at,
                        })
                        .await?;
                    }
                    Decoded::Unsupported(_) => {}
                }
                Ok(())
            }
            Err(error) => self.emit(NmeaEvent::Rejected(error.to_string())).await,
        }
    }

    fn idle_deadline(&self) -> Instant {
        self.last_valid + self.options.idle_timeout
    }

    fn silence(&self) -> End {
        End::Lost(format!(
            "aucune phrase valide depuis {} s",
            self.options.idle_timeout.as_secs()
        ))
    }

    async fn read_stream<R: AsyncRead + Unpin>(&mut self, mut reader: R) -> End {
        let mut framer = LineFramer::new();
        let mut buffer = vec![0_u8; 4_096];
        loop {
            let read = tokio::select! {
                read = reader.read(&mut buffer) => read,
                () = sleep_until(self.idle_deadline()) => return self.silence(),
            };
            let n = match read {
                Ok(0) => return End::Lost("liaison fermée par l'équipement".to_owned()),
                Ok(n) => n,
                Err(error) => return End::Lost(error.to_string()),
            };
            for line in framer.push(&buffer[..n]) {
                if let Err(gone) = self.line(line).await {
                    return gone.into();
                }
            }
        }
    }

    async fn read_udp(&mut self, socket: UdpSocket) -> End {
        let mut buffer = vec![0_u8; UDP_BUFFER];
        loop {
            let received = tokio::select! {
                r = socket.recv_from(&mut buffer) => r,
                () = sleep_until(self.idle_deadline()) => return self.silence(),
            };
            let n = match received {
                Ok((n, _peer)) => n,
                Err(error) => return End::Lost(error.to_string()),
            };
            let mut framer = LineFramer::new();
            let mut lines = framer.push(&buffer[..n]);
            lines.extend(framer.finish());
            for line in lines {
                if let Err(gone) = self.line(line).await {
                    return gone.into();
                }
            }
        }
    }

    /// Rejoue un fichier au rythme des heures enregistrées (écarts de
    /// plus de 10 s ramenés à zéro).
    async fn replay(&mut self, content: &[u8]) -> Result<(), Gone> {
        let mut last_time: Option<NaiveTime> = None;
        let mut untimed = 0_u32;
        for raw in content.split(|&b| b == b'\n') {
            let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
            if raw.len() > MAX_LINE_BYTES {
                self.line(Err(FrameError::TooLong)).await?;
                continue;
            }
            let Ok(line) = std::str::from_utf8(raw) else {
                self.line(Err(FrameError::NotText)).await?;
                continue;
            };
            match sentence_time(line) {
                Some(time) => {
                    untimed = 0;
                    if let Some(previous) = last_time {
                        if let Ok(delta) = (time - previous).to_std() {
                            if delta <= Duration::from_secs(10) {
                                sleep(delta).await;
                            }
                        }
                    }
                    last_time = Some(time);
                }
                None => {
                    untimed += 1;
                    // Fichier sans heures : cadence de 10 lignes/s.
                    if untimed % 50 == 0 {
                        sleep(Duration::from_millis(100)).await;
                    }
                }
            }
            self.sentence(line).await?;
        }
        Ok(())
    }
}

async fn connected(session: &Session<'_>, transport: &Transport) -> Result<(), Gone> {
    session
        .emit(NmeaEvent::State(ConnectionState::Connected {
            server_name: Some(transport.describe()),
            server_version: None,
        }))
        .await
}

/// Ouvre la liaison et la lit jusqu'à sa perte.
async fn session(transport: &Transport, session: &mut Session<'_>) -> End {
    let open_timeout = session.options.connect_timeout;
    match transport {
        Transport::Tcp { host, port } => {
            let stream =
                match timeout(open_timeout, TcpStream::connect((host.as_str(), *port))).await {
                    Ok(Ok(stream)) => stream,
                    Ok(Err(error)) => return End::Lost(format!("connexion impossible : {error}")),
                    Err(_) => return End::Lost("délai de connexion dépassé".to_owned()),
                };
            if let Err(gone) = connected(session, transport).await {
                return gone.into();
            }
            session.last_valid = Instant::now();
            session.read_stream(stream).await
        }
        Transport::Udp { port } => {
            let socket = match UdpSocket::bind(("0.0.0.0", *port)).await {
                Ok(socket) => socket,
                Err(error) => {
                    return End::Lost(format!("écoute impossible sur le port {port} : {error}"))
                }
            };
            if let Err(gone) = connected(session, transport).await {
                return gone.into();
            }
            session.last_valid = Instant::now();
            session.read_udp(socket).await
        }
        Transport::Serial { path, baud_rate } => {
            let builder = tokio_serial::new(path.as_str(), *baud_rate);
            let port = match tokio_serial::SerialStream::open(&builder) {
                Ok(port) => port,
                Err(error) => {
                    return End::Lost(format!("ouverture de {path} impossible : {error}"))
                }
            };
            if let Err(gone) = connected(session, transport).await {
                return gone.into();
            }
            session.last_valid = Instant::now();
            session.read_stream(port).await
        }
        Transport::Replay { path } => {
            let content = match read_replay(path).await {
                Ok(content) => content,
                Err(reason) => return End::Lost(reason),
            };
            if let Err(gone) = connected(session, transport).await {
                return gone.into();
            }
            match session.replay(&content).await {
                Ok(()) => End::Lost("fin de l'enregistrement".to_owned()),
                Err(gone) => gone.into(),
            }
        }
    }
}

async fn read_replay(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|e| format!("lecture de {} impossible : {e}", path.display()))?;
    if !meta.is_file() {
        return Err(format!("{} n'est pas un fichier", path.display()));
    }
    if meta.len() > MAX_REPLAY_BYTES {
        return Err(format!(
            "fichier trop volumineux ({} Mio maximum)",
            MAX_REPLAY_BYTES / (1024 * 1024)
        ));
    }
    tokio::fs::read(path)
        .await
        .map_err(|e| format!("lecture de {} impossible : {e}", path.display()))
}

/// Ports série présents sur la machine (appel bloquant).
#[must_use]
pub fn serial_ports() -> Vec<String> {
    match tokio_serial::available_ports() {
        Ok(ports) => ports.into_iter().map(|p| p.port_name).collect(),
        Err(error) => {
            tracing::warn!(%error, "énumération des ports série impossible");
            Vec::new()
        }
    }
}

/// Lit la liaison jusqu'à l'arrêt demandé par `shutdown` (valeur `true`
/// ou émetteur fermé) ou la fermeture de `events`.
pub async fn run(
    transport: Transport,
    options: ClientOptions,
    events: mpsc::Sender<NmeaEvent>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut attempt: u32 = 0;
    loop {
        if *shutdown.borrow() {
            break;
        }
        attempt = attempt.saturating_add(1);
        if events
            .send(NmeaEvent::State(ConnectionState::Connecting { attempt }))
            .await
            .is_err()
        {
            return;
        }
        let mut state = Session {
            options,
            events: &events,
            origin: transport.origin(),
            source: transport.source_kind(),
            replay: matches!(transport, Transport::Replay { .. }),
            limiter: RateLimiter::new(options.max_sentences_per_second),
            last_valid: Instant::now(),
            receiving: false,
        };
        let end = tokio::select! {
            end = session(&transport, &mut state) => end,
            _ = shutdown.changed() => break,
        };
        if state.receiving {
            // Des données ont circulé : reprise rapide.
            attempt = 1;
        }
        let reason = match end {
            End::Lost(reason) => reason,
            End::Gone => return,
        };
        let delay = options.backoff(attempt);
        tracing::warn!(link = %transport.describe(), %reason, "liaison NMEA perdue");
        let state = ConnectionState::Disconnected {
            reason,
            retry_in_ms: u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
        };
        if events.send(NmeaEvent::State(state)).await.is_err() {
            return;
        }
        tokio::select! {
            () = sleep(delay) => {}
            _ = shutdown.changed() => break,
        }
    }
    let _ = events
        .send(NmeaEvent::State(ConnectionState::Stopped))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_bounded() {
        let o = ClientOptions::default();
        assert_eq!(o.backoff(1), Duration::from_secs(1));
        assert_eq!(o.backoff(3), Duration::from_secs(4));
        assert_eq!(o.backoff(50), Duration::from_secs(30));
    }

    #[tokio::test]
    async fn rate_limiter_reports_first_drop_only() {
        let mut limiter = RateLimiter::new(2);
        assert_eq!(limiter.check(), None);
        assert_eq!(limiter.check(), None);
        assert_eq!(limiter.check(), Some(true));
        assert_eq!(limiter.check(), Some(false));
    }
}
