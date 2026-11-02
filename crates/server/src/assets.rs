//! Vehicles, their meters, and their custom fields on the server (decisions #030, #052).
//!
//! Who may do what (plan §6 roles, `domain::access`):
//! - **See** a vehicle: any role that covers it (on it, its group, or all vehicles).
//! - **Add** one: the household's Admin, in Admin mode. It gets copies of its profile's
//!   meters and custom fields (the odometer in the person's miles/km preference).
//! - **Edit, archive** one: the Admin role on it, in Admin mode. **Delete** one (for mistakes):
//!   the household's Admin, typing its name. **Move** one to another group (which changes who
//!   can see it): the household's Admin, after a preview of who gains and loses access.
//! - **Meters and custom fields:** the Maintainer role on it, in Maintainer or Admin mode.
//!
//! Every change is a command made as the person, so it's in the audit trail.

use domain::access::{
    AdminAction, AssetAction, AssetScope, access_change, permits_admin, permits_on_asset, role_on,
};
use domain::api::{AssetSummary, AssetView, AssetsOutcome, GarageView, GroupSummary, MoveChange};
use domain::asset::{Asset, AssetId, AssetPatch};
use domain::asset_form::{AssetForm, FieldForm, MeterForm, Problem, confirms_name};
use domain::command::{Change, Clearable, Record};
use domain::custom_field::{CustomField, CustomFieldId, CustomFieldPatch, FieldKey};
use domain::garage::{GarageOrderId, GarageOrderPatch, MAX_ORDER_LENGTH};
use domain::group::GroupId;
use domain::i18n::Translator;
use domain::id::{DatabaseId, Id};
use domain::log_area;
use domain::meter::{Meter, MeterId, MeterPatch};
use domain::preferences::{Preferences, PreferencesId};
use domain::profile::profile;
use domain::time::Clock;
use storage::Database;
use storage::assets::{AssetWriter, CustomFieldWriter, GarageOrderWriter, MeterWriter};
use storage::commands::RecordWriter;

use crate::commands::issue;
use crate::health::Health;
use crate::ids::new_id;
use crate::sessions::SignedIn;

/// What vehicle requests need.
pub struct AssetsContext<'a> {
    /// The database.
    pub database: &'a Database,
    /// Its ID (the household preferences' record ID).
    pub database_id: DatabaseId,
    /// Who is asking.
    pub user: &'a SignedIn,
    /// The server's health.
    pub health: Health,
    /// The real clock.
    pub clock: &'a (dyn Clock + Sync),
}

/// Logs a failure and answers "unavailable".
pub(crate) fn unavailable(what: &str, error: &dyn std::fmt::Display) -> AssetsOutcome {
    tracing::error!(target: log_area::APP, what, %error, "vehicle request failed");
    AssetsOutcome::Unavailable
}

impl AssetsContext<'_> {
    /// Saves a change as the person.
    pub(crate) async fn save<R: Record, W: RecordWriter<R> + Sync>(
        &self,
        writer: &W,
        change: Change<R>,
    ) -> Result<(), AssetsOutcome> {
        let actor = domain::access::Actor {
            user: self.user.user,
            mode: self.user.mode,
        };
        issue(
            self.database,
            writer,
            change,
            self.user.device,
            Some(actor),
            self.health,
            self.clock,
        )
        .await
        .map(|_| ())
        .map_err(|error| unavailable("save", &error))
    }

    /// A new record ID.
    pub(crate) fn new_id<T>(&self) -> Result<Id<T>, AssetsOutcome> {
        new_id(self.clock.now()).map_err(|error| unavailable("new id", &error))
    }

    /// Whether the person may do a household-wide `action` now.
    pub(crate) fn may(&self, action: AdminAction) -> bool {
        permits_admin(&self.user.grants, self.user.mode, action)
    }

    /// The vehicle as the permission check sees it, if it exists and the person can see it.
    async fn visible(&self, id: AssetId) -> Result<AssetScope, AssetsOutcome> {
        let scope = self
            .database
            .asset_scope(id)
            .await
            .map_err(|error| unavailable("asset scope", &error))?
            .ok_or(AssetsOutcome::NotFound)?;
        if role_on(&self.user.grants, scope).is_none() {
            return Err(AssetsOutcome::NotFound);
        }
        Ok(scope)
    }

    /// The vehicle, if the person may do `action` on it now.
    async fn allowed(&self, id: AssetId, action: AssetAction) -> Result<Asset, AssetsOutcome> {
        let scope = self.visible(id).await?;
        if !permits_on_asset(&self.user.grants, self.user.mode, action, scope) {
            return Err(AssetsOutcome::NotAllowed);
        }
        self.database
            .asset(id)
            .await
            .map_err(|error| unavailable("asset", &error))?
            .ok_or(AssetsOutcome::NotFound)
    }

    /// Whether `group` exists.
    async fn group_exists(&self, group: GroupId) -> Result<bool, AssetsOutcome> {
        let groups = self
            .database
            .groups()
            .await
            .map_err(|error| unavailable("groups", &error))?;
        Ok(groups.iter().any(|(id, _)| *id == group))
    }
}

