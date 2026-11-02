//! Where things happen: coordinates, places, and saved places (plan §3 Event location;
//! decisions #038, #039, #059).
//!
//! - **Coordinates** are whole numbers of degrees × 10⁷ (about 1 cm), like every stored
//!   quantity (plan §3). Floating-point degrees exist only at the edges (typing, the browser,
//!   distances).
//! - A **place** is a city, a state or province (`region`), and an ISO country code. Every event
//!   needs one (milestone 6); the geocoder suggests it from coordinates (milestone 5).
//! - A **fix** is coordinates from a device, with its accuracy and when it was taken; one
//!   older than 20 minutes is stale.
//! - A **saved place** is one person's alias, e.g. "Home garage": a name, coordinates, a
//!   radius, and its place. Choosing one, or being inside its radius, copies it all into the
//!   event, so editing a saved place never changes past events (#059).
//!
//! # Examples
//!
//! ```
//! use domain::location::Coordinates;
//!
//! let asheville = Coordinates::parse_pair("35.5951, -82.5515").unwrap();
//! assert_eq!(asheville.lat_e7(), 355_951_000);
//! let boone = Coordinates::from_degrees(36.2168, -81.6746).unwrap();
//! let km = asheville.distance_m(boone) / 1000.0;
//! assert!((km - 105.0).abs() < 5.0, "{km}");
//! ```

use serde::{Deserialize, Serialize};

use crate::command::{Patch, Record};
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::label::{Label, LabelError};
use crate::time::UnixMillis;
use crate::user::UserId;

/// Degrees are stored as whole numbers of this many parts.
const E7: f64 = 10_000_000.0;
/// The largest latitude, in degrees × 10⁷.
const MAX_LAT_E7: i32 = 900_000_000;
/// The largest longitude, in degrees × 10⁷.
const MAX_LON_E7: i32 = 1_800_000_000;
/// The Earth's mean radius, in meters (for distances).
const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// A fix older than this is stale (plan §3: "e.g. 20+ minutes old").
pub const STALE_FIX_MILLIS: i64 = 20 * 60 * 1000;
/// A new saved place's radius, in meters (#059).
pub const DEFAULT_RADIUS_M: u32 = 150;
/// The smallest radius a saved place may have, in meters.
pub const MIN_RADIUS_M: u32 = 25;
/// The largest radius a saved place may have, in meters.
pub const MAX_RADIUS_M: u32 = 2_000;
/// The most characters a city or region name may have.
pub const PLACE_NAME_MAX_CHARS: usize = 100;
/// The most characters an IANA time zone name may have.
const TIME_ZONE_MAX_CHARS: usize = 64;

/// Why a location (or part of one) was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LocationError {
    /// Latitude outside −90…90.
    LatitudeOutOfRange,
    /// Longitude outside −180…180.
    LongitudeOutOfRange,
    /// Coordinates that aren't two decimal numbers.
    NotCoordinates,
    /// A saved place's radius outside 25 m…2 km.
    RadiusOutOfRange,
    /// No city.
    CityEmpty,
    /// No state or province.
    RegionEmpty,
    /// A city or region longer than [`PLACE_NAME_MAX_CHARS`].
    PlaceNameTooLong,
    /// Not a two-letter country code.
    CountryInvalid,
    /// Not an IANA time zone name.
    TimeZoneInvalid,
    /// A saved place's name.
    Name(LabelError),
}

impl LocationError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::LatitudeOutOfRange => messages::LOCATION_LATITUDE_RANGE,
            Self::LongitudeOutOfRange => messages::LOCATION_LONGITUDE_RANGE,
            Self::NotCoordinates => messages::LOCATION_NOT_COORDINATES,
            Self::RadiusOutOfRange => messages::LOCATION_RADIUS_RANGE,
            Self::CityEmpty => messages::LOCATION_CITY_EMPTY,
            Self::RegionEmpty => messages::LOCATION_REGION_EMPTY,
            Self::PlaceNameTooLong => messages::LOCATION_PLACE_TOO_LONG,
            Self::CountryInvalid => messages::LOCATION_COUNTRY_INVALID,
            Self::TimeZoneInvalid => messages::LOCATION_TIME_ZONE_INVALID,
            Self::Name(error) => error.message(),
        }
    }
}

// --- Coordinates ---------------------------------------------------------------------------

/// A point on the Earth, in whole degrees × 10⁷ (latitude −90…90, longitude −180…180).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawCoordinates", into = "RawCoordinates")]
pub struct Coordinates {
    lat_e7: i32,
    lon_e7: i32,
}

