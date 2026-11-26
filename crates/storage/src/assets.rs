//! The `groups`, `assets`, `meters`, and `custom_fields` tables (decisions #030, #052): their
//! writers for the command pipeline, and the queries the server needs.

use domain::access::AssetScope;
use domain::asset::{Asset, AssetId, AssetPatch, Identifier, InService, LifecycleStatus};
use domain::custom_field::{
    CustomField, CustomFieldId, CustomFieldPatch, FieldKey, FieldKind, FieldRange,
};
use domain::dates::CalendarDate;
use domain::formula::Formula;
use domain::garage::{GarageOrder, GarageOrderId, GarageOrderPatch};
use domain::group::{Group, GroupId, GroupPatch};
use domain::id::Id;
use domain::meter::{EventKind, Meter, MeterId, MeterPatch};
use domain::notes::Notes;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{
    clearable, id_from_sql, id_to_sql, invalid, json_from_sql, json_to_sql, name, required,
    required_name,
};
use crate::{Database, StorageError};

/// An optional named value's column text.
fn optional_name(
    value: Option<&impl serde::Serialize>,
    column: &'static str,
) -> Result<Option<String>, StorageError> {
    value.map(|value| name(value, column)).transpose()
}

/// Runs a soft delete (`UPDATE … SET deleted = 1, sequence = ? WHERE id = ?`; each table's
/// statement is written out, since sqlx only takes literal SQL).
async fn soft_delete(
    connection: &mut SqliteConnection,
    statement: &'static str,
    id: Vec<u8>,
    sequence: i64,
) -> Result<(), StorageError> {
    sqlx::query(statement)
        .bind(sequence)
        .bind(id)
        .execute(connection)
        .await?;
    Ok(())
}

// --- Groups ------------------------------------------------------------------------------

/// Writes the `groups` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct GroupWriter;

impl RecordWriter<Group> for GroupWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Group>,
        values: &GroupPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO groups (id, name, icon, sequence) VALUES (?, ?, ?, ?)")
            .bind(id_to_sql(id))
            .bind(required(values.name.as_ref(), "name")?.as_str())
            .bind(name(required(values.icon.as_ref(), "icon")?, "icon")?)
            .bind(sequence)
            .execute(connection)
            .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Group>,
        patch: &GroupPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE groups SET name = coalesce(?, name), icon = coalesce(?, icon), sequence = ?
             WHERE id = ?",
        )
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(optional_name(patch.icon.as_ref(), "icon")?)
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Group>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        soft_delete(
            connection,
            "UPDATE groups SET deleted = 1, sequence = ? WHERE id = ?",
            id_to_sql(id),
            sequence,
        )
        .await
    }
}

fn group_from_row(row: &SqliteRow) -> Result<(GroupId, Group), StorageError> {
    let name: String = row.try_get("name")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "groups.id")?,
        Group {
            name: domain::group::GroupName::parse(&name).map_err(|_| {
                StorageError::InvalidValue {
                    column: "groups.name",
                }
            })?,
            icon: required_name(row.try_get("icon")?, "groups.icon")?,
        },
    ))
}

// --- Assets ------------------------------------------------------------------------------

/// Writes the `assets` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct AssetWriter;

