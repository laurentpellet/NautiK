//! Fichiers météo GRIB : bibliothèque locale, import, téléchargement GFS
//! et champs de vent pour la carte.
//!
//! Les fichiers sont conservés dans le répertoire de données de
//! l'application et exploités hors ligne. Le frontend ne transmet jamais
//! de chemin : un fichier est désigné par un identifiant (nom simple,
//! validé) et l'import passe par une boîte de dialogue native.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Timelike, Utc};
use nav_weather::{decode, download_gfs, Bounds, GfsRequest, WindForecast, WindVector};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

use nav_engine::CommandResult;

/// Extensions reconnues.
const EXTENSIONS: [&str; 4] = ["grb2", "grib2", "grb", "grib"];
/// Taille maximale d'un fichier importé (octets).
pub const MAX_IMPORT_BYTES: u64 = nav_weather::grib2::MAX_FILE_BYTES as u64;
/// Nombre de vecteurs de vent envoyés à la carte (bornes).
const MIN_POINTS: usize = 10;
const MAX_POINTS: usize = 5_000;
/// Événement de progression du téléchargement.
pub const DOWNLOAD_EVENT: &str = "weather-download";

/// Fichier de la bibliothèque.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherFileView {
    /// Identifiant (nom du fichier).
    pub id: String,
    /// Taille (octets).
    pub size_bytes: u64,
    /// Heure de référence du modèle.
    pub reference_time: Option<DateTime<Utc>>,
    /// Échéances disponibles.
    pub valid_times: Vec<DateTime<Utc>>,
    /// Emprise.
    pub bounds: Option<Bounds>,
    /// Champs ignorés.
    pub skipped: Vec<String>,
    /// Fichier inexploitable : raison.
    pub error: Option<String>,
}

/// Champ de vent d'une échéance.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindFieldView {
    /// Fichier.
    pub id: String,
    /// Heure de référence du modèle.
    pub reference_time: DateTime<Utc>,
    /// Heure de validité.
    pub valid_time: DateTime<Utc>,
    /// Indice de l'échéance.
    pub step: usize,
    /// Nombre d'échéances.
    pub steps: usize,
    /// Vecteurs (éclaircis).
    pub points: Vec<WindVector>,
}

/// Progression d'un téléchargement.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: usize,
    total: usize,
}

/// Bibliothèque de fichiers météo.
#[derive(Debug)]
pub struct WeatherStore {
    dir: PathBuf,
    cache: Mutex<HashMap<String, Arc<WindForecast>>>,
    downloading: AtomicBool,
}

/// Identifiant acceptable : nom simple avec une extension GRIB.
fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 120
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && Path::new(id)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Nom de fichier sûr dérivé d'un nom quelconque (extension `.grb2`).
fn safe_id(name: &str) -> String {
    let stem: String = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("meteo")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    let stem = stem.trim_matches('_');
    format!("{}.grb2", if stem.is_empty() { "meteo" } else { stem })
}

fn view(id: &str, size_bytes: u64, forecast: &WindForecast) -> WeatherFileView {
    WeatherFileView {
        id: id.to_owned(),
        size_bytes,
        reference_time: Some(forecast.reference_time),
        valid_times: forecast.steps.iter().map(|s| s.valid_time).collect(),
        bounds: Some(forecast.bounds()),
        skipped: forecast.skipped.clone(),
        error: None,
    }
}

