#!/bin/sh
set -eu

readonly_clang_version='Apple clang version 17.0.0 (clang-1700.0.13.5)'
readonly_ld_version='@(#)PROGRAM:ld PROJECT:ld-1167.5'
readonly_sdk_version='15.5'
readonly_sdk_build='24F74'
readonly_deployment_target='11.0'
readonly_ld_build_version='1167.5'

if [ "$(uname -s)" != "Darwin" ] || [ "$(uname -m)" != "arm64" ]; then
  echo "private desktop UI packaging requires arm64 macOS" >&2
  exit 2
fi
if [ "$#" -ne 2 ]; then
  echo "usage: package-ui-macos.sh ABSOLUTE_NEW_OUTPUT_DIRECTORY ABSOLUTE_ENGINE_PACKAGE" >&2
  exit 2
fi
for command in cmp file find mktemp nm otool plutil sed shasum sort xcrun; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "required private desktop UI packaging tool is unavailable: $command" >&2
    exit 2
  fi
done

repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd -P)
output=$1
engine_root=$2
case "$output" in /*) ;; *) echo "output directory must be absolute" >&2; exit 2 ;; esac
case "$engine_root" in /*) ;; *) echo "engine package must be absolute" >&2; exit 2 ;; esac
if [ -e "$output" ] || [ -L "$output" ]; then
  echo "output directory must not already exist or be a symbolic link" >&2
  exit 2
fi
if [ ! -d "$engine_root/SemapraxPrivate.app" ] || [ -L "$engine_root" ]; then
  echo "engine package is missing or linked" >&2
  exit 2
fi
if find "$engine_root/SemapraxPrivate.app" -type l -print | grep . >/dev/null; then
  echo "engine package contains a symbolic link" >&2
  exit 2
fi

clang_tool=$(xcrun --sdk macosx --find clang)
ld_tool=$(xcrun --sdk macosx --find ld)
sdk_path=$(xcrun --sdk macosx --show-sdk-path)
if [ ! -x "$clang_tool" ] || [ ! -x "$ld_tool" ] || [ ! -d "$sdk_path" ] ||
   [ "$("$clang_tool" --version | sed -n '1p')" != "$readonly_clang_version" ] ||
   [ "$("$ld_tool" -v 2>&1 | sed -n '1p')" != "$readonly_ld_version" ] ||
   [ "$(xcrun --sdk macosx --show-sdk-version)" != "$readonly_sdk_version" ] ||
   [ "$(xcrun --sdk macosx --show-sdk-build-version)" != "$readonly_sdk_build" ]; then
  echo "private desktop UI toolchain does not match the exact pinned macOS lane" >&2
  exit 2
fi

scratch=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/semaprax-desktop-ui-v1.XXXXXX")
cleanup() {
  case "$scratch" in
    "${RUNNER_TEMP:-${TMPDIR:-/tmp}}"/semaprax-desktop-ui-v1.*) rm -rf -- "$scratch" ;;
    *) echo "refusing to remove unexpected private desktop UI scratch path: $scratch" >&2 ;;
  esac
}
trap cleanup EXIT INT TERM

engine_app="$engine_root/SemapraxPrivate.app"
ui_source="$repo/platform-tests/desktop-native/ui-macos.m"

build_ui() {
  destination=$1
  SOURCE_DATE_EPOCH=1 ZERO_AR_DATE=1 "$clang_tool" \
    -isysroot "$sdk_path" \
    -mmacosx-version-min="$readonly_deployment_target" \
    --ld-path="$ld_tool" -fobjc-arc -fvisibility=hidden -std=c11 \
    -pedantic-errors -Wall -Wextra -Werror -O2 \
    -framework Cocoa "$ui_source" -o "$destination"
}

mkdir -p "$scratch/ui-first" "$scratch/ui-second"
build_ui "$scratch/ui-first/SemapraxPrivate"
build_ui "$scratch/ui-second/SemapraxPrivate"
if ! cmp -s "$scratch/ui-first/SemapraxPrivate" "$scratch/ui-second/SemapraxPrivate"; then
  echo "private desktop UI executable is not byte-reproducible" >&2
  exit 1
fi

app="$output/SemapraxPrivateUI.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$repo/platform-tests/desktop-native/Info-ui.plist" "$app/Contents/Info.plist"
cp "$scratch/ui-first/SemapraxPrivate" "$app/Contents/MacOS/SemapraxPrivate"
cp "$engine_app/Contents/MacOS/SemapraxPrivate" "$app/Contents/Resources/SemapraxPrivateEngine"
cp "$engine_app/Contents/Resources/SemapraxPrivateProvider.dylib" "$app/Contents/Resources/SemapraxPrivateProvider.dylib"
cp "$engine_app/Contents/Resources/SemapraxPrivateProvider.spxnabi3" "$app/Contents/Resources/SemapraxPrivateProvider.spxnabi3"
chmod 755 "$app/Contents/MacOS/SemapraxPrivate" "$app/Contents/Resources/SemapraxPrivateEngine"

write_engine_manifest() {
  manifest_engine=$1
  manifest_destination=$2
  manifest_digest=$(shasum -a 256 "$manifest_engine" |
    sed -n 's/^\([0-9a-f][0-9a-f]*\)  .*$/\1/p')
  case "$manifest_digest" in
    *[!0-9a-f]*|'') echo "private desktop UI engine digest is not lowercase hexadecimal" >&2; exit 1 ;;
  esac
  if [ "${#manifest_digest}" -ne 64 ]; then
    echo "private desktop UI engine digest has the wrong width" >&2
    exit 1
  fi
  printf 'semaprax.private-desktop-engine-sha256.v1 %s\n' "$manifest_digest" >"$manifest_destination"
}
write_engine_manifest \
  "$app/Contents/Resources/SemapraxPrivateEngine" \
  "$app/Contents/Resources/SemapraxPrivateEngine.sha256"

plutil -lint "$app/Contents/Info.plist" >/dev/null
if [ "$(plutil -extract CFBundlePackageType raw -o - "$app/Contents/Info.plist")" != 'APPL' ] ||
   [ "$(plutil -extract CFBundleExecutable raw -o - "$app/Contents/Info.plist")" != 'SemapraxPrivate' ] ||
   plutil -extract LSBackgroundOnly raw -o - "$app/Contents/Info.plist" >/dev/null 2>&1; then
  echo "private desktop UI property-list contract changed" >&2
  exit 1
fi

ui="$app/Contents/MacOS/SemapraxPrivate"
engine="$app/Contents/Resources/SemapraxPrivateEngine"
provider="$app/Contents/Resources/SemapraxPrivateProvider.dylib"
for binary in "$ui" "$engine" "$provider"; do
  if [ -L "$binary" ] || ! file "$binary" | grep -F 'Mach-O 64-bit' | grep -F 'arm64' >/dev/null; then
    echo "private desktop UI artifact is not an arm64 64-bit Mach-O: $binary" >&2
    exit 1
  fi
  if ! otool -l "$binary" | grep -F 'cmd LC_UUID' >/dev/null; then
    echo "private desktop UI artifact lacks the mandatory Mach-O UUID: $binary" >&2
    exit 1
  fi
done
for executable in "$ui" "$engine"; do
  if ! otool -hv "$executable" | grep -E 'MH_MAGIC_64[[:space:]]+ARM64.*EXECUTE' >/dev/null; then
    echo "private desktop UI executable Mach-O header changed: $executable" >&2
    exit 1
  fi
done
if ! otool -hv "$provider" | grep -E 'MH_MAGIC_64[[:space:]]+ARM64.*DYLIB' >/dev/null; then
  echo "private desktop UI provider Mach-O header changed" >&2
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
for binary in "$ui" "$engine" "$provider"; do
  actual_build_version=$(otool -l "$binary" | sed -n '/cmd LC_BUILD_VERSION/,+7p' | sed 's/^[[:space:]]*//')
  if [ "$actual_build_version" != "$expected_build_version" ]; then
    echo "private desktop UI Mach-O build-version command changed: $binary" >&2
    exit 1
  fi