impl RecordWriter<Asset> for AssetWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Asset>,
        values: &AssetPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let unset = |field: &Option<domain::command::Clearable<String>>| {
            field
                .clone()
                .and_then(domain::command::Clearable::into_option)
        };
        let in_service = values
            .in_service
            .clone()
            .and_then(domain::command::Clearable::into_option);
        sqlx::query(
            "INSERT INTO assets (id, kind, name, make, model, year, colour, description,
                                 group_id, parent_id, status, identifiers, main_photo,
                                 in_service_since, in_service_estimated, sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(required(values.kind.as_ref(), "kind")?.as_str())
        .bind(required(values.name.as_ref(), "name")?.as_str())
        .bind(unset(&values.make))
        .bind(unset(&values.model))
        .bind(
            values
                .year
                .and_then(domain::command::Clearable::into_option),
        )
        .bind(unset(&values.colour))
        .bind(
            values
                .description
                .clone()
                .unwrap_or_default()
                .as_str()
                .to_owned(),
        )
        .bind(id_to_sql(required(values.group, "group_id")?))
        .bind(
            values
                .parent
                .and_then(domain::command::Clearable::into_option)
                .map(id_to_sql),
        )
        .bind(name(&values.status.unwrap_or_default(), "status")?)
        .bind(json_to_sql(
            &values.identifiers.clone().unwrap_or_default(),
            "identifiers",
        )?)
        .bind(
            values
                .main_photo
                .and_then(domain::command::Clearable::into_option)
                .map(id_to_sql),
        )
        .bind(
            in_service
                .as_ref()
                .map(|since| since.on.as_str().to_owned()),
        )
        .bind(in_service.as_ref().is_some_and(|since| since.estimated))
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Asset>,
        patch: &AssetPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // Optional columns can be cleared, so each has a "changed?" flag (see `clearable`).
        let make = clearable(patch.make.as_ref(), |text| text);
        let model = clearable(patch.model.as_ref(), |text| text);
        let year = clearable(patch.year.as_ref(), |year| year);
        let colour = clearable(patch.colour.as_ref(), |text| text);
        let parent = clearable(patch.parent.as_ref(), id_to_sql);
        let main_photo = clearable(patch.main_photo.as_ref(), id_to_sql);
        // Its two columns change together.
        let in_service_on = clearable(patch.in_service.as_ref(), |since| {
            since.on.as_str().to_owned()
        });
        let in_service_estimated = clearable(patch.in_service.as_ref(), |since| since.estimated);
        let identifiers = patch
            .identifiers
            .as_ref()
            .map(|list| json_to_sql(list, "identifiers"))
            .transpose()?;
        sqlx::query(
            "UPDATE assets SET
                 kind = coalesce(?, kind),
                 name = coalesce(?, name),
                 make = CASE WHEN ? THEN ? ELSE make END,
                 model = CASE WHEN ? THEN ? ELSE model END,
                 year = CASE WHEN ? THEN ? ELSE year END,
                 colour = CASE WHEN ? THEN ? ELSE colour END,
                 description = coalesce(?, description),
                 group_id = coalesce(?, group_id),
                 parent_id = CASE WHEN ? THEN ? ELSE parent_id END,
                 status = coalesce(?, status),
                 identifiers = coalesce(?, identifiers),
                 main_photo = CASE WHEN ? THEN ? ELSE main_photo END,
                 in_service_since = CASE WHEN ? THEN ? ELSE in_service_since END,
                 in_service_estimated = CASE WHEN ? THEN coalesce(?, 0) ELSE in_service_estimated END,
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.kind.as_ref().map(|kind| kind.as_str().to_owned()))
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(make.0)
        .bind(make.1)
        .bind(model.0)
        .bind(model.1)
        .bind(year.0)
        .bind(year.1)
        .bind(colour.0)
        .bind(colour.1)
        .bind(
            patch
                .description
                .as_ref()
                .map(|notes| notes.as_str().to_owned()),
        )
        .bind(patch.group.map(id_to_sql))
        .bind(parent.0)
        .bind(parent.1)
        .bind(optional_name(patch.status.as_ref(), "status")?)
        .bind(identifiers)
        .bind(main_photo.0)
        .bind(main_photo.1)
        .bind(in_service_on.0)
        .bind(in_service_on.1)
        .bind(in_service_estimated.0)
        .bind(in_service_estimated.1)
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Asset>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        soft_delete(
            connection,
            "UPDATE assets SET deleted = 1, sequence = ? WHERE id = ?",
            id_to_sql(id),
            sequence,
        )
        .await
    }
}

fn asset_from_row(row: &SqliteRow) -> Result<(AssetId, Asset), StorageError> {
    let kind: String = row.try_get("kind")?;
    let name: String = row.try_get("name")?;
    let description: String = row.try_get("description")?;
    let identifiers: String = row.try_get("identifiers")?;
    let year: Option<i64> = row.try_get("year")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "assets.id")?,
        Asset {
            kind: domain::asset::AssetKind::parse(&kind).map_err(invalid("assets.kind"))?,
            name: domain::asset::AssetName::parse(&name).map_err(invalid("assets.name"))?,
            make: row.try_get("make")?,
            model: row.try_get("model")?,
            year: year
                .map(u16::try_from)
                .transpose()
                .map_err(invalid("assets.year"))?,
            colour: row.try_get("colour")?,
            description: Notes::parse(&description).map_err(invalid("assets.description"))?,
            group: id_from_sql(row.try_get::<&[u8], _>("group_id")?, "assets.group_id")?,
            parent: row
                .try_get::<Option<&[u8]>, _>("parent_id")?
                .map(|bytes| id_from_sql(bytes, "assets.parent_id"))
                .transpose()?,
            status: required_name::<LifecycleStatus>(row.try_get("status")?, "assets.status")?,
            identifiers: json_from_sql::<Vec<Identifier>>(&identifiers, "assets.identifiers")?,
            main_photo: row
                .try_get::<Option<&[u8]>, _>("main_photo")?
                .map(|bytes| id_from_sql(bytes, "assets.main_photo"))
                .transpose()?,
            in_service: row
                .try_get::<Option<String>, _>("in_service_since")?
                .map(|on| -> Result<InService, StorageError> {
                    Ok(InService {
                        on: CalendarDate::parse(&on).map_err(invalid("assets.in_service_since"))?,
                        estimated: row.try_get("in_service_estimated")?,
                    })
                })
                .transpose()?,
        },
    ))
}

