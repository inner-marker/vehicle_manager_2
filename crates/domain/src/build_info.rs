//! Which exact build is running (phase-3.md 1c, GitHub #37): the commit, the image tag it was
//! built for, and when, beside the version (which only changes at a release).
//!
//! The values come in at compile time as environment variables ([`COMMIT_ENV`] and the
//! rest): the image workflows pass them to `docker build`, and for a local build the app's
//! `build.rs` reads the commit from `git`. [`BuildInfo::from_parts`] tidies whatever arrived,
//! so a missing or odd value never breaks anything; it just isn't shown.

use serde::{Deserialize, Serialize};

/// The commit the build came from (`git rev-parse HEAD`, full or short).
pub const COMMIT_ENV: &str = "VEHICLEMANAGER_BUILD_COMMIT";

/// `true` when a local build had uncommitted changes.
pub const MODIFIED_ENV: &str = "VEHICLEMANAGER_BUILD_MODIFIED";

/// The image tag the build was made for (`dev`, `milestone-p3-m1-groundwork`, `0.3.0`, …).
pub const TAG_ENV: &str = "VEHICLEMANAGER_BUILD_TAG";

/// When the build was made, as shown (`2026-11-08 19:02 UTC`).
pub const DATE_ENV: &str = "VEHICLEMANAGER_BUILD_DATE";

/// How many of the commit's characters are shown, as `git` does by default.
const SHORT_COMMIT: usize = 7;

/// The longest image tag kept (Docker's own limit).
const MAX_TAG: usize = 128;

/// The longest build date kept.
const MAX_DATE: usize = 32;

/// One build's identity. Every part is optional: a build outside CI has no tag or date,
/// and one without `git` has no commit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildInfo {
    /// The commit, shortened to seven characters.
    pub commit: Option<String>,
    /// Whether the build had uncommitted changes (local builds only).
    pub modified: bool,
    /// The image tag it was built for; `None` for a build not made by the image workflows.
    pub image_tag: Option<String>,
    /// When it was built, as shown.
    pub built_at: Option<String>,
}

/// `value`, trimmed, if it's non-empty, at most `max` characters, and only `allowed` ones.
fn tidy(value: Option<&str>, max: usize, allowed: impl Fn(char) -> bool) -> Option<String> {
    let value = value?.trim();
    (!value.is_empty() && value.chars().count() <= max && value.chars().all(allowed))
        .then(|| value.to_owned())
}

impl BuildInfo {
    /// A build's identity from its raw values (each as the environment gave it, or `None`).
    /// A commit that isn't hexadecimal, a tag with characters Docker doesn't allow, or an
    /// over-long value is dropped.
    #[must_use]
    pub fn from_parts(
        commit: Option<&str>,
        modified: Option<&str>,
        image_tag: Option<&str>,
        built_at: Option<&str>,
    ) -> Self {
        let commit = tidy(commit, 40, |c| c.is_ascii_hexdigit()).map(|commit| {
            commit
                .chars()
                .take(SHORT_COMMIT)
                .collect::<String>()
                .to_ascii_lowercase()
        });
        Self {
            commit,
            modified: modified.is_some_and(|value| value.trim() == "true"),
            image_tag: tidy(image_tag, MAX_TAG, |c| {
                c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')
            }),
            built_at: tidy(built_at, MAX_DATE, |c| {
                c.is_ascii_alphanumeric() || matches!(c, ' ' | ':' | '-' | '+' | 'T' | 'Z')
            }),
        }
    }

    /// The build in one line, e.g. `dev · a885ac6 · 2026-11-08 19:02 UTC`. A build without an
    /// image tag says `local` (the caller's word for "local build"), and one with uncommitted
    /// changes adds `modified`.
    #[must_use]
    pub fn label(&self, local: &str, modified: &str) -> String {
        let mut parts = vec![self.image_tag.clone().unwrap_or_else(|| local.to_owned())];
        if let Some(commit) = &self.commit {
            parts.push(commit.clone());
        }
        if self.modified {
            parts.push(modified.to_owned());
        }
        if let Some(built_at) = &self.built_at {
            parts.push(built_at.clone());
        }
        parts.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_build_shows_its_tag_commit_and_date() {
        let build = BuildInfo::from_parts(
            Some("a885ac64833b7b9d9577a9feb9934521f8f37d00"),
            None,
            Some("milestone-p3-m1-groundwork"),
            Some("2026-11-08 19:02 UTC"),
        );
        assert_eq!(build.commit.as_deref(), Some("a885ac6"));
        assert_eq!(
            build.label("local build", "modified"),
            "milestone-p3-m1-groundwork · a885ac6 · 2026-11-08 19:02 UTC"
        );
    }

    #[test]
    fn a_local_build_says_so_and_marks_uncommitted_changes() {
        let build = BuildInfo::from_parts(Some("A885AC6"), Some("true"), None, None);
        assert_eq!(
            build.label("local build", "modified"),
            "local build · a885ac6 · modified"
        );
        let unknown = BuildInfo::from_parts(None, None, None, None);
        assert_eq!(unknown, BuildInfo::default());
        assert_eq!(unknown.label("local build", "modified"), "local build");
    }

    #[test]
    fn odd_or_empty_values_are_dropped() {
        let build = BuildInfo::from_parts(
            Some("not-a-commit"),
            Some("yes"),
            Some("dev; rm -rf /"),
            Some("   "),
        );
        assert_eq!(build, BuildInfo::default());
        let long_tag = "x".repeat(129);
        assert_eq!(
            BuildInfo::from_parts(None, None, Some(&long_tag), None).image_tag,
            None
        );
        assert_eq!(
            BuildInfo::from_parts(None, None, Some(" 0.3.0 "), None)
                .image_tag
                .as_deref(),
            Some("0.3.0")
        );
    }

    #[test]
    fn an_older_server_without_a_build_reads_as_unknown() {
        let read: BuildInfo = serde_json::from_str("{}").unwrap();
        assert_eq!(read, BuildInfo::default());
    }
}
