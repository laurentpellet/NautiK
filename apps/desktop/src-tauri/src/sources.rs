//! Choix et configuration de la source de données (simulateur, Signal K
//! ou NMEA 0183).
//!
//! Les réglages (mode, adresse) sont en base ; le jeton Signal K est dans
//! le coffre du système et n'est jamais renvoyé au frontend : celui-ci
//! sait seulement s'il existe.

use std::path::PathBuf;

use nav_core::{ConnectionState, Priorities};
use nav_nmea0183::Transport;
use nav_signalk::{SignalKConfig, Token};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

use crate::engine::SourceMode;
use crate::routing::CommandResult;
use crate::runtime::{
    emit_connection, emit_rejections, spawn_nmea, spawn_signalk, AppState, Rejections,
};
use crate::secrets::{blocking, SIGNALK_TOKEN};

/// Clé des réglages de source.
pub const SOURCES_SETTING: &str = "sources";

/// Nombre maximal d'entrées de priorité.
pub const MAX_PRIORITIES: usize = 16;

/// Liaison NMEA 0183 (réglages persistés et échangés avec l'interface).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NmeaSettings {
    /// Client TCP.
    Tcp {
        /// Hôte.
        host: String,
        /// Port.
        port: u16,
    },
    /// Écoute UDP.
    Udp {
        /// Port.
        port: u16,
    },
    /// Port série.
    Serial {
        /// Nom du port.
        path: String,
        /// Débit.
        baud_rate: u32,
    },
    /// Rejeu d'un fichier enregistré.
    Replay {
        /// Chemin absolu du fichier.
        path: String,
    },
}

impl NmeaSettings {
    /// Réglages normalisés (espaces retirés).
    fn trimmed(self) -> Self {
        match self {
            Self::Tcp { host, port } => Self::Tcp {
                host: host.trim().to_owned(),
                port,
            },
            Self::Serial { path, baud_rate } => Self::Serial {
                path: path.trim().to_owned(),
                baud_rate,
            },
            Self::Replay { path } => Self::Replay {
                path: path.trim().to_owned(),
            },
            udp @ Self::Udp { .. } => udp,
        }
    }

    /// Liaison correspondante.
    #[must_use]
    pub fn transport(&self) -> Transport {
        match self {
            Self::Tcp { host, port } => Transport::Tcp {
                host: host.clone(),
                port: *port,
            },
            Self::Udp { port } => Transport::Udp { port: *port },
            Self::Serial { path, baud_rate } => Transport::Serial {
                path: path.clone(),
                baud_rate: *baud_rate,
            },
            Self::Replay { path } => Transport::Replay {
                path: PathBuf::from(path),
            },
        }
    }

    fn is_replay(&self) -> bool {
        matches!(self, Self::Replay { .. })
    }
}

/// Réglages persistés.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSettings {
    /// Source active.
    pub mode: SourceMode,
    /// Adresse du serveur Signal K (conservée même en mode simulateur).
    pub signalk_url: Option<String>,
    /// Liaison NMEA 0183 (conservée quel que soit le mode).
    #[serde(default)]
    pub nmea: Option<NmeaSettings>,
    /// Priorités des sources, de la plus prioritaire à la moins
    /// prioritaire (identifiants ou préfixes, ex. `nmea0183.GP`).
    #[serde(default)]
    pub priorities: Vec<String>,
}

/// Réglages et état, pour l'interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesView {
    /// Source active.
    pub mode: SourceMode,
    /// Adresse du serveur Signal K.
    pub signalk_url: Option<String>,
    /// Liaison NMEA 0183.
    pub nmea: Option<NmeaSettings>,
    /// Priorités des sources.
    pub priorities: Vec<String>,
    /// Un jeton est enregistré dans le coffre.
    pub token_stored: bool,
    /// État de la liaison (hors mode simulateur).
    pub connection: Option<ConnectionState>,
    /// Données rejetées par la liaison courante.
    pub rejections: Rejections,
}

/// Valide une liaison NMEA. Le fichier d'un rejeu doit exister lorsque
/// la liaison est activée.
fn validate_nmea(nmea: NmeaSettings, active: bool) -> CommandResult<NmeaSettings> {
    let nmea = nmea.trimmed();
    nmea.transport().validate().map_err(|e| e.to_string())?;
    if let (true, NmeaSettings::Replay { path }) = (active, &nmea) {
        if !std::path::Path::new(path).is_file() {
            return Err(format!("fichier d'enregistrement introuvable : {path}"));
        }
    }
    Ok(nmea)
}