// --- Meters ------------------------------------------------------------------------------

/// Writes the `meters` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct MeterWriter;

impl RecordWriter<Meter> for MeterWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Meter>,
        values: &MeterPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO meters (id, asset_id, name, dimension, unit, precision, description,
                                 required_for, direction, reset_to, key, formula, sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(required(values.asset, "asset_id")?))
        .bind(required(values.name.as_ref(), "name")?.as_str())
        .bind(name(
            required(values.dimension.as_ref(), "dimension")?,
            "dimension",
        )?)
        .bind(name(required(values.unit.as_ref(), "unit")?, "unit")?)
        .bind(i64::from(required(values.precision, "precision")?))
        .bind(
            values
                .description
                .clone()
                .and_then(domain::command::Clearable::into_option),
        )
        .bind(json_to_sql(
            &values.required_for.clone().unwrap_or_default(),
            "required_for",
        )?)
        .bind(name(&values.direction.unwrap_or_default(), "direction")?)
        .bind(
            values
                .reset_to
                .and_then(domain::command::Clearable::into_option),
        )
        .bind(values.key.as_ref().map(|key| key.as_str().to_owned()))
        .bind(
            values
                .formula
                .as_ref()
                .map(|formula| formula.as_str().to_owned()),
        )
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Meter>,
        patch: &MeterPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let description = clearable(patch.description.as_ref(), |text| text);
        let reset_to = clearable(patch.reset_to.as_ref(), |value| value);
        let required_for = patch
            .required_for
            .as_ref()
            .map(|kinds| json_to_sql(kinds, "required_for"))
            .transpose()?;
        sqlx::query(
            "UPDATE meters SET
                 asset_id = coalesce(?, asset_id),
                 name = coalesce(?, name),
                 dimension = coalesce(?, dimension),
                 unit = coalesce(?, unit),
                 precision = coalesce(?, precision),
                 description = CASE WHEN ? THEN ? ELSE description END,
                 required_for = coalesce(?, required_for),
                 direction = coalesce(?, direction),
                 reset_to = CASE WHEN ? THEN ? ELSE reset_to END,
                 key = coalesce(?, key),
                 formula = coalesce(?, formula),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.asset.map(id_to_sql))
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(optional_name(patch.dimension.as_ref(), "dimension")?)
        .bind(optional_name(patch.unit.as_ref(), "unit")?)
        .bind(patch.precision.map(i64::from))
        .bind(description.0)
        .bind(description.1)
        .bind(required_for)
        .bind(optional_name(patch.direction.as_ref(), "direction")?)
        .bind(reset_to.0)
        .bind(reset_to.1)
        .bind(patch.key.as_ref().map(|key| key.as_str().to_owned()))
        .bind(
            patch
                .formula
                .as_ref()
                .map(|formula| formula.as_str().to_owned()),
        )
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Meter>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        soft_delete(
            connection,
            "UPDATE meters SET deleted = 1, sequence = ? WHERE id = ?",
            id_to_sql(id),
            sequence,
        )
        .await
    }
}

fn meter_from_row(row: &SqliteRow) -> Result<(MeterId, Meter), StorageError> {
    let name: String = row.try_get("name")?;
    let name = domain::label::Label::parse(&name).map_err(invalid("meters.name"))?;
    let precision: i64 = row.try_get("precision")?;
    let required_for: String = row.try_get("required_for")?;
    let key: Option<String> = row.try_get("key")?;
    let formula: Option<String> = row.try_get("formula")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "meters.id")?,
        Meter {
            asset: id_from_sql(row.try_get::<&[u8], _>("asset_id")?, "meters.asset_id")?,
            // Until the server gives an older meter its key (at startup), one from its name.
            key: match key {
                Some(key) => FieldKey::parse(&key).map_err(invalid("meters.key"))?,
                None => FieldKey::from_label(&name, &[]),
            },
            formula: formula
                .as_deref()
                .map(Formula::parse)
                .transpose()
                .map_err(invalid("meters.formula"))?,
            name,
            dimension: required_name(row.try_get("dimension")?, "meters.dimension")?,
            unit: required_name(row.try_get("unit")?, "meters.unit")?,
            precision: u8::try_from(precision).map_err(invalid("meters.precision"))?,
            description: row.try_get("description")?,
            required_for: json_from_sql::<Vec<EventKind>>(&required_for, "meters.required_for")?,
            direction: required_name(row.try_get("direction")?, "meters.direction")?,
            reset_to: row.try_get("reset_to")?,
        },
    ))
}

