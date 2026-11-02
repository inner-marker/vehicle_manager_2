#!/bin/sh
# Builds the geocoder's data for development (decision #059), the same way the image does:
# the three frozen source files from this repository's GitHub release, checked against
# sources.sha256, then the pinned GDAL image runs build.sh. Needs Docker (e.g. Colima).
#
#   scripts/geodata/fetch.sh
#
# Writes .geodata/out/geodata-<version>.gpkg (git-ignored). The server finds it there in
# development. Keep these values the same as the Dockerfile's geodata stage
# (crates/app/tests/deploy_config.rs checks).
set -eu

GEODATA_VERSION=2026-11-02
GEODATA_SOURCES=https://github.com/inner-marker/vehicle_manager_2/releases/download/geodata-sources-2026-11-02
GDAL_IMAGE=ghcr.io/osgeo/gdal:ubuntu-small-3.13.3@sha256:64250faf833c06d4b21afce4c27190039ba7ab58d70f0eebc87cf77d929c0b40

root="$(cd "$(dirname "$0")/../.." && pwd)"
sources="$root/.geodata/sources"
out="$root/.geodata/out"
mkdir -p "$sources" "$out"

# Download only what's missing, then check every file (a bad download is deleted).
while read -r _ file; do
    if [ ! -f "$sources/$file" ]; then
        curl -fsSL -o "$sources/$file" "$GEODATA_SOURCES/$file"
    fi
done < "$root/scripts/geodata/sources.sha256"
cp "$root/scripts/geodata/sources.sha256" "$sources/"
if ! (cd "$sources" && shasum -a 256 -c sources.sha256); then
    echo "A source file doesn't match its checksum; deleting the downloads." >&2
    rm -f "$sources"/*.zip
    exit 1
fi

docker run --rm \
    -v "$sources:/src:ro" \
    -v "$root/scripts/geodata:/scripts:ro" \
    -v "$out:/out" \
    "$GDAL_IMAGE" sh /scripts/build.sh /src "/out/geodata-$GEODATA_VERSION.gpkg" "$GEODATA_VERSION"
echo "Built $out/geodata-$GEODATA_VERSION.gpkg"
