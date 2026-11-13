//! Relais des événements du moteur (`nav-engine`) vers le frontend, sous
//! forme d'événements Tauri. Les noms et contenus sont inchangés : voir
//! [`EngineEvent::name`].

use nav_engine::{EngineEvent, Service};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::error::RecvError;

fn emit(app: &AppHandle, event: &EngineEvent) -> tauri::Result<()> {
    let name = event.name();
    match event {
        EngineEvent::Instruments(v) => app.emit(name, v),
        EngineEvent::Navigation(v) => app.emit(name, v),
        EngineEvent::SourceStatus(v) => app.emit(name, v),
        EngineEvent::Rejections(v) => app.emit(name, v),
        EngineEvent::Ais(v) => app.emit(name, v),
        EngineEvent::Alarms(v) => app.emit(name, v),
    }
}

/// Relaie les événements du moteur jusqu'à son arrêt.
pub async fn forward(app: AppHandle, service: Arc<Service>) {
    let mut rx = service.subscribe();
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(error) = emit(&app, &event) {
                    tracing::warn!(event = event.name(), %error, "émission impossible");
                }
            }
            Err(RecvError::Lagged(missed)) => {
                // Les instantanés suivants rétablissent l'état complet.
                tracing::warn!(missed, "événements moteur non relayés (interface lente)");
            }
            Err(RecvError::Closed) => return,
        }
    }
}