/// [`Coordinates`] as stored and sent, checked when read.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct RawCoordinates {
    lat_e7: i32,
    lon_e7: i32,
}

impl TryFrom<RawCoordinates> for Coordinates {
    type Error = String;

    fn try_from(raw: RawCoordinates) -> Result<Self, Self::Error> {
        Self::from_e7(raw.lat_e7, raw.lon_e7).map_err(|error| format!("{error:?}"))
    }
}

impl From<Coordinates> for RawCoordinates {
    fn from(point: Coordinates) -> Self {
        Self {
            lat_e7: point.lat_e7,
            lon_e7: point.lon_e7,
        }
    }
}

/// Degrees as a whole number of degrees × 10⁷, if within ±`max_e7`.
fn degrees_to_e7(degrees: f64, max_e7: i32) -> Option<i32> {
    let scaled = (degrees * E7).round();
    if !scaled.is_finite() || scaled.abs() > f64::from(max_e7) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "checked above: finite, whole, and within ±1.8e9, which fits an i32"
    )]
    Some(scaled as i32)
}

impl Coordinates {
    /// A point from whole degrees × 10⁷.
    ///
    /// # Errors
    ///
    /// [`LocationError::LatitudeOutOfRange`] or [`LocationError::LongitudeOutOfRange`].
    pub fn from_e7(lat_e7: i32, lon_e7: i32) -> Result<Self, LocationError> {
        if !(-MAX_LAT_E7..=MAX_LAT_E7).contains(&lat_e7) {
            return Err(LocationError::LatitudeOutOfRange);
        }
        if !(-MAX_LON_E7..=MAX_LON_E7).contains(&lon_e7) {
            return Err(LocationError::LongitudeOutOfRange);
        }
        Ok(Self { lat_e7, lon_e7 })
    }

    /// A point from decimal degrees (e.g. from the browser), rounded to 10⁻⁷ degrees.
    ///
    /// # Errors
    ///
    /// [`LocationError::LatitudeOutOfRange`] or [`LocationError::LongitudeOutOfRange`] (also
    /// for NaN or infinity).
    pub fn from_degrees(latitude: f64, longitude: f64) -> Result<Self, LocationError> {
        let lat = degrees_to_e7(latitude, MAX_LAT_E7).ok_or(LocationError::LatitudeOutOfRange)?;
        let lon = degrees_to_e7(longitude, MAX_LON_E7).ok_or(LocationError::LongitudeOutOfRange)?;
        Self::from_e7(lat, lon)
    }

    /// A point from typed latitude and longitude, each in decimal degrees ("35.5951").
    ///
    /// # Errors
    ///
    /// [`LocationError::NotCoordinates`] if either isn't a number, or the range errors.
    pub fn parse(latitude: &str, longitude: &str) -> Result<Self, LocationError> {
        let number = |text: &str| {
            text.trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .ok_or(LocationError::NotCoordinates)
        };
        Self::from_degrees(number(latitude)?, number(longitude)?)
    }

    /// A point from one pasted pair, "latitude, longitude" (as map apps copy it), with a comma
    /// or spaces between.
    ///
    /// # Errors
    ///
    /// As [`Coordinates::parse`].
    pub fn parse_pair(text: &str) -> Result<Self, LocationError> {
        let parts: Vec<&str> = text
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .collect();
        match parts.as_slice() {
            [latitude, longitude] => Self::parse(latitude, longitude),
            _ => Err(LocationError::NotCoordinates),
        }
    }

    /// Latitude in degrees × 10⁷.
    #[must_use]
    pub const fn lat_e7(self) -> i32 {
        self.lat_e7
    }

    /// Longitude in degrees × 10⁷.
    #[must_use]
    pub const fn lon_e7(self) -> i32 {
        self.lon_e7
    }

    /// Latitude in decimal degrees.
    #[must_use]
    pub fn latitude(self) -> f64 {
        f64::from(self.lat_e7) / E7
    }

    /// Longitude in decimal degrees.
    #[must_use]
    pub fn longitude(self) -> f64 {
        f64::from(self.lon_e7) / E7
    }

    /// The great-circle distance to `other`, in meters (haversine, on a sphere: within about
    /// 0.5 %, plenty for "am I at this place?").
    #[must_use]
    pub fn distance_m(self, other: Self) -> f64 {
        let (lat1, lat2) = (self.latitude().to_radians(), other.latitude().to_radians());
        let half_lat = (lat2 - lat1) / 2.0;
        let half_lon = (other.longitude() - self.longitude()).to_radians() / 2.0;
        let a = half_lat.sin().powi(2) + lat1.cos() * lat2.cos() * half_lon.sin().powi(2);
        2.0 * EARTH_RADIUS_M * a.sqrt().min(1.0).asin()
    }
}

