//! Intégration systemd (`Type=notify`) : signalement de disponibilité,
//! d'arrêt et chien de garde (`WatchdogSec=`), via le protocole
//! `sd_notify` (datagramme sur `$NOTIFY_SOCKET`), sans dépendance.
//!
//! Le chien de garde n'est réarmé que si la boucle moteur progresse :
//! si elle se bloque, systemd arrête puis relance le service. Hors
//! systemd (ou hors Linux), ces fonctions sont sans effet.

use std::time::Duration;

/// Au-delà de ce retard du dernier pas de boucle, le chien de garde
/// n'est plus réarmé.
pub const STALL_THRESHOLD: Duration = Duration::from_secs(5);

/// Période de réarmement : la moitié du délai configuré, comme le
/// recommande systemd. `None` si le chien de garde n'est pas actif pour
/// ce processus.
#[must_use]
pub fn watchdog_period(usec: Option<&str>, pid: Option<&str>, own_pid: u32) -> Option<Duration> {
    if let Some(pid) = pid {
        if pid.trim().parse::<u32>().ok()? != own_pid {
            return None;
        }
    }
    let usec: u64 = usec?.trim().parse().ok()?;
    (usec > 0).then(|| Duration::from_micros(usec / 2))
}

/// Canal de notification vers systemd.
pub struct Notifier {
    #[cfg(target_os = "linux")]
    socket: std::os::unix::net::UnixDatagram,
    #[cfg(target_os = "linux")]
    addr: std::os::unix::net::SocketAddr,
}

impl Notifier {
    /// Notificateur décrit par `$NOTIFY_SOCKET`, s'il existe.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let path = std::env::var("NOTIFY_SOCKET").ok()?;
        match Self::connect(&path) {
            Ok(notifier) => Some(notifier),
            Err(error) => {
                tracing::warn!(%error, "notification systemd indisponible");
                None
            }
        }
    }

    /// Notificateur vers une socket donnée (`@nom` : socket abstraite).
    ///
    /// # Errors
    ///
    /// Erreur d'entrée/sortie, ou plate-forme sans systemd.
    #[cfg(target_os = "linux")]
    pub fn connect(path: &str) -> std::io::Result<Self> {
        use std::os::linux::net::SocketAddrExt;
        use std::os::unix::net::{SocketAddr, UnixDatagram};
        let addr = match path.strip_prefix('@') {
            Some(name) => SocketAddr::from_abstract_name(name.as_bytes())?,
            None => SocketAddr::from_pathname(path)?,
        };
        Ok(Self {
            socket: UnixDatagram::unbound()?,
            addr,
        })
    }

    /// Notificateur vers une socket donnée.
    ///
    /// # Errors
    ///
    /// Toujours : systemd n'existe que sous Linux.
    #[cfg(not(target_os = "linux"))]
    pub fn connect(_path: &str) -> std::io::Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "systemd n'est disponible que sous Linux",
        ))
    }

    /// Envoie un message (`READY=1`, `WATCHDOG=1`, `STOPPING=1`,
    /// `STATUS=…`). Un échec est journalisé.
    pub fn notify(&self, message: &str) {
        #[cfg(target_os = "linux")]
        if let Err(error) = self.socket.send_to_addr(message.as_bytes(), &self.addr) {
            tracing::warn!(%error, message, "notification systemd non transmise");
        }
        #[cfg(not(target_os = "linux"))]
        let _ = message;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchdog_period_is_half_the_configured_delay() {
        assert_eq!(
            watchdog_period(Some("30000000"), None, 42),
            Some(Duration::from_secs(15))
        );
        assert_eq!(
            watchdog_period(Some("30000000"), Some("42"), 42),
            Some(Duration::from_secs(15))
        );
        assert_eq!(
            watchdog_period(Some("30000000"), Some("7"), 42),
            None,
            "autre processus"
        );
        assert_eq!(watchdog_period(None, None, 42), None);
        assert_eq!(watchdog_period(Some("0"), None, 42), None);
        assert_eq!(watchdog_period(Some("abc"), None, 42), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn notifications_reach_the_socket() {
        let dir = tempfile::tempdir().expect("tmp");
        let path = dir.path().join("notify");
        let server = std::os::unix::net::UnixDatagram::bind(&path).expect("bind");
        let notifier = Notifier::connect(path.to_str().expect("chemin")).expect("connexion");
        notifier.notify("READY=1");
        let mut buf = [0u8; 64];
        let n = server.recv(&mut buf).expect("reçu");
        assert_eq!(&buf[..n], b"READY=1");
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn notifier_is_unavailable_outside_linux() {
        assert!(Notifier::connect("/run/systemd/notify").is_err());
    }
}
