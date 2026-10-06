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

# The runtime baseline each archive promises. A built binary that needs more
# than this is rejected below, so the manifest's `runtime` record is checked
# evidence rather than a statement of intent.
GLIBC_BASELINE=2.35
MACOS_BASELINE=
case "$target" in
    x86_64-unknown-linux-gnu) runtime_os=linux; runtime_cpu=x86_64 ;;
    aarch64-unknown-linux-gnu) runtime_os=linux; runtime_cpu=aarch64 ;;
    aarch64-apple-darwin) runtime_os=macos; runtime_cpu=aarch64; MACOS_BASELINE=11.0 ;;
    x86_64-apple-darwin) runtime_os=macos; runtime_cpu=x86_64; MACOS_BASELINE=10.12 ;;
    *) fail "unsupported Unix release target" ;;
esac
rust_identity=$(rustc -vV) || fail "Rust host query failed"
host=$(printf '%s\n' "$rust_identity" | sed -n 's/^host: //p')
[ "$host" = "$target" ] || fail "Rust host does not equal the requested release target"

readme_template=packaging/archive/README.unix.md
[ -f "$readme_template" ] || fail "archive README template is missing: $readme_template"

[ -n "$output_root" ] || fail "output root must not be empty"
case "$output_root" in
    /*) ;;
    *) working_root=$(pwd -P) || fail "working directory cannot be resolved"
       output_root="$working_root/$output_root" ;;
esac
# The caller owns this directory and keeps it quiescent during packaging.
mkdir -p "$output_root"
output_root=$(CDPATH= cd -P "$output_root" && pwd -P) || fail "output root cannot be resolved"
package_name="semaprax-$tag-$target"
build_root="$output_root/build-$target"
package_root="$output_root/$package_name"
archive="$output_root/$package_name.tar.gz"
smoke_root="$output_root/smoke-$target"
[ ! -e "$build_root" ] && [ ! -L "$build_root" ] || fail "build path already exists"
[ ! -e "$package_root" ] && [ ! -L "$package_root" ] || fail "package staging path already exists"
[ ! -e "$archive" ] && [ ! -L "$archive" ] || fail "archive path already exists"
[ ! -e "$smoke_root" ] && [ ! -L "$smoke_root" ] || fail "smoke extraction path already exists"
mkdir "$build_root" "$package_root" "$smoke_root"
mkdir "$package_root/smoke"

# version_gt A B: succeeds when dotted numeric version A is greater than B.
version_gt() {
    awk -v a="$1" -v b="$2" 'BEGIN {
        na = split(a, x, "."); nb = split(b, y, ".")
        n = (na > nb) ? na : nb
        for (i = 1; i <= n; i++) {
            xi = (i <= na) ? x[i] + 0 : 0
            yi = (i <= nb) ? y[i] + 0 : 0
            if (xi > yi) exit 0
            if (xi < yi) exit 1
        }
        exit 1
    }'
}

# json_array: newline-separated names on stdin to a JSON array of strings.
json_array() {
    awk 'BEGIN { printf "[" }
        NF { gsub(/\\/, "\\\\"); gsub(/"/, "\\\""); printf "%s\"%s\"", (n++ ? ", " : ""), $0 }
        END { print "]" }'
}

# Linux: every GLIBC_x.y symbol-version requirement must be at or below the
# baseline. Sets runtime_min_libc (highest requirement found) and runtime_libs
# (sorted unique DT_NEEDED names).
inspect_linux_binary_set() {
    versions=""
    libs=""
    for binary in "$@"; do
        if command -v readelf >/dev/null 2>&1; then
            info=$(readelf --version-info "$binary") || fail "readelf --version-info failed for $binary"
            dyn=$(readelf -d "$binary") || fail "readelf -d failed for $binary"
        elif command -v objdump >/dev/null 2>&1; then
            info=$(objdump -T "$binary") || fail "objdump -T failed for $binary"
            dyn=$(objdump -p "$binary") || fail "objdump -p failed for $binary"
        else
            fail "neither readelf nor objdump is available to check the glibc baseline"
        fi
        found=$(printf '%s\n' "$info" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/^GLIBC_//' || true)
        versions="$versions
$found"
        needed=$(printf '%s\n' "$dyn" | sed -n -e 's/.*(NEEDED)[^[]*\[\(.*\)\].*/\1/p' -e 's/^ *NEEDED  *//p')
        libs="$libs
$needed"
    done
    runtime_min_libc=""
    for required in $(printf '%s\n' "$versions" | sort -u); do
        [ -n "$required" ] || continue
        if version_gt "$required" "$GLIBC_BASELINE"; then
            fail "binary requires GLIBC_$required, above the glibc $GLIBC_BASELINE baseline"
        fi
        if [ -z "$runtime_min_libc" ] || version_gt "$required" "$runtime_min_libc"; then
            runtime_min_libc=$required
        fi
    done
    [ -n "$runtime_min_libc" ] || fail "no GLIBC version requirement found; not a dynamic glibc binary"
    runtime_libs=$(printf '%s\n' "$libs" | sed '/^$/d' | sort -u)
}