/// Coordinates from a device, with how sure it is and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fix {
    /// Where.
    pub coordinates: Coordinates,
    /// The accuracy radius the device reported, in meters, if any.
    pub accuracy_m: Option<u32>,
    /// When the device took it.
    pub taken_at: UnixMillis,
}

impl Fix {
    /// Whether it's more than 20 minutes old at `now` (worth flagging, plan §3).
    #[must_use]
    pub const fn is_stale(&self, now: UnixMillis) -> bool {
        now.as_millis() - self.taken_at.as_millis() > STALE_FIX_MILLIS
    }
}

/// Where an event's location came from (plan §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LocationSource {
    /// The device's location.
    Gps,
    /// A saved place.
    SavedPlace,
    /// Picked on a map (post-MVP, #039; kept so it needs no migration).
    Map,
    /// Typed in.
    Manual,
}

// --- Places --------------------------------------------------------------------------------

/// A city or a state or province: trimmed, 1–100 characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PlaceName(String);

impl PlaceName {
    /// A name from what was typed or looked up; `empty` is the error for nothing at all.
    fn parse_or(input: &str, empty: LocationError) -> Result<Self, LocationError> {
        let name = input.trim();
        match name.chars().count() {
            0 => Err(empty),
            length if length > PLACE_NAME_MAX_CHARS => Err(LocationError::PlaceNameTooLong),
            _ => Ok(Self(name.to_owned())),
        }
    }

    /// A city or region name.
    ///
    /// # Errors
    ///
    /// [`LocationError::CityEmpty`] for nothing, or [`LocationError::PlaceNameTooLong`].
    pub fn parse(input: &str) -> Result<Self, LocationError> {
        Self::parse_or(input, LocationError::CityEmpty)
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for PlaceName {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<PlaceName> for String {
    fn from(name: PlaceName) -> Self {
        name.0
    }
}

/// An ISO 3166-1 alpha-2 country code, e.g. `US` (stored upper case).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CountryCode([u8; 2]);

impl CountryCode {
    /// A code from two letters, any case.
    ///
    /// # Errors
    ///
    /// [`LocationError::CountryInvalid`] unless it's exactly two ASCII letters.
    pub fn parse(input: &str) -> Result<Self, LocationError> {
        match input.trim().as_bytes() {
            [a, b] if a.is_ascii_alphabetic() && b.is_ascii_alphabetic() => {
                Ok(Self([a.to_ascii_uppercase(), b.to_ascii_uppercase()]))
            }
            _ => Err(LocationError::CountryInvalid),
        }
    }

    /// The code, e.g. "US".
    #[must_use]
    pub fn as_str(&self) -> &str {
        // Always two ASCII letters (checked in `parse`), so always valid UTF-8.
        core::str::from_utf8(&self.0).unwrap_or("??")
    }
}

impl TryFrom<String> for CountryCode {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<CountryCode> for String {
    fn from(code: CountryCode) -> Self {
        code.as_str().to_owned()
    }
}

/// An IANA time zone name, e.g. `America/New_York` (from the geocoder, #059).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TimeZone(String);

impl TimeZone {
    /// A zone name, checked for its shape only (letters, digits, `/`, `_`, `-`, `+`), not
    /// against the zone database.
    ///
    /// # Errors
    ///
    /// [`LocationError::TimeZoneInvalid`].
    pub fn parse(input: &str) -> Result<Self, LocationError> {
        let name = input.trim();
        let allowed = |c: char| c.is_ascii_alphanumeric() || "/_-+".contains(c);
        let valid = (1..=TIME_ZONE_MAX_CHARS).contains(&name.len())
            && name.chars().all(allowed)
            && !name.starts_with('/')
            && !name.ends_with('/');
        valid
            .then(|| Self(name.to_owned()))
            .ok_or(LocationError::TimeZoneInvalid)
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TimeZone {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<TimeZone> for String {
    fn from(zone: TimeZone) -> Self {
        zone.0
    }
}

/// A city, a state or province, and a country: what every event records (plan §3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Place {
    /// The city or town.
    pub city: PlaceName,
    /// The state or province.
    pub region: PlaceName,
    /// The country.
    pub country: CountryCode,
}

impl Place {
    /// A place from what was typed (or a lookup's suggestion, as edited).
    ///
    /// # Errors
    ///
    /// [`LocationError::CityEmpty`], [`LocationError::RegionEmpty`],
    /// [`LocationError::PlaceNameTooLong`], or [`LocationError::CountryInvalid`].
    pub fn parse(city: &str, region: &str, country: &str) -> Result<Self, LocationError> {
        Ok(Self {
            city: PlaceName::parse_or(city, LocationError::CityEmpty)?,
            region: PlaceName::parse_or(region, LocationError::RegionEmpty)?,
            country: CountryCode::parse(country)?,
        })
    }
}

// --- Nearby places (#061) ------------------------------------------------------------------

/// What kind of place a nearby place is, from its GeoNames feature code and population
/// (population only approximates size).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlaceKind {
    /// A national capital (`PPLC`).
    Capital,
    /// A section of a bigger place (`PPLX`), e.g. a neighbourhood.
    District,
    /// A small named locality (`PPLL`).
    Locality,
    /// 100,000 people or more.
    City,
    /// 10,000 people or more.
    Town,
    /// Fewer than 10,000 people.
    Village,
}

/// The administrative seat a place is, if any (GeoNames `PPLA`–`PPLA5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Seat {
    /// Of a state or province (`PPLA`).
    Region,
    /// Of a county or similar (`PPLA2`).
    County,
    /// Of a smaller division (`PPLA3`–`PPLA5`).
    Local,
}

impl PlaceKind {
    /// The kind's name.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Capital => messages::PLACE_KIND_CAPITAL,
            Self::District => messages::PLACE_KIND_DISTRICT,
            Self::Locality => messages::PLACE_KIND_LOCALITY,
            Self::City => messages::PLACE_KIND_CITY,
            Self::Town => messages::PLACE_KIND_TOWN,
            Self::Village => messages::PLACE_KIND_VILLAGE,
        }
    }
}

impl Seat {
    /// The role's name, e.g. "county seat".
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Region => messages::PLACE_SEAT_REGION,
            Self::County => messages::PLACE_SEAT_COUNTY,
            Self::Local => messages::PLACE_SEAT_LOCAL,
        }
    }
}