async fn parse(bytes: Vec<u8>) -> CommandResult<WindForecast> {
    tokio::task::spawn_blocking(move || {
        let decoded = decode(&bytes).map_err(|e| e.to_string())?;
        WindForecast::from_decoded(decoded).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("analyse interrompue : {e}"))?
}

impl WeatherStore {
    /// Bibliothèque dans `dir` (créé si besoin à la première écriture).
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            cache: Mutex::new(HashMap::new()),
            downloading: AtomicBool::new(false),
        }
    }

    fn path_of(&self, id: &str) -> CommandResult<PathBuf> {
        if is_valid_id(id) {
            Ok(self.dir.join(id))
        } else {
            Err(format!("identifiant de fichier météo invalide : {id:?}"))
        }
    }

    fn cached(&self, id: &str) -> Option<Arc<WindForecast>> {
        self.cache.lock().ok()?.get(id).cloned()
    }

    fn remember(&self, id: &str, forecast: Arc<WindForecast>) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(id.to_owned(), forecast);
        }
    }

    fn forget(&self, id: &str) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.remove(id);
        }
    }

    async fn load(&self, id: &str) -> CommandResult<Arc<WindForecast>> {
        if let Some(f) = self.cached(id) {
            return Ok(f);
        }
        let path = self.path_of(id)?;
        let size = tokio::fs::metadata(&path)
            .await
            .map_err(|e| format!("fichier illisible : {e}"))?
            .len();
        if size > MAX_IMPORT_BYTES {
            return Err(format!("fichier trop volumineux ({} Mo)", size / 1_048_576));
        }
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| format!("fichier illisible : {e}"))?;
        let forecast = Arc::new(parse(bytes).await?);
        self.remember(id, Arc::clone(&forecast));
        Ok(forecast)
    }

    /// Fichiers de la bibliothèque (les plus récents d'abord) ; un
    /// fichier inexploitable est listé avec son erreur.
    ///
    /// # Errors
    ///
    /// Message si le répertoire ne peut pas être lu.
    pub async fn list(&self) -> CommandResult<Vec<WeatherFileView>> {
        let mut entries = match tokio::fs::read_dir(&self.dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("répertoire météo illisible : {e}")),
        };
        let mut files = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("répertoire météo illisible : {e}"))?
        {
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !is_valid_id(&id) {
                continue;
            }
            let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
            files.push(match self.load(&id).await {
                Ok(f) => view(&id, size, &f),
                Err(error) => WeatherFileView {
                    id,
                    size_bytes: size,
                    reference_time: None,
                    valid_times: Vec::new(),
                    bounds: None,
                    skipped: Vec::new(),
                    error: Some(error),
                },
            });
        }
        files.sort_by(|a, b| {
            b.reference_time
                .cmp(&a.reference_time)
                .then(a.id.cmp(&b.id))
        });
        Ok(files)
    }

    /// Enregistre des données GRIB déjà vérifiées (écriture atomique).
    async fn store(
        &self,
        name: &str,
        bytes: Vec<u8>,
        forecast: WindForecast,
    ) -> CommandResult<WeatherFileView> {
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| format!("répertoire météo : {e}"))?;
        // Nom unique : suffixe numérique si le nom existe déjà.
        let base = safe_id(name);
        let mut id = base.clone();
        let mut n = 1;
        while tokio::fs::try_exists(self.path_of(&id)?)
            .await
            .unwrap_or(false)
        {
            n += 1;
            id = base.replacen(".grb2", &format!("-{n}.grb2"), 1);
        }
        let path = self.path_of(&id)?;
        let partial = path.with_extension("part");
        let size = bytes.len() as u64;
        tokio::fs::write(&partial, &bytes)
            .await
            .map_err(|e| format!("écriture impossible : {e}"))?;
        tokio::fs::rename(&partial, &path)
            .await
            .map_err(|e| format!("écriture impossible : {e}"))?;
        let forecast = Arc::new(forecast);
        self.remember(&id, Arc::clone(&forecast));
        tracing::info!(%id, size, steps = forecast.steps.len(), "fichier météo enregistré");
        Ok(view(&id, size, &forecast))
    }

    /// Importe un fichier GRIB (vérifié avant copie).
    ///
    /// # Errors
    ///
    /// Message si le fichier est illisible, trop volumineux ou sans vent.
    pub async fn import(&self, source: &Path) -> CommandResult<WeatherFileView> {
        let size = tokio::fs::metadata(source)
            .await
            .map_err(|e| format!("fichier illisible : {e}"))?
            .len();
        if size > MAX_IMPORT_BYTES {
            return Err(format!(
                "fichier trop volumineux ({} Mo, maximum {} Mo)",
                size / 1_048_576,
                MAX_IMPORT_BYTES / 1_048_576
            ));
        }
        let bytes = tokio::fs::read(source)
            .await
            .map_err(|e| format!("fichier illisible : {e}"))?;
        let forecast = parse(bytes.clone()).await?;
        let name = source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("meteo");
        self.store(name, bytes, forecast).await
    }

    /// Supprime un fichier de la bibliothèque.
    ///
    /// # Errors
    ///
    /// Message si l'identifiant est invalide ou le fichier introuvable.
    pub async fn delete(&self, id: &str) -> CommandResult<()> {
        let path = self.path_of(id)?;
        self.forget(id);
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| format!("suppression impossible : {e}"))?;
        tracing::info!(%id, "fichier météo supprimé");
        Ok(())
    }

    /// Vent d'une échéance, limité à la zone affichée.
    ///
    /// # Errors
    ///
    /// Message si le fichier ou l'échéance n'existent pas.
    pub async fn wind_field(
        &self,
        id: &str,
        step: usize,
        area: Option<Bounds>,
        max_points: usize,
    ) -> CommandResult<WindFieldView> {
        if area.is_some_and(|a| !a.is_valid()) {
            return Err("zone d'affichage invalide".to_owned());
        }
        let forecast = self.load(id).await?;
        let valid_time = forecast
            .steps
            .get(step)
            .map(|s| s.valid_time)
            .ok_or_else(|| format!("échéance {step} inexistante"))?;
        let max_points = max_points.clamp(MIN_POINTS, MAX_POINTS);
        let points = forecast.sample(step, area, max_points);
        Ok(WindFieldView {
            id: id.to_owned(),
            reference_time: forecast.reference_time,
            valid_time,
            step,
            steps: forecast.steps.len(),
            points,
        })
    }
}

