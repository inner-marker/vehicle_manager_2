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
fn compose_passes_the_credential_variable_the_server_reads() {
    // server::auth reads this name (its own test pins it to env_var_name("TEST_CREDENTIAL")).
    let credential = env_var_name("TEST_CREDENTIAL");
    assert!(
        COMPOSE.contains(&format!("{credential}: ${{{credential}")),
        "compose.yaml must pass {credential}"
    );
    assert!(
        ENV_EXAMPLE.contains(&format!("{credential}=")),
        ".env.example must list {credential}"
    );
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

#[test]
fn env_example_leaves_the_credential_empty() {
    // An empty credential makes the server refuse everything (decisions.md #024), so a
    // copied-but-unedited .env can't leave the server open with a known value.
    let credential = env_var_name("TEST_CREDENTIAL");
    let line = ENV_EXAMPLE
        .lines()
        .find(|line| line.starts_with(&format!("{credential}=")))
        .unwrap_or_default();
    assert_eq!(line, format!("{credential}="));
}