/// A place's kind and seat from its GeoNames feature code and population.
#[must_use]
pub fn place_kind(feature_code: &str, population: i64) -> (PlaceKind, Option<Seat>) {
    let seat = match feature_code {
        "PPLA" => Some(Seat::Region),
        "PPLA2" => Some(Seat::County),
        "PPLA3" | "PPLA4" | "PPLA5" => Some(Seat::Local),
        _ => None,
    };
    let kind = match feature_code {
        "PPLC" => PlaceKind::Capital,
        "PPLX" => PlaceKind::District,
        "PPLL" => PlaceKind::Locality,
        _ if population >= 100_000 => PlaceKind::City,
        _ if population >= 10_000 => PlaceKind::Town,
        _ => PlaceKind::Village,
    };
    (kind, seat)
}

/// Whether `name` is a numbered part of the place `of`, e.g. "Paris 04 Hôtel-de-Ville" of
/// "Paris" or "Lyon 3e" of "Lyon": `of`'s name, a space, then a digit (#061). GeoNames codes
/// some cities' districts as ordinary places; this finds them by name.
#[must_use]
pub fn is_numbered_part(name: &str, of: &str) -> bool {
    name.strip_prefix(of)
        .and_then(|rest| rest.strip_prefix(' '))
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// An eight-point compass direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Compass {
    /// North.
    N,
    /// Northeast.
    NE,
    /// East.
    E,
    /// Southeast.
    SE,
    /// South.
    S,
    /// Southwest.
    SW,
    /// West.
    W,
    /// Northwest.
    NW,
}

impl Compass {
    /// The direction's short name, e.g. "NE".
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::N => messages::COMPASS_N,
            Self::NE => messages::COMPASS_NE,
            Self::E => messages::COMPASS_E,
            Self::SE => messages::COMPASS_SE,
            Self::S => messages::COMPASS_S,
            Self::SW => messages::COMPASS_SW,
            Self::W => messages::COMPASS_W,
            Self::NW => messages::COMPASS_NW,
        }
    }

    /// The nearest of the eight directions to a bearing in degrees (0 = north, clockwise).
    #[must_use]
    pub fn from_bearing(degrees: f64) -> Self {
        const ALL: [Compass; 8] = [
            Compass::N,
            Compass::NE,
            Compass::E,
            Compass::SE,
            Compass::S,
            Compass::SW,
            Compass::W,
            Compass::NW,
        ];
        // Turned by half a sector, so each direction covers 45° around its own bearing;
        // past 360° it's north again.
        let turned = degrees.rem_euclid(360.0) + 22.5;
        ALL.into_iter()
            .zip(1_u8..)
            .find(|(_, n)| turned < f64::from(*n) * 45.0)
            .map_or(Self::N, |(direction, _)| direction)
    }
}