/// Turns `Result<AssetsOutcome, AssetsOutcome>` into the outcome either way.
fn either(result: Result<AssetsOutcome, AssetsOutcome>) -> AssetsOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

// --- The Garage and one vehicle ----------------------------------------------------------

/// The Garage: the groups and vehicles the person can see.
pub async fn garage(context: &AssetsContext<'_>) -> AssetsOutcome {
    either(garage_view(context).await)
}

async fn garage_view(context: &AssetsContext<'_>) -> Result<AssetsOutcome, AssetsOutcome> {
    let database = context.database;
    let groups = database
        .groups()
        .await
        .map_err(|error| unavailable("groups", &error))?;
    let all = database
        .assets()
        .await
        .map_err(|error| unavailable("assets", &error))?;
    let grants = &context.user.grants;
    // Each vehicle the person's grants cover, with their role on it.
    let assets: Vec<AssetSummary> = all
        .into_iter()
        .filter_map(|(id, asset)| {
            let scope = AssetScope {
                id,
                group: asset.group,
            };
            role_on(grants, scope).map(|role| AssetSummary { id, asset, role })
        })
        .collect();
    let can_manage_groups = context.may(AdminAction::ManageGroups);
    // Each group with how many visible vehicles it has; empty ones only for the Admin.
    let groups: Vec<GroupSummary> = groups
        .into_iter()
        .map(|(id, group)| GroupSummary {
            id,
            group,
            vehicles: assets
                .iter()
                .filter(|summary| summary.asset.group == id)
                .count(),
        })
        .filter(|summary| summary.vehicles > 0 || can_manage_groups)
        .collect();
    let order = database
        .garage_order(context.user.user.cast())
        .await
        .map_err(|error| unavailable("garage order", &error))?
        .unwrap_or_default()
        .order;
    tracing::debug!(target: log_area::APP, user = %context.user.user, vehicles = assets.len(), groups = groups.len(), "garage");
    Ok(AssetsOutcome::Garage {
        view: GarageView {
            groups,
            assets,
            can_add: context.may(AdminAction::AddAsset),
            can_manage_groups,
            sort: person_preferences(context).await?.garage_sort,
            order,
        },
    })
}

/// One vehicle's page.
pub async fn view(context: &AssetsContext<'_>, id: AssetId) -> AssetsOutcome {
    either(asset_view(context, id).await)
}

async fn asset_view(
    context: &AssetsContext<'_>,
    id: AssetId,
) -> Result<AssetsOutcome, AssetsOutcome> {
    let scope = context.visible(id).await?;
    let database = context.database;
    let asset = database
        .asset(id)
        .await
        .map_err(|error| unavailable("asset", &error))?
        .ok_or(AssetsOutcome::NotFound)?;
    let meters = database
        .meters_of(id)
        .await
        .map_err(|error| unavailable("meters", &error))?;
    let fields = database
        .custom_fields_of(id)
        .await
        .map_err(|error| unavailable("custom fields", &error))?;
    let grants = &context.user.grants;
    let mode = context.user.mode;
    Ok(AssetsOutcome::Asset {
        view: Box::new(AssetView {
            id,
            asset,
            role: role_on(grants, scope).ok_or(AssetsOutcome::NotFound)?,
            can_manage: permits_on_asset(grants, mode, AssetAction::ManageAsset, scope),
            can_configure: permits_on_asset(grants, mode, AssetAction::ConfigureMaintenance, scope),
            can_move: context.may(AdminAction::ManageGroups),
            meters,
            fields,
        }),
    })
}