/// Valide la liste des priorités (entrées vides retirées).
fn validate_priorities(priorities: Vec<String>) -> CommandResult<Vec<String>> {
    let entries: Vec<String> = priorities
        .into_iter()
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect();
    if entries.len() > MAX_PRIORITIES {
        return Err(format!("{MAX_PRIORITIES} priorités au maximum"));
    }
    for entry in &entries {
        let valid = entry.len() <= 64
            && entry
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':'));
        if !valid {
            return Err(format!("priorité invalide : {entry}"));
        }
    }
    Ok(entries)
}

/// Valide une demande de réglages. L'adresse est normalisée.
///
/// # Errors
///
/// Message lisible si la source demandée n'est pas correctement
/// configurée.
pub fn validate(
    mode: SourceMode,
    signalk_url: Option<&str>,
    nmea: Option<NmeaSettings>,
    priorities: Vec<String>,
) -> CommandResult<SourceSettings> {
    let url = signalk_url.map(str::trim).filter(|u| !u.is_empty());
    let normalized = match url {
        Some(u) => Some(
            SignalKConfig::new(u, None)
                .map_err(|e| e.to_string())?
                .display_url(),
        ),
        None => None,
    };
    if mode == SourceMode::SignalK && normalized.is_none() {
        return Err("adresse du serveur Signal K requise".to_owned());
    }
    let nmea = nmea
        .map(|n| validate_nmea(n, mode == SourceMode::Nmea0183))
        .transpose()?;
    if mode == SourceMode::Nmea0183 && nmea.is_none() {
        return Err("liaison NMEA 0183 requise".to_owned());
    }
    Ok(SourceSettings {
        mode,
        signalk_url: normalized,
        nmea,
        priorities: validate_priorities(priorities)?,
    })
}

async fn token_stored(state: &AppState) -> CommandResult<bool> {
    blocking(&state.secrets, |s| s.get(SIGNALK_TOKEN))
        .await
        .map(|t| t.is_some())
        .map_err(|e| e.to_string())
}

pub(crate) async fn view(state: &AppState, settings: SourceSettings) -> CommandResult<SourcesView> {
    Ok(SourcesView {
        mode: settings.mode,
        signalk_url: settings.signalk_url,
        nmea: settings.nmea,
        priorities: settings.priorities,
        token_stored: token_stored(state).await?,
        connection: state.connection(),
        rejections: state.rejections(),
    })
}

/// Réglages enregistrés (simulateur par défaut). Des réglages illisibles
/// sont signalés et remplacés par le simulateur.
pub async fn load(state: &AppState) -> SourceSettings {
    match state
        .db
        .get_setting::<SourceSettings>(SOURCES_SETTING)
        .await
    {
        Ok(settings) => settings.unwrap_or_default(),
        Err(error) => {
            tracing::error!(%error, "réglages de source illisibles : simulateur utilisé");
            SourceSettings::default()
        }
    }
}

/// Applique des réglages : arrête la source précédente, change le mode
/// du moteur et démarre le connecteur nécessaire.
pub async fn apply(
    app: &AppHandle,
    state: &AppState,
    settings: &SourceSettings,
) -> CommandResult<()> {
    let generation = state.stop_source();
    emit_connection(app, None);
    emit_rejections(app, &Rejections::default());
    let replay = settings.nmea.as_ref().is_some_and(NmeaSettings::is_replay);
    {
        let mut engine = state.engine();
        engine.set_priorities(Priorities::new(settings.priorities.clone()));
        engine.set_mode(settings.mode, replay);
    }
    tracing::info!(mode = ?settings.mode, "source de données");
    match settings.mode {
        SourceMode::Simulator => {
            tracing::warn!("source de position SIMULÉE active");
            Ok(())
        }
        SourceMode::SignalK => start_signalk(app, state, settings, generation).await,
        SourceMode::Nmea0183 => {
            let transport = settings
                .nmea
                .as_ref()
                .ok_or("liaison NMEA 0183 requise")?
                .transport();
            transport.validate().map_err(|e| e.to_string())?;
            if replay {
                tracing::warn!("REJEU d'enregistrement : position non réelle");
            }
            spawn_nmea(app.clone(), state, transport, generation);
            Ok(())
        }
    }
}

async fn start_signalk(
    app: &AppHandle,
    state: &AppState,
    settings: &SourceSettings,
    generation: u64,
) -> CommandResult<()> {
    let url = settings
        .signalk_url
        .as_deref()
        .ok_or("adresse du serveur Signal K requise")?;
    let token = blocking(&state.secrets, |s| s.get(SIGNALK_TOKEN))
        .await
        .map_err(|e| e.to_string())?
        .map(|t| Token::new(&t))
        .transpose()
        .map_err(|_| "jeton enregistré invalide : saisissez-le à nouveau".to_owned())?;
    let config = SignalKConfig::new(url, token).map_err(|e| e.to_string())?;
    spawn_signalk(app.clone(), state, config, generation);
    Ok(())
}

