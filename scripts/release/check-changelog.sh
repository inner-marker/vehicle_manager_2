#!/usr/bin/env bash
# Fails unless CHANGELOG.md has notes for the version in Cargo.toml. Run by the
# PR-to-main gate and the release workflow (plan §8 CI/CD).
#
# Usage: scripts/release/check-changelog.sh [changelog-file] [version]
set -euo pipefail

changelog=${1:-CHANGELOG.md}
version=${2:-$("$(dirname "$0")/workspace-version.sh")}

"$(dirname "$0")/changelog-section.sh" "$version" "$changelog" > /dev/null
echo "$changelog has notes for version $version"
