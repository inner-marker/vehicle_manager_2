#!/usr/bin/env bash
# Tests for the release scripts, run against fixtures/CHANGELOG.md.
# Run locally or in CI: scripts/release/test.sh
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
fixture="$here/fixtures/CHANGELOG.md"
failures=0

pass() { echo "ok   - $1"; }
fail() { echo "FAIL - $1"; failures=$((failures + 1)); }

# expect_success <name> <command...>: the command must exit 0.
expect_success() {
    local name=$1
    shift
    if "$@" > /dev/null 2>&1; then pass "$name"; else fail "$name"; fi
}

# expect_failure <name> <command...>: the command must exit non-zero.
expect_failure() {
    local name=$1
    shift
    if "$@" > /dev/null 2>&1; then fail "$name"; else pass "$name"; fi
}

# --- check-version-tag.sh ---
expect_success "matching tag passes" "$here/check-version-tag.sh" v0.1.0 0.1.0
expect_failure "tag without the v prefix fails" "$here/check-version-tag.sh" 0.1.0 0.1.0
expect_failure "tag for another version fails" "$here/check-version-tag.sh" v0.2.0 0.1.0

# --- changelog-section.sh ---
notes=$("$here/changelog-section.sh" 0.2.0 "$fixture")
if [[ "$notes" == *"Second release feature"* ]]; then pass "section has its own notes"; else fail "section has its own notes"; fi
if [[ "$notes" != *"First release fix"* && "$notes" != *"Not released yet"* ]]; then
    pass "section stops at the next heading"
else
    fail "section stops at the next heading"
fi
if [[ "$notes" != *"## [0.2.0]"* ]]; then pass "section omits its own heading"; else fail "section omits its own heading"; fi
expect_failure "missing version fails" "$here/changelog-section.sh" 9.9.9 "$fixture"
expect_failure "empty section fails" "$here/changelog-section.sh" 0.0.9 "$fixture"
expect_failure "missing changelog file fails" "$here/changelog-section.sh" 0.1.0 "$here/fixtures/does-not-exist.md"
# "0.1.0" must not match a longer version that starts with it, e.g. "0.1.0-rc.1" or "0.1.00".
expect_failure "partial version does not match" "$here/changelog-section.sh" 0.1 "$fixture"

# --- check-changelog.sh ---
expect_success "changelog with the version passes" "$here/check-changelog.sh" "$fixture" 0.1.0
expect_failure "changelog without the version fails" "$here/check-changelog.sh" "$fixture" 1.0.0

if [ "$failures" -gt 0 ]; then
    echo "$failures release script test(s) failed" >&2
    exit 1
fi
echo "all release script tests passed"
