//! Geometry: reading GeoPackage geometry, point-in-polygon, and distance to a boundary.
//! Pure functions; no I/O.
//!
//! A GeoPackage stores each geometry as a small header (magic `GP`, version, flags, the
//! spatial reference ID, an optional envelope) followed by standard **WKB** (OGC Simple
//! Features, binary). Only what the geocoder needs is read: 2D points, polygons, and
//! multipolygons, in either byte order. Coordinates are longitude (x) and latitude (y), in
//! degrees (EPSG:4326).

/// A point: longitude (x) and latitude (y), in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Longitude, degrees.
    pub x: f64,
    /// Latitude, degrees.
    pub y: f64,
}

/// A polygon: an outer ring and any holes, each a closed list of points.
#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    /// The outer boundary.
    pub exterior: Vec<Point>,
    /// Holes (e.g. a lake, or an enclave belonging to another state).
    pub holes: Vec<Vec<Point>>,
}

/// What a geometry column holds, as far as the geocoder cares.
#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    /// A city.
    Point(Point),
    /// A country or region: one or more polygons.
    Polygons(Vec<Polygon>),
}

/// Why a geometry couldn't be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryError {
    /// Not a GeoPackage geometry (no `GP` header).
    NotGeoPackage,
    /// The bytes end too soon.
    Truncated,
    /// A geometry type the geocoder doesn't read (e.g. a line, or 3D).
    Unsupported(u32),
    /// The geometry is flagged empty.
    Empty,
}

/// WKB geometry type codes (2D).
const WKB_POINT: u32 = 1;
const WKB_POLYGON: u32 = 3;
const WKB_MULTIPOLYGON: u32 = 6;

/// Meters per degree of latitude (and of longitude at the equator), for local distances.
const METERS_PER_DEGREE: f64 = 111_320.0;

