#!/bin/sh
set -eu

readonly_rust_version='rustc 1.97.1 (8bab26f4f 2026-07-14)'
readonly_rust_llvm='LLVM version: 22.1.6'
readonly_clang_version='Apple clang version 21.0.0 (clang-2100.1.1.101)'
readonly_ld_version='@(#)PROGRAM:ld PROJECT:ld-1267'
readonly_sdk_version='26.5'
readonly_sdk_build='25F70'
readonly_deployment_target='11.0'
readonly_ld_build_version='1267.0'
readonly_provider_id='@rpath/SemapraxPrivateProvider.dylib'
readonly_app_signature_id='semaprax.private.desktop.v1'

if [ "$(uname -s)" != "Darwin" ] || [ "$(uname -m)" != "arm64" ]; then
  echo "private desktop macOS packaging requires arm64 macOS" >&2
  exit 2
fi
if [ "$#" -ne 1 ]; then
  echo "usage: package-macos.sh ABSOLUTE_NEW_OUTPUT_DIRECTORY" >&2
  exit 2
fi
for command in cargo codesign cmp file find mktemp nm otool plutil rustc sed shasum sort xcrun; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "required private desktop packaging tool is unavailable: $command" >&2
    exit 2
  fi
done

repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd -P)
output=$1
case "$output" in /*) ;; *) echo "output directory must be absolute" >&2; exit 2 ;; esac
if [ -e "$output" ] || [ -L "$output" ]; then
  echo "output directory must not already exist or be a symbolic link" >&2
  exit 2
fi

if [ "$(rustc --version)" != "$readonly_rust_version" ]; then
  echo "desktop Rust compiler is not the exact pinned version" >&2
  rustc --version >&2
  exit 2
fi
if ! rustc -vV | grep -F "$readonly_rust_llvm" >/dev/null; then
  echo "desktop Rust LLVM is not the exact pinned version" >&2
  rustc -vV >&2
  exit 2
fi
clang_tool=$(xcrun --sdk macosx --find clang)
ld_tool=$(xcrun --sdk macosx --find ld)
sdk_path=$(xcrun --sdk macosx --show-sdk-path)
if [ ! -x "$clang_tool" ] || [ ! -x "$ld_tool" ] || [ ! -d "$sdk_path" ]; then
  echo "xcrun did not resolve the pinned macOS compiler, linker, and SDK" >&2
  exit 2
fi
if [ "$("$clang_tool" --version | sed -n '1p')" != "$readonly_clang_version" ]; then
  echo "desktop Apple Clang is not the exact pinned version" >&2
  "$clang_tool" --version >&2
  exit 2
fi
if [ "$("$ld_tool" -v 2>&1 | sed -n '1p')" != "$readonly_ld_version" ]; then
  echo "desktop Apple linker is not the exact pinned version" >&2
  "$ld_tool" -v >&2
  exit 2
fi
if [ "$(xcrun --sdk macosx --show-sdk-version)" != "$readonly_sdk_version" ] ||
   [ "$(xcrun --sdk macosx --show-sdk-build-version)" != "$readonly_sdk_build" ]; then
  echo "desktop macOS SDK is not the exact pinned version and build" >&2
  xcrun --sdk macosx --show-sdk-version >&2
  xcrun --sdk macosx --show-sdk-build-version >&2
  exit 2
fi
if [ -n "${CARGO_ENCODED_RUSTFLAGS:-}" ]; then
  echo "desktop packaging rejects ambient CARGO_ENCODED_RUSTFLAGS" >&2
  exit 2
fi

scratch=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/semaprax-desktop-v3.XXXXXX")
cleanup() {
  case "$scratch" in
    "${RUNNER_TEMP:-${TMPDIR:-/tmp}}"/semaprax-desktop-v3.*) rm -rf -- "$scratch" ;;
    *) echo "refusing to remove unexpected private desktop scratch path: $scratch" >&2 ;;
  esac
}
trap cleanup EXIT INT TERM

build_once() {
  label=$1
  build="$scratch/$label"
  target="$scratch/target-$label"
  mkdir -p "$build"
  source_file="$build/provider.c"
  descriptor_file="$build/SemapraxPrivateProvider.spxnabi3"
  provider_file="$build/SemapraxPrivateProvider.dylib"
  SDKROOT="$sdk_path" MACOSX_DEPLOYMENT_TARGET="$readonly_deployment_target" \
    SOURCE_DATE_EPOCH=1 ZERO_AR_DATE=1 \
    CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER="$clang_tool" \
    RUSTFLAGS="--remap-path-prefix=$target=/semaprax-private-desktop-target -C link-arg=--ld-path=$ld_tool" \
    CARGO_TARGET_DIR="$target" cargo run --quiet --offline --locked \
    -p semaprax-native-host --features unstable-desktop-app-harness \
    --bin private-desktop-v3-fixture -- "$source_file" "$descriptor_file"
  SOURCE_DATE_EPOCH=1 ZERO_AR_DATE=1 "$clang_tool" -isysroot "$sdk_path" \
    -mmacosx-version-min="$readonly_deployment_target" \
    --ld-path="$ld_tool" \
    -std=c11 -pedantic-errors -Wall -Wextra -Werror -O2 -dynamiclib \
    -Wl,-reproducible \
    -Wl,-install_name,"$readonly_provider_id" \
    "$source_file" -o "$provider_file"
  SDKROOT="$sdk_path" MACOSX_DEPLOYMENT_TARGET="$readonly_deployment_target" \
    SOURCE_DATE_EPOCH=1 ZERO_AR_DATE=1 \
    CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER="$clang_tool" \
    RUSTFLAGS="--remap-path-prefix=$target=/semaprax-private-desktop-target -C codegen-units=1 -C link-arg=--ld-path=$ld_tool -C link-arg=-Wl,-reproducible -C link-arg=-Wl,-x" \
    CARGO_TARGET_DIR="$target" cargo rustc --quiet --offline --locked --release \
    -p semaprax-native-host --features unstable-desktop-app-harness \
    --bin private-desktop-v3-app -- -C link-arg=-Wl,-no_adhoc_codesign
  SDKROOT="$sdk_path" MACOSX_DEPLOYMENT_TARGET="$readonly_deployment_target" \
    SOURCE_DATE_EPOCH=1 ZERO_AR_DATE=1 \
    CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER="$clang_tool" \
    RUSTFLAGS="--remap-path-prefix=$target=/semaprax-private-desktop-target -C link-arg=--ld-path=$ld_tool" \
  CARGO_TARGET_DIR="$target" cargo run --quiet --offline --locked \
    -p semaprax-native-host --features unstable-desktop-app-harness \
    --bin private-desktop-macho-uuid -- \
    "$target/release/private-desktop-v3-app" "$build/SemapraxPrivate"
}

package_once() {
  label=$1
  build="$scratch/$label"
  package_root="$scratch/package-$label"
  app="$package_root/SemapraxPrivate.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  cp platform-tests/desktop-native/Info.plist "$app/Contents/Info.plist"
  cp "$build/SemapraxPrivate" "$app/Contents/MacOS/SemapraxPrivate"
  cp "$build/SemapraxPrivateProvider.dylib" "$app/Contents/Resources/SemapraxPrivateProvider.dylib"
  cp "$build/SemapraxPrivateProvider.spxnabi3" "$app/Contents/Resources/SemapraxPrivateProvider.spxnabi3"
  chmod 755 "$app/Contents/MacOS/SemapraxPrivate"
  codesign --force --sign - --timestamp=none \
    --identifier "$readonly_app_signature_id" "$app"
  codesign --verify --strict "$app"
}

cd "$repo"
build_once first
build_once second
for artifact in provider.c SemapraxPrivateProvider.spxnabi3 SemapraxPrivateProvider.dylib SemapraxPrivate; do
  if ! cmp -s "$scratch/first/$artifact" "$scratch/second/$artifact"; then
    echo "private desktop artifact is not reproducible: $artifact" >&2
    shasum -a 256 "$scratch/first/$artifact" "$scratch/second/$artifact" >&2
    exit 1
  fi
done

package_once first
package_once second
first_app="$scratch/package-first/SemapraxPrivate.app"
second_app="$scratch/package-second/SemapraxPrivate.app"
first_inventory=$(find "$first_app" -mindepth 1 -print | sed "s#^$first_app/##" | LC_ALL=C sort)
second_inventory=$(find "$second_app" -mindepth 1 -print | sed "s#^$second_app/##" | LC_ALL=C sort)
if [ "$first_inventory" != "$second_inventory" ]; then
  echo "independently signed private desktop package inventories differ" >&2
  exit 1
fi
for relative in $(find "$first_app" -type f -print | sed "s#^$first_app/##" | LC_ALL=C sort); do
  if ! cmp -s "$first_app/$relative" "$second_app/$relative"; then
    echo "independently signed private desktop package file is not reproducible: $relative" >&2
    shasum -a 256 "$first_app/$relative" "$second_app/$relative" >&2
    exit 1
  fi
done

mkdir -p "$output"
cp -R "$first_app" "$output/SemapraxPrivate.app"
app="$output/SemapraxPrivate.app"
codesign --verify --strict "$app"

plutil -lint "$app/Contents/Info.plist" >/dev/null
if [ "$(plutil -extract CFBundlePackageType raw -o - "$app/Contents/Info.plist")" != 'APPL' ] ||
   [ "$(plutil -extract LSBackgroundOnly raw -o - "$app/Contents/Info.plist")" != 'true' ]; then
  echo "private desktop property list contract changed" >&2
  exit 1
fi

executable="$app/Contents/MacOS/SemapraxPrivate"
provider="$app/Contents/Resources/SemapraxPrivateProvider.dylib"
for binary in "$executable" "$provider"; do
  if [ -L "$binary" ] || ! file "$binary" | grep -F 'Mach-O 64-bit' | grep -F 'arm64' >/dev/null; then
    echo "private desktop artifact is not an arm64 64-bit Mach-O: $binary" >&2
    exit 1
  fi
  if ! otool -l "$binary" | grep -F 'cmd LC_UUID' >/dev/null; then
    echo "private desktop artifact lacks the mandatory Mach-O UUID: $binary" >&2
    exit 1
  fi
done
if ! otool -hv "$executable" | grep -E 'MH_MAGIC_64[[:space:]]+ARM64.*EXECUTE' >/dev/null; then
  echo "private desktop executable Mach-O header changed" >&2
  exit 1
fi
if ! otool -hv "$provider" | grep -E 'MH_MAGIC_64[[:space:]]+ARM64.*DYLIB' >/dev/null; then
  echo "private desktop provider Mach-O header changed" >&2
  exit 1
fi
expected_build_version="cmd LC_BUILD_VERSION
cmdsize 32
platform 1
minos $readonly_deployment_target
sdk $readonly_sdk_version
ntools 1
tool 3
version $readonly_ld_build_version"
for binary in "$executable" "$provider"; do
  actual_build_version=$(otool -l "$binary" | sed -n '/cmd LC_BUILD_VERSION/,+7p' | sed 's/^[[:space:]]*//')
  if [ "$actual_build_version" != "$expected_build_version" ]; then
    echo "private desktop Mach-O build-version command changed: $binary" >&2
    printf '%s\n' "$actual_build_version" >&2
    exit 1
  fi
