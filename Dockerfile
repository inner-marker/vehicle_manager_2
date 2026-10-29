# syntax=docker/dockerfile:1
#
# The server image: the Dioxus fullstack server plus its web UI (plan §7 Building the
# app image). linux/amd64 for the DS1019+ NAS (decisions.md #025).
#
#   docker build -t vehicle_manager_2 .
#
# Two stages: a build stage with the Rust toolchain and the Dioxus CLI, and a slim
# runtime stage that ships only the bundle and CA certificates (no compiler, no source).
#
# Never add `-C target-cpu=native` or similar: the NAS CPU (Intel J3455) has no AVX,
# so a binary tuned for the build machine could crash there (#025).

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
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cd crates/app && dx bundle --platform web --release --out-dir /out

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
