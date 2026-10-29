//! Where a native app keeps its files: the per-user data folder.
//!
//! The folder is the platform's data directory joined with
//! `domain::identity::DATA_DIR_NAME`, never the display name, so renaming the app
//! can't lose anyone's data (AGENTS.md §3, decision #016):
//! - macOS: `~/Library/Application Support/vehiclemanager`
//! - Linux: `$XDG_DATA_HOME/vehiclemanager` (usually `~/.local/share/vehiclemanager`)
//! - Windows: `%APPDATA%\vehiclemanager`
//!
//! The server doesn't use this: its data directory comes from `VEHICLEMANAGER_DATA_DIR`
//! (`crate::logging::DATA_DIR_ENV_NAME`). Native-only; the web app has no files.

use std::path::{Path, PathBuf};

use domain::identity::DATA_DIR_NAME;

/// The app's data folder for the current user, or `None` if the platform doesn't
/// report a data directory (e.g. no home directory). Doesn't create it.
#[must_use]
pub fn platform_data_folder() -> Option<PathBuf> {
    dirs::data_dir().map(|base| data_folder_in(&base))
}

/// The app's data folder inside a platform data directory `base`.
#[must_use]
pub fn data_folder_in(base: &Path) -> PathBuf {
    base.join(DATA_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_folder_is_named_after_the_data_dir_name_not_the_display_name() {
        let base = Path::new("/home/someone/.local/share");
        assert_eq!(
            data_folder_in(base),
            Path::new("/home/someone/.local/share/vehiclemanager")
        );
    }

    #[test]
    fn platform_data_folder_ends_in_the_data_dir_name() {
        // CI runners and developer machines all have a home directory.
        let folder = platform_data_folder().unwrap();
        assert!(folder.ends_with(DATA_DIR_NAME));
    }
}
