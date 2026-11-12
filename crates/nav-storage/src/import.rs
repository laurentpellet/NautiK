//! Écriture groupée (import GPX) en une seule transaction.

use nav_routing::{Route, Track, Waypoint};

use crate::routes::save_route_in;
use crate::tracks::create_track_in;
use crate::waypoints::upsert_waypoint;
use crate::{Db, StorageError};

impl Db {
    /// Enregistre waypoints, routes et traces terminées en une seule
    /// transaction : tout est écrit, ou rien.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite (rien n'est écrit).
    pub async fn save_all(
        &self,
        waypoints: &[Waypoint],
        routes: &[Route],
        tracks: &[Track],
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        for waypoint in waypoints {
            upsert_waypoint(&mut tx, waypoint).await?;
        }
        for route in routes {
            save_route_in(&mut tx, route).await?;
        }
        for track in tracks {
            create_track_in(&mut tx, track, false).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
