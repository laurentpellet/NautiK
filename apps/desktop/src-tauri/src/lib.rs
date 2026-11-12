//! Application desktop NautiK (Tauri 2).
//!
//! Cette crate est une fine couche d'adaptation : elle expose au
//! frontend Svelte des commandes typées, un protocole local de tuiles et
//! des événements, qui délèguent au moteur Rust (`nav-*`). Aucune logique
//! métier n'est implémentée ici.

mod charts;
mod commands;
mod position;

use anyhow::Context;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

use crate::charts::{resolve_charts_dir, to_http_response, ChartState, TILE_SCHEME};

/// Niveau de journalisation par défaut si `RUST_LOG` n'est pas défini.
const DEFAULT_LOG_FILTER: &str = "info";

fn init_tracing() -> anyhow::Result<()> {
    let filter =
        EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new(DEFAULT_LOG_FILTER))?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init()
        .map_err(|e| anyhow::anyhow!(e))
        .context("initialisation de tracing")
}

/// Démarre l'application desktop.
///
/// # Errors
///
/// Retourne une erreur si les journaux ou le runtime Tauri ne peuvent
/// pas être initialisés.
pub fn run() -> anyhow::Result<()> {
    init_tracing()?;
    tracing::info!(version = nav_core::VERSION, "démarrage de NautiK desktop");

    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol(TILE_SCHEME, |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_owned();
            tauri::async_runtime::spawn(async move {
                let reply = match app.try_state::<ChartState>() {
                    Some(state) => state.serve_tile(&path).await,
                    None => nav_charts::TileReply {
                        status: 503,
                        content_type: None,
                        body: Vec::new(),
                    },
                };
                responder.respond(to_http_response(reply));
            });
        })
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .context("répertoire de données de l'application")?;
            let charts_dir = resolve_charts_dir(&data_dir)?;
            tracing::info!(dir = %charts_dir.display(), "répertoire des cartes");
            app.manage(ChartState::new(charts_dir));
            position::spawn_simulator(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_engine_info,
            commands::get_chart_catalog
        ])
        .run(tauri::generate_context!())
        .context("exécution de l'application Tauri")
}
