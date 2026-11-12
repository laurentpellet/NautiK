//! Tests d'intégration des repositories SQLite.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use chrono::{DateTime, Duration, Utc};
use nav_core::GeoPosition;
use nav_routing::{Route, Track, TrackPoint, Waypoint};
use nav_storage::{Db, StorageError};
use uuid::Uuid;

fn p(lat: f64, lon: f64) -> GeoPosition {
    GeoPosition::new(lat, lon).expect("pos")
}

fn t(seconds: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(1_800_000_000 + seconds, 123_456_789).expect("t")
}

fn route(name: &str) -> Route {
    Route::from_positions(
        name,
        &[p(-14.968, -147.638), p(-15.0, -147.9), p(-15.1, -148.17)],
        t(0),
    )
    .expect("route")
}

fn point(seconds: i64, lat: f64) -> TrackPoint {
    TrackPoint {
        timestamp: t(seconds),
        position: p(lat, -147.6),
        sog_mps: Some(2.5),
        cog_rad: Some(1.0),
        depth_m: None,
    }
}

fn empty_track(name: &str) -> Track {
    Track {
        id: Uuid::new_v4(),
        name: name.to_owned(),
        started_at: t(0),
        ended_at: None,
        points: Vec::new(),
    }
}

async fn waypoint_rows(db_path: &std::path::Path) -> usize {
    let db = Db::open(db_path).await.expect("open");
    let routes = db.list_routes().await.expect("routes");
    let standalone = db.list_waypoints().await.expect("wps");
    db.close().await;
    routes.iter().map(|r| r.waypoints.len()).sum::<usize>() + standalone.len()
}

#[tokio::test]
async fn route_persists_across_reopen() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("nautik.db");
    let original = route("Rangiroa → Tikehau");
    {
        let db = Db::open(&path).await.expect("open");
        db.save_route(&original).await.expect("save");
        db.close().await;
    }
    let db = Db::open(&path)
        .await
        .expect("reopen (migrations idempotentes)");
    let loaded = db
        .get_route(original.id)
        .await
        .expect("get")
        .expect("présente");
    assert_eq!(loaded, original, "aller-retour exact, horodatages compris");
    db.close().await;
}

#[tokio::test]
async fn saving_a_route_replaces_its_waypoints() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("nautik.db");
    let db = Db::open(&path).await.expect("open");
    let mut r = route("R");
    db.save_route(&r).await.expect("save");
    r.remove_waypoint(1, t(10)).expect("remove");
    r.reverse(t(11));
    db.save_route(&r).await.expect("resave");
    let loaded = db.get_route(r.id).await.expect("get").expect("présente");
    assert_eq!(loaded.waypoints.len(), 2);
    assert_eq!(loaded.waypoints, r.waypoints, "ordre conservé");
    db.close().await;
    assert_eq!(
        waypoint_rows(&path).await,
        2,
        "le waypoint retiré est supprimé"
    );
}

#[tokio::test]
async fn list_and_delete_routes() {
    let db = Db::open_in_memory().await.expect("db");
    db.save_route(&route("b")).await.expect("save");
    let a = route("A");
    db.save_route(&a).await.expect("save");
    let names: Vec<_> = db
        .list_routes()
        .await
        .expect("list")
        .into_iter()
        .map(|r| r.name)
        .collect();
    assert_eq!(names, ["A", "b"], "tri insensible à la casse");
    assert!(db.delete_route(a.id).await.expect("delete"));
    assert!(!db.delete_route(a.id).await.expect("delete again"));
    assert_eq!(db.list_routes().await.expect("list").len(), 1);
    assert!(db.get_route(a.id).await.expect("get").is_none());
}

