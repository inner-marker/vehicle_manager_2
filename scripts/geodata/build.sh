#!/bin/sh
# Builds the geocoder's GeoPackage (decision #059) from the three frozen source zips.
# Runs inside the pinned GDAL image (the Dockerfile's geodata stage, or fetch-geodata.sh).
#
#   build.sh <folder with the zips and sources.sha256> <output .gpkg> <data version>
#
# The layout it writes is the one crates/geocoder/src/lib.rs documents.
set -eu

src="$1"
out="$2"
version="$3"
# Boundary simplification, in degrees (about 110 m at the equator).
tolerance="${GEODATA_SIMPLIFY:-0.001}"

cd "$src"
# Refuse anything but the frozen sources.
sha256sum -c sources.sha256

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
rm -f "$out"

# Countries: Natural Earth's corrected code (ISO_A2_EH fixes France, Norway, Kosovo); the
# few areas with no code at all are left out, so points there get the nearest country.
ogr2ogr -f GPKG "$out" "/vsizip/$src/ne_10m_admin_0_countries.zip/ne_10m_admin_0_countries.shp" \
    -nln countries -nlt PROMOTE_TO_MULTI -lco GEOMETRY_NAME=geom -lco FID=fid \
    -simplify "$tolerance" -dialect SQLite \
    -sql "SELECT ISO_A2_EH AS country_code, NAME AS name, geometry FROM ne_10m_admin_0_countries WHERE ISO_A2_EH NOT IN ('-99', '')"

# States and provinces; the few with no name take the English name, then the country's.
ogr2ogr -f GPKG -update "$out" "/vsizip/$src/ne_10m_admin_1_states_provinces.zip/ne_10m_admin_1_states_provinces.shp" \
    -nln regions -nlt PROMOTE_TO_MULTI -lco GEOMETRY_NAME=geom -lco FID=fid \
    -simplify "$tolerance" -dialect SQLite \
    -sql "SELECT iso_a2 AS country_code, COALESCE(NULLIF(name, ''), NULLIF(name_en, ''), admin) AS name, geometry FROM ne_10m_admin_1_states_provinces WHERE iso_a2 NOT IN ('-99', '')"

# Cities: GeoNames is tab-separated with no header and unquoted names, so it's rewritten as
# a CSV with quoted names (name, country code, time zone, latitude, longitude).
unzip -q -o cities500.zip cities500.txt -d "$work"
awk -F '\t' 'BEGIN { OFS = ","; print "name,country_code,time_zone,lat,lon" }
    { name = $2; gsub(/"/, "\"\"", name); print "\"" name "\"", $9, $18, $5, $6 }' \
    "$work/cities500.txt" > "$work/cities.csv"
ogr2ogr -f GPKG -update "$out" "$work/cities.csv" \
    -nln cities -nlt POINT -a_srs EPSG:4326 -lco GEOMETRY_NAME=geom -lco FID=fid \
    -oo X_POSSIBLE_NAMES=lon -oo Y_POSSIBLE_NAMES=lat -oo KEEP_GEOM_COLUMNS=NO -oo AUTODETECT_TYPE=NO

# What the geocoder checks, and what About shows.
attribution='Contains GeoNames data (CC BY 4.0, geonames.org). Boundaries: Natural Earth (public domain, naturalearthdata.com).'
{
    echo 'key,value'
    echo 'format,vehicle-manager-geodata'
    echo 'format_version,1'
    echo "data_version,$version"
    echo "\"attribution\",\"$attribution\""
    echo "simplify_degrees,$tolerance"
    while read -r sum file; do echo "\"source:$file\",\"sha256:$sum\""; done < sources.sha256
} > "$work/info.csv"
ogr2ogr -f GPKG -update "$out" "$work/info.csv" -nln geodata_info -oo AUTODETECT_TYPE=NO

ogrinfo -so -q "$out"
