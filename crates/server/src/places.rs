//! Place lookup and each person's saved places on the server (decisions #038, #059).
//!
//! - **The place data** (a GeoPackage, `geocoder`) is opened once at startup. It's looked for,
//!   in order: the path in `VEHICLEMANAGER_GEODATA_FILE`; the newest `geodata-*.gpkg` in a
//!   `geodata` folder beside the server (the image's `/app/geodata`); the newest in
//!   `.geodata/out` in the working folder or any folder above it (development,
//!   `scripts/geodata/fetch.sh`). If none opens, the server carries on: lookups say
//!   "unavailable", saved places and typed places still work, and an Admin message says so.
//! - **Saved places are each person's own** (#059): every request works only on the signed-in
//!   person's, and someone else's is "not found".

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use domain::access::Actor;
use domain::api::{Located, PlaceDataInfo, PlaceSuggestion, PlacesOutcome};
use domain::command::{Change, Patch};
use domain::location::{
    Coordinates, SavedPlace, SavedPlaceForm, SavedPlaceId, SavedPlacePatch, nearest_containing,
};
use domain::log_area;
use domain::time::{Clock, UnixMillis};
use geocoder::Geocoder;
use storage::Database;
use storage::places::SavedPlaceWriter;

use crate::commands::issue;
use crate::health::Health;
use crate::ids::new_id;
use crate::sessions::SignedIn;

/// The environment variable naming the place data file, without the app prefix.
pub const GEODATA_FILE_ENV_NAME: &str = "GEODATA_FILE";
/// The folder beside the server binary that holds the data (the image's `/app/geodata`).
pub const GEODATA_DIR_NAME: &str = "geodata";
/// Where `scripts/geodata/fetch.sh` writes the data in development.
pub const DEV_GEODATA_DIR: &str = ".geodata/out";

/// The place data, once [`load`] has tried to open it.
enum GeocoderState {
    Ready(Geocoder),
    Unavailable { since: UnixMillis, reason: String },
}

static GEOCODER: OnceLock<GeocoderState> = OnceLock::new();

/// The newest `geodata-*.gpkg` in `folder` (names carry the date, so the last name wins).
fn newest_in(folder: &Path) -> Option<PathBuf> {
    std::fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let named = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("geodata-"));
            let gpkg = path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("gpkg"));
            named && gpkg
        })
        .max()
}

/// Where the place data is: `explicit` if given; else the newest in `geodata/` beside the
/// server (`exe_dir`); else the newest in `.geodata/out` in `working_dir` or a folder above it.
#[must_use]
pub fn find_geodata(
    explicit: Option<&Path>,
    exe_dir: Option<&Path>,
    working_dir: &Path,
) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return Some(path.to_path_buf());
    }
    exe_dir
        .and_then(|dir| newest_in(&dir.join(GEODATA_DIR_NAME)))
        .or_else(|| {
            working_dir
                .ancestors()
                .find_map(|dir| newest_in(&dir.join(DEV_GEODATA_DIR)))
        })
}

/// Opens the place data once per process (later calls keep the first result). `explicit` is
/// the `VEHICLEMANAGER_GEODATA_FILE` value, if set.
pub async fn load(explicit: Option<PathBuf>, now: UnixMillis) {
    if GEOCODER.get().is_some() {
        return;
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let working_dir = std::env::current_dir().unwrap_or_default();
    let state = match find_geodata(explicit.as_deref(), exe_dir.as_deref(), &working_dir) {
        None => {
            tracing::warn!(target: log_area::APP, "no place data found; place lookup is unavailable");
            GeocoderState::Unavailable {
                since: now,
                reason: "no geodata-*.gpkg file was found".to_owned(),
            }
        }
        Some(path) => open(&path, now).await,
    };
    let _ = GEOCODER.set(state);
}

async fn open(path: &Path, now: UnixMillis) -> GeocoderState {
    let started = std::time::Instant::now();
    let size_mb = std::fs::metadata(path).map_or(0, |meta| meta.len() / 1_000_000);
    match Geocoder::open(path).await {
        Ok(geocoder) => {
            let load_ms = started.elapsed().as_millis();
            tracing::info!(target: log_area::APP, path = %path.display(), data_version = %geocoder.info().data_version, size_mb, load_ms, "place data opened");
            GeocoderState::Ready(geocoder)
        }
        Err(error) => {
            tracing::error!(target: log_area::APP, path = %path.display(), %error, "can't open the place data; place lookup is unavailable");
            GeocoderState::Unavailable {
                since: now,
                reason: error.to_string(),
            }
        }
    }
}

/// The open place data, if [`load`] opened it.
#[must_use]
pub fn geocoder() -> Option<&'static Geocoder> {
    match GEOCODER.get()? {
        GeocoderState::Ready(geocoder) => Some(geocoder),
        GeocoderState::Unavailable { .. } => None,
    }
}