/// Libère le verrou de téléchargement en fin de commande.
struct DownloadGuard<'a>(&'a AtomicBool);

impl Drop for DownloadGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Fichiers météo disponibles.
#[tauri::command]
pub async fn list_weather_files(
    store: State<'_, WeatherStore>,
) -> CommandResult<Vec<WeatherFileView>> {
    store.list().await
}

/// Importe un fichier GRIB choisi par l'utilisateur. `None` si annulé.
#[tauri::command]
pub async fn import_weather_file(
    app: AppHandle,
    store: State<'_, WeatherStore>,
) -> CommandResult<Option<WeatherFileView>> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .set_title("Importer un fichier GRIB")
        .add_filter("GRIB", &EXTENSIONS)
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = rx.await.map_err(|_| "dialogue interrompu".to_owned())? else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|_| "chemin de fichier non pris en charge".to_owned())?;
    store.import(&path).await.map(Some)
}

/// Télécharge le vent GFS de la zone (Internet requis), puis l'enregistre
/// dans la bibliothèque pour un usage hors ligne.
#[tauri::command]
pub async fn download_weather(
    app: AppHandle,
    store: State<'_, WeatherStore>,
    area: Bounds,
    horizon_h: u32,
) -> CommandResult<WeatherFileView> {
    let request = GfsRequest { area, horizon_h };
    request.validate().map_err(|e| e.to_string())?;
    if store.downloading.swap(true, Ordering::SeqCst) {
        return Err("un téléchargement est déjà en cours".to_owned());
    }
    let _guard = DownloadGuard(&store.downloading);
    tracing::info!(?area, horizon_h, "téléchargement GFS");
    let downloaded = download_gfs(request, Utc::now(), |done, total| {
        let _ = app.emit(DOWNLOAD_EVENT, Progress { done, total });
    })
    .await
    .map_err(|e| {
        tracing::warn!(error = %e, "téléchargement GFS échoué");
        e.to_string()
    })?;
    let name = format!(
        "gfs-{}-{:02}z-{}.grb2",
        downloaded.run.format("%Y%m%d"),
        downloaded.run.hour(),
        Utc::now().format("%H%M%S")
    );
    store
        .store(&name, downloaded.bytes, downloaded.forecast)
        .await
}

/// Supprime un fichier météo (confirmation demandée par l'interface).
#[tauri::command]
pub async fn delete_weather_file(store: State<'_, WeatherStore>, id: String) -> CommandResult<()> {
    store.delete(&id).await
}

