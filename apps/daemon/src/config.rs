//! Configuration du service : ligne de commande et variables
//! d'environnement (sans dépendance supplémentaire).
//!
//! Priorité : option > variable `NAUTIK_*` > `STATE_DIRECTORY` (fourni
//! par systemd) > valeur par défaut.

use std::net::SocketAddr;
use std::path::PathBuf;

/// Adresse d'écoute par défaut : boucle locale uniquement. L'ouverture
/// au réseau du bord est un choix explicite (`--listen 0.0.0.0:8375`).
pub const DEFAULT_LISTEN: &str = "127.0.0.1:8375";

/// Répertoire de données par défaut (hors systemd).
pub const DEFAULT_DATA_DIR: &str = "nautik-data";

/// Aide de la ligne de commande.
pub const USAGE: &str = "\
nautikd — service de navigation NautiK (sans interface)

Usage : nautikd [options]

Options :
  --data-dir <dir>   Répertoire des données (base, jeton, secrets).
                     Défaut : $NAUTIK_DATA_DIR, $STATE_DIRECTORY, ./nautik-data
  --listen <addr>    Adresse d'écoute de l'API (défaut : $NAUTIK_LISTEN,
                     127.0.0.1:8375). 0.0.0.0:8375 ouvre l'API au réseau.
  --rotate-token     Génère un nouveau jeton d'accès puis quitte.
  -h, --help         Affiche cette aide.
  -V, --version      Affiche la version.

Le jeton d'accès est dans <data-dir>/api-token (lisible du seul service).
";

/// Action demandée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Démarrer le service.
    Run(Config),
    /// Renouveler le jeton d'accès.
    RotateToken(Config),
    /// Afficher l'aide.
    Help,
    /// Afficher la version.
    Version,
}

/// Configuration du service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Répertoire des données.
    pub data_dir: PathBuf,
    /// Adresse d'écoute de l'API.
    pub listen: SocketAddr,
}

/// Lecture d'une variable d'environnement (injectable pour les tests).
pub trait Env {
    /// Valeur de la variable, si elle est définie et non vide.
    fn var(&self, name: &str) -> Option<String>;
}

/// Environnement du processus.
pub struct ProcessEnv;

impl Env for ProcessEnv {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.trim().is_empty())
    }
}

fn parse_listen(value: &str) -> Result<SocketAddr, String> {
    value
        .trim()
        .parse()
        .map_err(|_| format!("adresse d'écoute invalide « {value} » (ex. 127.0.0.1:8375)"))
}

/// Analyse les arguments (sans le nom du programme).
///
/// # Errors
///
/// Message lisible si une option est inconnue, incomplète ou invalide.
pub fn parse<I, S>(args: I, env: &dyn Env) -> Result<Command, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut data_dir: Option<PathBuf> = None;
    let mut listen: Option<SocketAddr> = None;
    let mut rotate = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let arg = arg.as_ref();
        match arg {
            "-h" | "--help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "--rotate-token" => rotate = true,
            "--data-dir" | "--listen" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("valeur manquante après {arg}"))?;
                let value = value.as_ref();
                if arg == "--data-dir" {
                    if value.trim().is_empty() {
                        return Err("répertoire de données vide".to_owned());
                    }
                    data_dir = Some(PathBuf::from(value));
                } else {
                    listen = Some(parse_listen(value)?);
                }
            }
            other => return Err(format!("option inconnue « {other} » (voir --help)")),
        }
    }
    let data_dir = data_dir
        .or_else(|| env.var("NAUTIK_DATA_DIR").map(PathBuf::from))
        // systemd peut fournir plusieurs répertoires séparés par « : ».
        .or_else(|| {
            env.var("STATE_DIRECTORY")
                .and_then(|v| v.split(':').next().map(PathBuf::from))
        })
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR));
    let listen = match listen {
        Some(listen) => listen,
        None => parse_listen(
            &env.var("NAUTIK_LISTEN")
                .unwrap_or_else(|| DEFAULT_LISTEN.to_owned()),
        )?,
    };
    let config = Config { data_dir, listen };
    Ok(if rotate {
        Command::RotateToken(config)
    } else {
        Command::Run(config)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakeEnv(HashMap<&'static str, &'static str>);

    impl Env for FakeEnv {
        fn var(&self, name: &str) -> Option<String> {
            self.0.get(name).map(|v| (*v).to_owned())
        }
    }

    fn env(vars: &[(&'static str, &'static str)]) -> FakeEnv {
        FakeEnv(vars.iter().copied().collect())
    }

    fn run(args: &[&str], e: &FakeEnv) -> Config {
        match parse(args, e).expect("valide") {
            Command::Run(c) => c,
            other => panic!("Run attendu : {other:?}"),
        }
    }

    #[test]
    fn defaults_listen_on_loopback_only() {
        let c = run(&[], &env(&[]));
        assert_eq!(c.data_dir, PathBuf::from(DEFAULT_DATA_DIR));
        assert_eq!(c.listen, DEFAULT_LISTEN.parse().expect("adresse"));
        assert!(c.listen.ip().is_loopback());
    }

    #[test]
    fn options_override_environment() {
        let e = env(&[
            ("NAUTIK_DATA_DIR", "/srv/nautik"),
            ("STATE_DIRECTORY", "/var/lib/nautik:/autre"),
            ("NAUTIK_LISTEN", "0.0.0.0:9000"),
        ]);
        let c = run(&[], &e);
        assert_eq!(c.data_dir, PathBuf::from("/srv/nautik"));
        assert_eq!(c.listen.port(), 9000);
        let c = run(&["--data-dir", "/tmp/x", "--listen", "[::1]:8000"], &e);
        assert_eq!(c.data_dir, PathBuf::from("/tmp/x"));
        assert_eq!(c.listen, "[::1]:8000".parse().expect("adresse"));
        let c = run(&[], &env(&[("STATE_DIRECTORY", "/var/lib/nautik:/autre")]));
        assert_eq!(c.data_dir, PathBuf::from("/var/lib/nautik"));
    }

    #[test]
    fn invalid_arguments_are_reported() {
        let e = env(&[]);
        assert!(parse(["--listen"], &e).is_err());
        assert!(parse(["--listen", "pas-une-adresse"], &e).is_err());
        assert!(parse(["--data-dir", " "], &e).is_err());
        assert!(parse(["--inconnue"], &e).is_err());
        assert!(parse::<[&str; 0], &str>([], &env(&[("NAUTIK_LISTEN", "x")])).is_err());
    }

    #[test]
    fn help_version_and_rotation() {
        let e = env(&[]);
        assert_eq!(parse(["--help"], &e), Ok(Command::Help));
        assert_eq!(parse(["-V"], &e), Ok(Command::Version));
        assert!(matches!(
            parse(["--rotate-token", "--data-dir", "d"], &e),
            Ok(Command::RotateToken(c)) if c.data_dir.as_path() == std::path::Path::new("d")
        ));
    }
}