// --- Custom fields -----------------------------------------------------------------------

/// Writes the `custom_fields` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct CustomFieldWriter;

/// A range as its three columns: `(has_range, min, max)`.
fn range_columns(range: Option<FieldRange>) -> (bool, Option<i64>, Option<i64>) {
    range.map_or((false, None, None), |range| (true, range.min, range.max))
}

impl RecordWriter<CustomField> for CustomFieldWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<CustomField>,
        values: &CustomFieldPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let (has_range, min, max) = range_columns(
            values
                .range
                .and_then(domain::command::Clearable::into_option),
        );
        sqlx::query(
            "INSERT INTO custom_fields (id, asset_id, shown_on, key, label, kind, enabled,
                                        required_for, has_range, range_min, range_max, sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(required(values.asset, "asset_id")?))
        .bind(json_to_sql(
            required(values.shown_on.as_ref(), "shown_on")?,
            "shown_on",
        )?)
        .bind(required(values.key.as_ref(), "key")?.as_str())
        .bind(required(values.label.as_ref(), "label")?.as_str())
        .bind(json_to_sql(
            required(values.kind.as_ref(), "kind")?,
            "kind",
        )?)
        .bind(values.enabled.unwrap_or(true))
        .bind(json_to_sql(
            &values.required_for.clone().unwrap_or_default(),
            "required_for",
        )?)
        .bind(has_range)
        .bind(min)
        .bind(max)
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<CustomField>,
        patch: &CustomFieldPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // The key is fixed once made (#057), so a patch's key is ignored.
        // The range's three columns change together, or not at all.
        let range_change = patch
            .range
            .map(|change| range_columns(change.into_option()));
        let kind = patch
            .kind
            .as_ref()
            .map(|kind| json_to_sql(kind, "kind"))
            .transpose()?;
        sqlx::query(
            "UPDATE custom_fields SET
                 asset_id = coalesce(?, asset_id),
                 shown_on = coalesce(?, shown_on),
                 label = coalesce(?, label),
                 kind = coalesce(?, kind),
                 enabled = coalesce(?, enabled),
                 required_for = coalesce(?, required_for),
                 has_range = CASE WHEN ? THEN ? ELSE has_range END,
                 range_min = CASE WHEN ? THEN ? ELSE range_min END,
                 range_max = CASE WHEN ? THEN ? ELSE range_max END,
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.asset.map(id_to_sql))
        .bind(
            patch
                .shown_on
                .as_ref()
                .map(|kinds| json_to_sql(kinds, "shown_on"))
                .transpose()?,
        )
        .bind(patch.label.as_ref().map(|label| label.as_str().to_owned()))
        .bind(kind)
        .bind(patch.enabled)
        .bind(
            patch
                .required_for
                .as_ref()
                .map(|kinds| json_to_sql(kinds, "required_for"))
                .transpose()?,
        )
        .bind(range_change.is_some())
        .bind(range_change.map(|(has, _, _)| has))
        .bind(range_change.is_some())
        .bind(range_change.and_then(|(_, min, _)| min))
        .bind(range_change.is_some())
        .bind(range_change.and_then(|(_, _, max)| max))
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<CustomField>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        soft_delete(
            connection,
            "UPDATE custom_fields SET deleted = 1, sequence = ? WHERE id = ?",
            id_to_sql(id),
            sequence,
        )
        .await
    }
}

