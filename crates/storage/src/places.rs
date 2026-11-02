//! The `saved_places` table (decision #059): each person's saved places, their writer for the
//! command pipeline, and the queries the server needs.

use domain::id::Id;
use domain::location::{
    Coordinates, Place, SavedPlace, SavedPlaceId, SavedPlacePatch, check_radius,
};
use domain::user::UserId;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{id_from_sql, id_to_sql, required};
use crate::{Database, StorageError};

/// Writes the `saved_places` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct SavedPlaceWriter;

impl RecordWriter<SavedPlace> for SavedPlaceWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<SavedPlace>,
        values: &SavedPlacePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let coordinates = required(values.coordinates.as_ref(), "coordinates")?;
        let place = required(values.place.as_ref(), "place")?;
        sqlx::query(
            "INSERT INTO saved_places (id, owner_id, name, lat_e7, lon_e7, radius_m, city,
                                       region, country_code, sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(*required(values.owner.as_ref(), "owner_id")?))
        .bind(required(values.name.as_ref(), "name")?.as_str())
        .bind(coordinates.lat_e7())
        .bind(coordinates.lon_e7())
        .bind(*required(values.radius_m.as_ref(), "radius_m")?)
        .bind(place.city.as_str())
        .bind(place.region.as_str())
        .bind(place.country.as_str())
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<SavedPlace>,
        patch: &SavedPlacePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // The owner is fixed once made, so a patch's owner is ignored. Coordinates (both
        // columns) and the place (all three) each change together.
        let place = patch.place.as_ref();
        sqlx::query(
            "UPDATE saved_places SET
                 name = coalesce(?, name),
                 lat_e7 = coalesce(?, lat_e7),
                 lon_e7 = coalesce(?, lon_e7),
                 radius_m = coalesce(?, radius_m),
                 city = coalesce(?, city),
                 region = coalesce(?, region),
                 country_code = coalesce(?, country_code),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(patch.coordinates.map(Coordinates::lat_e7))
        .bind(patch.coordinates.map(Coordinates::lon_e7))
        .bind(patch.radius_m)
        .bind(place.map(|place| place.city.as_str().to_owned()))
        .bind(place.map(|place| place.region.as_str().to_owned()))
        .bind(place.map(|place| place.country.as_str().to_owned()))
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<SavedPlace>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE saved_places SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

/// Turns any conversion error into "this column holds an invalid value".
fn invalid<E>(column: &'static str) -> impl Fn(E) -> StorageError {
    move |_| StorageError::InvalidValue { column }
}

fn saved_place_from_row(row: &SqliteRow) -> Result<(SavedPlaceId, SavedPlace), StorageError> {
    let name: String = row.try_get("name")?;
    let city: String = row.try_get("city")?;
    let region: String = row.try_get("region")?;
    let country: String = row.try_get("country_code")?;
    let radius: i64 = row.try_get("radius_m")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "saved_places.id")?,
        SavedPlace {
            owner: id_from_sql(
                row.try_get::<&[u8], _>("owner_id")?,
                "saved_places.owner_id",
            )?,
            name: domain::label::Label::parse(&name).map_err(invalid("saved_places.name"))?,
            coordinates: Coordinates::from_e7(row.try_get("lat_e7")?, row.try_get("lon_e7")?)
                .map_err(invalid("saved_places.lat_e7"))?,
            radius_m: u32::try_from(radius)
                .ok()
                .and_then(|radius| check_radius(radius).ok())
                .ok_or(StorageError::InvalidValue {
                    column: "saved_places.radius_m",
                })?,
            place: Place::parse(&city, &region, &country).map_err(invalid("saved_places.city"))?,
        },
    ))
}

impl Database {
    /// `owner`'s saved places (not deleted), by name (ignoring case).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn saved_places_of(
        &self,
        owner: UserId,
    ) -> Result<Vec<(SavedPlaceId, SavedPlace)>, StorageError> {
        sqlx::query(
            "SELECT id, owner_id, name, lat_e7, lon_e7, radius_m, city, region, country_code
             FROM saved_places WHERE owner_id = ? AND deleted = 0
             ORDER BY name COLLATE NOCASE, id",
        )
        .bind(id_to_sql(owner))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(saved_place_from_row)
        .collect()
    }

    /// One saved place, if it exists and isn't deleted.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn saved_place(&self, id: SavedPlaceId) -> Result<Option<SavedPlace>, StorageError> {
        sqlx::query(
            "SELECT id, owner_id, name, lat_e7, lon_e7, radius_m, city, region, country_code
             FROM saved_places WHERE id = ? AND deleted = 0",
        )
        .bind(id_to_sql(id))
        .fetch_optional(self.pool())
        .await?
        .as_ref()
        .map(|row| saved_place_from_row(row).map(|(_, place)| place))
        .transpose()
    }
}
