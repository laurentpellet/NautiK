//! Choix et configuration de la source de données (simulateur ou
//! Signal K).
//!
//! Les réglages (mode, adresse) sont en base ; le jeton Signal K est dans
//! le coffre du système et n'est jamais renvoyé au frontend : celui-ci
//! sait seulement s'il existe.

use nav_signalk::{ConnectionState, SignalKConfig, Token};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::engine::SourceMode;
use crate::routing::CommandResult;
use crate::runtime::{emit_connection, spawn_signalk, AppState};
use crate::secrets::{blocking, SIGNALK_TOKEN};

/// Clé des réglages de source.
pub const SOURCES_SETTING: &str = "sources";

/// Réglages persistés.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSettings {
    /// Source active.
    pub mode: SourceMode,
    /// Adresse du serveur Signal K (conservée même en mode simulateur).
    pub signalk_url: Option<String>,
}

/// Réglages et état, pour l'interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesView {
    /// Source active.
    pub mode: SourceMode,
    /// Adresse du serveur Signal K.
    pub signalk_url: Option<String>,
    /// Un jeton est enregistré dans le coffre.
    pub token_stored: bool,
    /// État de la connexion Signal K (mode Signal K uniquement).
    pub connection: Option<ConnectionState>,
}

/// Valide une demande de réglages. L'adresse est normalisée.
///
/// # Errors
///
/// Message lisible si le mode Signal K est demandé sans adresse valide.
pub fn validate(mode: SourceMode, signalk_url: Option<&str>) -> CommandResult<SourceSettings> {
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
    Ok(SourceSettings {
        mode,
        signalk_url: normalized,
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
        token_stored: token_stored(state).await?,
        connection: state.connection(),
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
/// du moteur et démarre le client Signal K si nécessaire.
pub async fn apply(
    app: &AppHandle,
    state: &AppState,
    settings: &SourceSettings,
) -> CommandResult<()> {
    let generation = state.stop_signalk();
    emit_connection(app, None);
    state.engine().set_mode(settings.mode);
    tracing::info!(mode = ?settings.mode, "source de données");
    if settings.mode != SourceMode::SignalK {
        tracing::warn!("source de position SIMULÉE active");
        return Ok(());
    }
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

/// Modifie la source. `token` : nouveau jeton (absent = inchangé) ;
/// `clear_token` : supprime le jeton enregistré.
#[tauri::command]
pub async fn set_sources(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: SourceMode,
    signalk_url: Option<String>,
    token: Option<String>,
    clear_token: bool,
) -> CommandResult<SourcesView> {
    let settings = validate(mode, signalk_url.as_deref())?;
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
            validate(SourceMode::Simulator, None),
            Ok(SourceSettings::default())
        );
        assert!(validate(SourceMode::SignalK, None).is_err());
        assert!(validate(SourceMode::SignalK, Some("  ")).is_err());
        assert!(validate(SourceMode::SignalK, Some("ftp://x")).is_err());
        assert_eq!(
            validate(SourceMode::SignalK, Some(" http://boat.local:3000/ ")),
            Ok(SourceSettings {
                mode: SourceMode::SignalK,
                signalk_url: Some("http://boat.local:3000".to_owned()),
            })
        );
        // L'adresse est conservée (et validée) en mode simulateur.
        assert!(validate(SourceMode::Simulator, Some("pas une url")).is_err());
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
        })
        .expect("json");
        assert_eq!(json["mode"], "signalK");
        assert_eq!(json["signalkUrl"], "http://h");
    }
}
