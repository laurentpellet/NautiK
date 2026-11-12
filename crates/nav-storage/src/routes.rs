//! Repository des routes.
//!
//! Les waypoints d'une route sont enregistrés avec elle. Un waypoint
//! retiré d'une route (ou dont la route est supprimée) et qui n'est plus
//! référencé par aucune route est supprimé : les waypoints isolés se
//! gèrent séparément.

use std::collections::HashSet;

use nav_routing::Route;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};
use uuid::Uuid;

use crate::codec::{time_from_db, time_to_db, uuid_from_db};
use crate::waypoints::{upsert_waypoint, waypoint_from_row, WAYPOINT_COLUMNS};
use crate::{Db, StorageError};

const ROUTE_COLUMNS: &str = "id, name, description, created_at, updated_at";

async fn route_waypoint_ids(
    conn: &mut SqliteConnection,
    route_id: Uuid,
) -> Result<Vec<String>, StorageError> {
    Ok(
        sqlx::query_scalar("SELECT waypoint_id FROM route_points WHERE route_id = ?")
            .bind(route_id.to_string())
            .fetch_all(conn)
            .await?,
    )
}

async fn delete_orphans(
    conn: &mut SqliteConnection,
    candidates: impl IntoIterator<Item = String>,
) -> Result<(), StorageError> {
    for id in candidates {
        sqlx::query(
            "DELETE FROM waypoints WHERE id = ?1 \
             AND NOT EXISTS (SELECT 1 FROM route_points WHERE waypoint_id = ?1)",
        )
        .bind(id)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

impl Db {
    /// Enregistre (crée ou remplace) une route et ses waypoints, en une
    /// transaction.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite (rien n'est écrit).
    pub async fn save_route(&self, route: &Route) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        save_route_in(&mut tx, route).await?;
        tx.commit().await?;
        Ok(())
    }
}

pub(crate) async fn save_route_in(
    conn: &mut SqliteConnection,
    route: &Route,
) -> Result<(), StorageError> {
    {
        let previous = route_waypoint_ids(&mut *conn, route.id).await?;
        sqlx::query(
            "INSERT INTO routes (id, name, description, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, \
               description = excluded.description, updated_at = excluded.updated_at",
        )
        .bind(route.id.to_string())
        .bind(&route.name)
        .bind(&route.description)
        .bind(time_to_db(route.created_at))
        .bind(time_to_db(route.updated_at))
        .execute(&mut *conn)
        .await?;
        sqlx::query("DELETE FROM route_points WHERE route_id = ?")
            .bind(route.id.to_string())
            .execute(&mut *conn)
            .await?;
        for (sequence, waypoint) in route.waypoints.iter().enumerate() {
            upsert_waypoint(&mut *conn, waypoint).await?;
            sqlx::query(
                "INSERT INTO route_points (route_id, waypoint_id, sequence) VALUES (?, ?, ?)",
            )
            .bind(route.id.to_string())
            .bind(waypoint.id.to_string())
            .bind(i64::try_from(sequence).unwrap_or(i64::MAX))
            .execute(&mut *conn)
            .await?;
        }
        let kept: HashSet<String> = route.waypoints.iter().map(|w| w.id.to_string()).collect();
        delete_orphans(conn, previous.into_iter().filter(|id| !kept.contains(id))).await?;
        Ok(())
    }
}

impl Db {
    /// Charge une route.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée corrompue.
    pub async fn get_route(&self, id: Uuid) -> Result<Option<Route>, StorageError> {
        let mut conn = self.pool.acquire().await?;
        let row = sqlx::query(&format!("SELECT {ROUTE_COLUMNS} FROM routes WHERE id = ?"))
            .bind(id.to_string())
            .fetch_optional(&mut *conn)
            .await?;
        match row {
            Some(row) => Ok(Some(load_route(&mut conn, &row).await?)),
            None => Ok(None),
        }
    }

    /// Liste les routes, par nom.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée corrompue.
    pub async fn list_routes(&self) -> Result<Vec<Route>, StorageError> {
        let mut conn = self.pool.acquire().await?;
        let rows = sqlx::query(&format!(
            "SELECT {ROUTE_COLUMNS} FROM routes ORDER BY name COLLATE NOCASE, id"
        ))
        .fetch_all(&mut *conn)
        .await?;
        let mut routes = Vec::with_capacity(rows.len());
        for row in &rows {
            routes.push(load_route(&mut conn, row).await?);
        }
        Ok(routes)
    }

    /// Supprime une route et ses waypoints devenus inutilisés. Retourne
    /// `false` si la route n'existait pas.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn delete_route(&self, id: Uuid) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        let previous = route_waypoint_ids(&mut tx, id).await?;
        let deleted = sqlx::query("DELETE FROM routes WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?
            .rows_affected();
        delete_orphans(&mut tx, previous).await?;
        tx.commit().await?;
        Ok(deleted > 0)
    }
}

async fn load_route(conn: &mut SqliteConnection, row: &SqliteRow) -> Result<Route, StorageError> {
    let id: String = row.try_get("id")?;
    let waypoints = sqlx::query(&format!(
        "SELECT {WAYPOINT_COLUMNS} FROM route_points rp \
         JOIN waypoints w ON w.id = rp.waypoint_id \
         WHERE rp.route_id = ? ORDER BY rp.sequence"
    ))
    .bind(&id)
    .fetch_all(conn)
    .await?
    .iter()
    .map(waypoint_from_row)
    .collect::<Result<Vec<_>, _>>()?;
    if waypoints.len() < 2 {
        return Err(StorageError::Corrupt(format!(
            "route {id} : {} waypoint(s)",
            waypoints.len()
        )));
    }
    Ok(Route {
        id: uuid_from_db(&id)?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        waypoints,
        created_at: time_from_db(row.try_get("created_at")?)?,
        updated_at: time_from_db(row.try_get("updated_at")?)?,
    })
}