#[tokio::test]
async fn standalone_waypoints_crud() {
    let db = Db::open_in_memory().await.expect("db");
    let mut w = Waypoint::new("Mouillage", p(-14.97, -147.64), Some("Sable"), t(0)).expect("wp");
    w.color = Some("#ff8800".to_owned());
    db.save_waypoint(&w).await.expect("save");
    db.save_route(&route("R")).await.expect("route");
    let listed = db.list_waypoints().await.expect("list");
    assert_eq!(
        listed,
        vec![w.clone()],
        "les waypoints de route sont exclus"
    );

    w.name = "Mouillage nord".to_owned();
    db.save_waypoint(&w).await.expect("update");
    assert_eq!(
        db.list_waypoints().await.expect("list")[0].name,
        "Mouillage nord"
    );

    assert!(db.delete_waypoint(w.id).await.expect("delete"));
    assert!(!db.delete_waypoint(w.id).await.expect("absent"));
}

#[tokio::test]
async fn waypoint_used_by_route_cannot_be_deleted() {
    let db = Db::open_in_memory().await.expect("db");
    let r = route("R");
    db.save_route(&r).await.expect("save");
    let used = r.waypoints[0].id;
    assert!(matches!(
        db.delete_waypoint(used).await,
        Err(StorageError::WaypointInUse(id)) if id == used
    ));
}

#[tokio::test]
async fn shared_waypoint_survives_deletion_of_one_route() {
    let db = Db::open_in_memory().await.expect("db");
    let r1 = route("R1");
    let mut r2 = route("R2");
    r2.waypoints[0] = r1.waypoints[0].clone();
    db.save_route(&r1).await.expect("save");
    db.save_route(&r2).await.expect("save");
    db.delete_route(r1.id).await.expect("delete");
    let kept = db.get_route(r2.id).await.expect("get").expect("présente");
    assert_eq!(kept.waypoints[0].id, r1.waypoints[0].id);
}

#[tokio::test]
async fn track_recording_lifecycle() {
    let db = Db::open_in_memory().await.expect("db");
    let track = empty_track("Sortie");
    db.create_track(&track, true).await.expect("create");
    db.append_track_points(track.id, &[point(0, -14.96), point(5, -14.95)])
        .await
        .expect("append");
    db.append_track_points(track.id, &[point(10, -14.94)])
        .await
        .expect("append");

    let summaries = db.list_tracks().await.expect("list");
    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].is_recording);
    assert_eq!(summaries[0].point_count, 3);
    assert_eq!(summaries[0].ended_at, None);

    db.finish_track(track.id, t(12)).await.expect("finish");
    let loaded = db
        .get_track(track.id)
        .await
        .expect("get")
        .expect("présente");
    assert_eq!(loaded.points.len(), 3);
    assert_eq!(loaded.points[2], point(10, -14.94));
    assert_eq!(loaded.ended_at, Some(t(12)));

    assert!(matches!(
        db.append_track_points(track.id, &[point(20, -14.9)]).await,
        Err(StorageError::TrackNotRecording(_))
    ));
}

#[tokio::test]
async fn appending_to_unknown_track_fails() {
    let db = Db::open_in_memory().await.expect("db");
    let id = Uuid::new_v4();
    assert!(matches!(
        db.append_track_points(id, &[point(0, 0.0)]).await,
        Err(StorageError::NotFound(x)) if x == id
    ));
    assert!(matches!(
        db.finish_track(id, t(0)).await,
        Err(StorageError::NotFound(_))
    ));
    assert!(db.get_track(id).await.expect("get").is_none());
    assert!(!db.delete_track(id).await.expect("delete"));
}

#[tokio::test]
async fn imported_track_is_stored_complete() {
    let db = Db::open_in_memory().await.expect("db");
    let track = Track {
        ended_at: Some(t(10)),
        points: vec![point(0, 1.0), point(10, 1.1)],
        ..empty_track("Import")
    };
    db.create_track(&track, false).await.expect("create");
    assert_eq!(
        db.get_track(track.id).await.expect("get"),
        Some(track.clone())
    );
    assert!(db.delete_track(track.id).await.expect("delete"));
    assert!(db.list_tracks().await.expect("list").is_empty());
}