done
for binary in "$ui" "$engine" "$provider"; do
  load_commands=$(otool -l "$binary")
  if [ "$(printf '%s\n' "$load_commands" | sed -n '1p')" != "$binary:" ]; then
    echo "private desktop UI Mach-O load-command header changed: $binary" >&2
    exit 1
  fi
  if printf '%s\n' "$load_commands" | sed '1d' | grep -E 'LC_RPATH|@loader_path|@executable_path|/private/|/Users/|/Volumes/|target/' >/dev/null; then
    echo "private desktop UI package contains an ambient or build-local load path: $binary" >&2
    exit 1
  fi
done

actual_ui_images=$(otool -L "$ui" | sed -n '2,$s/^[[:space:]]*\([^[:space:]]*\).*/\1/p')
expected_ui_images='/System/Library/Frameworks/Cocoa.framework/Versions/A/Cocoa
/System/Library/Frameworks/Foundation.framework/Versions/C/Foundation
/usr/lib/libobjc.A.dylib
/usr/lib/libSystem.B.dylib
/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit
/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation'
if [ "$actual_ui_images" != "$expected_ui_images" ]; then
  echo "private desktop UI framework dependency allowlist changed" >&2
  printf '%s\n' "$actual_ui_images" >&2
  exit 1
fi
if [ "$(nm -gjU "$ui" | LC_ALL=C sort -u)" != '__mh_execute_header' ]; then
  echo "private desktop UI export allowlist changed" >&2
  exit 1
