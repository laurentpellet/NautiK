//! Repository des waypoints.

use nav_routing::Waypoint;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};
use uuid::Uuid;

use crate::codec::{position_from_db, time_from_db, time_to_db, uuid_from_db};
use crate::{Db, StorageError};

pub(crate) const WAYPOINT_COLUMNS: &str =
    "w.id, w.name, w.latitude, w.longitude, w.description, w.icon, w.color, w.created_at, w.updated_at";

pub(crate) fn waypoint_from_row(row: &SqliteRow) -> Result<Waypoint, StorageError> {
    Ok(Waypoint {
        id: uuid_from_db(row.try_get("id")?)?,
        name: row.try_get("name")?,
        position: position_from_db(row.try_get("latitude")?, row.try_get("longitude")?)?,
        description: row.try_get("description")?,
        icon: row.try_get("icon")?,
        color: row.try_get("color")?,
        created_at: time_from_db(row.try_get("created_at")?)?,
        updated_at: time_from_db(row.try_get("updated_at")?)?,
    })
}

pub(crate) async fn upsert_waypoint(
    conn: &mut SqliteConnection,
    w: &Waypoint,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO waypoints \
           (id, name, latitude, longitude, description, icon, color, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET \
           name = excluded.name, latitude = excluded.latitude, \
           longitude = excluded.longitude, description = excluded.description, \
           icon = excluded.icon, color = excluded.color, updated_at = excluded.updated_at",
    )
    .bind(w.id.to_string())
    .bind(&w.name)
    .bind(w.position.latitude_deg())
    .bind(w.position.longitude_deg())
    .bind(&w.description)
    .bind(&w.icon)
    .bind(&w.color)
    .bind(time_to_db(w.created_at))
    .bind(time_to_db(w.updated_at))
    .execute(conn)
    .await?;
    Ok(())
}

impl Db {
    /// Enregistre (crée ou met à jour) un waypoint.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn save_waypoint(&self, waypoint: &Waypoint) -> Result<(), StorageError> {
        let mut conn = self.pool.acquire().await?;
        upsert_waypoint(&mut conn, waypoint).await
    }

    /// Liste les waypoints isolés (non utilisés par une route), par nom.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée corrompue.
    pub async fn list_waypoints(&self) -> Result<Vec<Waypoint>, StorageError> {
        let rows = sqlx::query(&format!(
            "SELECT {WAYPOINT_COLUMNS} FROM waypoints w \
             WHERE NOT EXISTS (SELECT 1 FROM route_points rp WHERE rp.waypoint_id = w.id) \
             ORDER BY w.name COLLATE NOCASE, w.id"
        ))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(waypoint_from_row).collect()
    }

    /// Supprime un waypoint isolé. Retourne `false` s'il n'existait pas.
    ///
    /// # Errors
    ///
    /// [`StorageError::WaypointInUse`] s'il appartient à une route.
    pub async fn delete_waypoint(&self, id: Uuid) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        let used: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM route_points WHERE waypoint_id = ? LIMIT 1")
                .bind(id.to_string())
                .fetch_optional(&mut *tx)
                .await?;
        if used.is_some() {
            return Err(StorageError::WaypointInUse(id));
        }
        let deleted = sqlx::query("DELETE FROM waypoints WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?
            .rows_affected();
        tx.commit().await?;
        Ok(deleted > 0)
    }
}