impl Coordinates {
    /// The initial bearing from here to `other`, in degrees from north, clockwise (0…360).
    #[must_use]
    pub fn bearing_to(self, other: Self) -> f64 {
        let (lat1, lat2) = (self.latitude().to_radians(), other.latitude().to_radians());
        let dlon = (other.longitude() - self.longitude()).to_radians();
        let y = dlon.sin() * lat2.cos();
        let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlon.cos();
        y.atan2(x).to_degrees().rem_euclid(360.0)
    }
}

/// A place near a point, for the picker's choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NearbyPlace {
    /// Its name, e.g. "Paris".
    pub name: PlaceName,
    /// How far, in whole meters.
    pub distance_m: u32,
    /// Which way from the point.
    pub direction: Compass,
    /// What kind of place.
    pub kind: PlaceKind,
    /// The seat it is, if any.
    pub seat: Option<Seat>,
}

impl NearbyPlace {
    /// Whether it's part of a bigger place (never the picker's default choice, #061).
    #[must_use]
    pub const fn is_district(&self) -> bool {
        matches!(self.kind, PlaceKind::District)
    }
}

// --- Saved places --------------------------------------------------------------------------

/// The ID of a saved place.
pub type SavedPlaceId = Id<SavedPlace>;

/// One person's named place, e.g. "Home garage": an alias whose name and location are
/// copied into an event (#059).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPlace {
    /// Whose it is; only they see and change it.
    pub owner: UserId,
    /// Its name, e.g. "Joe's Auto".
    pub name: Label,
    /// Where it is.
    pub coordinates: Coordinates,
    /// How close counts as being there, in meters (25…2,000).
    pub radius_m: u32,
    /// Its city, state or province, and country.
    pub place: Place,
}

impl SavedPlace {
    /// Whether `point` is within its radius.
    #[must_use]
    pub fn contains(&self, point: Coordinates) -> bool {
        self.coordinates.distance_m(point) <= f64::from(self.radius_m)
    }
}

/// Checks a saved place's radius.
///
/// # Errors
///
/// [`LocationError::RadiusOutOfRange`] outside 25 m…2 km.
pub fn check_radius(radius_m: u32) -> Result<u32, LocationError> {
    (MIN_RADIUS_M..=MAX_RADIUS_M)
        .contains(&radius_m)
        .then_some(radius_m)
        .ok_or(LocationError::RadiusOutOfRange)
}

/// The saved place `point` is in, if any: of those whose radius contains it, the one whose
/// center is nearest. Its distance comes back too, in meters.
#[must_use]
pub fn nearest_containing(
    places: &[(SavedPlaceId, SavedPlace)],
    point: Coordinates,
) -> Option<(SavedPlaceId, f64)> {
    places
        .iter()
        .map(|(id, place)| (*id, place, place.coordinates.distance_m(point)))
        .filter(|(_, place, distance)| *distance <= f64::from(place.radius_m))
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(id, _, distance)| (id, distance))
}

/// A saved place as typed: name, coordinates, radius, and its place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPlaceForm {
    /// Its name.
    pub name: String,
    /// Latitude in decimal degrees, as typed.
    pub latitude: String,
    /// Longitude in decimal degrees, as typed.
    pub longitude: String,
    /// Radius in meters.
    pub radius_m: u32,
    /// The city or town.
    pub city: String,
    /// The state or province.
    pub region: String,
    /// The two-letter country code.
    pub country: String,
}

impl SavedPlaceForm {
    /// The form as `owner`'s saved place.
    ///
    /// # Errors
    ///
    /// The first [`LocationError`] found.
    pub fn parse(&self, owner: UserId) -> Result<SavedPlace, LocationError> {
        Ok(SavedPlace {
            owner,
            name: Label::parse(&self.name).map_err(LocationError::Name)?,
            coordinates: Coordinates::parse(&self.latitude, &self.longitude)?,
            radius_m: check_radius(self.radius_m)?,
            place: Place::parse(&self.city, &self.region, &self.country)?,
        })
    }
}

/// The changeable fields of a [`SavedPlace`]; `Some` means "set to this".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPlacePatch {
    /// See [`SavedPlace::owner`] (set when created).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<UserId>,
    /// See [`SavedPlace::name`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Label>,
    /// See [`SavedPlace::coordinates`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Coordinates>,
    /// See [`SavedPlace::radius_m`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius_m: Option<u32>,
    /// See [`SavedPlace::place`] (city, region, and country change together).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<Place>,
}

