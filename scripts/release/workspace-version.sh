#!/usr/bin/env bash
# Prints the app version: `version` in [workspace.package] of the root Cargo.toml,
# which every crate inherits (plan §8 Versioning). Used by the other release scripts.
#
# Usage: scripts/release/workspace-version.sh
set -euo pipefail

# All crates share the workspace version, so any member works; `domain` is the
# one with no optional features, so metadata is quick to read.
version=$(cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.name == "domain") | .version')

if [ -z "$version" ] || [ "$version" = "null" ]; then
    echo "error: could not read the workspace version from Cargo.toml" >&2
    exit 1
fi
echo "$version"
