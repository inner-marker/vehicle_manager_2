//! Entry point for every target: the web UI, desktop and mobile apps, and
//! (with the `server` feature) the fullstack server.
//!
//! Kept thin: the app's modules live in the `app` library (`lib.rs`).

use app::logging::{self, LogHandles};
use domain::log_area;
use tracing::subscriber::SetGlobalDefaultError;

fn main() {
    // Logging must start before launching, so Dioxus's own logger sees ours and steps aside.
    let log_handles = match start_logging() {
        Ok(log_handles) => Some(log_handles),
        // Another subscriber got there first; it still receives our events.
        Err(error) => {
            tracing::warn!(target: log_area::APP, %error, "debug log not installed");
            None
        }
    };
    tracing::info!(target: log_area::APP, version = env!("CARGO_PKG_VERSION"), "app starting");

    launch(log_handles);
}

/// Server logging: with `VEHICLEMANAGER_DATA_DIR` set (the Docker image sets `/data`),
/// rotating log files and the crash file go to its `logs/` folder; otherwise
/// (e.g. `dx serve`) the console and in-app viewer only.
#[cfg(feature = "server")]
fn start_logging() -> Result<LogHandles, SetGlobalDefaultError> {
    let data_dir_var = domain::identity::env_var_name(logging::DATA_DIR_ENV_NAME);
    match std::env::var_os(data_dir_var) {
        Some(data_dir) => {
            let log_dir = std::path::Path::new(&data_dir).join(logging::LOG_DIR_NAME);
            logging::init_with_log_dir(&log_dir, logging::FileLogPolicy::SERVER)
        }
        None => logging::init(),
    }
}

/// Client logging: console and in-app viewer. Desktop log files come with milestone 4.
#[cfg(not(feature = "server"))]
fn start_logging() -> Result<LogHandles, SetGlobalDefaultError> {
    logging::init()
}

/// The server: the Dioxus app (pages, assets, server functions) behind the auth layer,
/// which guards every `/api/` path (`server::auth`).
#[cfg(feature = "server")]
fn launch(log_handles: Option<LogHandles>) {
    use dioxus::prelude::dioxus_server::axum::Router;
    use dioxus::prelude::{DioxusRouterExt, ServeConfig};

    let auth = server::auth::AuthConfig::from_env();
    // Called again on every hot reload in development, so it rebuilds the router each time.
    dioxus::serve(move || {
        let log_handles = log_handles.clone();
        let auth = auth.clone();
        async move {
            let mut config = ServeConfig::new();
            // The log viewer reads the handles from context.
            if let Some(log_handles) = log_handles {
                config = config.context(log_handles);
            }
            let app = Router::new().serve_dioxus_application(config, app::ui::App);
            Ok(server::auth::protect(app, auth))
        }
    })
}

/// Web, desktop, and mobile clients.
#[cfg(not(feature = "server"))]
fn launch(log_handles: Option<LogHandles>) {
    let mut launcher = dioxus::LaunchBuilder::new();
    // The log viewer reads the handles from context.
    if let Some(log_handles) = log_handles {
        launcher = launcher.with_context(log_handles);
    }
    launcher.launch(app::ui::App);
}