fi

actual_inventory=$(find "$app" -mindepth 1 -print | sed "s#^$app/##" | LC_ALL=C sort)
expected_inventory='Contents
Contents/Info.plist
Contents/MacOS
Contents/MacOS/SemapraxPrivate
Contents/Resources
Contents/Resources/SemapraxPrivateEngine
Contents/Resources/SemapraxPrivateEngine.sha256
Contents/Resources/SemapraxPrivateProvider.dylib
Contents/Resources/SemapraxPrivateProvider.spxnabi3'
if [ "$actual_inventory" != "$expected_inventory" ] || find "$app" -type l -print | grep . >/dev/null; then
  echo "private desktop UI macOS inventory changed or contains a symbolic link" >&2
  printf '%s\n' "$actual_inventory" >&2
  exit 1
fi

assert_rejected_without_result() {
  rejected_ui=$1
  rejected_result=$2
  if "$rejected_ui" "$rejected_result"; then
    echo "hostile private desktop UI engine unexpectedly executed successfully" >&2
    exit 1
  fi
  if [ -e "$rejected_result" ] || [ -L "$rejected_result" ]; then
    echo "rejected private desktop UI engine published a result" >&2
    exit 1
  fi
}

mismatch_app="$scratch/mismatch/SemapraxPrivateUI.app"
mkdir -p "$scratch/mismatch"
cp -R "$app" "$mismatch_app"
mismatch_engine="$mismatch_app/Contents/Resources/SemapraxPrivateEngine"
case "$mismatch_engine" in "$scratch"/*) ;; *) echo "unexpected mismatch-engine path" >&2; exit 1 ;; esac
printf '\000' >>"$mismatch_engine"
assert_rejected_without_result \
  "$mismatch_app/Contents/MacOS/SemapraxPrivate" \
  "$scratch/mismatch-result.txt"

timeout_app="$scratch/timeout/SemapraxPrivateUI.app"
mkdir -p "$scratch/timeout"
cp -R "$app" "$timeout_app"
timeout_source="$repo/platform-tests/desktop-native/timeout-engine-macos.c"
timeout_engine="$scratch/timeout-engine"
SOURCE_DATE_EPOCH=1 ZERO_AR_DATE=1 "$clang_tool" \
  -isysroot "$sdk_path" \
  -mmacosx-version-min="$readonly_deployment_target" \
  --ld-path="$ld_tool" -std=c11 -pedantic-errors -Wall -Wextra -Werror -O2 \
  "$timeout_source" -o "$timeout_engine"
if [ ! -x "$timeout_engine" ] ||
   ! file "$timeout_engine" | grep -F 'Mach-O 64-bit' | grep -F 'arm64' >/dev/null; then
  echo "bounded silent macOS UI timeout probe is unavailable" >&2
  exit 1
fi
cp "$timeout_engine" "$timeout_app/Contents/Resources/SemapraxPrivateEngine"
write_engine_manifest \
  "$timeout_app/Contents/Resources/SemapraxPrivateEngine" \
  "$timeout_app/Contents/Resources/SemapraxPrivateEngine.sha256"
assert_rejected_without_result \
  "$timeout_app/Contents/MacOS/SemapraxPrivate" \
  "$scratch/timeout-result.txt"

result="$scratch/ui-result.txt"
"$ui" "$result"
expected='SEMAPRAX_DESKTOP_UI_V1_OK platform=macos lifecycle=launch,window,shown,control,close,terminate accessibility=button-name engine=calls-2-replay-exact'
if [ ! -f "$result" ] || [ "$(sed -n '1p' "$result")" != "$expected" ] || [ "$(sed -n '2p' "$result")" != '' ]; then
  echo "unexpected packaged macOS desktop UI result" >&2
  [ -f "$result" ] && cat "$result" >&2
  exit 1
fi
printf '%s\n' "$expected"