/// Saves the person's custom Garage order (#056): their own, so anyone signed in may.
pub async fn set_order(context: &AssetsContext<'_>, order: Vec<AssetId>) -> AssetsOutcome {
    either(save_order(context, order).await)
}

async fn save_order(
    context: &AssetsContext<'_>,
    mut order: Vec<AssetId>,
) -> Result<AssetsOutcome, AssetsOutcome> {
    order.truncate(MAX_ORDER_LENGTH);
    let id: GarageOrderId = context.user.user.cast();
    let exists = context
        .database
        .garage_order(id)
        .await
        .map_err(|error| unavailable("garage order", &error))?
        .is_some();
    let patch = GarageOrderPatch { order: Some(order) };
    let change = if exists {
        Change::Update { id, patch }
    } else {
        Change::Create { id, values: patch }
    };
    context.save(&GarageOrderWriter, change).await?;
    tracing::debug!(target: log_area::APP, user = %context.user.user, "garage order saved");
    Ok(AssetsOutcome::Done)
}

// --- Adding, editing, archiving, deleting ------------------------------------------------

/// Adds a vehicle from `form`, with copies of its profile's meters and custom fields.
pub async fn add(context: &AssetsContext<'_>, form: &AssetForm) -> AssetsOutcome {
    either(add_asset(context, form).await)
}

async fn add_asset(
    context: &AssetsContext<'_>,
    form: &AssetForm,
) -> Result<AssetsOutcome, AssetsOutcome> {
    if !context.may(AdminAction::AddAsset) {
        return Err(AssetsOutcome::NotAllowed);
    }
    let asset = form.parse().map_err(invalid)?;
    let kind_profile = profile(&asset.kind).ok_or_else(|| invalid(Problem::UnknownKind))?;
    if !context.group_exists(asset.group).await? {
        return Err(invalid(Problem::UnknownGroup));
    }
    let id: AssetId = context.new_id()?;
    context
        .save(
            &AssetWriter,
            Change::Create {
                id,
                values: AssetPatch::all(asset),
            },
        )
        .await?;
    let distance = person_preferences(context).await?.distance;
    let english = Translator::english();
    for meter in kind_profile.new_meters(id, distance, english) {
        let meter_id: MeterId = context.new_id()?;
        let values = MeterPatch::all(meter);
        context
            .save(
                &MeterWriter,
                Change::Create {
                    id: meter_id,
                    values,
                },
            )
            .await?;
    }
    for field in kind_profile.new_fields(id, english) {
        let field_id: CustomFieldId = context.new_id()?;
        let values = CustomFieldPatch::all(field);
        context
            .save(
                &CustomFieldWriter,
                Change::Create {
                    id: field_id,
                    values,
                },
            )
            .await?;
    }
    tracing::info!(target: log_area::APP, user = %context.user.user, asset = %id, kind = kind_profile.id.as_str(), "vehicle added");
    Ok(AssetsOutcome::Added { id })
}

/// The preferences in effect for the person (for a new odometer's unit).
async fn person_preferences(context: &AssetsContext<'_>) -> Result<Preferences, AssetsOutcome> {
    let mine: PreferencesId = context.user.user.cast();
    let household: PreferencesId = context.database_id.cast();
    let database = context.database;
    let mine = database
        .preference_choices(mine)
        .await
        .map_err(|error| unavailable("preferences", &error))?;
    let household = database
        .preference_choices(household)
        .await
        .map_err(|error| unavailable("preferences", &error))?;
    Ok(Preferences::resolve(&mine, &household))
}

fn invalid(problem: Problem) -> AssetsOutcome {
    AssetsOutcome::Invalid { problem }
}

/// Changes a vehicle's details (not its kind or group) to `form`'s, including its lifecycle
/// status (archiving is setting it to retired or disposed).
pub async fn edit(context: &AssetsContext<'_>, id: AssetId, form: &AssetForm) -> AssetsOutcome {
    either(edit_asset(context, id, form).await)
}

