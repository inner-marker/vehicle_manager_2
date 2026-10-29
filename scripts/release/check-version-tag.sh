#!/usr/bin/env bash
# Fails unless a release tag is exactly "v" + the workspace version (e.g. v0.1.0).
#
# Usage: scripts/release/check-version-tag.sh <tag> [version]
#   version defaults to the workspace version; passing it lets tests skip cargo.
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "usage: $0 <tag> [version]" >&2
    exit 2
fi
tag=$1
version=${2:-$("$(dirname "$0")/workspace-version.sh")}

if [ "$tag" != "v$version" ]; then
    echo "error: tag $tag doesn't match the Cargo.toml version $version (expected v$version)" >&2
    exit 1
fi
echo "tag $tag matches version $version"
