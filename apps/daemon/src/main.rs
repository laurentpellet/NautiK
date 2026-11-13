//! Service NautiK autonome (`nautikd`).
//!
//! Exécute le moteur de navigation sans interface graphique (Raspberry
//! Pi, OpenPlotter, serveur du bord) : acquisition, navigation, traces,
//! AIS et alarmes tournent en permanence ; les écrans se connectent par
//! l'API distante (`nav-api` : REST + WebSocket, jeton d'accès).
//!
//! Sous systemd (`Type=notify`), le service signale sa disponibilité et
//! réarme le chien de garde tant que la boucle moteur progresse.

mod config;
mod files;
mod systemd;

use std::future::Future;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use chrono::Utc;
use nav_core::EngineInfo;
use nav_engine::{position, Engine, Service};
use nav_storage::Db;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::config::{Command, Config, ProcessEnv};
use crate::systemd::Notifier;

/// Niveau de journalisation par défaut si `RUST_LOG` n'est pas défini.
const DEFAULT_LOG_FILTER: &str = "info";

/// Nom du fichier de base de données.
const DB_FILE_NAME: &str = "nautik.db";

/// Délai accordé aux connexions pour se terminer à l'arrêt.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

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

fn prepare_data_dir(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("création de {}", dir.display()))
}

async fn open_database(dir: &Path) -> anyhow::Result<Db> {
    let path = dir.join(DB_FILE_NAME);
    info!(path = %path.display(), "base de données");
    let db = Db::open(&path)
        .await
        .with_context(|| format!("ouverture de {}", path.display()))?;
    // Après un arrêt brutal (coupure d'alimentation) : base vérifiée,
    // anomalie signalée sans empêcher le démarrage.
    db.check_integrity().await;
    db.close_interrupted_tracks()
        .await
        .context("clôture des enregistrements interrompus")?;
    Ok(db)
}

/// Réarme le chien de garde tant que la boucle moteur progresse.
async fn watchdog(service: Arc<Service>, notifier: Arc<Notifier>, period: Duration) {
    let mut interval = tokio::time::interval(period);
    loop {
        interval.tick().await;
        let alive = service.last_tick().is_some_and(|t| {
            (Utc::now() - t)
                .to_std()
                .map_or(true, |age| age < systemd::STALL_THRESHOLD)
        });
        if alive {
            notifier.notify("WATCHDOG=1");
        } else {
            tracing::error!("boucle moteur bloquée : chien de garde non réarmé");
        }
    }
}

/// Attend SIGTERM ou SIGINT (Ctrl+C).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = term.recv() => {}
                    _ = tokio::signal::ctrl_c() => {}
                }
            }
            Err(error) => {
                tracing::warn!(%error, "SIGTERM non surveillé");
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "attente du signal d'arrêt impossible");
        std::future::pending::<()>().await;
    }
}

