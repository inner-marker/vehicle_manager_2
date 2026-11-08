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

/// Desktop logging: rotating log files and the crash file in the data folder's `logs/`
/// (`app::data_folder`), plus the console and in-app viewer.
#[cfg(all(feature = "desktop", not(feature = "server")))]
fn start_logging() -> Result<LogHandles, SetGlobalDefaultError> {
    let Some(data_folder) = app::data_folder::platform_data_folder() else {
        let handles = logging::init()?;
        // Logged only now, once the subscriber exists to record it.
        tracing::warn!(target: log_area::APP, "no data folder on this platform; not writing log files");
        return Ok(handles);
    };
    logging::init_with_log_dir(
        &data_folder.join(logging::LOG_DIR_NAME),
        logging::FileLogPolicy::DESKTOP,
    )
}

/// Web and mobile logging: console and in-app viewer. Mobile log files come with Phase 4.
#[cfg(not(any(feature = "server", feature = "desktop")))]
fn start_logging() -> Result<LogHandles, SetGlobalDefaultError> {
    logging::init()
}

/// The server: the Dioxus app (pages, assets, server functions) behind the auth layer,
/// which guards every `/api/` path (`server::auth`).
#[cfg(feature = "server")]
fn launch(log_handles: Option<LogHandles>) {
    use dioxus::prelude::dioxus_server::axum::Router;
    use dioxus::prelude::{DioxusRouterExt, ServeConfig};

    // Every protected API request needs a signed-in session (decision #028).
    let auth = server::auth::AuthConfig::new(server::auth::SessionSource::new(
        || server::startup::database().cloned(),
        std::sync::Arc::new(logging::SystemClock),
    ));
    let paths = server_paths();
    // Before the router: the auth layer names the session cookie after it.
    server::startup::set_instance(server_instance());
    // Called again on every hot reload in development, so it rebuilds the router each time.
    dioxus::serve(move || {
        let log_handles = log_handles.clone();
        let auth = auth.clone();
        let paths = paths.clone();
        async move {
            // Opens and checks the database and starts the jobs once; later calls (hot
            // reloads) reuse them.
            server::startup::start(
                &paths,
                std::sync::Arc::new(logging::SystemClock),
                server_timers(),
            )
            .await;
            let mut config = ServeConfig::new();
            // The log viewer reads the handles from context.
            if let Some(log_handles) = log_handles {
                config = config.context(log_handles);
            }
            // Uploads and downloads of photos and PDFs are plain routes (their bodies are
            // files), inside the same auth layer.
            let app = Router::new()
                .merge(app::file_routes::routes())
                .serve_dioxus_application(config, app::ui::App);
            Ok(server::auth::protect(app, auth))
        }
    })
}

/// Where the server keeps its database: `VEHICLEMANAGER_DATA_DIR` (the Docker image
/// sets `/data`), or `dev-data/` in the working directory for local `dx serve`
/// (git-ignored).
#[cfg(feature = "server")]
fn server_database_dir() -> std::path::PathBuf {
    let data_dir_var = domain::identity::env_var_name(logging::DATA_DIR_ENV_NAME);
    std::env::var_os(&data_dir_var).map_or_else(
        || {
            tracing::warn!(target: log_area::DATABASE, env_var = %data_dir_var, "data directory not set; using dev-data/ for local development");
            std::path::PathBuf::from(DEV_DATA_DIR)
        },
        std::path::PathBuf::from,
    )
}

/// The server's folders: the data directory, the snapshot folder from
/// `VEHICLEMANAGER_SNAPSHOT_DIR` if set (otherwise `<data dir>/snapshots`), and the place data
/// file from `VEHICLEMANAGER_GEODATA_FILE` if set (otherwise the server looks for it).
#[cfg(feature = "server")]
fn server_paths() -> server::startup::ServerPaths {
    // Compose passes an unfilled optional setting as an empty value: treat that as unset.
    let snapshot_override = std::env::var_os(domain::identity::env_var_name(
        server::startup::SNAPSHOT_DIR_ENV_NAME,
    ))
    .filter(|value| !value.is_empty())
    .map(std::path::PathBuf::from);
    // The place data file, if set; otherwise the server looks for it (server::places).
    let geodata_file = std::env::var_os(domain::identity::env_var_name(
        server::places::GEODATA_FILE_ENV_NAME,
    ))
    .filter(|value| !value.is_empty())
    .map(std::path::PathBuf::from);
    server::startup::ServerPaths::new(server_database_dir(), snapshot_override)
        .with_geodata_file(geodata_file)
}

/// The environment variable for test timers (#054), without the app prefix: 1 to 5 minutes,
/// honoured only by builds with the `test-timers` feature (test images, never releases).
#[cfg(feature = "server")]
const TEST_MINUTES_ENV_NAME: &str = "TEST_MINUTES";

