//! Point d'entrée de l'application desktop NautiK.

// Pas de console supplémentaire sous Windows en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    match nautik_desktop_lib::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // tracing peut ne pas être initialisé si l'erreur survient tôt.
            eprintln!("NautiK : arrêt sur erreur : {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