/// Exécute le service jusqu'à ce que `shutdown` se termine. `on_ready`
/// reçoit l'adresse d'écoute effective.
async fn run<F, R>(config: Config, shutdown: F, on_ready: R) -> anyhow::Result<()>
where
    F: Future<Output = ()>,
    R: FnOnce(SocketAddr),
{
    prepare_data_dir(&config.data_dir)?;
    let token = files::load_or_create_token(&config.data_dir)?;
    let secrets = Arc::new(files::FileStore::open(&config.data_dir)?);
    let db = open_database(&config.data_dir).await?;
    let start = position::simulated_start_position().context("position simulée invalide")?;
    let service = Service::new(
        db,
        Engine::new(start, position::SIMULATED_SPEED_MPS),
        secrets,
    );

    // Réglages enregistrés, source de données, puis boucle moteur.
    nav_engine::simulator::restore(&service).await;
    nav_engine::alarms::restore(&service).await;
    nav_engine::sources::restore(&service).await;
    let engine = tokio::spawn(Arc::clone(&service).run());

    let listener = TcpListener::bind(config.listen)
        .await
        .with_context(|| format!("écoute sur {}", config.listen))?;
    let addr = listener.local_addr().context("adresse d'écoute")?;
    if addr.ip().is_loopback() {
        info!(%addr, "API à l'écoute (accès local uniquement)");
    } else {
        tracing::warn!(%addr, "API ouverte au réseau : jeton requis, réservez-la au réseau du bord");
    }
    let (stop_tx, stop_rx) = watch::channel(false);
    let mut server_stop = stop_rx.clone();
    let server = tokio::spawn(nav_api::serve(
        listener,
        nav_api::router(Arc::clone(&service), token, stop_rx),
        async move {
            let _ = server_stop.wait_for(|stop| *stop).await;
        },
    ));

    let notifier = Notifier::from_env().map(Arc::new);
    let pid = std::process::id();
    let period = systemd::watchdog_period(
        std::env::var("WATCHDOG_USEC").ok().as_deref(),
        std::env::var("WATCHDOG_PID").ok().as_deref(),
        pid,
    );
    let watchdog_task = match (&notifier, period) {
        (Some(n), Some(period)) => {
            info!(?period, "chien de garde systemd actif");
            Some(tokio::spawn(watchdog(
                Arc::clone(&service),
                Arc::clone(n),
                period,
            )))
        }
        _ => None,
    };
    if let Some(n) = &notifier {
        n.notify(&format!("READY=1\nSTATUS=API sur {addr}"));
    }
    on_ready(addr);

    shutdown.await;
    info!("arrêt demandé");
    if let Some(n) = &notifier {
        n.notify("STOPPING=1");
    }
    if let Some(task) = watchdog_task {
        task.abort();
    }
    // Trace en cours : terminée proprement plutôt qu'interrompue.
    if service.engine().recording_track_id().is_some() {
        if let Err(error) = nav_engine::routing::stop_recording(&service).await {
            tracing::warn!(%error, "trace non clôturée");
        }
    }
    let _ = stop_tx.send(true);
    service.shutdown();
    match tokio::time::timeout(SHUTDOWN_GRACE, server).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(error))) => tracing::error!(%error, "serveur API"),
        Ok(Err(error)) => tracing::error!(%error, "tâche du serveur API"),
        Err(_) => tracing::warn!("connexions API interrompues à l'arrêt"),
    }
    let _ = tokio::time::timeout(SHUTDOWN_GRACE, engine).await;
    info!("arrêt du service NautiK");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let command = match config::parse(std::env::args().skip(1), &ProcessEnv) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("nautikd : {message}");
            std::process::exit(2);
        }
    };
    match command {
        Command::Help => {
            print!("{}", config::USAGE);
            Ok(())
        }
        Command::Version => {
            println!("nautikd {}", nav_core::VERSION);
            Ok(())
        }
        Command::RotateToken(config) => {
            prepare_data_dir(&config.data_dir)?;
            files::rotate_token(&config.data_dir).context("renouvellement du jeton")?;
            // Le jeton n'est jamais affiché : il se lit dans son fichier.
            println!(
                "Nouveau jeton enregistré dans {} (redémarrez le service).",
                config.data_dir.join(files::TOKEN_FILE).display()
            );
            Ok(())
        }
        Command::Run(config) => {
            init_tracing()?;
            let engine = EngineInfo::current();
            info!(
                version = %engine.version,
                protocol = engine.protocol_version,
                data_dir = %config.data_dir.display(),
                "démarrage du service NautiK"
            );
            run(config, shutdown_signal(), |_| {}).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::oneshot;

    async fn http_get(addr: SocketAddr, path: &str, token: Option<&str>) -> String {
        let mut stream = tokio::net::TcpStream::connect(addr)
            .await
            .expect("connexion");
        let auth = token.map_or_else(String::new, |t| format!("Authorization: Bearer {t}\r\n"));
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n{auth}Connection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).await.expect("envoi");
        let mut response = String::new();
        stream.read_to_string(&mut response).await.expect("réponse");
        response
    }

    #[tokio::test]
    async fn service_runs_headless_and_stops_cleanly() {
        let dir = tempfile::tempdir().expect("tmp");
        let config = Config {
            data_dir: dir.path().join("donnees"),
            listen: "127.0.0.1:0".parse().expect("adresse"),
        };
        let (ready_tx, ready_rx) = oneshot::channel();
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(run(
            config,
            async move {
                let _ = stop_rx.await;
            },
            move |addr| {
                let _ = ready_tx.send(addr);
            },
        ));
        let addr = tokio::time::timeout(Duration::from_secs(10), ready_rx)
            .await
            .expect("prêt à temps")
            .expect("adresse");

        // Fichiers créés : base, jeton.
        let token_path = dir.path().join("donnees").join(files::TOKEN_FILE);
        let token = std::fs::read_to_string(&token_path).expect("jeton");
        assert!(dir.path().join("donnees").join(DB_FILE_NAME).exists());

        // La boucle tourne sans aucune interface.
        let mut healthy = false;
        for _ in 0..50 {
            if http_get(addr, "/health", None)
                .await
                .starts_with("HTTP/1.1 200")
            {
                healthy = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(healthy, "boucle moteur active");
        assert!(http_get(addr, "/api/v1/alarms", None)
            .await
            .starts_with("HTTP/1.1 401"));
        let alarms = http_get(addr, "/api/v1/alarms", Some(token.trim())).await;
        assert!(alarms.starts_with("HTTP/1.1 200"), "{alarms}");
        assert!(alarms.contains("positionLost"));

        stop_tx.send(()).expect("arrêt");
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .expect("arrêt à temps")
            .expect("tâche")
            .expect("arrêt sans erreur");

        // Redémarrage : même jeton, base reprise.
        let reused = files::load_or_create_token(&dir.path().join("donnees")).expect("jeton");
        assert_eq!(reused.expose(), token.trim());
    }

    #[tokio::test]
    async fn busy_port_is_reported() {
        let taken = TcpListener::bind("127.0.0.1:0").await.expect("port");
        let dir = tempfile::tempdir().expect("tmp");
        let config = Config {
            data_dir: dir.path().to_path_buf(),
            listen: taken.local_addr().expect("adresse"),
        };
        let error = run(config, std::future::pending::<()>(), |_| {})
            .await
            .expect_err("port occupé");
        assert!(error.to_string().contains("écoute sur"), "{error}");
    }
}