async fn edit_asset(
    context: &AssetsContext<'_>,
    id: AssetId,
    form: &AssetForm,
) -> Result<AssetsOutcome, AssetsOutcome> {
    let current = context.allowed(id, AssetAction::ManageAsset).await?;
    // The kind and group can't change here, whatever the form says.
    let wanted = AssetForm {
        kind: current.kind.as_str().to_owned(),
        group: current.group,
        ..form.clone()
    }
    .parse()
    .map_err(invalid)?;
    let patch = changes(&current, &wanted);
    if !domain::command::Patch::changed_fields(&patch).is_empty() {
        context
            .save(&AssetWriter, Change::Update { id, patch })
            .await?;
    }
    tracing::debug!(target: log_area::APP, user = %context.user.user, asset = %id, "vehicle edited");
    Ok(AssetsOutcome::Done)
}

/// Only the fields that differ, so a change never overwrites someone else's newer change to
/// a field this person didn't touch (#043).
fn changes(current: &Asset, wanted: &Asset) -> AssetPatch {
    /// `Some(change)` when `wanted` differs from `current`.
    fn differs<T: PartialEq + Clone>(current: &T, wanted: &T) -> Option<T> {
        (current != wanted).then(|| wanted.clone())
    }
    /// The same for an optional field, as a clear-or-set change.
    fn clears<T: PartialEq + Clone>(
        current: Option<&T>,
        wanted: Option<&T>,
    ) -> Option<Clearable<T>> {
        (current != wanted).then(|| Clearable::from_option(wanted.cloned()))
    }
    AssetPatch {
        kind: None,
        name: differs(&current.name, &wanted.name),
        make: clears(current.make.as_ref(), wanted.make.as_ref()),
        model: clears(current.model.as_ref(), wanted.model.as_ref()),
        year: clears(current.year.as_ref(), wanted.year.as_ref()),
        colour: clears(current.colour.as_ref(), wanted.colour.as_ref()),
        description: differs(&current.description, &wanted.description),
        group: None,
        parent: None,
        status: differs(&current.status, &wanted.status),
        identifiers: differs(&current.identifiers, &wanted.identifiers),
    }
}

/// Deletes a vehicle added by mistake (soft delete), if `typed_name` is its name.
pub async fn delete(context: &AssetsContext<'_>, id: AssetId, typed_name: &str) -> AssetsOutcome {
    either(delete_asset(context, id, typed_name).await)
}

async fn delete_asset(
    context: &AssetsContext<'_>,
    id: AssetId,
    typed_name: &str,
) -> Result<AssetsOutcome, AssetsOutcome> {
    if !context.may(AdminAction::AddAsset) {
        return Err(AssetsOutcome::NotAllowed);
    }
    let asset = context.allowed(id, AssetAction::ManageAsset).await?;
    if !confirms_name(typed_name, &asset.name) {
        return Err(invalid(Problem::NameMismatch));
    }
    context
        .save(&AssetWriter, Change::<Asset>::Delete { id })
        .await?;
    tracing::info!(target: log_area::APP, user = %context.user.user, asset = %id, "vehicle deleted");
    Ok(AssetsOutcome::Done)
}

// --- Moving between groups ---------------------------------------------------------------

/// Who gains, loses, or changes access if the vehicle moves to `to`.
pub async fn preview_move(context: &AssetsContext<'_>, id: AssetId, to: GroupId) -> AssetsOutcome {
    either(move_preview(context, id, to).await)
}

async fn move_preview(
    context: &AssetsContext<'_>,
    id: AssetId,
    to: GroupId,
) -> Result<AssetsOutcome, AssetsOutcome> {
    let from = check_move(context, id, to).await?;
    let database = context.database;
    let grants: Vec<_> = database
        .all_grants()
        .await
        .map_err(|error| unavailable("grants", &error))?
        .into_iter()
        .map(|(_, grant)| grant)
        .collect();
    let users = database
        .users()
        .await
        .map_err(|error| unavailable("users", &error))?;
    // Each affected person, with their name for the preview.
    let changes = access_change(&grants, id, from, to)
        .into_iter()
        .map(|change| MoveChange {
            user: change.user,
            display_name: users
                .iter()
                .find(|stored| stored.id == change.user)
                .map(|stored| stored.user.display_name.clone())
                .unwrap_or_default(),
            before: change.before,
            after: change.after,
        })
        .collect();
    Ok(AssetsOutcome::MovePreview { changes })
}