#[tokio::test]
async fn interrupted_recordings_are_closed_without_inventing_times() {
    let db = Db::open_in_memory().await.expect("db");
    let with_points = empty_track("Avec points");
    let empty = Track {
        started_at: t(100),
        ..empty_track("Vide")
    };
    db.create_track(&with_points, true).await.expect("create");
    db.create_track(&empty, true).await.expect("create");
    db.append_track_points(with_points.id, &[point(3, 1.0), point(42, 1.1)])
        .await
        .expect("append");

    assert_eq!(db.close_interrupted_tracks().await.expect("close"), 2);
    assert_eq!(db.close_interrupted_tracks().await.expect("idempotent"), 0);

    let a = db.get_track(with_points.id).await.expect("get").expect("a");
    assert_eq!(a.ended_at, Some(t(42)), "fin = dernier point réel");
    let b = db.get_track(empty.id).await.expect("get").expect("b");
    assert_eq!(b.ended_at, Some(t(100)), "trace vide : fin = début");
    assert!(db
        .list_tracks()
        .await
        .expect("list")
        .iter()
        .all(|s| !s.is_recording));
}

#[tokio::test]
async fn tracks_are_listed_newest_first() {
    let db = Db::open_in_memory().await.expect("db");
    let old = empty_track("Ancienne");
    let new = Track {
        started_at: t(0) + Duration::days(1),
        ..empty_track("Récente")
    };
    db.create_track(&old, false).await.expect("create");
    db.create_track(&new, false).await.expect("create");
    let names: Vec<_> = db
        .list_tracks()
        .await
        .expect("list")
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, ["Récente", "Ancienne"]);
}

#[tokio::test]
async fn duplicate_track_id_is_rejected_atomically() {
    let db = Db::open_in_memory().await.expect("db");
    let track = Track {
        points: vec![point(0, 1.0)],
        ..empty_track("T")
    };
    db.create_track(&track, false).await.expect("create");
    let duplicate = Track {
        points: vec![point(1, 2.0), point(2, 2.1)],
        ..track.clone()
    };
    assert!(matches!(
        db.create_track(&duplicate, false).await,
        Err(StorageError::Database(_))
    ));
    let summaries = db.list_tracks().await.expect("list");
    assert_eq!(summaries[0].point_count, 1, "aucun point du doublon écrit");
}

#[tokio::test]
async fn save_all_is_atomic() {
    let db = Db::open_in_memory().await.expect("db");
    let existing = Track {
        ended_at: Some(t(1)),
        points: vec![point(0, 1.0)],
        ..empty_track("Existante")
    };
    db.create_track(&existing, false).await.expect("create");
    let w = Waypoint::new("W", p(1.0, 1.0), None, t(0)).expect("wp");
    let r = route("R");
    // La trace en double fait échouer l'ensemble.
    let result = db
        .save_all(
            std::slice::from_ref(&w),
            std::slice::from_ref(&r),
            std::slice::from_ref(&existing),
        )
        .await;
    assert!(matches!(result, Err(StorageError::Database(_))));
    assert!(db.list_waypoints().await.expect("wps").is_empty());
    assert!(db.list_routes().await.expect("routes").is_empty());

    let new_track = Track {
        id: Uuid::new_v4(),
        ..existing
    };
    db.save_all(&[w], &[r], &[new_track]).await.expect("import");
    assert_eq!(db.list_waypoints().await.expect("wps").len(), 1);
    assert_eq!(db.list_routes().await.expect("routes").len(), 1);
    assert_eq!(db.list_tracks().await.expect("tracks").len(), 2);
}

#[tokio::test]
async fn open_fails_on_unwritable_location() {
    let dir = tempfile::tempdir().expect("dir");
    // Un répertoire existant ne peut pas être ouvert comme base.
    assert!(Db::open(dir.path()).await.is_err());
}