# macOS: the LC_BUILD_VERSION minos (or LC_VERSION_MIN_MACOSX version) of
# every binary must be at or below the deployment target set for the build.
# Sets runtime_min_os (highest minos found) and runtime_libs.
inspect_macos_binary_set() {
    runtime_min_os=""
    libs=""
    for binary in "$@"; do
        if command -v vtool >/dev/null 2>&1; then
            load=$(vtool -show-build "$binary") || fail "vtool -show-build failed for $binary"
        elif command -v otool >/dev/null 2>&1; then
            load=$(otool -l "$binary") || fail "otool -l failed for $binary"
        else
            fail "neither vtool nor otool is available to check the macOS deployment target"
        fi
        minos=$(printf '%s\n' "$load" | awk '
            /cmd LC_BUILD_VERSION/ { mode = 1 }
            /cmd LC_VERSION_MIN_MACOSX/ { mode = 2 }
            mode == 1 && $1 == "minos" { print $2; mode = 0 }
            mode == 2 && $1 == "version" { print $2; mode = 0 }' | head -n 1)
        [ -n "$minos" ] || fail "no macOS minimum-version load command found in $binary"
        if version_gt "$minos" "$MACOS_BASELINE"; then
            fail "binary targets macOS $minos, above the macOS $MACOS_BASELINE baseline"
        fi
        if [ -z "$runtime_min_os" ] || version_gt "$minos" "$runtime_min_os"; then
            runtime_min_os=$minos
        fi
        deps=$(otool -L "$binary" | sed '1d' | awk '{ print $1 }') || fail "otool -L failed for $binary"
        libs="$libs
$deps"
    done
    runtime_libs=$(printf '%s\n' "$libs" | sed '/^$/d' | sort -u)
}

if [ "$runtime_os" = macos ]; then
    # Pin the deployment target explicitly so the binaries' minos is the
    # documented baseline rather than whatever the runner's SDK defaults to.
    MACOSX_DEPLOYMENT_TARGET=$MACOS_BASELINE
    export MACOSX_DEPLOYMENT_TARGET
fi

SEMAPRAX_BUILD_COMMIT=$commit cargo build --locked --release --target "$target" --target-dir "$build_root" -p semaprax -p semaprax-toolchain --bin semaprax-full --bin semapraxd
cp "$build_root/$target/release/semaprax-full" "$package_root/semaprax"
cp "$build_root/$target/release/semapraxd" "$package_root/semapraxd"
cp LICENSE "$package_root/"
# The archive README is rendered from a small maintained template; the
# repository README is not packaged because its links are checkout-relative.
sed -e "s|{{TAG}}|$tag|g" -e "s|{{VERSION}}|$version|g" -e "s|{{TARGET}}|$target|g" "$readme_template" > "$package_root/README.md"
if grep -q '{{' "$package_root/README.md"; then
    fail "archive README still has an unreplaced placeholder"
fi

if [ "$runtime_os" = linux ]; then
    inspect_linux_binary_set "$package_root/semaprax" "$package_root/semapraxd"
    runtime_min_os_json=null
    runtime_libc_family=glibc
    runtime_min_libc_json="\"$runtime_min_libc\""
else
    inspect_macos_binary_set "$package_root/semaprax" "$package_root/semapraxd"
    runtime_min_os_json="\"$runtime_min_os\""
    runtime_libc_family=libsystem
    runtime_min_libc_json=null
fi
runtime_libs_json=$(printf '%s\n' "$runtime_libs" | json_array)
printf '%s\n' \
    '{' \
    '  "schema": "semaprax.release-artifact.v1",' \
    "  \"version\": \"$version\"," \
    "  \"commit\": \"$commit\"," \
    "  \"target\": \"$target\"," \
    '  "maturity": "beta",' \
    '  "binaries": ["semaprax", "semapraxd"],' \
    '  "nonclaims": [' \
    '    "production-ready",' \
    '    "stable language ABI",' \
    '    "stable public protocol",' \
    '    "safety-critical suitability"' \
    '  ],' \
    '  "runtime": {' \
    "    \"os\": \"$runtime_os\"," \
    "    \"min_os_version\": $runtime_min_os_json," \
    "    \"cpu\": \"$runtime_cpu\"," \
    "    \"libc_family\": \"$runtime_libc_family\"," \
    "    \"min_libc_version\": $runtime_min_libc_json," \
    "    \"dynamic_libraries\": $runtime_libs_json" \
    '  }' \
    '}' > "$package_root/release-manifest.json"
printf '%s\n' \
    'module app;' \
    '' \
    '@id("app.main")' \
    'fn main() -> i64 { 42 }' > "$package_root/smoke/meaning.spx"

tar -czf "$archive" -C "$output_root" "$package_name"
tar -xzf "$archive" -C "$smoke_root"
unpacked="$smoke_root/$package_name"
human_version=$("$unpacked/semaprax" --version) || fail "human version smoke failed"
[ "$human_version" = "semaprax $version ($commit)" ] || fail "human version smoke disagrees"
json_version=$("$unpacked/semaprax" version --json) || fail "JSON version smoke failed"
[ "$json_version" = "{\"schema\":\"semaprax.version.v1\",\"version\":\"$version\",\"commit\":\"$commit\",\"maturity\":\"beta\",\"rust_min\":\"1.88\"}" ] || fail "JSON version smoke disagrees"
"$unpacked/semaprax" check "$unpacked/smoke/meaning.spx"
run_result=$("$unpacked/semaprax" run "$unpacked/smoke/meaning.spx") || fail "run smoke failed"
[ "$run_result" = 42 ] || fail "run smoke disagrees"
printf '%s\n' "$archive"