/// Moves a vehicle to group `to`.
pub async fn move_to(context: &AssetsContext<'_>, id: AssetId, to: GroupId) -> AssetsOutcome {
    either(move_asset(context, id, to).await)
}

async fn move_asset(
    context: &AssetsContext<'_>,
    id: AssetId,
    to: GroupId,
) -> Result<AssetsOutcome, AssetsOutcome> {
    let from = check_move(context, id, to).await?;
    if from != to {
        let patch = AssetPatch {
            group: Some(to),
            ..AssetPatch::default()
        };
        context
            .save(&AssetWriter, Change::Update { id, patch })
            .await?;
    }
    tracing::info!(target: log_area::APP, user = %context.user.user, asset = %id, from = %from, to = %to, "vehicle moved");
    Ok(AssetsOutcome::Done)
}

/// Checks a move is allowed and the group exists; returns the vehicle's current group.
async fn check_move(
    context: &AssetsContext<'_>,
    id: AssetId,
    to: GroupId,
) -> Result<GroupId, AssetsOutcome> {
    if !context.may(AdminAction::ManageGroups) {
        return Err(AssetsOutcome::NotAllowed);
    }
    let scope = context.visible(id).await?;
    if !context.group_exists(to).await? {
        return Err(invalid(Problem::UnknownGroup));
    }
    Ok(scope.group)
}

// --- Meters ------------------------------------------------------------------------------

/// Adds a meter to a vehicle.
pub async fn add_meter(
    context: &AssetsContext<'_>,
    asset: AssetId,
    form: &MeterForm,
) -> AssetsOutcome {
    either(add_meter_to(context, asset, form).await)
}

async fn add_meter_to(
    context: &AssetsContext<'_>,
    asset: AssetId,
    form: &MeterForm,
) -> Result<AssetsOutcome, AssetsOutcome> {
    context
        .allowed(asset, AssetAction::ConfigureMaintenance)
        .await?;
    let parsed = form.parse().map_err(invalid)?;
    let meter = Meter {
        asset,
        name: parsed.name,
        dimension: parsed.dimension,
        unit: parsed.unit,
        precision: parsed.precision,
        description: parsed.description,
        required_for: parsed.required_for,
    };
    let id: MeterId = context.new_id()?;
    context
        .save(
            &MeterWriter,
            Change::Create {
                id,
                values: MeterPatch::all(meter),
            },
        )
        .await?;
    tracing::debug!(target: log_area::APP, asset = %asset, meter = %id, "meter added");
    Ok(AssetsOutcome::Done)
}

/// Changes a meter's settings (not what it measures).
pub async fn edit_meter(
    context: &AssetsContext<'_>,
    id: MeterId,
    form: &MeterForm,
) -> AssetsOutcome {
    either(edit_meter_settings(context, id, form).await)
}

async fn edit_meter_settings(
    context: &AssetsContext<'_>,
    id: MeterId,
    form: &MeterForm,
) -> Result<AssetsOutcome, AssetsOutcome> {
    let current = existing_meter(context, id).await?;
    // What a meter measures is fixed: its readings depend on it.
    let parsed = MeterForm {
        dimension: current.dimension,
        ..form.clone()
    }
    .parse()
    .map_err(invalid)?;
    let patch = MeterPatch {
        name: (parsed.name != current.name).then_some(parsed.name),
        unit: (parsed.unit != current.unit).then_some(parsed.unit),
        precision: (parsed.precision != current.precision).then_some(parsed.precision),
        description: (parsed.description != current.description)
            .then(|| Clearable::from_option(parsed.description)),
        required_for: (parsed.required_for != current.required_for).then_some(parsed.required_for),
        ..MeterPatch::default()
    };
    if !domain::command::Patch::changed_fields(&patch).is_empty() {
        context
            .save(&MeterWriter, Change::Update { id, patch })
            .await?;
    }
    Ok(AssetsOutcome::Done)
}

/// Removes a meter.
pub async fn delete_meter(context: &AssetsContext<'_>, id: MeterId) -> AssetsOutcome {
    either(remove_meter(context, id).await)
}

async fn remove_meter(
    context: &AssetsContext<'_>,
    id: MeterId,
) -> Result<AssetsOutcome, AssetsOutcome> {
    existing_meter(context, id).await?;
    context
        .save(&MeterWriter, Change::<Meter>::Delete { id })
        .await?;
    Ok(AssetsOutcome::Done)
}