impl SavedPlacePatch {
    /// Every field of `place`, for creating it.
    #[must_use]
    pub fn all(place: SavedPlace) -> Self {
        Self {
            owner: Some(place.owner),
            name: Some(place.name),
            coordinates: Some(place.coordinates),
            radius_m: Some(place.radius_m),
            place: Some(place.place),
        }
    }
}

impl Patch for SavedPlacePatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        [
            ("owner", self.owner.is_some()),
            ("name", self.name.is_some()),
            ("coordinates", self.coordinates.is_some()),
            ("radius_m", self.radius_m.is_some()),
            ("place", self.place.is_some()),
        ]
        .into_iter()
        .filter_map(|(field, set)| set.then_some(field))
        .collect()
    }

    fn keep_only(&mut self, fields: &[&str]) {
        let keep = |field: &str| fields.contains(&field);
        if !keep("owner") {
            self.owner = None;
        }
        if !keep("name") {
            self.name = None;
        }
        if !keep("coordinates") {
            self.coordinates = None;
        }
        if !keep("radius_m") {
            self.radius_m = None;
        }
        if !keep("place") {
            self.place = None;
        }
    }
}

impl Record for SavedPlace {
    const ENTITY: &'static str = "saved_place";
    type Patch = SavedPlacePatch;
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::test_support::test_id;

    fn point(lat: f64, lon: f64) -> Coordinates {
        Coordinates::from_degrees(lat, lon).unwrap()
    }

    fn saved(n: u32, lat: f64, lon: f64, radius_m: u32) -> (SavedPlaceId, SavedPlace) {
        (
            test_id(n),
            SavedPlace {
                owner: test_id(1),
                name: Label::parse(&format!("Place {n}")).unwrap(),
                coordinates: point(lat, lon),
                radius_m,
                place: Place::parse("Asheville", "North Carolina", "us").unwrap(),
            },
        )
    }

    #[test]
    fn coordinates_are_range_checked() {
        assert!(Coordinates::from_degrees(90.0, 180.0).is_ok());
        assert!(Coordinates::from_degrees(-90.0, -180.0).is_ok());
        assert_eq!(
            Coordinates::from_degrees(90.000_001, 0.0),
            Err(LocationError::LatitudeOutOfRange)
        );
        assert_eq!(
            Coordinates::from_degrees(0.0, -180.1),
            Err(LocationError::LongitudeOutOfRange)
        );
        assert_eq!(
            Coordinates::from_degrees(f64::NAN, 0.0),
            Err(LocationError::LatitudeOutOfRange)
        );
        assert_eq!(
            Coordinates::from_e7(0, 1_800_000_001),
            Err(LocationError::LongitudeOutOfRange)
        );
    }

    #[test]
    fn typed_and_pasted_coordinates() {
        let expected = Coordinates::from_e7(355_951_000, -825_515_000).unwrap();
        assert_eq!(Coordinates::parse(" 35.5951", "-82.5515 "), Ok(expected));
        for pasted in ["35.5951, -82.5515", "35.5951,-82.5515", "35.5951 -82.5515"] {
            assert_eq!(Coordinates::parse_pair(pasted), Ok(expected), "{pasted}");
        }
        for bad in ["", "35.5951", "north, west", "1, 2, 3", "inf, 0"] {
            assert!(Coordinates::parse_pair(bad).is_err(), "{bad}");
        }
        assert_eq!(
            Coordinates::parse_pair("95, 0"),
            Err(LocationError::LatitudeOutOfRange)
        );
    }

