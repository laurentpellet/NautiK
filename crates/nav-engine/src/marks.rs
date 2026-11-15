//! Marques personnelles : création, modification, suppression, import et
//! export KML. Chaque entrée est validée avant d'être enregistrée.

use chrono::{DateTime, Utc};
use nav_core::GeoPosition;
use nav_routing::kml;
use nav_routing::mark::MAX_MARKS;
use nav_routing::{Mark, MarkIcon};
use serde::Serialize;

use crate::routing::parse_id;
use crate::service::Service;
use crate::CommandResult;

fn text<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

/// Bilan d'un import KML.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarksImportReport {
    /// Marques importées.
    pub imported: usize,
    /// Éléments écartés et raisons.
    pub warnings: Vec<String>,
}

/// Liste les marques.
///
/// # Errors
///
/// Message lisible en cas d'erreur de la base.
pub async fn list(service: &Service) -> CommandResult<Vec<Mark>> {
    service.db.list_marks().await.map_err(text)
}

/// Crée (`id` absent) ou modifie une marque.
///
/// # Errors
///
/// Message lisible si une entrée est invalide, la marque introuvable ou
/// le nombre maximal de marques atteint.
pub async fn save(
    service: &Service,
    id: Option<&str>,
    name: &str,
    position: GeoPosition,
    icon: &str,
    note: Option<&str>,
    now: DateTime<Utc>,
) -> CommandResult<Mark> {
    let icon = MarkIcon::from_code(icon).ok_or_else(|| format!("icône inconnue : « {icon} »"))?;
    let mark = match id {
        Some(id) => {
            let mut mark = service
                .db
                .get_mark(parse_id(id)?)
                .await
                .map_err(text)?
                .ok_or("marque introuvable")?;
            mark.edit(name, position, icon, note, now).map_err(text)?;
            mark
        }
        None => {
            if service.db.count_marks().await.map_err(text)? >= MAX_MARKS {
                return Err(format!("nombre maximal de marques atteint ({MAX_MARKS})"));
            }
            Mark::new(name, position, icon, note, now).map_err(text)?
        }
    };
    service.db.save_mark(&mark).await.map_err(text)?;
    Ok(mark)
}

/// Supprime une marque.
///
/// # Errors
///
/// Message lisible si l'identifiant est invalide ou la marque introuvable.
pub async fn delete(service: &Service, id: &str) -> CommandResult<()> {
    if !service.db.delete_mark(parse_id(id)?).await.map_err(text)? {
        return Err("marque introuvable".to_owned());
    }
    Ok(())
}

/// Importe les marques d'un document KML, en une transaction.
///
/// # Errors
///
/// Message lisible si le document est invalide ou dépasse le nombre
/// maximal de marques ; rien n'est alors enregistré.
pub async fn import_kml(
    service: &Service,
    xml: &str,
    now: DateTime<Utc>,
) -> CommandResult<MarksImportReport> {
    let import = kml::parse(xml, now).map_err(|e| format!("KML invalide : {e}"))?;
    let existing = service.db.count_marks().await.map_err(text)?;
    if existing + import.marks.len() > MAX_MARKS {
        return Err(format!(
            "import refusé : {} marques en plus des {existing} existantes dépasseraient \
             le maximum ({MAX_MARKS})",
            import.marks.len()
        ));
    }
    service.db.save_marks(&import.marks).await.map_err(text)?;
    tracing::info!(
        marks = import.marks.len(),
        warnings = import.warnings.len(),
        "import KML"
    );
    Ok(MarksImportReport {
        imported: import.marks.len(),
        warnings: import.warnings,
    })
}

/// Toutes les marques au format KML.
///
/// # Errors
///
/// Message lisible en cas d'erreur de la base.
pub async fn export_kml(service: &Service) -> CommandResult<String> {
    Ok(kml::write(&list(service).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_service;

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    #[tokio::test]
    async fn create_edit_delete_mark() {
        let s = test_service().await;
        let now = Utc::now();
        let m = save(
            &s,
            None,
            " Patate ",
            p(-17.5, -149.6),
            "danger",
            Some("1,2 m"),
            now,
        )
        .await
        .expect("création");
        assert_eq!(m.name, "Patate");
        let id = m.id.to_string();
        let edited = save(
            &s,
            Some(&id),
            "Patate",
            p(-17.51, -149.6),
            "dive",
            None,
            now,
        )
        .await
        .expect("modification");
        assert_eq!(edited.icon, MarkIcon::Dive);
        assert_eq!(edited.note, None);
        assert_eq!(list(&s).await.expect("liste"), vec![edited]);
        delete(&s, &id).await.expect("suppression");
        assert!(delete(&s, &id).await.is_err(), "déjà supprimée");
    }

    #[tokio::test]
    async fn invalid_inputs_are_rejected() {
        let s = test_service().await;
        let now = Utc::now();
        let pos = p(0.0, 0.0);
        assert!(save(&s, None, "  ", pos, "flag", None, now).await.is_err());
        let err = save(&s, None, "A", pos, "licorne", None, now)
            .await
            .expect_err("icône");
        assert!(err.contains("icône inconnue"), "{err}");
        assert!(save(&s, Some("../x"), "A", pos, "flag", None, now)
            .await
            .is_err());
        let unknown = uuid::Uuid::new_v4().to_string();
        assert!(save(&s, Some(&unknown), "A", pos, "flag", None, now)
            .await
            .is_err());
        assert!(list(&s).await.expect("liste").is_empty());
    }

    #[tokio::test]
    async fn kml_round_trip_and_invalid_documents() {
        let s = test_service().await;
        let now = Utc::now();
        save(
            &s,
            None,
            "Baie & co",
            p(-17.5, -149.6),
            "anchorage",
            Some("<sable>"),
            now,
        )
        .await
        .expect("création");
        let xml = export_kml(&s).await.expect("export");
        let other = test_service().await;
        let report = import_kml(&other, &xml, now).await.expect("import");
        assert_eq!(report.imported, 1);
        let back = list(&other).await.expect("liste");
        assert_eq!(back[0].name, "Baie & co");
        assert_eq!(back[0].note.as_deref(), Some("<sable>"));
        assert_eq!(back[0].icon, MarkIcon::Anchorage);

        let err = import_kml(&other, "<gpx/>", now)
            .await
            .expect_err("pas KML");
        assert!(err.contains("KML invalide"), "{err}");
        assert_eq!(list(&other).await.expect("liste").len(), 1);
    }
}