/// Réglages et état de la source.
#[tauri::command]
pub async fn get_sources(state: State<'_, AppState>) -> CommandResult<SourcesView> {
    let settings = load(&state).await;
    view(&state, settings).await
}

/// Demande de modification de la source.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesRequest {
    /// Source à activer.
    pub mode: SourceMode,
    /// Adresse du serveur Signal K.
    pub signalk_url: Option<String>,
    /// Liaison NMEA 0183.
    pub nmea: Option<NmeaSettings>,
    /// Priorités des sources.
    #[serde(default)]
    pub priorities: Vec<String>,
    /// Nouveau jeton Signal K (absent = inchangé).
    pub token: Option<String>,
    /// Supprime le jeton enregistré.
    #[serde(default)]
    pub clear_token: bool,
}

/// Modifie la source.
#[tauri::command]
pub async fn set_sources(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SourcesRequest,
) -> CommandResult<SourcesView> {
    let SourcesRequest {
        mode,
        signalk_url,
        nmea,
        priorities,
        token,
        clear_token,
    } = request;
    let settings = validate(mode, signalk_url.as_deref(), nmea, priorities)?;
    let new_token = token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(Token::new)
        .transpose()
        .map_err(|e| e.to_string())?;
    if clear_token {
        blocking(&state.secrets, |s| s.delete(SIGNALK_TOKEN))
            .await
            .map_err(|e| e.to_string())?;
    }
    if let Some(token) = new_token {
        let value = token.expose().to_owned();
        blocking(&state.secrets, move |s| s.set(SIGNALK_TOKEN, &value))
            .await
            .map_err(|e| e.to_string())?;
    }
    state
        .db
        .set_setting(SOURCES_SETTING, &settings)
        .await
        .map_err(|e| e.to_string())?;
    apply(&app, &state, &settings).await?;
    view(&state, settings).await
}

/// Ports série présents sur la machine.
#[tauri::command]
pub async fn list_serial_ports() -> CommandResult<Vec<String>> {
    tauri::async_runtime::spawn_blocking(nav_nmea0183::serial_ports)
        .await
        .map_err(|e| e.to_string())
}