/// The timer lengths: normal, or test timers when this is a test build and
/// `VEHICLEMANAGER_TEST_MINUTES` is set. Says so in the log either way it's set.
#[cfg(feature = "server")]
fn server_timers() -> domain::timers::Timers {
    let env_var = domain::identity::env_var_name(TEST_MINUTES_ENV_NAME);
    let setting = std::env::var(&env_var).ok();
    match domain::timers::Timers::from_setting(setting.as_deref(), cfg!(feature = "test-timers")) {
        Ok(timers) => {
            if let Some(minutes) = timers.test_minutes() {
                tracing::warn!(target: log_area::APP, minutes, "TEST TIMERS ON: Admin mode, sign-out, jobs, and recovery codes use minutes; for testing only");
            }
            timers
        }
        Err(error) => {
            tracing::warn!(target: log_area::APP, env_var = %env_var, %error, "test timers setting ignored");
            domain::timers::Timers::NORMAL
        }
    }
}

/// This server's instance name from `VEHICLEMANAGER_INSTANCE` (decision #062): empty for the
/// main server, `test` for the test server beside it. An invalid name is ignored, with a
/// warning, and the server runs as the main one.
#[cfg(feature = "server")]
fn server_instance() -> Option<domain::identity::InstanceName> {
    let env_var = domain::identity::env_var_name(server::startup::INSTANCE_ENV_NAME);
    let setting = std::env::var(&env_var).unwrap_or_default();
    match domain::identity::InstanceName::parse(&setting) {
        Ok(Some(instance)) => {
            tracing::warn!(target: log_area::APP, instance = instance.as_str(), "this is a second server, not the household's main one");
            Some(instance)
        }
        Ok(None) => None,
        Err(error) => {
            tracing::warn!(target: log_area::APP, env_var = %env_var, %error, "instance name ignored; running as the main server");
            None
        }
    }
}

/// The local development data folder, used when `VEHICLEMANAGER_DATA_DIR` isn't set.
#[cfg(feature = "server")]
const DEV_DATA_DIR: &str = "dev-data";

/// Web, desktop, and mobile clients.
#[cfg(not(feature = "server"))]
fn launch(log_handles: Option<LogHandles>) {
    #[cfg(feature = "desktop")]
    use_saved_server_address();

    let mut launcher = dioxus::LaunchBuilder::new();
    // The log viewer reads the handles from context.
    if let Some(log_handles) = log_handles {
        launcher = launcher.with_context(log_handles);
    }
    #[cfg(feature = "desktop")]
    {
        launcher = launcher.with_cfg(desktop_config());
    }
    launcher.launch(app::ui::App);
}

/// The desktop window's starting size, in logical pixels (a laptop screen, with room to spare).
#[cfg(all(feature = "desktop", not(feature = "server")))]
const WINDOW_SIZE: (f64, f64) = (1200.0, 820.0);

/// The smallest the window may be: about a phone's width, which the app already lays out for.
#[cfg(all(feature = "desktop", not(feature = "server")))]
const WINDOW_MIN_SIZE: (f64, f64) = (400.0, 600.0);

/// The desktop window: its title, starting and smallest size, and never kept on top (Dioxus
/// keeps a debug build's window on top of others by default). No right-click menu, so no
/// Reload or Inspect, and no developer tools (Dioxus turns them on only with that menu). No
/// menu bar on Windows and Linux, where Dioxus's default one shows inside the window; macOS
/// keeps the default, whose Edit menu makes copy and paste work in text fields.
///
/// Not covered: Windows' `WebView2` still answers F5 and F12 itself; Dioxus 0.7 doesn't expose
/// the setting that turns its keyboard shortcuts off.
#[cfg(all(feature = "desktop", not(feature = "server")))]
fn desktop_config() -> dioxus::desktop::Config {
    use dioxus::desktop::{Config, LogicalSize, WindowBuilder};

    let window = WindowBuilder::new()
        .with_title(domain::identity::DISPLAY_NAME)
        .with_inner_size(LogicalSize::new(WINDOW_SIZE.0, WINDOW_SIZE.1))
        .with_min_inner_size(LogicalSize::new(WINDOW_MIN_SIZE.0, WINDOW_MIN_SIZE.1))
        .with_always_on_top(false);
    let config = Config::new()
        .with_window(window)
        .with_disable_context_menu(true);
    if cfg!(target_os = "macos") {
        config
    } else {
        config.with_menu(None)
    }
}

/// Points server functions at the saved server address (`app::settings`), if there is
/// one. Must run before launching: Dioxus keeps the first address it's given for the
/// whole run, and otherwise uses the local development server, `http://127.0.0.1:8080`
/// (decision #027).
#[cfg(all(feature = "desktop", not(feature = "server")))]
fn use_saved_server_address() {
    let saved = app::data_folder::platform_data_folder()
        .and_then(|data_folder| app::settings::saved_server_address(&data_folder));
    match saved {
        Some(address) => {
            tracing::info!(target: log_area::APP, address = address.as_str(), "using the saved server address");
            // Dioxus needs a `&'static str`; this runs once per process, so leaking one
            // short string is the intended use.
            dioxus::prelude::dioxus_fullstack::set_server_url(address.as_str().to_owned().leak());
        }
        None => {
            tracing::debug!(target: log_area::APP, "no saved server address; using the development server");
        }
    }
}