/// Since when, and why, place lookup is unavailable (for the Admin message), if it is.
#[must_use]
pub fn lookup_problem() -> Option<(UnixMillis, String)> {
    match GEOCODER.get()? {
        GeocoderState::Ready(_) => None,
        GeocoderState::Unavailable { since, reason } => Some((*since, reason.clone())),
    }
}

/// What a places request needs.
pub struct PlacesContext<'a> {
    /// The database.
    pub database: &'a Database,
    /// Who is asking.
    pub user: &'a SignedIn,
    /// The server's health (changes are refused while unhealthy).
    pub health: Health,
    /// The real clock.
    pub clock: &'a (dyn Clock + Sync),
    /// The place data, if it's open.
    pub geocoder: Option<&'a Geocoder>,
}

fn unavailable(what: &str, error: &dyn std::fmt::Display) -> PlacesOutcome {
    tracing::error!(target: log_area::APP, what, %error, "places request failed");
    PlacesOutcome::Unavailable
}

fn either(result: Result<PlacesOutcome, PlacesOutcome>) -> PlacesOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

impl PlacesContext<'_> {
    /// Saves a change to one of the person's places, as them.
    async fn save(&self, change: Change<SavedPlace>) -> Result<(), PlacesOutcome> {
        let actor = Actor {
            user: self.user.user,
            mode: self.user.mode,
        };
        issue(
            self.database,
            &SavedPlaceWriter,
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

    /// The person's saved places.
    async fn mine(&self) -> Result<Vec<(SavedPlaceId, SavedPlace)>, PlacesOutcome> {
        self.database
            .saved_places_of(self.user.user)
            .await
            .map_err(|error| unavailable("saved places", &error))
    }

    /// One of the person's saved places; someone else's is "not found".
    async fn own(&self, id: SavedPlaceId) -> Result<SavedPlace, PlacesOutcome> {
        self.database
            .saved_place(id)
            .await
            .map_err(|error| unavailable("saved place", &error))?
            .filter(|place| place.owner == self.user.user)
            .ok_or(PlacesOutcome::NotFound)
    }
}

// --- Place lookup --------------------------------------------------------------------------

/// What's known about `point`: the person's saved place it's in, and the suggested place.
pub async fn locate(context: &PlacesContext<'_>, point: Coordinates) -> PlacesOutcome {
    either(located(context, point).await)
}

async fn located(
    context: &PlacesContext<'_>,
    point: Coordinates,
) -> Result<PlacesOutcome, PlacesOutcome> {
    let mine = context.mine().await?;
    let saved = nearest_containing(&mine, point).and_then(|(id, _)| {
        mine.iter()
            .find(|(each, _)| *each == id)
            .map(|(id, place)| (*id, place.clone()))
    });
    let suggestion = match context.geocoder {
        None => None,
        Some(geocoder) => match geocoder.lookup(point).await {
            Ok(found) => Some(PlaceSuggestion {
                place: found.place,
                country_name: found.country_name,
                time_zone: found.time_zone,
                inside_country: found.inside_country,
                data_version: found.data_version,
                nearby: found.nearby,
            }),
            Err(error) => {
                tracing::error!(target: log_area::APP, %error, "place lookup failed");
                None
            }
        },
    };
    tracing::debug!(target: log_area::APP, user = %context.user.user, saved = saved.is_some(), suggested = suggestion.is_some(), "located");
    Ok(PlacesOutcome::Located {
        located: Box::new(Located { saved, suggestion }),
    })
}

/// Every country, by name (empty while place lookup is unavailable).
pub async fn countries(context: &PlacesContext<'_>) -> PlacesOutcome {
    let Some(geocoder) = context.geocoder else {
        return PlacesOutcome::Countries {
            countries: Vec::new(),
        };
    };
    match geocoder.countries().await {
        Ok(countries) => PlacesOutcome::Countries { countries },
        Err(error) => unavailable("countries", &error),
    }
}

/// About the place data.
#[must_use]
pub fn data_info(geocoder: Option<&Geocoder>) -> PlacesOutcome {
    PlacesOutcome::DataInfo {
        info: geocoder.map(|geocoder| PlaceDataInfo {
            data_version: geocoder.info().data_version.clone(),
            attribution: geocoder.info().attribution.clone(),
        }),
    }
}

// --- Saved places --------------------------------------------------------------------------

/// The person's saved places, by name.
pub async fn list(context: &PlacesContext<'_>) -> PlacesOutcome {
    match context.mine().await {
        Ok(places) => PlacesOutcome::Places { places },
        Err(outcome) => outcome,
    }
}

/// Adds a saved place for the person.
pub async fn add(context: &PlacesContext<'_>, form: &SavedPlaceForm) -> PlacesOutcome {
    either(add_place(context, form).await)
}

async fn add_place(
    context: &PlacesContext<'_>,
    form: &SavedPlaceForm,
) -> Result<PlacesOutcome, PlacesOutcome> {
    let place = form
        .parse(context.user.user)
        .map_err(|problem| PlacesOutcome::Invalid { problem })?;
    let id: SavedPlaceId =
        new_id(context.clock.now()).map_err(|error| unavailable("new id", &error))?;
    context
        .save(Change::Create {
            id,
            values: SavedPlacePatch::all(place),
        })
        .await?;
    tracing::debug!(target: log_area::APP, user = %context.user.user, place = %id, "saved place added");
    Ok(PlacesOutcome::Added { id })
}

/// Changes one of the person's saved places (only the fields that differ, #043).
pub async fn edit(
    context: &PlacesContext<'_>,
    id: SavedPlaceId,
    form: &SavedPlaceForm,
) -> PlacesOutcome {
    either(edit_place(context, id, form).await)
}

async fn edit_place(
    context: &PlacesContext<'_>,
    id: SavedPlaceId,
    form: &SavedPlaceForm,
) -> Result<PlacesOutcome, PlacesOutcome> {
    let current = context.own(id).await?;
    let wanted = form
        .parse(context.user.user)
        .map_err(|problem| PlacesOutcome::Invalid { problem })?;
    let patch = SavedPlacePatch {
        owner: None,
        name: (wanted.name != current.name).then_some(wanted.name),
        coordinates: (wanted.coordinates != current.coordinates).then_some(wanted.coordinates),
        radius_m: (wanted.radius_m != current.radius_m).then_some(wanted.radius_m),
        place: (wanted.place != current.place).then_some(wanted.place),
    };
    if !patch.changed_fields().is_empty() {
        context.save(Change::Update { id, patch }).await?;
    }
    Ok(PlacesOutcome::Done)
}

/// Removes one of the person's saved places. Past events keep their own copy (#059).
pub async fn delete(context: &PlacesContext<'_>, id: SavedPlaceId) -> PlacesOutcome {
    either(delete_place(context, id).await)
}

async fn delete_place(
    context: &PlacesContext<'_>,
    id: SavedPlaceId,
) -> Result<PlacesOutcome, PlacesOutcome> {
    context.own(id).await?;
    context.save(Change::<SavedPlace>::Delete { id }).await?;
    tracing::debug!(target: log_area::APP, user = %context.user.user, place = %id, "saved place removed");
    Ok(PlacesOutcome::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    #[test]
    fn the_data_file_is_found_in_order() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("app");
        let project = temp.path().join("project");
        let deep = project.join("crates").join("app");
        std::fs::create_dir_all(&deep).unwrap();

        // Nothing anywhere.
        assert_eq!(find_geodata(None, Some(&exe), &deep), None);

        // Development: .geodata/out in a folder above the working one; the newest name wins.
        touch(
            &project
                .join(DEV_GEODATA_DIR)
                .join("geodata-2026-01-01.gpkg"),
        );
        touch(
            &project
                .join(DEV_GEODATA_DIR)
                .join("geodata-2026-11-02.gpkg"),
        );
        touch(&project.join(DEV_GEODATA_DIR).join("notes.txt"));
        assert_eq!(
            find_geodata(None, Some(&exe), &deep),
            Some(
                project
                    .join(DEV_GEODATA_DIR)
                    .join("geodata-2026-11-02.gpkg")
            )
        );

        // The image: geodata/ beside the server comes first.
        touch(&exe.join(GEODATA_DIR_NAME).join("geodata-2025-06-01.gpkg"));
        assert_eq!(
            find_geodata(None, Some(&exe), &deep),
            Some(exe.join(GEODATA_DIR_NAME).join("geodata-2025-06-01.gpkg"))
        );

        // An explicit file wins over both.
        let chosen = temp.path().join("elsewhere.gpkg");
        assert_eq!(find_geodata(Some(&chosen), Some(&exe), &deep), Some(chosen));
    }

    #[test]
    fn the_env_var_comes_from_the_app_id() {
        assert_eq!(
            domain::identity::env_var_name(GEODATA_FILE_ENV_NAME),
            "VEHICLEMANAGER_GEODATA_FILE"
        );
    }
}