done
if [ "$(otool -D "$provider" | sed -n '2p')" != "$readonly_provider_id" ]; then
  echo "private desktop provider install identity changed" >&2
  otool -D "$provider" >&2
  exit 1
fi
for binary in "$executable" "$provider"; do
  load_commands=$(otool -l "$binary")
  if [ "$(printf '%s\n' "$load_commands" | sed -n '1p')" != "$binary:" ]; then
    echo "private desktop Mach-O load-command header changed: $binary" >&2
    exit 1
  fi
  if printf '%s\n' "$load_commands" | sed '1d' | grep -E 'LC_RPATH|@loader_path|@executable_path|/private/|/Users/|/Volumes/|target/' >/dev/null; then
    echo "private desktop Mach-O contains an ambient or build-local load path: $binary" >&2
    exit 1
  fi
done

actual_executable_images=$(otool -L "$executable" | sed -n '2,$s/^[[:space:]]*\([^[:space:]]*\).*/\1/p')
expected_executable_images='/usr/lib/libSystem.B.dylib
/usr/lib/libiconv.2.dylib'
if [ "$actual_executable_images" != "$expected_executable_images" ]; then
  echo "private desktop executable dependency allowlist changed" >&2
  printf '%s\n' "$actual_executable_images" >&2
  exit 1
