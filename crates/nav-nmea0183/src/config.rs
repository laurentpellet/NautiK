//! Configuration d'une liaison NMEA 0183, validée à la construction.

use std::path::PathBuf;

use nav_core::SourceKind;
use thiserror::Error;

/// Débits série acceptés (NMEA 0183 : 4800 ; AIS et multiplexeurs :
/// 38400 et plus).
pub const BAUD_RATES: [u32; 6] = [4_800, 9_600, 19_200, 38_400, 57_600, 115_200];

/// Port TCP/UDP NMEA usuel.
pub const DEFAULT_PORT: u16 = 10_110;

/// Configuration refusée.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    /// Hôte invalide.
    #[error("hôte invalide : {0}")]
    Host(String),
    /// Port invalide.
    #[error("port invalide (1 à 65535)")]
    Port,
    /// Port série invalide.
    #[error("port série invalide : {0}")]
    SerialPath(String),
    /// Débit non pris en charge.
    #[error("débit non pris en charge : {0} (4800, 9600, 19200, 38400, 57600 ou 115200)")]
    BaudRate(u32),
    /// Fichier d'enregistrement invalide.
    #[error("fichier d'enregistrement invalide : {0}")]
    ReplayPath(String),
}

/// Liaison NMEA 0183.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    /// Client TCP (multiplexeur, passerelle Wi-Fi, Signal K…).
    Tcp {
        /// Hôte (nom ou adresse IP).
        host: String,
        /// Port.
        port: u16,
    },
    /// Écoute UDP (diffusion sur le réseau local).
    Udp {
        /// Port d'écoute.
        port: u16,
    },
    /// Port série (USB, RS-422).
    Serial {
        /// Nom du port (`COM3`, `/dev/ttyUSB0`).
        path: String,
        /// Débit.
        baud_rate: u32,
    },
    /// Rejeu d'un fichier enregistré (essais, démonstration). Les
    /// données sont marquées comme rejouées, jamais comme réelles.
    Replay {
        /// Fichier.
        path: PathBuf,
    },
}

fn is_valid_serial_path(path: &str) -> bool {
    let windows = path
        .strip_prefix(r"\\.\")
        .unwrap_or(path)
        .strip_prefix("COM")
        .is_some_and(|n| !n.is_empty() && n.len() <= 3 && n.bytes().all(|b| b.is_ascii_digit()));
    let unix = path.strip_prefix("/dev/").is_some_and(|name| {
        !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/'))
            && !name.split('/').any(|part| part.is_empty() || part == "..")
    });
    windows || unix
}

impl Transport {
    /// Valide la configuration.
    ///
    /// # Errors
    ///
    /// [`ConfigError`] si un paramètre est invalide.
    pub fn validate(&self) -> Result<(), ConfigError> {
        match self {
            Self::Tcp { host, port } => {
                let ok = !host.is_empty()
                    && host.len() <= 253
                    && host.bytes().all(|b| {
                        b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'_')
                    });
                if !ok {
                    return Err(ConfigError::Host(host.clone()));
                }
                if *port == 0 {
                    return Err(ConfigError::Port);
                }
            }
            Self::Udp { port } => {
                if *port == 0 {
                    return Err(ConfigError::Port);
                }
            }
            Self::Serial { path, baud_rate } => {
                if !is_valid_serial_path(path) {
                    return Err(ConfigError::SerialPath(path.clone()));
                }
                if !BAUD_RATES.contains(baud_rate) {
                    return Err(ConfigError::BaudRate(*baud_rate));
                }
            }
            Self::Replay { path } => {
                if !path.is_absolute() {
                    return Err(ConfigError::ReplayPath(path.display().to_string()));
                }
            }
        }
        Ok(())
    }

    /// Description lisible (journaux, interface).
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Tcp { host, port } => format!("TCP {host}:{port}"),
            Self::Udp { port } => format!("UDP, port {port}"),
            Self::Serial { path, baud_rate } => format!("série {path} ({baud_rate} Bd)"),
            Self::Replay { path } => format!(
                "rejeu de {}",
                path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned()
                )
            ),
        }
    }

    /// Nature des données produites.
    #[must_use]
    pub fn source_kind(&self) -> SourceKind {
        match self {
            Self::Replay { .. } => SourceKind::Replay,
            _ => SourceKind::Nmea0183,
        }
    }

    /// Préfixe des identifiants de source.
    #[must_use]
    pub fn origin(&self) -> &'static str {
        match self {
            Self::Replay { .. } => "replay",
            _ => "nmea0183",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_tcp_and_udp() {
        let tcp = |h: &str, p| Transport::Tcp {
            host: h.to_owned(),
            port: p,
        };
        assert!(tcp("192.168.1.10", 10110).validate().is_ok());
        assert!(tcp("boat.local", 2000).validate().is_ok());
        assert!(tcp("::1", 2000).validate().is_ok());
        assert_eq!(tcp("", 1).validate(), Err(ConfigError::Host(String::new())));
        assert!(tcp("http://x", 1).validate().is_err());
        assert!(tcp("a b", 1).validate().is_err());
        assert_eq!(tcp("x", 0).validate(), Err(ConfigError::Port));
        assert!(Transport::Udp { port: 10110 }.validate().is_ok());
        assert_eq!(
            Transport::Udp { port: 0 }.validate(),
            Err(ConfigError::Port)
        );
    }

    #[test]
    fn validates_serial_ports() {
        let serial = |p: &str, b| Transport::Serial {
            path: p.to_owned(),
            baud_rate: b,
        };
        for ok in [
            "COM3",
            "COM12",
            r"\\.\COM10",
            "/dev/ttyUSB0",
            "/dev/serial/by-id/usb-FTDI_x-if00",
        ] {
            assert!(serial(ok, 4800).validate().is_ok(), "{ok}");
        }
        for bad in [
            "",
            "COM",
            "COMx",
            "C:\\Windows\\x",
            "/etc/passwd",
            "/dev/../etc/passwd",
            "/dev/",
            "tty0",
        ] {
            assert!(
                matches!(
                    serial(bad, 4800).validate(),
                    Err(ConfigError::SerialPath(_))
                ),
                "{bad}"
            );
        }
        assert_eq!(
            serial("COM3", 1234).validate(),
            Err(ConfigError::BaudRate(1234))
        );
    }

    #[test]
    fn replay_requires_absolute_path_and_is_flagged() {
        assert!(Transport::Replay {
            path: PathBuf::from("relatif.nmea")
        }
        .validate()
        .is_err());
        let replay = Transport::Replay {
            path: std::env::temp_dir().join("x.nmea"),
        };
        assert!(replay.validate().is_ok());
        assert_eq!(replay.source_kind(), SourceKind::Replay);
        assert!(replay.source_kind().is_simulated());
        assert_eq!(replay.describe(), "rejeu de x.nmea");
    }
}