/// A meter, if the person may configure its vehicle now.
async fn existing_meter(context: &AssetsContext<'_>, id: MeterId) -> Result<Meter, AssetsOutcome> {
    let meter = context
        .database
        .meter(id)
        .await
        .map_err(|error| unavailable("meter", &error))?
        .ok_or(AssetsOutcome::NotFound)?;
    context
        .allowed(meter.asset, AssetAction::ConfigureMaintenance)
        .await?;
    Ok(meter)
}

// --- Custom fields -----------------------------------------------------------------------

/// Adds a custom field to a vehicle.
pub async fn add_field(
    context: &AssetsContext<'_>,
    asset: AssetId,
    form: &FieldForm,
) -> AssetsOutcome {
    either(add_field_to(context, asset, form).await)
}

async fn add_field_to(
    context: &AssetsContext<'_>,
    asset: AssetId,
    form: &FieldForm,
) -> Result<AssetsOutcome, AssetsOutcome> {
    context
        .allowed(asset, AssetAction::ConfigureMaintenance)
        .await?;
    let label = form.parse().map_err(invalid)?;
    let taken = context
        .database
        .field_keys_of(asset)
        .await
        .map_err(|error| unavailable("field keys", &error))?;
    let field = CustomField {
        asset,
        applies_to: form.applies_to,
        key: FieldKey::from_label(&label, &taken),
        label,
        kind: form.kind.clone(),
        enabled: form.enabled,
        required: form.required,
        range: form.range,
    };
    let id: CustomFieldId = context.new_id()?;
    context
        .save(
            &CustomFieldWriter,
            Change::Create {
                id,
                values: CustomFieldPatch::all(field),
            },
        )
        .await?;
    tracing::debug!(target: log_area::APP, asset = %asset, field = %id, "custom field added");
    Ok(AssetsOutcome::Done)
}

/// Changes a custom field's label, on/off, required, or range (not its kind or event kind).
pub async fn edit_field(
    context: &AssetsContext<'_>,
    id: CustomFieldId,
    form: &FieldForm,
) -> AssetsOutcome {
    either(edit_field_settings(context, id, form).await)
}

async fn edit_field_settings(
    context: &AssetsContext<'_>,
    id: CustomFieldId,
    form: &FieldForm,
) -> Result<AssetsOutcome, AssetsOutcome> {
    let current = existing_field(context, id).await?;
    // Its kind and event kind are fixed: values depend on them.
    let checked = FieldForm {
        applies_to: current.applies_to,
        kind: current.kind.clone(),
        ..form.clone()
    };
    let label = checked.parse().map_err(invalid)?;
    let patch = CustomFieldPatch {
        label: (label != current.label).then_some(label),
        enabled: (checked.enabled != current.enabled).then_some(checked.enabled),
        required: (checked.required != current.required).then_some(checked.required),
        range: (checked.range != current.range).then(|| Clearable::from_option(checked.range)),
        ..CustomFieldPatch::default()
    };
    if !domain::command::Patch::changed_fields(&patch).is_empty() {
        context
            .save(&CustomFieldWriter, Change::Update { id, patch })
            .await?;
    }
    Ok(AssetsOutcome::Done)
}

/// Removes a custom field.
pub async fn delete_field(context: &AssetsContext<'_>, id: CustomFieldId) -> AssetsOutcome {
    either(remove_field(context, id).await)
}

async fn remove_field(
    context: &AssetsContext<'_>,
    id: CustomFieldId,
) -> Result<AssetsOutcome, AssetsOutcome> {
    existing_field(context, id).await?;
    context
        .save(&CustomFieldWriter, Change::<CustomField>::Delete { id })
        .await?;
    Ok(AssetsOutcome::Done)
}

/// A custom field, if the person may configure its vehicle now.
async fn existing_field(
    context: &AssetsContext<'_>,
    id: CustomFieldId,
) -> Result<CustomField, AssetsOutcome> {
    let field = context
        .database
        .custom_field(id)
        .await
        .map_err(|error| unavailable("custom field", &error))?
        .ok_or(AssetsOutcome::NotFound)?;
    context
        .allowed(field.asset, AssetAction::ConfigureMaintenance)
        .await?;
    Ok(field)
}
