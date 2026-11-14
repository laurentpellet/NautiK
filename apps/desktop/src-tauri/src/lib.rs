//! Application desktop NautiK (Tauri 2).
//!
//! Cette crate est une fine couche d'adaptation : elle expose au
//! frontend Svelte des commandes typées, un protocole local de tuiles et
//! des événements, qui délèguent au moteur Rust partagé (`nav-engine`,
//! également utilisé par le service `nautikd`). Aucune logique métier
//! n'est implémentée ici.

mod alarms;
mod charts;
mod commands;
mod enc;
mod events;
mod gpx_io;
mod routing;
mod secrets;
mod simulator;
mod sources;
mod weather;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use nav_engine::{position, Engine, Service};
use nav_storage::Db;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

use crate::charts::{
    resolve_charts_dir, to_http_response, ChartState, CHARTS_DIR_SETTING, TILE_SCHEME,
};
use crate::secrets::KeyringStore;

/// État géré par Tauri : le moteur partagé.
pub(crate) type AppState = Arc<Service>;

/// Niveau de journalisation par défaut si `RUST_LOG` n'est pas défini.
const DEFAULT_LOG_FILTER: &str = "info";

/// Nom du fichier de base de données (répertoire local de l'application).
const DB_FILE_NAME: &str = "nautik.db";

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

fn open_database(app: &tauri::App) -> anyhow::Result<Db> {
    let dir = app
        .path()
        .app_local_data_dir()
        .context("répertoire local de l'application")?;
    std::fs::create_dir_all(&dir).with_context(|| format!("création de {}", dir.display()))?;
    let path = dir.join(DB_FILE_NAME);
    tracing::info!(path = %path.display(), "base de données");
    let db = tauri::async_runtime::block_on(Db::open(&path))
        .with_context(|| format!("ouverture de {}", path.display()))?;
    // Après un arrêt brutal : base vérifiée, anomalie journalisée.
    tauri::async_runtime::block_on(db.check_integrity());
    tauri::async_runtime::block_on(db.close_interrupted_tracks())
        .context("clôture des enregistrements interrompus")?;
    Ok(db)
}

/// Démarre l'application desktop.
///
/// # Errors
///
/// Retourne une erreur si les journaux, la base de données ou le runtime
/// Tauri ne peuvent pas être initialisés.
pub fn run() -> anyhow::Result<()> {
    init_tracing()?;
    tracing::info!(version = nav_core::VERSION, "démarrage de NautiK desktop");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            app.manage(weather::WeatherStore::new(data_dir.join("weather")));

            let db = open_database(app)?;
            // Dossier de cartes choisi dans les Paramètres ; un réglage
            // illisible est signalé et le dossier par défaut utilisé.
            let chosen = tauri::async_runtime::block_on(
                db.get_setting::<Option<String>>(CHARTS_DIR_SETTING),
            )
            .unwrap_or_else(|error| {
                tracing::error!(%error, "réglage du dossier des cartes illisible");
                None
            })
            .flatten()
            .map(PathBuf::from);
            let (charts_dir, source) = resolve_charts_dir(&data_dir, chosen)?;
            tracing::info!(dir = %charts_dir.display(), ?source, "répertoire des cartes");
            app.manage(ChartState::new(charts_dir, source));
            let start =
                position::simulated_start_position().context("position simulée invalide")?;
            let secrets = Arc::new(KeyringStore::new(&app.config().identifier));
            let service = Service::new(
                db,
                Engine::new(start, position::SIMULATED_SPEED_MPS),
                secrets,
            );
            app.manage(Arc::clone(&service));

            // Relais des événements, réglages enregistrés, source (un
            // échec est journalisé et visible dans l'interface : état de
            // connexion absent), puis boucle moteur.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::spawn(events::forward(handle, Arc::clone(&service)));
                nav_engine::simulator::restore(&service).await;
                nav_engine::alarms::restore(&service).await;
                nav_engine::sources::restore(&service).await;
                service.run().await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_engine_info,
            commands::get_chart_catalog,
            commands::choose_charts_dir,
            commands::reset_charts_dir,
            commands::reload_charts,
            enc::get_enc_folder,
            enc::choose_enc_dir,
            enc::clear_enc_dir,
            enc::inspect_enc_cell,
            routing::list_routes,
            routing::create_route,
            routing::reverse_route,
            routing::delete_route,
            routing::activate_route,
            routing::deactivate_route,
            routing::get_navigation_state,
            routing::list_waypoints,
            routing::delete_waypoint,
            routing::start_track_recording,
            routing::stop_track_recording,
            routing::list_tracks,
            routing::get_track,
            routing::delete_track,
            gpx_io::import_gpx,
            gpx_io::export_route_gpx,
            gpx_io::export_track_gpx,
            sources::get_sources,
            sources::set_sources,
            sources::list_serial_ports,
            sources::choose_nmea_recording,
            simulator::get_simulator_settings,
            simulator::set_simulator_settings,
            weather::list_weather_files,
            weather::import_weather_file,
            weather::download_weather,
            weather::delete_weather_file,
            weather::get_wind_field,
            alarms::get_alarms,
            alarms::drop_anchor,
            alarms::raise_anchor,
            alarms::acknowledge_alarm,
            alarms::set_alarm_enabled,
            alarms::list_alarm_events,
        ])
        .run(tauri::generate_context!())
        .context("exécution de l'application Tauri")
}
