#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "private callable-v3 simulator evidence requires an arm64 macOS host" >&2
  exit 1
fi

for command in cargo codesign file grep mktemp otool sed xcrun; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "required iOS simulator tool is unavailable: $command" >&2
    exit 1
  fi
done

scratch="$(mktemp -d "${TMPDIR:-/tmp}/semaprax-ios-simulator-v3.XXXXXX")"
booted_udid=""
cleanup() {
  if [[ -n "$booted_udid" ]]; then
    xcrun simctl shutdown "$booted_udid" >/dev/null 2>&1 || true
  fi
  case "$scratch" in
    "${TMPDIR:-/tmp}"/semaprax-ios-simulator-v3.*)
      rm -rf -- "$scratch"
      ;;
    *)
      echo "refusing to remove unexpected simulator scratch path: $scratch" >&2
      ;;
  esac
}
trap cleanup EXIT INT TERM

sdk_path="$(xcrun --sdk iphonesimulator --show-sdk-path)"
sdk_version="$(xcrun --sdk iphonesimulator --show-sdk-version)"
if [[ -z "$sdk_path" || -z "$sdk_version" ]]; then
  echo "the iPhone Simulator SDK is unavailable" >&2
  exit 1
fi
minimum_ios_version="15.0"
export IPHONEOS_DEPLOYMENT_TARGET="$minimum_ios_version"

provider_source="$scratch/semaprax-ios-v3.c"
cargo run --locked -p semaprax-native-host \
  --features unstable-ios-simulator-harness \
  --bin private-ios-simulator-v3-fixture -- "$provider_source"
test -s "$provider_source"

cargo rustc --locked -p semaprax-native-host \
  --target aarch64-apple-ios-sim \
  --release \
  --features unstable-ios-simulator-harness \
  --lib --crate-type staticlib

host_archive="target/aarch64-apple-ios-sim/release/libsemaprax_native_host.a"
test -s "$host_archive"
target_triple="arm64-apple-ios${minimum_ios_version}-simulator"

for optimization in 0 2; do
  executable="$scratch/semaprax-ios-v3-O$optimization"
  xcrun --sdk iphonesimulator clang \
    -target "$target_triple" \
    -isysroot "$sdk_path" \
    -std=c11 \
    "-O$optimization" \
    -Wall -Wextra -Werror -pedantic \
    "$provider_source" \
    "$host_archive" \
    -framework Security \
    -liconv -lm \
    -o "$executable"

  file "$executable" | grep -F "Mach-O 64-bit executable arm64" >/dev/null
  otool -hv "$executable" | grep -E 'ARM64|arm64' >/dev/null
  linked_images="$(otool -L "$executable")"
  if grep -E 'libloading|AI-Lang|target/|@rpath|@loader_path|@executable_path' <<<"$linked_images" >/dev/null; then
    echo "simulator evidence linked a forbidden non-system image" >&2
    exit 1
  fi
  codesign --force --sign - --timestamp=none "$executable"
  codesign --verify --strict "$executable"
done

device_line="$(xcrun simctl list devices available | sed -n '/iPhone.*(Shutdown)/{p;q;}')"
booted_udid="$(sed -E 's/.*\(([0-9A-Fa-f-]{36})\).*/\1/' <<<"$device_line")"
if [[ ! "$booted_udid" =~ ^[0-9A-Fa-f-]{36}$ ]]; then
  echo "no available shutdown iPhone Simulator was found" >&2
  booted_udid=""
  exit 1
fi
xcrun simctl boot "$booted_udid"
xcrun simctl bootstatus "$booted_udid" -b

for optimization in 0 2; do
  expected="SEMAPRAX_IOS_SIM_V3_OK O$optimization target=arm64-simulator finalizers=1:13,0:11 publication=no-owned allocations=0"
  output="$(xcrun simctl spawn "$booted_udid" "$scratch/semaprax-ios-v3-O$optimization" "O$optimization")"
  grep -Fx "$expected" <<<"$output" >/dev/null
done
