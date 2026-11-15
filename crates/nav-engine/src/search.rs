//! Recherche de lieux : coordonnées saisies, marques, waypoints (isolés
//! ou de routes), îles et amers téléchargés. Tout est local.

use nav_core::GeoPosition;
use nav_routing::search::{fold, match_rank, parse_coordinates};
use serde::Serialize;

use crate::service::Service;
use crate::CommandResult;

/// Nombre maximal de résultats.
pub const MAX_RESULTS: usize = 50;
/// Longueur maximale d'une requête (caractères).
pub const MAX_QUERY_CHARS: usize = 80;
/// Longueur minimale d'une requête par nom (caractères).
pub const MIN_QUERY_CHARS: usize = 2;

fn text<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

/// Nature d'un résultat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaceKind {
    /// Coordonnées saisies.
    Coordinates,
    /// Marque personnelle.
    Mark,
    /// Waypoint isolé.
    Waypoint,
    /// Waypoint d'une route.
    RoutePoint,
    /// Île ou amer téléchargé.
    Poi,
}

/// Résultat de recherche.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    /// Nature.
    pub kind: PlaceKind,
    /// Nom.
    pub name: String,
    /// Précision (route, type d'amer…).
    pub detail: Option<String>,
    /// Position.
    pub position: GeoPosition,
}

/// Recherche un lieu. Résultats triés par pertinence puis par nature.
///
/// # Errors
///
/// Message lisible si la requête est trop longue ou la base illisible.
pub async fn search(service: &Service, query: &str) -> CommandResult<Vec<Place>> {
    let query = query.trim();
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(format!(
            "recherche trop longue (maximum {MAX_QUERY_CHARS} caractères)"
        ));
    }
    if let Some(position) = parse_coordinates(query) {
        return Ok(vec![Place {
            kind: PlaceKind::Coordinates,
            name: query.to_owned(),
            detail: None,
            position,
        }]);
    }
    if query.chars().count() < MIN_QUERY_CHARS {
        return Ok(Vec::new());
    }
    let q = fold(query);
    let mut found: Vec<(u8, Place)> = Vec::new();
    let mut push = |name: &str, kind, detail: Option<String>, position| {
        if let Some(rank) = match_rank(name, &q) {
            found.push((
                rank,
                Place {
                    kind,
                    name: name.to_owned(),
                    detail,
                    position,
                },
            ));
        }
    };
    for m in service.db.list_marks().await.map_err(text)? {
        push(&m.name, PlaceKind::Mark, m.note, m.position);
    }
    for w in service.db.list_waypoints().await.map_err(text)? {
        push(&w.name, PlaceKind::Waypoint, None, w.position);
    }
    for r in service.db.list_routes().await.map_err(text)? {
        for w in &r.waypoints {
            push(
                &w.name,
                PlaceKind::RoutePoint,
                Some(r.name.clone()),
                w.position,
            );
        }
    }
    for p in service.db.list_pois().await.map_err(text)? {
        if let Some(name) = &p.name {
            push(name, PlaceKind::Poi, Some(p.kind.to_string()), p.position);
        }
    }
    found.sort_by(|(ra, a), (rb, b)| {
        ra.cmp(rb)
            .then(a.kind.cmp(&b.kind))
            .then_with(|| fold(&a.name).cmp(&fold(&b.name)))
    });
    Ok(found
        .into_iter()
        .map(|(_, p)| p)
        .take(MAX_RESULTS)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_service;
    use chrono::Utc;

    fn p(lat: f64, lon: f64) -> GeoPosition {
        GeoPosition::new(lat, lon).expect("pos")
    }

    #[tokio::test]
    async fn finds_coordinates_marks_and_route_points() {
        let s = test_service().await;
        let now = Utc::now();
        crate::marks::save(
            &s,
            None,
            "Passe Téavapiti",
            p(-17.6, -149.4),
            "danger",
            None,
            now,
        )
        .await
        .expect("marque");
        crate::marks::save(&s, None, "Teava", p(-17.7, -149.4), "flag", None, now)
            .await
            .expect("marque");
        crate::routing::create_route(&s, "Tour", &[p(-17.5, -149.6), p(-17.4, -149.5)])
            .await
            .expect("route");
        let routes = crate::routing::list_routes(&s).await.expect("routes");
        let wp_name = routes[0].route.waypoints[0].name.clone();

        let found = search(&s, "teava").await.expect("recherche");
        let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Teava", "Passe Téavapiti"], "pertinence");
        assert_eq!(found[1].kind, PlaceKind::Mark);

        let found = search(&s, &wp_name).await.expect("recherche");
        assert_eq!(found[0].kind, PlaceKind::RoutePoint);
        assert_eq!(found[0].detail.as_deref(), Some("Tour"));

        let coords = search(&s, "17°32.1'S 149°34.2'W").await.expect("coord");
        assert_eq!(coords.len(), 1);
        assert_eq!(coords[0].kind, PlaceKind::Coordinates);
    }

    #[tokio::test]
    async fn short_long_and_unknown_queries() {
        let s = test_service().await;
        assert!(search(&s, "a").await.expect("courte").is_empty());
        assert!(search(&s, "  ").await.expect("vide").is_empty());
        assert!(search(&s, "inconnu").await.expect("absente").is_empty());
        assert!(search(&s, &"x".repeat(200)).await.is_err());
    }
}
