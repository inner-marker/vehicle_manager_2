# syntax=docker/dockerfile:1
#
# The server image: the Dioxus fullstack server plus its web UI (plan §7 Building the
# app image). linux/amd64 for the Synology NAS (decisions.md #025).
#
#   docker build -t vehicle_manager_2 .
#
# Three stages: the geocoder's data (GDAL, decision #059), a build stage with the Rust
# toolchain and the Dioxus CLI, and a slim runtime stage that ships only the bundle, the
# geocoder data, and CA certificates (no compiler, no GDAL, no source).
#
# Never add `-C target-cpu=native` or similar: the NAS CPU has no AVX,
# so a binary tuned for the build machine could crash there (#025).

# ---------- Geocoder data stage (decision #059) ----------
# The official GDAL image, pinned by digest, turns the three frozen source files into one
# GeoPackage. The sources come from this repository's GitHub release, each checked against
# its SHA-256 before use (scripts/geodata/sources.sha256 lists the same checksums;
# crates/app/tests/deploy_config.rs checks they agree). Only the .gpkg leaves this stage.
FROM ghcr.io/osgeo/gdal:ubuntu-small-3.13.3@sha256:64250faf833c06d4b21afce4c27190039ba7ab58d70f0eebc87cf77d929c0b40 AS geodata
ARG GEODATA_VERSION=2026-11-02.2
ARG GEODATA_SOURCES=https://github.com/inner-marker/vehicle_manager_2/releases/download/geodata-sources-2026-11-02
ADD --checksum=sha256:74157e02b1a45d3bdc9ff76149bcef1e1521db5f59054ad5b6df85e3f97d4b69 ${GEODATA_SOURCES}/cities500.zip /src/
ADD --checksum=sha256:ce1ac7036499a0edd641fbc093cd209a98f96a49d2eca8480aaacad35138a7f6 ${GEODATA_SOURCES}/ne_10m_admin_0_countries.zip /src/
ADD --checksum=sha256:efc59726337323058f9446210adc96673179cd344e053666ee3d28cb58ba2b05 ${GEODATA_SOURCES}/ne_10m_admin_1_states_provinces.zip /src/
COPY scripts/geodata/ /scripts/
RUN cp /scripts/sources.sha256 /src/ \
    && mkdir /out \
    && sh /scripts/build.sh /src "/out/geodata-${GEODATA_VERSION}.gpkg" "${GEODATA_VERSION}"

# ---------- Build stage ----------
FROM rust:1.95.0-trixie AS build

# The Dioxus CLI, pinned to the same version as the `dioxus` crate (Cargo.toml). Its
# own layer, so it's rebuilt only when this line changes.
RUN cargo install --locked dioxus-cli@0.7.10

WORKDIR /src
COPY . .

# `dx bundle` builds the server binary and the WASM client, and lays them out as
# /out/server + /out/public/ (verified with dx 0.7.10). rust-toolchain.toml installs the
# pinned toolchain and the wasm32 target on first use. The cache mounts keep crate
# downloads and build output between builds.
#
# TEST_TIMERS=true builds in test timers (decision #054): only the Dev image workflow sets it
# (dev and milestone images); a release never does, so a release can't shorten its timers.
#
# BUILD_COMMIT, BUILD_TAG, and BUILD_DATE name this exact build on Diagnostics (phase-3.md
# 1c, GitHub #37); the image workflows set them, and the app reads them at compile time as
# VEHICLEMANAGER_BUILD_* (domain::build_info; crates/app/tests/deploy_config.rs checks the
# names). Declared here, just before the build, so they never invalidate earlier layers.
ARG TEST_TIMERS=false
ARG BUILD_COMMIT=""
ARG BUILD_TAG=""
ARG BUILD_DATE=""
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cd crates/app \
    && if [ "$TEST_TIMERS" = "true" ]; then FEATURES="--features test-timers"; else FEATURES=""; fi \
    && VEHICLEMANAGER_BUILD_COMMIT="$BUILD_COMMIT" \
       VEHICLEMANAGER_BUILD_TAG="$BUILD_TAG" \
       VEHICLEMANAGER_BUILD_DATE="$BUILD_DATE" \
       dx bundle --platform web --release $FEATURES --out-dir /out

# ---------- Runtime stage ----------
FROM debian:trixie-slim

# CA certificates for any outgoing HTTPS (none yet; cheap to have, plan §7).
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# A non-root default user. compose.yaml runs the container as the NAS's own
# `vehiclemanager` user instead, so files in the data folder belong to it.
RUN useradd --uid 10001 --user-group --no-create-home --shell /usr/sbin/nologin appuser \
    && mkdir /data \
    && chown 10001:10001 /data

# The server serves public/ from the folder next to its binary.
WORKDIR /app
COPY --from=build /out/server /app/server
COPY --from=build /out/public /app/public
# The geocoder's data, read-only (decision #059).
COPY --from=geodata /out/ /app/geodata/

# Listen on all interfaces inside the container; exposure is decided on the host
# (compose.yaml publishes to NAS loopback only, plan §7 Networking).
# VEHICLEMANAGER_DATA_DIR must match domain::identity::env_var_name("DATA_DIR");
# crates/app/tests/deploy_config.rs checks it.
ENV IP=0.0.0.0 \
    PORT=8080 \
    VEHICLEMANAGER_DATA_DIR=/data

USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/app/server"]