/// Vent d'une échéance pour la carte.
#[tauri::command]
pub async fn get_wind_field(
    store: State<'_, WeatherStore>,
    id: String,
    step: usize,
    area: Option<Bounds>,
    max_points: usize,
) -> CommandResult<WindFieldView> {
    store.wind_field(&id, step, area, max_points).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Message GRIB2 minimal : vent U ou V à 10 m, grille 2 × 2,
    /// compactage simple 8 bits, valeurs `(R + X) / 10`.
    fn message(number: u8, hour: u32, values: [u8; 4]) -> Vec<u8> {
        fn section(n: u8, body: &[u8]) -> Vec<u8> {
            let mut s = ((body.len() + 5) as u32).to_be_bytes().to_vec();
            s.push(n);
            s.extend_from_slice(body);
            s
        }
        let mut body = Vec::new();
        let mut s1 = vec![0u8; 16];
        s1[7..9].copy_from_slice(&2026u16.to_be_bytes());
        s1[9] = 11;
        s1[10] = 12;
        body.extend(section(1, &s1));
        let mut s3 = vec![0u8; 67];
        s3[1..5].copy_from_slice(&4u32.to_be_bytes());
        s3[25..29].copy_from_slice(&2u32.to_be_bytes());
        s3[29..33].copy_from_slice(&2u32.to_be_bytes());
        s3[41..45].copy_from_slice(&(0x8000_0000u32 | 14_000_000).to_be_bytes());
        s3[45..49].copy_from_slice(&210_000_000u32.to_be_bytes());
        s3[58..62].copy_from_slice(&1_000_000u32.to_be_bytes());
        s3[62..66].copy_from_slice(&1_000_000u32.to_be_bytes());
        body.extend(section(3, &s3));
        let mut s4 = vec![0u8; 29];
        s4[4] = 2;
        s4[5] = number;
        s4[12] = 1;
        s4[13..17].copy_from_slice(&hour.to_be_bytes());
        s4[17] = 103;
        s4[19..23].copy_from_slice(&10u32.to_be_bytes());
        body.extend(section(4, &s4));
        let mut s5 = vec![0u8; 16];
        s5[0..4].copy_from_slice(&4u32.to_be_bytes());
        s5[12..14].copy_from_slice(&1u16.to_be_bytes());
        s5[14] = 8;
        body.extend(section(5, &s5));
        body.extend(section(6, &[255]));
        body.extend(section(7, &values));
        let mut m = b"GRIB".to_vec();
        m.extend([0, 0, 0, 2]);
        m.extend(((16 + body.len() + 4) as u64).to_be_bytes());
        m.extend(body);
        m.extend(b"7777");
        m
    }

    fn grib() -> Vec<u8> {
        let mut f = Vec::new();
        for hour in [0, 3] {
            f.extend(message(2, hour, [30, 30, 30, 30]));
            f.extend(message(3, hour, [40, 40, 40, 40]));
        }
        f
    }

    #[test]
    fn validates_identifiers() {
        assert!(is_valid_id("gfs-20261112-06z.grb2"));
        assert!(is_valid_id("meteo.GRIB"));
        for bad in [
            "",
            "../x.grb2",
            "a/b.grb2",
            "a\\b.grb2",
            ".cache.grb2",
            "x.txt",
            "C:x.grb2",
            "x.grb2.part",
        ] {
            assert!(!is_valid_id(bad), "{bad}");
        }
        assert_eq!(safe_id("Tahiti 3j (PW).grb"), "Tahiti_3j__PW.grb2");
        assert_eq!(safe_id("..."), "meteo.grb2");
    }

    #[tokio::test]
    async fn imports_lists_samples_and_deletes() {
        let tmp = tempfile::tempdir().expect("tmp");
        let store = WeatherStore::new(tmp.path().join("weather"));
        assert!(store.list().await.expect("vide").is_empty());

        let source = tmp.path().join("Tuamotu.grb2");
        std::fs::write(&source, grib()).expect("write");
        let imported = store.import(&source).await.expect("import");
        assert_eq!(imported.id, "Tuamotu.grb2");
        assert_eq!(imported.valid_times.len(), 2);
        let again = store.import(&source).await.expect("import");
        assert_eq!(again.id, "Tuamotu-2.grb2", "pas d'écrasement");

        // Un fichier corrompu est listé avec son erreur.
        std::fs::write(tmp.path().join("weather/bad.grb2"), b"GRIB tronque").expect("write");
        let files = store.list().await.expect("list");
        assert_eq!(files.len(), 3);
        assert!(files
            .iter()
            .any(|f| f.id == "bad.grb2" && f.error.is_some()));

        let field = store
            .wind_field("Tuamotu.grb2", 1, None, 100)
            .await
            .expect("champ");
        assert_eq!((field.step, field.steps, field.points.len()), (1, 2, 4));
        // (−0 + 30)/10 = 3 m/s vers l'est, 4 m/s vers le nord : 5 m/s.
        assert!((field.points[0].speed_mps - 5.0).abs() < 1e-6);
        assert!(store
            .wind_field("Tuamotu.grb2", 2, None, 100)
            .await
            .is_err());
        assert!(store
            .wind_field("../secret.grb2", 0, None, 100)
            .await
            .is_err());

        store.delete("Tuamotu.grb2").await.expect("delete");
        assert!(store
            .wind_field("Tuamotu.grb2", 0, None, 100)
            .await
            .is_err());
        assert!(store.delete("../../etc.grb2").await.is_err());
    }

    #[tokio::test]
    async fn rejects_files_without_wind() {
        let tmp = tempfile::tempdir().expect("tmp");
        let store = WeatherStore::new(tmp.path().join("weather"));
        let source = tmp.path().join("u.grb2");
        std::fs::write(&source, message(2, 0, [1, 2, 3, 4])).expect("write");
        assert!(store.import(&source).await.is_err());
        let text = tmp.path().join("t.grb2");
        std::fs::write(&text, "pas un grib").expect("write");
        assert!(store.import(&text).await.is_err());
        assert!(!tmp.path().join("weather").exists(), "rien n'est copié");
    }
}