fn custom_field_from_row(row: &SqliteRow) -> Result<(CustomFieldId, CustomField), StorageError> {
    let shown_on: String = row.try_get("shown_on")?;
    let required_for: String = row.try_get("required_for")?;
    let key: String = row.try_get("key")?;
    let label: String = row.try_get("label")?;
    let kind: String = row.try_get("kind")?;
    let has_range: bool = row.try_get("has_range")?;
    let range = has_range.then(|| -> Result<FieldRange, StorageError> {
        Ok(FieldRange {
            min: row.try_get("range_min")?,
            max: row.try_get("range_max")?,
        })
    });
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "custom_fields.id")?,
        CustomField {
            asset: id_from_sql(
                row.try_get::<&[u8], _>("asset_id")?,
                "custom_fields.asset_id",
            )?,
            shown_on: json_from_sql::<Vec<EventKind>>(&shown_on, "custom_fields.shown_on")?,
            key: FieldKey::parse(&key).map_err(|_| StorageError::InvalidValue {
                column: "custom_fields.key",
            })?,
            label: domain::label::Label::parse(&label).map_err(|_| StorageError::InvalidValue {
                column: "custom_fields.label",
            })?,
            kind: json_from_sql::<FieldKind>(&kind, "custom_fields.kind")?,
            enabled: row.try_get("enabled")?,
            required_for: json_from_sql::<Vec<EventKind>>(
                &required_for,
                "custom_fields.required_for",
            )?,
            range: range.transpose()?,
        },
    ))
}

// --- Garage orders -----------------------------------------------------------------------

/// Writes the `garage_orders` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct GarageOrderWriter;

impl RecordWriter<GarageOrder> for GarageOrderWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<GarageOrder>,
        values: &GarageOrderPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO garage_orders (id, vehicles, pinned, sequence) VALUES (?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(json_to_sql(
            &values.order.clone().unwrap_or_default(),
            "vehicles",
        )?)
        .bind(json_to_sql(
            &values.pinned.clone().unwrap_or_default(),
            "pinned",
        )?)
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<GarageOrder>,
        patch: &GarageOrderPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let vehicles = patch
            .order
            .as_ref()
            .map(|order| json_to_sql(order, "vehicles"))
            .transpose()?;
        let pinned = patch
            .pinned
            .as_ref()
            .map(|pinned| json_to_sql(pinned, "pinned"))
            .transpose()?;
        sqlx::query(
            "UPDATE garage_orders SET vehicles = coalesce(?, vehicles), \
             pinned = coalesce(?, pinned), sequence = ? WHERE id = ?",
        )
        .bind(vehicles)
        .bind(pinned)
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<GarageOrder>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        soft_delete(
            connection,
            "UPDATE garage_orders SET deleted = 1, sequence = ? WHERE id = ?",
            id_to_sql(id),
            sequence,
        )
        .await
    }
}

// --- Queries -----------------------------------------------------------------------------

