#!/usr/bin/env sh
set -eu

fail() {
    echo "release package rejected: $1" >&2
    exit 2
}

[ "$#" -eq 4 ] || fail "expected TAG COMMIT TARGET OUTPUT_ROOT"
tag=$1
commit=$2
target=$3
output_root=$4

versions=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml)
[ -n "$versions" ] || fail "Cargo package version is missing"
[ "$(printf '%s\n' "$versions" | wc -l | tr -d ' ')" -eq 1 ] || fail "Cargo package version is ambiguous"
version=$versions
[ "$tag" = "v$version" ] || fail "tag does not equal v plus the Cargo package version"
[ "${#commit}" -eq 40 ] || fail "commit must be exactly 40 lowercase hexadecimal characters"
case "$commit" in *[!0-9a-f]*) fail "commit must be exactly 40 lowercase hexadecimal characters" ;; esac

case "$target" in
    x86_64-unknown-linux-gnu|aarch64-apple-darwin) ;;
    *) fail "unsupported Unix release target" ;;
esac
host=$(rustc -vV | sed -n 's/^host: //p')
[ "$host" = "$target" ] || fail "Rust host does not equal the requested release target"

package_name="semaprax-$tag-$target"
package_root="$output_root/$package_name"
archive="$output_root/$package_name.tar.gz"
smoke_root="$output_root/smoke-$target"
[ ! -e "$package_root" ] || fail "package staging path already exists"
[ ! -e "$archive" ] || fail "archive path already exists"
[ ! -e "$smoke_root" ] || fail "smoke extraction path already exists"
mkdir -p "$package_root/smoke" "$smoke_root"

SEMAPRAX_BUILD_COMMIT=$commit cargo build --locked --release --target "$target" --bin semaprax --bin semapraxd
cp "target/$target/release/semaprax" "$package_root/semaprax"
cp "target/$target/release/semapraxd" "$package_root/semapraxd"
cp LICENSE README.md "$package_root/"
printf '%s\n' \
    '{' \
    '  "schema": "semaprax.release-artifact.v1",' \
    "  \"version\": \"$version\"," \
    "  \"commit\": \"$commit\"," \
    "  \"target\": \"$target\"," \
    '  "maturity": "pre-alpha",' \
    '  "binaries": ["semaprax", "semapraxd"],' \
    '  "nonclaims": [' \
    '    "production-ready",' \
    '    "stable language ABI",' \
    '    "stable public protocol",' \
    '    "safety-critical suitability"' \
    '  ]' \
    '}' > "$package_root/release-manifest.json"
printf '%s\n' \
    'module release.smoke;' \
    '' \
    '@id("release.smoke.main")' \
    'fn main() -> i64 { 42 }' > "$package_root/smoke/meaning.spx"

tar -czf "$archive" -C "$output_root" "$package_name"
tar -xzf "$archive" -C "$smoke_root"
unpacked="$smoke_root/$package_name"
[ "$("$unpacked/semaprax" --version)" = "semaprax $version ($commit)" ] || fail "human version smoke disagrees"
[ "$("$unpacked/semaprax" version --json)" = "{\"schema\":\"semaprax.version.v1\",\"version\":\"$version\",\"commit\":\"$commit\",\"maturity\":\"pre-alpha\",\"rust_min\":\"1.88\"}" ] || fail "JSON version smoke disagrees"
"$unpacked/semaprax" check "$unpacked/smoke/meaning.spx"
[ "$("$unpacked/semaprax" run "$unpacked/smoke/meaning.spx")" = 42 ] || fail "run smoke disagrees"
printf '%s\n' "$archive"
