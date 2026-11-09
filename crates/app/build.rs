//! Tells a local build which commit it came from (phase-3.md 1c, GitHub #37).
//!
//! The image workflows pass the commit, tag, and date as environment variables
//! (`VEHICLEMANAGER_BUILD_COMMIT`, `VEHICLEMANAGER_BUILD_TAG`, `VEHICLEMANAGER_BUILD_DATE`),
//! which `app::build_info` reads directly; this script then does nothing. Otherwise (a local
//! build) it asks `git` for the commit and whether there are uncommitted changes
//! (`VEHICLEMANAGER_BUILD_MODIFIED`). Without `git` or a repository (e.g. inside Docker), the
//! build is just "local build". A build script must never fail the build, so every problem
//! means "unknown".

use std::path::{Path, PathBuf};
use std::process::Command;

/// The variables the image workflows set; a change to any of them rebuilds the app.
const FROM_CI: [&str; 3] = [
    "VEHICLEMANAGER_BUILD_COMMIT",
    "VEHICLEMANAGER_BUILD_TAG",
    "VEHICLEMANAGER_BUILD_DATE",
];

/// `git` run in `root` with `args`: its trimmed output, or `None` if it failed.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    for name in FROM_CI {
        println!("cargo:rerun-if-env-changed={name}");
    }
    println!("cargo:rerun-if-env-changed=VEHICLEMANAGER_BUILD_MODIFIED");
    if std::env::var("VEHICLEMANAGER_BUILD_COMMIT").is_ok_and(|commit| !commit.trim().is_empty()) {
        return;
    }

    // The repository root: crates/app/../..
    let Some(root) = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .and_then(|app| app.parent()?.parent().map(Path::to_path_buf))
    else {
        return;
    };
    // A new commit or a change to what's staged rebuilds the app, so its commit stays right.
    // ("Modified" is checked then too: an edit not yet staged shows from the next one.)
    let git_dir = root.join(".git");
    for watched in ["HEAD", "index", "refs/heads"] {
        let path = git_dir.join(watched);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    let Some(commit) = git(&root, &["rev-parse", "HEAD"]) else {
        return;
    };
    println!("cargo:rustc-env=VEHICLEMANAGER_BUILD_COMMIT={commit}");
    let modified = git(&root, &["status", "--porcelain", "--untracked-files=no"])
        .is_some_and(|changes| !changes.is_empty());
    if modified {
        println!("cargo:rustc-env=VEHICLEMANAGER_BUILD_MODIFIED=true");
    }
}