impl Database {
    /// Every group (not deleted), in name order.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn groups(&self) -> Result<Vec<(GroupId, Group)>, StorageError> {
        sqlx::query("SELECT id, name, icon FROM groups WHERE deleted = 0 ORDER BY name")
            .fetch_all(self.pool())
            .await?
            .iter()
            .map(group_from_row)
            .collect()
    }

    /// Every asset (not deleted), archived ones included, in name order.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn assets(&self) -> Result<Vec<(AssetId, Asset)>, StorageError> {
        sqlx::query(
            "SELECT id, kind, name, make, model, year, colour, description, group_id, parent_id,
                    status, identifiers, main_photo, in_service_since, in_service_estimated
             FROM assets WHERE deleted = 0 ORDER BY name",
        )
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(asset_from_row)
        .collect()
    }

    /// One asset, if it exists and isn't deleted.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn asset(&self, id: AssetId) -> Result<Option<Asset>, StorageError> {
        sqlx::query(
            "SELECT id, kind, name, make, model, year, colour, description, group_id, parent_id,
                    status, identifiers, main_photo, in_service_since, in_service_estimated
             FROM assets WHERE id = ? AND deleted = 0",
        )
        .bind(id_to_sql(id))
        .fetch_optional(self.pool())
        .await?
        .as_ref()
        .map(|row| asset_from_row(row).map(|(_, asset)| asset))
        .transpose()
    }

    /// An asset as the permission check sees it (its ID and group), if it exists.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn asset_scope(&self, id: AssetId) -> Result<Option<AssetScope>, StorageError> {
        sqlx::query("SELECT group_id FROM assets WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .map(|row| -> Result<AssetScope, StorageError> {
                Ok(AssetScope {
                    id,
                    group: id_from_sql(row.try_get::<&[u8], _>("group_id")?, "assets.group_id")?,
                })
            })
            .transpose()
    }

    /// How many assets (not deleted) are in a group.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn assets_in_group(&self, group: GroupId) -> Result<i64, StorageError> {
        Ok(
            sqlx::query_scalar("SELECT count(*) FROM assets WHERE group_id = ? AND deleted = 0")
                .bind(id_to_sql(group))
                .fetch_one(self.pool())
                .await?,
        )
    }

    /// A person's custom Garage order and pinned vehicles, if they've made either.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or the stored list is invalid.
    pub async fn garage_order(
        &self,
        id: GarageOrderId,
    ) -> Result<Option<GarageOrder>, StorageError> {
        let row =
            sqlx::query("SELECT vehicles, pinned FROM garage_orders WHERE id = ? AND deleted = 0")
                .bind(id_to_sql(id))
                .fetch_optional(self.pool())
                .await?;
        row.map(|row| -> Result<GarageOrder, StorageError> {
            let vehicles: String = row.try_get("vehicles")?;
            let pinned: String = row.try_get("pinned")?;
            Ok(GarageOrder {
                order: json_from_sql(&vehicles, "garage_orders.vehicles")?,
                pinned: json_from_sql(&pinned, "garage_orders.pinned")?,
            })
        })
        .transpose()
    }

    /// One asset's meters (not deleted), in name order.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn meters_of(&self, asset: AssetId) -> Result<Vec<(MeterId, Meter)>, StorageError> {
        sqlx::query(
            "SELECT id, asset_id, name, dimension, unit, precision, description, required_for,
                    direction, reset_to, key, formula
             FROM meters WHERE asset_id = ? AND deleted = 0 ORDER BY name",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(meter_from_row)
        .collect()
    }

    /// The meters (not deleted) without a stored key yet, each with its vehicle: every meter
    /// from before migration 0027 until the server gives it one.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn unkeyed_meters(&self) -> Result<Vec<(MeterId, AssetId)>, StorageError> {
        sqlx::query("SELECT id, asset_id FROM meters WHERE key IS NULL AND deleted = 0 ORDER BY id")
            .fetch_all(self.pool())
            .await?
            .iter()
            .map(|row| {
                Ok((
                    id_from_sql(row.try_get::<&[u8], _>("id")?, "meters.id")?,
                    id_from_sql(row.try_get::<&[u8], _>("asset_id")?, "meters.asset_id")?,
                ))
            })
            .collect()
    }

    /// One meter, if it exists and isn't deleted.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn meter(&self, id: MeterId) -> Result<Option<Meter>, StorageError> {
        sqlx::query(
            "SELECT id, asset_id, name, dimension, unit, precision, description, required_for,
                    direction, reset_to, key, formula
             FROM meters WHERE id = ? AND deleted = 0",
        )
        .bind(id_to_sql(id))
        .fetch_optional(self.pool())
        .await?
        .as_ref()
        .map(|row| meter_from_row(row).map(|(_, meter)| meter))
        .transpose()
    }

    /// One custom field definition, if it exists and isn't deleted.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn custom_field(
        &self,
        id: CustomFieldId,
    ) -> Result<Option<CustomField>, StorageError> {
        sqlx::query(
            "SELECT id, asset_id, shown_on, key, label, kind, enabled, required_for, has_range,
                    range_min, range_max
             FROM custom_fields WHERE id = ? AND deleted = 0",
        )
        .bind(id_to_sql(id))
        .fetch_optional(self.pool())
        .await?
        .as_ref()
        .map(|row| custom_field_from_row(row).map(|(_, field)| field))
        .transpose()
    }

    /// One asset's custom field definitions (not deleted), in label order.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn custom_fields_of(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(CustomFieldId, CustomField)>, StorageError> {
        sqlx::query(
            "SELECT id, asset_id, shown_on, key, label, kind, enabled, required_for, has_range,
                    range_min, range_max
             FROM custom_fields WHERE asset_id = ? AND deleted = 0 ORDER BY label",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(custom_field_from_row)
        .collect()
    }

    /// Every key one asset's custom fields have used, removed ones too (a key is never reused).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored key is invalid.
    pub async fn field_keys_of(&self, asset: AssetId) -> Result<Vec<FieldKey>, StorageError> {
        let keys: Vec<String> =
            sqlx::query_scalar("SELECT key FROM custom_fields WHERE asset_id = ? ORDER BY key")
                .bind(id_to_sql(asset))
                .fetch_all(self.pool())
                .await?;
        keys.iter()
            .map(|key| {
                FieldKey::parse(key).map_err(|_| StorageError::InvalidValue {
                    column: "custom_fields.key",
                })
            })
            .collect()
    }
}