/// Choix d'un enregistrement NMEA à rejouer (dialogue du système). Le
/// chemin vient toujours du dialogue, jamais d'une saisie libre.
#[tauri::command]
pub async fn choose_nmea_recording(app: AppHandle) -> CommandResult<Option<String>> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .set_title("Rejouer un enregistrement NMEA 0183")
        .add_filter("NMEA 0183", &["nmea", "log", "txt"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::engine::Engine;
    use crate::secrets::MemoryStore;
    use nav_core::GeoPosition;
    use nav_storage::Db;

    async fn state() -> AppState {
        let db = Db::open_in_memory().await.expect("db");
        let start = GeoPosition::new(-14.968, -147.638).expect("pos");
        AppState::new(
            db,
            Engine::new(start, 4.0),
            Arc::new(MemoryStore::default()),
        )
    }

    #[test]
    fn validation_requires_url_for_signalk() {
        assert_eq!(
            validate(SourceMode::Simulator, None, None, Vec::new()),
            Ok(SourceSettings::default())
        );
        assert!(validate(SourceMode::SignalK, None, None, Vec::new()).is_err());
        assert!(validate(SourceMode::SignalK, Some("  "), None, Vec::new()).is_err());
        assert!(validate(SourceMode::SignalK, Some("ftp://x"), None, Vec::new()).is_err());
        assert_eq!(
            validate(
                SourceMode::SignalK,
                Some(" http://boat.local:3000/ "),
                None,
                Vec::new()
            ),
            Ok(SourceSettings {
                mode: SourceMode::SignalK,
                signalk_url: Some("http://boat.local:3000".to_owned()),
                nmea: None,
                priorities: Vec::new(),
            })
        );
        // L'adresse est conservée (et validée) en mode simulateur.
        assert!(validate(SourceMode::Simulator, Some("pas une url"), None, Vec::new()).is_err());
    }

    #[test]
    fn validation_of_nmea_links() {
        assert_eq!(
            validate(SourceMode::Nmea0183, None, None, Vec::new()),
            Err("liaison NMEA 0183 requise".to_owned())
        );
        let tcp = NmeaSettings::Tcp {
            host: " 192.168.1.10 ".to_owned(),
            port: 10110,
        };
        let s = validate(SourceMode::Nmea0183, None, Some(tcp), Vec::new()).expect("tcp");
        assert_eq!(
            s.nmea,
            Some(NmeaSettings::Tcp {
                host: "192.168.1.10".to_owned(),
                port: 10110
            })
        );
        let serial = NmeaSettings::Serial {
            path: "/etc/passwd".to_owned(),
            baud_rate: 4800,
        };
        assert!(validate(SourceMode::Nmea0183, None, Some(serial), Vec::new()).is_err());
        let bad_baud = NmeaSettings::Serial {
            path: "COM3".to_owned(),
            baud_rate: 1200,
        };
        assert!(validate(SourceMode::Simulator, None, Some(bad_baud), Vec::new()).is_err());
    }

    #[test]
    fn replay_file_must_exist_when_active() {
        let missing = NmeaSettings::Replay {
            path: std::env::temp_dir()
                .join("nautik-absent-9a1.nmea")
                .display()
                .to_string(),
        };
        assert!(validate(
            SourceMode::Nmea0183,
            None,
            Some(missing.clone()),
            Vec::new()
        )
        .is_err());
        // Conservé tel quel lorsqu'il n'est pas utilisé.
        assert!(validate(SourceMode::Simulator, None, Some(missing), Vec::new()).is_ok());
        let file = tempfile::NamedTempFile::new().expect("tmp");
        let present = NmeaSettings::Replay {
            path: file.path().display().to_string(),
        };
        assert!(validate(SourceMode::Nmea0183, None, Some(present), Vec::new()).is_ok());
        let relative = NmeaSettings::Replay {
            path: "x.nmea".to_owned(),
        };
        assert!(validate(SourceMode::Simulator, None, Some(relative), Vec::new()).is_err());
    }

    #[test]
    fn nmea_settings_json_format() {
        let json = serde_json::to_value(NmeaSettings::Serial {
            path: "COM3".to_owned(),
            baud_rate: 4800,
        })
        .expect("json");
        assert_eq!(json["kind"], "serial");
        assert_eq!(json["baudRate"], 4800);
        let back: NmeaSettings =
            serde_json::from_str(r#"{"kind":"tcp","host":"h","port":2000}"#).expect("tcp");
        assert_eq!(
            back,
            NmeaSettings::Tcp {
                host: "h".to_owned(),
                port: 2000
            }
        );
    }

    #[test]
    fn priorities_are_validated() {
        let ok = validate(
            SourceMode::Simulator,
            None,
            None,
            vec![
                " nmea0183.GP ".to_owned(),
                String::new(),
                "signalk".to_owned(),
            ],
        )
        .expect("ok");
        assert_eq!(ok.priorities, vec!["nmea0183.GP", "signalk"]);
        for bad in ["a b", "x/y", &"x".repeat(65)] {
            assert!(
                validate(SourceMode::Simulator, None, None, vec![bad.to_owned()]).is_err(),
                "{bad}"
            );
        }
        let many = (0..=MAX_PRIORITIES).map(|i| format!("s{i}")).collect();
        assert!(validate(SourceMode::Simulator, None, None, many).is_err());
    }

    #[test]
    fn request_defaults_optional_fields() {
        let r: SourcesRequest = serde_json::from_str(
            r#"{"mode":"simulator","signalkUrl":null,"nmea":null,"token":null}"#,
        )
        .expect("requête");
        assert!(r.priorities.is_empty());
        assert!(!r.clear_token);
    }

    #[test]
    fn older_settings_without_nmea_are_read() {
        let old: SourceSettings =
            serde_json::from_str(r#"{"mode":"signalK","signalkUrl":"http://h"}"#).expect("old");
        assert_eq!(old.nmea, None);
        assert_eq!(old.mode, SourceMode::SignalK);
    }

    #[tokio::test]
    async fn settings_default_to_simulator_and_survive_corruption() {
        let s = state().await;
        assert_eq!(load(&s).await, SourceSettings::default());
        s.db.set_setting(SOURCES_SETTING, &"n'importe quoi")
            .await
            .expect("set");
        assert_eq!(load(&s).await, SourceSettings::default());
    }

    #[tokio::test]
    async fn view_never_exposes_the_token() {
        let s = state().await;
        s.secrets.set(SIGNALK_TOKEN, "tres-secret").expect("set");
        let v = view(&s, SourceSettings::default()).await.expect("view");
        assert!(v.token_stored);
        let json = serde_json::to_string(&v).expect("json");
        assert!(!json.contains("tres-secret"), "{json}");
        assert!(json.contains("\"tokenStored\":true"));
    }

    #[test]
    fn settings_serialize_camel_case() {
        let json = serde_json::to_value(SourceSettings {
            mode: SourceMode::SignalK,
            signalk_url: Some("http://h".to_owned()),
            nmea: None,
            priorities: Vec::new(),
        })
        .expect("json");
        assert_eq!(json["mode"], "signalK");
        assert_eq!(json["signalkUrl"], "http://h");
    }
}
