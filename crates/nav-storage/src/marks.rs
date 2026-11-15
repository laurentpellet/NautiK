//! Repository des marques personnelles.

use nav_routing::{Mark, MarkIcon};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, Transaction};
use uuid::Uuid;

use crate::codec::{position_from_db, time_from_db, time_to_db, uuid_from_db};
use crate::{Db, StorageError};

const COLUMNS: &str = "id, name, latitude, longitude, icon, note, created_at, updated_at";

fn mark_from_row(row: &SqliteRow) -> Result<Mark, StorageError> {
    let icon: String = row.try_get("icon")?;
    Ok(Mark {
        id: uuid_from_db(row.try_get("id")?)?,
        name: row.try_get("name")?,
        position: position_from_db(row.try_get("latitude")?, row.try_get("longitude")?)?,
        icon: MarkIcon::from_code(&icon)
            .ok_or_else(|| StorageError::Corrupt(format!("icône de marque « {icon} »")))?,
        note: row.try_get("note")?,
        created_at: time_from_db(row.try_get("created_at")?)?,
        updated_at: time_from_db(row.try_get("updated_at")?)?,
    })
}

async fn upsert(tx: &mut Transaction<'_, Sqlite>, mark: &Mark) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO marks (id, name, latitude, longitude, icon, note, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, latitude = excluded.latitude, \
           longitude = excluded.longitude, icon = excluded.icon, note = excluded.note, \
           updated_at = excluded.updated_at",
    )
    .bind(mark.id.to_string())
    .bind(&mark.name)
    .bind(mark.position.latitude_deg())
    .bind(mark.position.longitude_deg())
    .bind(mark.icon.code())
    .bind(&mark.note)
    .bind(time_to_db(mark.created_at))
    .bind(time_to_db(mark.updated_at))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl Db {
    /// Crée ou met à jour une marque.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn save_mark(&self, mark: &Mark) -> Result<(), StorageError> {
        self.save_marks(std::slice::from_ref(mark)).await
    }

    /// Crée ou met à jour plusieurs marques en une transaction (import) :
    /// en cas d'échec, aucune n'est enregistrée.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn save_marks(&self, marks: &[Mark]) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        for mark in marks {
            upsert(&mut tx, mark).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Toutes les marques, par nom.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée corrompue.
    pub async fn list_marks(&self) -> Result<Vec<Mark>, StorageError> {
        sqlx::query(&format!(
            "SELECT {COLUMNS} FROM marks ORDER BY name COLLATE NOCASE, id"
        ))
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(mark_from_row)
        .collect()
    }

    /// Une marque.
    ///
    /// # Errors
    ///
    /// [`StorageError`] en cas d'échec SQLite ou de donnée corrompue.
    pub async fn get_mark(&self, id: Uuid) -> Result<Option<Mark>, StorageError> {
        sqlx::query(&format!("SELECT {COLUMNS} FROM marks WHERE id = ?"))
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(mark_from_row)
            .transpose()
    }

    /// Nombre de marques enregistrées.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn count_marks(&self) -> Result<usize, StorageError> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM marks")
            .fetch_one(&self.pool)
            .await?;
        Ok(usize::try_from(n).unwrap_or(0))
    }

    /// Supprime une marque. Retourne `false` si elle n'existait pas.
    ///
    /// # Errors
    ///
    /// [`StorageError::Database`] en cas d'échec SQLite.
    pub async fn delete_mark(&self, id: Uuid) -> Result<bool, StorageError> {
        let deleted = sqlx::query("DELETE FROM marks WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(deleted > 0)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use chrono::{DateTime, Utc};
    use nav_core::GeoPosition;

    fn t(s: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000 + s, 0).expect("t")
    }

    fn mark(name: &str, icon: MarkIcon) -> Mark {
        Mark::new(
            name,
            GeoPosition::new(-17.5, -149.6).expect("p"),
            icon,
            Some("Fond de sable"),
            t(0),
        )
        .expect("marque")
    }

    #[tokio::test]
    async fn marks_round_trip_update_and_delete() {
        let db = Db::open_in_memory().await.expect("db");
        let mut a = mark("passe", MarkIcon::Danger);
        let b = mark("Baie", MarkIcon::Anchorage);
        db.save_marks(&[a.clone(), b.clone()])
            .await
            .expect("import");
        assert_eq!(db.count_marks().await.expect("n"), 2);
        let list = db.list_marks().await.expect("liste");
        assert_eq!(list, vec![b.clone(), a.clone()], "tri par nom sans casse");

        a.edit("Passe", a.position, MarkIcon::Dive, None, t(5))
            .expect("édition");
        db.save_mark(&a).await.expect("maj");
        let back = db.get_mark(a.id).await.expect("lecture").expect("présente");
        assert_eq!(back, a);
        assert_eq!(back.created_at, t(0), "création conservée");
        assert_eq!(db.count_marks().await.expect("n"), 2);

        assert!(db.delete_mark(a.id).await.expect("suppression"));
        assert!(!db.delete_mark(a.id).await.expect("déjà supprimée"));
        assert_eq!(db.get_mark(a.id).await.expect("lecture"), None);
    }

    #[tokio::test]
    async fn corrupt_mark_is_reported() {
        let db = Db::open_in_memory().await.expect("db");
        sqlx::query(
            "INSERT INTO marks VALUES ('x', 'n', 0, 0, 'licorne', NULL, \
             '2027-01-15T08:00:00.000000000Z', '2027-01-15T08:00:00.000000000Z')",
        )
        .execute(&db.pool)
        .await
        .expect("insertion brute");
        assert!(matches!(
            db.list_marks().await,
            Err(StorageError::Corrupt(_))
        ));
    }

    #[tokio::test]
    async fn invalid_coordinates_are_refused_by_the_schema() {
        let db = Db::open_in_memory().await.expect("db");
        let r = sqlx::query("INSERT INTO marks VALUES ('x', 'n', 95, 0, 'flag', NULL, 'a', 'a')")
            .execute(&db.pool)
            .await;
        assert!(r.is_err());
    }
}
