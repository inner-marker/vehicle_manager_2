//! Shops and the household's labor rate on the server (phase-6.md 2f; decision #089).
//!
//! Who may do what:
//! - **See** them: anyone signed in (a job's "Done by" suggests the shops, and a typed labor
//!   time is priced from the rates).
//! - **Change** them (add, change, or remove a shop; set the household's rate): a Maintainer or
//!   higher on any vehicle, in that mode. They're household-wide, like the rates they hold.
//!
//! Changes go to the server only, as templates' do: on the desktop they're read from its copy,
//! and changing them needs the server.

use domain::access::{Role, highest_role};
use domain::api::{ShopsOutcome, ShopsView};
use domain::command::Change;
use domain::log_area;
use domain::preferences::{Choice, Preferences, PreferencesId, PreferencesPatch};
use domain::shop::{LaborRate, Shop, ShopId, ShopPatch, check_shop};
use storage::preferences::PreferencesWriter;
use storage::shops::ShopWriter;

use crate::assets::{AssetsContext, unavailable};
use crate::sessions::SignedIn;

/// Whether `user` may change the shops and the rate: a Maintainer or higher, in that mode.
#[must_use]
pub fn may_manage(user: &SignedIn) -> bool {
    highest_role(&user.grants)
        .map(|role| role.min(user.mode.cap()))
        .is_some_and(|role| role >= Role::Maintainer)
}

/// Logs a failure and answers "unavailable".
fn failed(what: &str, error: &dyn std::fmt::Display) -> ShopsOutcome {
    let _ = unavailable(what, error);
    ShopsOutcome::Unavailable
}

fn either(result: Result<ShopsOutcome, ShopsOutcome>) -> ShopsOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

impl AssetsContext<'_> {
    fn household_preferences(&self) -> PreferencesId {
        self.database_id.cast()
    }

    async fn save_shop(&self, change: Change<Shop>) -> Result<(), ShopsOutcome> {
        self.save(&ShopWriter, change)
            .await
            .map_err(|_| ShopsOutcome::Unavailable)
    }

    async fn kept_shops(&self) -> Result<Vec<(ShopId, Shop)>, ShopsOutcome> {
        self.database
            .shops()
            .await
            .map_err(|error| failed("shops", &error))
    }

    fn managing(&self) -> Result<(), ShopsOutcome> {
        if may_manage(self.user) {
            Ok(())
        } else {
            Err(ShopsOutcome::NotAllowed)
        }
    }
}

/// Settings → Labor rates: the household's rate and currency, its shops, and whether the person
/// may change them.
pub async fn list(context: &AssetsContext<'_>) -> ShopsOutcome {
    either(list_shops(context).await)
}

async fn list_shops(context: &AssetsContext<'_>) -> Result<ShopsOutcome, ShopsOutcome> {
    let household = context
        .database
        .preference_choices(context.household_preferences())
        .await
        .map_err(|error| failed("preferences", &error))?;
    let effective = Preferences::resolve(&household, &household);
    Ok(ShopsOutcome::Shops {
        view: ShopsView {
            household_rate: household.labor_rate,
            currency: effective.currency,
            shops: context.kept_shops().await?,
            may_manage: may_manage(context.user),
        },
    })
}

/// Adds `shop`.
pub async fn add(context: &AssetsContext<'_>, shop: Shop) -> ShopsOutcome {
    either(add_shop(context, shop).await)
}

async fn add_shop(context: &AssetsContext<'_>, shop: Shop) -> Result<ShopsOutcome, ShopsOutcome> {
    context.managing()?;
    let kept = context.kept_shops().await?;
    check_shop(&shop, None, kept.iter().map(|(id, shop)| (*id, shop)))
        .map_err(|problem| ShopsOutcome::Invalid { problem })?;
    let id: ShopId = context.new_id().map_err(|_| ShopsOutcome::Unavailable)?;
    context
        .save_shop(Change::Create {
            id,
            values: ShopPatch::all(shop),
        })
        .await?;
    tracing::debug!(target: log_area::APP, shop = %id, "shop added");
    Ok(ShopsOutcome::Saved { id })
}

/// Changes shop `id` to `shop` (only what differs is written).
pub async fn edit(context: &AssetsContext<'_>, id: ShopId, shop: Shop) -> ShopsOutcome {
    either(edit_shop(context, id, shop).await)
}

async fn edit_shop(
    context: &AssetsContext<'_>,
    id: ShopId,
    shop: Shop,
) -> Result<ShopsOutcome, ShopsOutcome> {
    context.managing()?;
    let kept = context.kept_shops().await?;
    let Some((_, before)) = kept.iter().find(|(kept_id, _)| *kept_id == id) else {
        return Err(ShopsOutcome::NotFound);
    };
    check_shop(&shop, Some(id), kept.iter().map(|(id, shop)| (*id, shop)))
        .map_err(|problem| ShopsOutcome::Invalid { problem })?;
    let patch = ShopPatch::changes(before, shop);
    context.save_shop(Change::Update { id, patch }).await?;
    tracing::debug!(target: log_area::APP, shop = %id, "shop changed");
    Ok(ShopsOutcome::Saved { id })
}

/// Removes shop `id`. Jobs it did keep their "Done by" and their costs.
pub async fn delete(context: &AssetsContext<'_>, id: ShopId) -> ShopsOutcome {
    either(delete_shop(context, id).await)
}

async fn delete_shop(
    context: &AssetsContext<'_>,
    id: ShopId,
) -> Result<ShopsOutcome, ShopsOutcome> {
    context.managing()?;
    context
        .database
        .shop(id)
        .await
        .map_err(|error| failed("shop", &error))?
        .ok_or(ShopsOutcome::NotFound)?;
    context.save_shop(Change::Delete { id }).await?;
    tracing::debug!(target: log_area::APP, shop = %id, "shop removed");
    Ok(ShopsOutcome::Done)
}

/// Sets the household's labor rate (`None`: no rate).
pub async fn set_household_rate(
    context: &AssetsContext<'_>,
    rate: Option<LaborRate>,
) -> ShopsOutcome {
    either(set_rate(context, rate).await)
}

async fn set_rate(
    context: &AssetsContext<'_>,
    rate: Option<LaborRate>,
) -> Result<ShopsOutcome, ShopsOutcome> {
    context.managing()?;
    let id = context.household_preferences();
    let patch = PreferencesPatch {
        labor_rate: Some(rate.map_or(Choice::Default, Choice::Set)),
        ..PreferencesPatch::default()
    };
    let exists = context
        .database
        .preferences_exist(id)
        .await
        .map_err(|error| failed("preferences", &error))?;
    let change = if exists {
        Change::Update { id, patch }
    } else {
        Change::Create { id, values: patch }
    };
    context
        .save(&PreferencesWriter, change)
        .await
        .map_err(|_| ShopsOutcome::Unavailable)?;
    tracing::debug!(target: log_area::APP, set = rate.is_some(), "household labor rate changed");
    Ok(ShopsOutcome::Done)
}