/// Reads little- or big-endian numbers from a byte slice.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    little: bool,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8], at: usize) -> Self {
        Self {
            bytes,
            at,
            little: true,
        }
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], GeometryError> {
        let end = self.at.checked_add(N).ok_or(GeometryError::Truncated)?;
        let slice = self
            .bytes
            .get(self.at..end)
            .ok_or(GeometryError::Truncated)?;
        self.at = end;
        let mut out = [0; N];
        out.copy_from_slice(slice);
        Ok(out)
    }

    fn byte(&mut self) -> Result<u8, GeometryError> {
        Ok(self.take::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32, GeometryError> {
        let raw = self.take::<4>()?;
        Ok(if self.little {
            u32::from_le_bytes(raw)
        } else {
            u32::from_be_bytes(raw)
        })
    }

    fn f64(&mut self) -> Result<f64, GeometryError> {
        let raw = self.take::<8>()?;
        Ok(if self.little {
            f64::from_le_bytes(raw)
        } else {
            f64::from_be_bytes(raw)
        })
    }

    /// A count, refusing ones the remaining bytes can't possibly hold (each item is at least
    /// `min_item_bytes`), so a corrupt count can't make us allocate gigabytes.
    fn count(&mut self, min_item_bytes: usize) -> Result<usize, GeometryError> {
        let count = usize::try_from(self.u32()?).map_err(|_| GeometryError::Truncated)?;
        let left = self.bytes.len().saturating_sub(self.at);
        if count.saturating_mul(min_item_bytes) > left {
            return Err(GeometryError::Truncated);
        }
        Ok(count)
    }

    fn point(&mut self) -> Result<Point, GeometryError> {
        Ok(Point {
            x: self.f64()?,
            y: self.f64()?,
        })
    }

    fn ring(&mut self) -> Result<Vec<Point>, GeometryError> {
        let count = self.count(16)?;
        (0..count).map(|_| self.point()).collect()
    }

    fn polygon(&mut self) -> Result<Polygon, GeometryError> {
        let rings = self.count(4)?;
        let mut all = (0..rings)
            .map(|_| self.ring())
            .collect::<Result<Vec<_>, _>>()?
            .into_iter();
        Ok(Polygon {
            exterior: all.next().unwrap_or_default(),
            holes: all.collect(),
        })
    }

    /// One WKB geometry: its byte order, type, then its body.
    fn geometry(&mut self) -> Result<Geometry, GeometryError> {
        self.little = self.byte()? == 1;
        match self.u32()? {
            WKB_POINT => Ok(Geometry::Point(self.point()?)),
            WKB_POLYGON => Ok(Geometry::Polygons(vec![self.polygon()?])),
            WKB_MULTIPOLYGON => {
                let count = self.count(9)?;
                let mut polygons = Vec::with_capacity(count);
                for _ in 0..count {
                    match self.geometry()? {
                        Geometry::Polygons(mut inner) => polygons.append(&mut inner),
                        Geometry::Point(_) => return Err(GeometryError::Unsupported(WKB_POINT)),
                    }
                }
                Ok(Geometry::Polygons(polygons))
            }
            other => Err(GeometryError::Unsupported(other)),
        }
    }
}

/// Reads a GeoPackage geometry blob.
///
/// # Errors
///
/// [`GeometryError`] if it isn't a 2D point, polygon, or multipolygon in GeoPackage form.
pub fn from_gpkg(blob: &[u8]) -> Result<Geometry, GeometryError> {
    // Header: "GP", version, flags, srs_id (4 bytes), then the envelope the flags describe.
    let [b'G', b'P', _version, flags, ..] = blob else {
        return Err(GeometryError::NotGeoPackage);
    };
    if flags & 0b1_0000 != 0 {
        return Err(GeometryError::Empty);
    }
    let envelope_bytes = match (flags >> 1) & 0b111 {
        0 => 0,
        1 => 32,
        2 | 3 => 48,
        4 => 64,
        other => return Err(GeometryError::Unsupported(u32::from(other))),
    };
    Reader::new(blob, 8 + envelope_bytes).geometry()
}

/// Whether `point` is inside `ring` (even-odd rule; the ring needn't repeat its first point).
fn in_ring(ring: &[Point], point: Point) -> bool {
    let mut inside = false;
    let mut previous = match ring.last() {
        Some(last) => *last,
        None => return false,
    };
    for &current in ring {
        let crosses = (current.y > point.y) != (previous.y > point.y);
        if crosses {
            let x_at = (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                + current.x;
            if point.x < x_at {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

/// Whether `point` is inside any of `polygons` (inside an outer ring and not in its holes).
#[must_use]
pub fn contains(polygons: &[Polygon], point: Point) -> bool {
    polygons.iter().any(|polygon| {
        in_ring(&polygon.exterior, point) && !polygon.holes.iter().any(|hole| in_ring(hole, point))
    })
}

/// The longitude difference `to - from`, wrapped into −180…180 (so the antimeridian is near).
fn wrapped_dx(from: f64, to: f64) -> f64 {
    let dx = to - from;
    if dx > 180.0 {
        dx - 360.0
    } else if dx < -180.0 {
        dx + 360.0
    } else {
        dx
    }
}

/// The distance from the origin to the segment `start`–`end`, in the plane.
fn origin_to_segment(start: (f64, f64), end: (f64, f64)) -> f64 {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let length_squared = dx * dx + dy * dy;
    let along = if length_squared > 0.0 {
        (-(start.0 * dx + start.1 * dy) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (start.0 + along * dx).hypot(start.1 + along * dy)
}

/// The distance from `point` to the nearest edge of `polygons`, in meters.
///
/// Measured in a flat projection centred on the point (longitude scaled by the cosine of its
/// latitude): accurate to well under 1 % within a few hundred kilometers, which is all a
/// "nearest boundary" fallback needs (#038).
#[must_use]
pub fn distance_to_boundary_m(polygons: &[Polygon], point: Point) -> f64 {
    let scale_x = point.y.to_radians().cos().max(0.01) * METERS_PER_DEGREE;
    let project = |p: Point| {
        (
            wrapped_dx(point.x, p.x) * scale_x,
            (p.y - point.y) * METERS_PER_DEGREE,
        )
    };
    let mut best = f64::INFINITY;
    for polygon in polygons {
        for ring in core::iter::once(&polygon.exterior).chain(&polygon.holes) {
            let mut previous = match ring.last() {
                Some(last) => project(*last),
                None => continue,
            };
            for &vertex in ring {
                let current = project(vertex);
                best = best.min(origin_to_segment(previous, current));
                previous = current;
            }
        }
    }
    best
}

/// Writes a geometry as a GeoPackage blob (little-endian, no envelope, SRS 4326). Used to
/// build test data.
#[cfg(test)]
pub(crate) fn to_gpkg(geometry: &Geometry) -> Vec<u8> {
    fn point(out: &mut Vec<u8>, p: Point) {
        out.extend(p.x.to_le_bytes());
        out.extend(p.y.to_le_bytes());
    }
    fn len(out: &mut Vec<u8>, n: usize) {
        out.extend(u32::try_from(n).unwrap().to_le_bytes());
    }
    fn polygon(out: &mut Vec<u8>, polygon: &Polygon) {
        out.push(1);
        out.extend(WKB_POLYGON.to_le_bytes());
        len(out, 1 + polygon.holes.len());
        for ring in core::iter::once(&polygon.exterior).chain(&polygon.holes) {
            len(out, ring.len());
            for &p in ring {
                point(out, p);
            }
        }
    }
    let mut out = vec![b'G', b'P', 0, 0b0000_0001];
    out.extend(4326_i32.to_le_bytes());
    match geometry {
        Geometry::Point(p) => {
            out.push(1);
            out.extend(WKB_POINT.to_le_bytes());
            point(&mut out, *p);
        }
        Geometry::Polygons(polygons) => {
            out.push(1);
            out.extend(WKB_MULTIPOLYGON.to_le_bytes());
            len(&mut out, polygons.len());
            for each in polygons {
                polygon(&mut out, each);
            }
        }
    }
    out
}

/// A closed rectangle as a polygon, for tests.
#[cfg(test)]
pub(crate) fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon {
    Polygon {
        exterior: vec![
            Point { x: x0, y: y0 },
            Point { x: x1, y: y0 },
            Point { x: x1, y: y1 },
            Point { x: x0, y: y1 },
            Point { x: x0, y: y0 },
        ],
        holes: vec![],
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn at(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[test]
    fn geometry_round_trips_through_a_geopackage_blob() {
        let mut lake = rectangle(0.0, 0.0, 10.0, 10.0);
        lake.holes.push(rectangle(4.0, 4.0, 6.0, 6.0).exterior);
        let shapes = Geometry::Polygons(vec![lake, rectangle(20.0, 0.0, 21.0, 1.0)]);
        assert_eq!(from_gpkg(&to_gpkg(&shapes)), Ok(shapes));
        let city = Geometry::Point(at(-82.55, 35.59));
        assert_eq!(from_gpkg(&to_gpkg(&city)), Ok(city));
    }

    #[test]
    fn big_endian_and_an_envelope_are_read_too() {
        // A big-endian WKB point, after a header with an xy envelope (flags bits 1–3 = 1).
        let mut blob = vec![b'G', b'P', 0, 0b0000_0011];
        blob.extend(4326_i32.to_le_bytes());
        for value in [1.0_f64, 2.0, 3.0, 4.0] {
            blob.extend(value.to_le_bytes());
        }
        blob.push(0);
        blob.extend(WKB_POINT.to_be_bytes());
        blob.extend((-82.5_f64).to_be_bytes());
        blob.extend(35.5_f64.to_be_bytes());
        assert_eq!(from_gpkg(&blob), Ok(Geometry::Point(at(-82.5, 35.5))));
    }

    #[test]
    fn bad_blobs_are_refused_without_panicking() {
        assert_eq!(from_gpkg(b"nope"), Err(GeometryError::NotGeoPackage));
        let mut empty = vec![b'G', b'P', 0, 0b0001_0001];
        empty.extend([0; 4]);
        assert_eq!(from_gpkg(&empty), Err(GeometryError::Empty));
        let full = to_gpkg(&Geometry::Polygons(vec![rectangle(0.0, 0.0, 1.0, 1.0)]));
        for cut in 0..full.len() {
            assert!(from_gpkg(&full[..cut]).is_err(), "cut at {cut}");
        }
        // A line string (type 2) isn't read.
        let mut line = vec![b'G', b'P', 0, 1, 0, 0, 0, 0, 1];
        line.extend(2_u32.to_le_bytes());
        assert_eq!(from_gpkg(&line), Err(GeometryError::Unsupported(2)));
    }

    #[test]
    fn holes_are_outside() {
        let mut ring = rectangle(0.0, 0.0, 10.0, 10.0);
        ring.holes.push(rectangle(4.0, 4.0, 6.0, 6.0).exterior);
        let shapes = [ring];
        assert!(contains(&shapes, at(1.0, 1.0)));
        assert!(!contains(&shapes, at(5.0, 5.0)), "in the hole");
        assert!(!contains(&shapes, at(11.0, 5.0)));
    }

    #[test]
    fn distance_to_a_boundary_in_meters() {
        let square = [rectangle(0.0, 0.0, 1.0, 1.0)];
        // 0.01° west of the west edge at the equator: about 1.1 km.
        let west = distance_to_boundary_m(&square, at(-0.01, 0.5));
        assert!((west - 1_113.2).abs() < 1.0, "{west}");
        // Inside, 0.1° from the nearest (south) edge.
        let inside = distance_to_boundary_m(&square, at(0.5, 0.1));
        assert!((inside - 11_132.0).abs() < 5.0, "{inside}");
        // Across the antimeridian: a polygon touching 180° is near a point at −179.99°.
        let east = [rectangle(179.0, 0.0, 180.0, 1.0)];
        assert!(distance_to_boundary_m(&east, at(-179.99, 0.5)) < 1_200.0);
    }

    proptest! {
        #[test]
        fn a_rectangle_contains_exactly_the_points_within_it(
            x in -20.0f64..20.0, y in -20.0f64..20.0,
        ) {
            let shapes = [rectangle(-10.0, -10.0, 10.0, 10.0)];
            // Away from the edges, where the even-odd rule's tie-breaking doesn't matter.
            prop_assume!((x.abs() - 10.0).abs() > 1e-9 && (y.abs() - 10.0).abs() > 1e-9);
            let within = x.abs() < 10.0 && y.abs() < 10.0;
            prop_assert_eq!(contains(&shapes, at(x, y)), within);
        }

        #[test]
        fn distance_to_a_boundary_is_never_negative(x in -20.0f64..20.0, y in -60.0f64..60.0) {
            let shapes = [rectangle(-10.0, -10.0, 10.0, 10.0)];
            prop_assert!(distance_to_boundary_m(&shapes, at(x, y)) >= 0.0);
        }

        #[test]
        fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
            let mut blob = vec![b'G', b'P', 0, 1, 0, 0, 0, 0];
            blob.extend(bytes);
            let _ = from_gpkg(&blob);
        }
    }
}