fi
actual_provider_images=$(otool -L "$provider" | sed -n '2,$s/^[[:space:]]*\([^[:space:]]*\).*/\1/p')
expected_provider_images="$readonly_provider_id
/usr/lib/libSystem.B.dylib"
if [ "$actual_provider_images" != "$expected_provider_images" ]; then
  echo "private desktop provider dependency allowlist changed" >&2
  printf '%s\n' "$actual_provider_images" >&2
  exit 1
fi

actual_provider_exports=$(nm -gjU "$provider" | LC_ALL=C sort -u)
expected_provider_exports='_spx_91fcc6dc8d2360d0d2d82bdfd0ca0b858123bf94481701cb_settle_v3
_spx_bc155186b4bee926b067131fcade912528466cf75acd8afc_descriptor_v3
_spx_d72f6239e04d9f84af37553d6588219300c6dc73b1df6b21_execute_v3'
if [ "$actual_provider_exports" != "$expected_provider_exports" ]; then
  echo "private desktop provider export allowlist changed" >&2
  printf '%s\n' "$actual_provider_exports" >&2
  exit 1
fi
actual_app_exports=$(nm -gjU "$executable" | LC_ALL=C sort -u)
if [ "$actual_app_exports" != '__mh_execute_header' ]; then
  echo "private desktop application export allowlist changed" >&2
  printf '%s\n' "$actual_app_exports" >&2
  exit 1
fi

actual_inventory=$(find "$app" -mindepth 1 -print | sed "s#^$app/##" | LC_ALL=C sort)
expected_inventory='Contents
Contents/Info.plist
Contents/MacOS
Contents/MacOS/SemapraxPrivate
Contents/Resources
Contents/Resources/SemapraxPrivateProvider.dylib
Contents/Resources/SemapraxPrivateProvider.spxnabi3
Contents/_CodeSignature
Contents/_CodeSignature/CodeResources'
if [ "$actual_inventory" != "$expected_inventory" ] || find "$app" -type l -print | grep . >/dev/null; then
  echo "private desktop macOS package inventory changed or contains a symbolic link" >&2
  printf '%s\n' "$actual_inventory" >&2
  exit 1
fi

actual=$("$executable")
expected='SEMAPRAX_DESKTOP_V3_OK platform=macos calls=2 owner=0 payloads=41,43 replay=exact'
if [ "$actual" != "$expected" ]; then
  echo "unexpected packaged macOS result: $actual" >&2
  exit 1
fi
printf '%s\n' "$actual"