    #[test]
    fn coordinates_are_stored_as_e7_and_checked_when_read() {
        let json = serde_json::to_string(&point(35.5951, -82.5515)).unwrap();
        assert_eq!(json, r#"{"lat_e7":355951000,"lon_e7":-825515000}"#);
        let bad = r#"{"lat_e7":900000001,"lon_e7":0}"#;
        assert!(serde_json::from_str::<Coordinates>(bad).is_err());
    }

    #[test]
    fn distances_match_known_ones() {
        // Asheville to Boone, NC: about 105 km; London to Paris: about 344 km.
        let km = |a: Coordinates, b: Coordinates| a.distance_m(b) / 1000.0;
        let asheville_boone = km(point(35.5951, -82.5515), point(36.2168, -81.6746));
        assert!((asheville_boone - 105.0).abs() < 3.0, "{asheville_boone}");
        let london_paris = km(point(51.5074, -0.1278), point(48.8566, 2.3522));
        assert!((london_paris - 344.0).abs() < 3.0, "{london_paris}");
        // Across the antimeridian: two points 0.2° apart, not 359.8°.
        let across = km(point(0.0, 179.9), point(0.0, -179.9));
        assert!(across < 23.0, "{across}");
    }

    #[test]
    fn a_fix_goes_stale_after_twenty_minutes() {
        let fix = Fix {
            coordinates: point(35.0, -82.0),
            accuracy_m: Some(12),
            taken_at: UnixMillis::from_millis(1_000_000),
        };
        assert!(!fix.is_stale(UnixMillis::from_millis(1_000_000 + STALE_FIX_MILLIS)));
        assert!(fix.is_stale(UnixMillis::from_millis(1_000_001 + STALE_FIX_MILLIS)));
    }

    #[test]
    fn places_need_a_city_region_and_country() {
        let place = Place::parse(" Asheville ", "North Carolina", "us").unwrap();
        assert_eq!(place.city.as_str(), "Asheville");
        assert_eq!(place.country.as_str(), "US");
        assert_eq!(Place::parse("", "NC", "US"), Err(LocationError::CityEmpty));
        assert_eq!(
            Place::parse("A", " ", "US"),
            Err(LocationError::RegionEmpty)
        );
        assert_eq!(
            Place::parse("A", "B", "USA"),
            Err(LocationError::CountryInvalid)
        );
        assert_eq!(
            Place::parse(&"x".repeat(101), "B", "US"),
            Err(LocationError::PlaceNameTooLong)
        );
        let json = serde_json::to_string(&place.country).unwrap();
        assert_eq!(json, r#""US""#);
    }

    #[test]
    fn time_zones_are_iana_shaped() {
        for good in [
            "America/New_York",
            "UTC",
            "Etc/GMT+5",
            "America/Argentina/Buenos_Aires",
        ] {
            assert!(TimeZone::parse(good).is_ok(), "{good}");
        }
        for bad in ["", "America/New York", "/UTC", "Europe/", "Zürich"] {
            assert_eq!(
                TimeZone::parse(bad),
                Err(LocationError::TimeZoneInvalid),
                "{bad}"
            );
        }
    }

    #[test]
    fn the_nearest_containing_place_wins() {
        let places = [
            saved(1, 35.0, -82.0, 2_000), // big, center 1.1 km away
            saved(2, 35.01, -82.0, 150),  // the point is its center
            saved(3, 35.02, -82.0, 25),   // too small to reach
        ];
        let here = point(35.01, -82.0);
        assert_eq!(
            nearest_containing(&places, here).map(|(id, _)| id),
            Some(test_id(2))
        );
        let between = point(35.005, -82.0);
        assert_eq!(
            nearest_containing(&places, between).map(|(id, _)| id),
            Some(test_id(1))
        );
        assert_eq!(nearest_containing(&places, point(36.0, -82.0)), None);
    }

    #[test]
    fn kinds_and_seats_come_from_the_feature_code_and_population() {
        assert_eq!(place_kind("PPLC", 2_000_000), (PlaceKind::Capital, None));
        assert_eq!(place_kind("PPLX", 116_000), (PlaceKind::District, None));
        assert_eq!(place_kind("PPLL", 50), (PlaceKind::Locality, None));
        assert_eq!(place_kind("PPL", 100_000), (PlaceKind::City, None));
        assert_eq!(place_kind("PPL", 99_999), (PlaceKind::Town, None));
        assert_eq!(
            place_kind("PPLA2", 9_999),
            (PlaceKind::Village, Some(Seat::County))
        );
        assert_eq!(
            place_kind("PPLA", 500_000),
            (PlaceKind::City, Some(Seat::Region))
        );
        assert_eq!(
            place_kind("PPLA4", 0),
            (PlaceKind::Village, Some(Seat::Local))
        );
    }

    #[test]
    fn numbered_parts_of_a_place_are_found_by_name() {
        assert!(is_numbered_part("Paris 04 Hôtel-de-Ville", "Paris"));
        assert!(is_numbered_part("Lyon 3e Arrondissement", "Lyon"));
        assert!(is_numbered_part("Marseille 13", "Marseille"));
        assert!(!is_numbered_part("Paris", "Paris"), "itself");
        assert!(!is_numbered_part("Parisville 2", "Paris"), "a longer name");
        assert!(!is_numbered_part("Paris Hilton", "Paris"), "no number");
        assert!(!is_numbered_part("Springfield", "Paris"));
    }

    #[test]
    fn bearings_and_compass_points() {
        let here = point(35.0, -82.0);
        let compass = |lat: f64, lon: f64| Compass::from_bearing(here.bearing_to(point(lat, lon)));
        assert_eq!(compass(35.1, -82.0), Compass::N);
        assert_eq!(compass(35.0, -81.9), Compass::E);
        assert_eq!(compass(34.9, -82.0), Compass::S);
        assert_eq!(compass(35.0, -82.1), Compass::W);
        assert_eq!(compass(35.07, -81.93), Compass::NE);
        assert_eq!(compass(34.93, -82.07), Compass::SW);
        assert_eq!(Compass::from_bearing(359.0), Compass::N);
        assert_eq!(Compass::from_bearing(-10.0), Compass::N);
        assert_eq!(Compass::from_bearing(337.6), Compass::N);
        assert_eq!(Compass::from_bearing(337.4), Compass::NW);
    }

    #[test]
    fn a_saved_place_form_becomes_a_saved_place() {
        let form = SavedPlaceForm {
            name: "Home garage".into(),
            latitude: "35.5951".into(),
            longitude: "-82.5515".into(),
            radius_m: DEFAULT_RADIUS_M,
            city: "Asheville".into(),
            region: "North Carolina".into(),
            country: "US".into(),
        };
        let place = form.parse(test_id(7)).unwrap();
        assert_eq!(place.owner, test_id(7));
        assert_eq!(place.radius_m, 150);
        let too_big = SavedPlaceForm {
            radius_m: 2_001,
            ..form.clone()
        };
        assert_eq!(
            too_big.parse(test_id(7)),
            Err(LocationError::RadiusOutOfRange)
        );
        let unnamed = SavedPlaceForm {
            name: " ".into(),
            ..form
        };
        assert!(matches!(
            unnamed.parse(test_id(7)),
            Err(LocationError::Name(_))
        ));
    }

    #[test]
    fn a_patch_lists_and_keeps_its_fields() {
        let (_, place) = saved(1, 35.0, -82.0, 150);
        let mut patch = SavedPlacePatch::all(place);
        assert_eq!(
            patch.changed_fields(),
            vec!["owner", "name", "coordinates", "radius_m", "place"]
        );
        patch.keep_only(&["name", "place"]);
        assert_eq!(patch.changed_fields(), vec!["name", "place"]);
    }

    proptest! {
        #[test]
        fn degrees_round_trip_through_e7(lat in -90.0f64..=90.0, lon in -180.0f64..=180.0) {
            let point = Coordinates::from_degrees(lat, lon).unwrap();
            prop_assert!((point.latitude() - lat).abs() <= 0.5e-7 + f64::EPSILON * 90.0);
            prop_assert!((point.longitude() - lon).abs() <= 0.5e-7 + f64::EPSILON * 180.0);
            prop_assert_eq!(Coordinates::from_e7(point.lat_e7(), point.lon_e7()), Ok(point));
        }

        #[test]
        fn distance_is_symmetric_and_never_negative(
            a in (-90.0f64..=90.0, -180.0f64..=180.0),
            b in (-90.0f64..=90.0, -180.0f64..=180.0),
        ) {
            let (a, b) = (point(a.0, a.1), point(b.0, b.1));
            let there = a.distance_m(b);
            prop_assert!(there >= 0.0);
            prop_assert!((there - b.distance_m(a)).abs() < 1e-6);
            prop_assert!(a.distance_m(a) < 1e-6);
            // Never more than half the way round.
            prop_assert!(there <= core::f64::consts::PI * EARTH_RADIUS_M + 1.0);
        }

        #[test]
        fn the_match_is_the_nearest_place_that_contains_the_point(
            centers in proptest::collection::vec((35.0f64..35.05, -82.05f64..-82.0, MIN_RADIUS_M..=MAX_RADIUS_M), 1..8),
            here in (35.0f64..35.05, -82.05f64..-82.0),
        ) {
            let places: Vec<_> = centers
                .iter()
                .zip(1u32..)
                .map(|(&(lat, lon, radius), n)| saved(n, lat, lon, radius))
                .collect();
            let here = point(here.0, here.1);
            match nearest_containing(&places, here) {
                Some((id, distance)) => {
                    let chosen = &places.iter().find(|(place_id, _)| *place_id == id).unwrap().1;
                    prop_assert!(chosen.contains(here));
                    for (_, other) in places.iter().filter(|(_, other)| other.contains(here)) {
                        prop_assert!(distance <= other.coordinates.distance_m(here));
                    }
                }
                None => prop_assert!(places.iter().all(|(_, place)| !place.contains(here))),
            }
        }
    }
}
