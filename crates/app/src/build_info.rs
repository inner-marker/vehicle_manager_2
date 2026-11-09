//! This build's identity (phase-3.md 1c, GitHub #37), compiled in: from the image workflows'
//! build arguments (through the `Dockerfile`), or for a local build from `git` (`build.rs`).
//! See `domain::build_info`.

use domain::build_info::BuildInfo;

/// This build: the web app and the server of one image share it (they're built together);
/// the desktop app has its own.
#[must_use]
pub fn this_build() -> BuildInfo {
    // The names must match domain::build_info's *_ENV constants (a test checks them).
    BuildInfo::from_parts(
        option_env!("VEHICLEMANAGER_BUILD_COMMIT"),
        option_env!("VEHICLEMANAGER_BUILD_MODIFIED"),
        option_env!("VEHICLEMANAGER_BUILD_TAG"),
        option_env!("VEHICLEMANAGER_BUILD_DATE"),
    )
}

#[cfg(test)]
mod tests {
    use domain::build_info::{COMMIT_ENV, DATE_ENV, MODIFIED_ENV, TAG_ENV};

    #[test]
    fn the_compiled_in_names_match_the_domain_constants() {
        let source = include_str!("build_info.rs");
        for name in [COMMIT_ENV, MODIFIED_ENV, TAG_ENV, DATE_ENV] {
            assert!(
                source.contains(&format!("option_env!(\"{name}\")")),
                "this_build must read {name}"
            );
        }
        let script = include_str!("../build.rs");
        for name in [COMMIT_ENV, MODIFIED_ENV, TAG_ENV, DATE_ENV] {
            assert!(script.contains(name), "build.rs must know {name}");
        }
    }
}
