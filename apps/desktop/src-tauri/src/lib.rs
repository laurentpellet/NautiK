//! Application desktop NautiK (Tauri 2).
//!
//! Cette crate est une fine couche d'adaptation : elle expose au
//! frontend Svelte des commandes typées qui délèguent au moteur Rust
//! (`nav-*`). Aucune logique métier n'est implémentée ici.

mod commands;

use anyhow::Context;
use tracing_subscriber::EnvFilter;

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
        .invoke_handler(tauri::generate_handler![commands::get_engine_info])
        .run(tauri::generate_context!())
        .context("exécution de l'application Tauri")
}
