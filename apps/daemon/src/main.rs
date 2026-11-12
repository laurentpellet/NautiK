//! Service NautiK autonome.
//!
//! Exécute le moteur de navigation sans interface graphique (mode
//! Raspberry Pi / OpenPlotter). En phase 0, le service initialise les
//! journaux puis attend un signal d'arrêt.

use anyhow::Context;
use nav_core::EngineInfo;
use tracing::info;
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing()?;

    let engine = EngineInfo::current();
    info!(
        version = %engine.version,
        protocol = engine.protocol_version,
        "démarrage du service NautiK"
    );

    tokio::signal::ctrl_c()
        .await
        .context("attente du signal d'arrêt")?;

    info!("arrêt du service NautiK");
    Ok(())
}
