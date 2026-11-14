#!/usr/bin/env sh
# Writes crates/app/assets/THIRD-PARTY-NOTICES.txt (decision #079): the notices of the
# third-party work the app ships, for the app's Licenses link and the server image.
#
#   scripts/notices.sh           write the file
#   scripts/notices.sh --check   fail if the committed file isn't what this would write
#
# Needs cargo-about 0.9.3 (cargo install cargo-about --locked --features cli --version 0.9.3)
# and python3. Run it again whenever a dependency, Lucide, or pdf.js changes; the Main gate
# runs the check.
set -eu

cd "$(dirname "$0")/.."
out=crates/app/assets/THIRD-PARTY-NOTICES.txt
want_about="cargo-about 0.9.3"

have_about=$(cargo about --version 2>/dev/null || true)
if [ "$have_about" != "$want_about" ]; then
    echo "error: needs $want_about (found: ${have_about:-none})" >&2
    exit 1
fi

# The sources of every crate in Cargo.lock, for cargo-about and the two read below.
cargo fetch --locked --quiet

# A package's folder in the cargo registry, from cargo metadata.
dir_of() {
    cargo metadata --format-version 1 --all-features --locked 2>/dev/null |
        python3 -I -c '
import json, os, sys
name = sys.argv[1]
for package in json.load(sys.stdin)["packages"]:
    if package["name"] == name:
        print(os.path.dirname(package["manifest_path"]))
        break
' "$1"
}

sqlite_dir=$(dir_of libsqlite3-sys)
openssl_dir=$(dir_of openssl-src)

section() {
    printf '\n================================================================================\n'
    printf '%s\n' "$1"
    printf '================================================================================\n\n'
}

file() {
    printf -- '--- %s ---\n\n' "$1"
    cat "$2"
    printf '\n'
}

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
{
    printf 'Vehicle Manager: third-party notices\n\n'
    printf 'Vehicle Manager is built with the work of others, listed here with the licenses\n'
    printf 'and notices they ask to travel with it. Made by scripts/notices.sh.\n'

    section "Lucide icons (the app's icons)"
    file "ISC and MIT licenses" crates/app/assets/icons/LICENSE-lucide

    section "pdf.js (showing PDFs), with its image decoders"
    file "pdf.js: Apache License 2.0" crates/app/assets/pdfjs/LICENSE
    file "Its color profile" crates/app/assets/pdfjs/iccs/LICENSE
    for license in crates/app/assets/pdfjs/wasm/LICENSE_*; do
        file "$(basename "$license" | sed 's/^LICENSE_//')" "$license"
    done

    section "SQLCipher (the desktop app's encrypted database)"
    file "SQLCipher: BSD 3-Clause License" "$sqlite_dir/sqlcipher/LICENSE"

    section "OpenSSL (built into the desktop app on Windows)"
    file "OpenSSL: Apache License 2.0" "$openssl_dir/openssl/LICENSE.txt"

    section "Rust crates, by license"
    cargo about generate --workspace --all-features --locked --fail about.hbs 2>/dev/null
} > "$tmp"

if [ "${1:-}" = "--check" ]; then
    if ! cmp -s "$tmp" "$out"; then
        echo "error: $out is out of date; run scripts/notices.sh and commit it" >&2
        diff "$out" "$tmp" | head -20 >&2 || true
        exit 1
    fi
    echo "$out is current"
else
    mv "$tmp" "$out"
    chmod 644 "$out"
    trap - EXIT
    echo "wrote $out"
fi
