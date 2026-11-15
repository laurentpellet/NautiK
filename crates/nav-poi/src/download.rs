//! Téléchargement d'une région depuis un service Overpass (option
//! `download`). Appelé uniquement à la demande de l'utilisateur.

use std::time::Duration;

use crate::model::PoiError;
use crate::overpass::{parse_overpass, query_for, ParsedPois};
use crate::regions::PoiRegion;

/// Service Overpass public par défaut (https).
pub const DEFAULT_ENDPOINT: &str = "https://overpass-api.de/api/interpreter";

/// Délai maximal d'une requête (le service dispose de 180 s).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(240);
/// Taille maximale de la réponse (octets).
const MAX_RESPONSE_BYTES: usize = 128 * 1024 * 1024;

/// Télécharge et analyse les îles et amers d'une région.
///
/// # Errors
///
/// [`PoiError::Http`] en cas d'erreur réseau ou HTTP (dont 429 : service
/// saturé), [`PoiError::TooLarge`] si la réponse dépasse la limite,
/// [`PoiError::InvalidResponse`] si elle est illisible ou incomplète.
pub async fn download_region(region: &PoiRegion, endpoint: &str) -> Result<ParsedPois, PoiError> {
    if !endpoint.starts_with("https://") {
        return Err(PoiError::Http("adresse du service non chiffrée".to_owned()));
    }
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        // Identification générique de l'application, sans donnée personnelle.
        .user_agent(concat!("NautiK/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| PoiError::Http(e.to_string()))?;
    let body: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("data", &query_for(region))
        .finish();
    let mut response = client
        .post(endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| PoiError::Http(e.without_url().to_string()))?;
    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(PoiError::Http(
            "service saturé (HTTP 429) : réessayez dans quelques minutes".to_owned(),
        ));
    }
    if !status.is_success() {
        return Err(PoiError::Http(format!("HTTP {status}")));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
    {
        return Err(PoiError::TooLarge);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| PoiError::Http(e.without_url().to_string()))?
    {
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(PoiError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    let parsed = parse_overpass(&bytes)?;
    tracing::info!(
        component = crate::COMPONENT,
        region = region.id,
        pois = parsed.pois.len(),
        skipped = parsed.skipped,
        "îles et amers téléchargés"
    );
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regions::region;

    /// Accès réseau réel : `cargo test -p nav-poi --features download -- --ignored`.
    #[tokio::test]
    #[ignore = "accès au service Overpass public"]
    async fn downloads_a_small_region_from_overpass() {
        let r = region("wallis-futuna").expect("région");
        let parsed = download_region(r, DEFAULT_ENDPOINT)
            .await
            .expect("téléchargement");
        assert!(
            parsed
                .pois
                .iter()
                .any(|p| p.kind == crate::PoiKind::Island && p.name.is_some()),
            "au moins une île nommée"
        );
    }

    #[tokio::test]
    async fn refuses_unencrypted_endpoints() {
        let r = region("tonga").expect("région");
        let err = download_region(r, "http://overpass.example/api/interpreter")
            .await
            .expect_err("http refusé");
        assert!(err.to_string().contains("non chiffrée"), "{err}");
    }
}
