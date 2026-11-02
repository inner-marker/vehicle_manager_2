//! Checks that the deployment files agree with the code they configure.
//!
//! `Dockerfile` and `deploy/compose.yaml` can't call Rust, so they repeat names the
//! code derives (environment variables from `domain::identity::env_var_name`). These
//! tests fail if the copies drift, and guard the deployment's security-critical
//! settings (loopback-only port, non-root user).

use app::logging::DATA_DIR_ENV_NAME;
use domain::identity::{APP_ID, env_var_name};

const DOCKERFILE: &str = include_str!("../../../Dockerfile");
const COMPOSE: &str = include_str!("../../../deploy/compose.yaml");
const ENV_EXAMPLE: &str = include_str!("../../../deploy/.env.example");
const DEV_IMAGE_WORKFLOW: &str = include_str!("../../../.github/workflows/dev-image.yml");
const RELEASE_WORKFLOW: &str = include_str!("../../../.github/workflows/release.yml");
const MAIN_GATE_WORKFLOW: &str = include_str!("../../../.github/workflows/main-gate.yml");
const GEODATA_SOURCES: &str = include_str!("../../../scripts/geodata/sources.sha256");
const GEODATA_FETCH: &str = include_str!("../../../scripts/geodata/fetch.sh");

/// The image's data directory, which compose.yaml mounts the NAS folder onto.
const CONTAINER_DATA_DIR: &str = "/data";

#[test]
fn dockerfile_sets_the_data_dir_variable_the_server_reads() {
    let expected = format!("{}={CONTAINER_DATA_DIR}", env_var_name(DATA_DIR_ENV_NAME));
    assert!(
        DOCKERFILE.contains(&expected),
        "Dockerfile must set {expected}"
    );
}

#[test]
fn compose_mounts_the_data_folder_at_the_data_dir() {
    assert!(COMPOSE.contains(&format!("./data:{CONTAINER_DATA_DIR}")));
}

#[test]
fn compose_passes_the_optional_snapshot_folder_and_leaves_it_empty_by_default() {
    // server::startup::SNAPSHOT_DIR_ENV_NAME; its own test pins it to this name.
    let snapshot_dir = env_var_name("SNAPSHOT_DIR");
    assert!(
        COMPOSE.contains(&format!("{snapshot_dir}: ${{{snapshot_dir}:-}}")),
        "compose.yaml must pass {snapshot_dir}, empty by default"
    );
    assert!(
        ENV_EXAMPLE
            .lines()
            .any(|line| line == format!("{snapshot_dir}=")),
        ".env.example must list {snapshot_dir}, empty"
    );
}

#[test]
fn compose_passes_the_test_timers_setting_empty_by_default() {
    // main.rs's TEST_MINUTES_ENV_NAME (decision #054).
    let test_minutes = env_var_name("TEST_MINUTES");
    assert!(
        COMPOSE.contains(&format!("{test_minutes}: ${{{test_minutes}:-}}")),
        "compose.yaml must pass {test_minutes}, empty by default"
    );
    assert!(
        ENV_EXAMPLE
            .lines()
            .any(|line| line == format!("{test_minutes}=")),
        ".env.example must list {test_minutes}, empty"
    );
}

#[test]
fn only_the_dev_image_workflow_builds_in_test_timers() {
    assert!(
        DEV_IMAGE_WORKFLOW.contains("test-timers: true"),
        "dev and milestone images build in test timers"
    );
    for (name, workflow) in [
        ("release", RELEASE_WORKFLOW),
        ("main gate", MAIN_GATE_WORKFLOW),
    ] {
        assert!(
            !workflow.contains("test-timers"),
            "the {name} workflow must never build in test timers"
        );
    }
}

#[test]
fn the_retired_test_credential_is_gone_from_the_deploy_files() {
    // Retired in Phase 2 step 2e: signing in replaces it (decision #028).
    let credential = env_var_name("TEST_CREDENTIAL");
    assert!(
        !COMPOSE.contains(&credential),
        "compose.yaml still passes {credential}"
    );
    assert!(
        !ENV_EXAMPLE.contains(&credential),
        ".env.example still lists {credential}"
    );
}

#[test]
fn compose_publishes_the_port_on_loopback_only() {
    // Every published port must bind to 127.0.0.1: the LAN must never reach the app (plan §6).
    let port_lines: Vec<&str> = COMPOSE
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- \"") && line.contains(":8080"))
        .collect();
    assert!(!port_lines.is_empty(), "no published port found");
    for line in port_lines {
        assert!(
            line.starts_with("- \"127.0.0.1:"),
            "port not loopback-only: {line}"
        );
    }
}

#[test]
fn compose_project_and_container_are_named_after_the_app_id() {
    // The project name must match the Container Manager Project (docs/deploying-nas.md),
    // so `docker compose` over SSH updates that Project's container instead of clashing
    // with it.
    assert!(
        COMPOSE.contains(&format!("\nname: {APP_ID}\n")),
        "compose.yaml must set the project name to {APP_ID}"
    );
    assert!(
        COMPOSE.contains(&format!("container_name: {APP_ID}\n")),
        "compose.yaml must name the container {APP_ID}"
    );
}

#[test]
fn image_and_container_never_run_as_root() {
    assert!(
        DOCKERFILE.contains("\nUSER 10001:10001\n"),
        "Dockerfile must end as a non-root user"
    );
    assert!(
        COMPOSE.contains("user: \"${VEHICLEMANAGER_UID:?"),
        "compose.yaml must set the NAS user"
    );
}

/// The value of `NAME=value` (a Dockerfile `ARG` or a shell variable) in `text`.
fn assigned<'a>(text: &'a str, prefix: &str, name: &str) -> Option<&'a str> {
    let start = format!("{prefix}{name}=");
    text.lines()
        .find_map(|line| line.trim().strip_prefix(start.as_str()))
}

#[test]
fn the_geocoder_data_is_built_from_the_pinned_sources_everywhere() {
    // Decision #059: the Dockerfile and the development script fetch the same frozen files,
    // each checked against the SHA-256 in sources.sha256, and build with the same GDAL image.
    let sources: Vec<(&str, &str)> = GEODATA_SOURCES
        .lines()
        .filter_map(|line| line.split_once("  "))
        .collect();
    assert_eq!(sources.len(), 3, "cities, countries, and regions");
    for (sum, file) in &sources {
        assert_eq!(sum.len(), 64, "{file}: a SHA-256 is 64 hex digits");
        let add = format!("ADD --checksum=sha256:{sum} ${{GEODATA_SOURCES}}/{file} /src/");
        assert!(
            DOCKERFILE.contains(&add),
            "Dockerfile must fetch {file} with its checksum"
        );
    }
    for name in ["GEODATA_VERSION", "GEODATA_SOURCES"] {
        let in_dockerfile = assigned(DOCKERFILE, "ARG ", name);
        assert!(in_dockerfile.is_some(), "Dockerfile must set {name}");
        assert_eq!(
            in_dockerfile,
            assigned(GEODATA_FETCH, "", name),
            "{name} differs"
        );
    }
    let image = assigned(GEODATA_FETCH, "", "GDAL_IMAGE").expect("fetch.sh sets GDAL_IMAGE");
    assert!(
        image.contains("@sha256:"),
        "the GDAL image is pinned by digest"
    );
    assert!(
        DOCKERFILE.contains(&format!("FROM {image} AS geodata")),
        "the Dockerfile's geodata stage uses the same GDAL image"
    );
    // Only the data file reaches the runtime image.
    assert!(DOCKERFILE.contains("COPY --from=geodata /out/ /app/geodata/"));
}
