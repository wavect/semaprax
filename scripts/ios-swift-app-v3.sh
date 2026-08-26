#!/usr/bin/env bash
set -euo pipefail

readonly minimum_ios_version="15.0"
readonly expected_o0="SEMAPRAX_IOS_SWIFT_V1_OK mode=explicit optimization=O0 target=arm64-simulator handle=0001000001000001 wrong-thread=0000002d00000002 invalid=0000002d00000007 stale=0000002d00000008 finalizers=1:13,0:11 publication=no-owned allocations=0 handles=0 rf=1 om=1 ca=1 ef=1"
readonly expected_o2="SEMAPRAX_IOS_SWIFT_V1_OK mode=deinit optimization=O2 target=arm64-simulator handle=0001000001000001 wrong-thread=0000002d00000002 invalid=0000002d00000007 stale=0000002d00000008 finalizers=1:13,0:11 publication=no-owned allocations=0 handles=0 rf=1 om=1 ca=1 ef=1"

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "private Swift application evidence requires an arm64 macOS host" >&2
  exit 1
fi
for command in cargo codesign file grep mktemp rustc sed swiftc xcodebuild xcrun; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "required private Swift application tool is unavailable: $command" >&2
    exit 1
  fi
done
if ! xcodebuild -version | grep -E '^Xcode 26([.]|$)' >/dev/null; then
  echo "runner Xcode is not the source-locked major version 26" >&2
  exit 1
fi
if ! swiftc --version | grep -E 'Swift version 6([.]|$)' >/dev/null; then
  echo "runner Swift compiler is not the source-locked major version 6" >&2
  exit 1
fi

readonly scratch="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/semaprax-ios-swift-v1.XXXXXX")"
booted_udid=""
cleanup() {
  for bundle in dev.semaprax.private.swift.o0 dev.semaprax.private.swift.o2; do
    if [[ -n "$booted_udid" ]]; then xcrun simctl uninstall "$booted_udid" "$bundle" >/dev/null 2>&1 || true; fi
  done
  if [[ -n "$booted_udid" ]]; then xcrun simctl shutdown "$booted_udid" >/dev/null 2>&1 || true; fi
  case "$scratch" in
    "${RUNNER_TEMP:-${TMPDIR:-/tmp}}"/semaprax-ios-swift-v1.*) rm -rf -- "$scratch" ;;
    *) echo "refusing to remove unexpected private Swift scratch path: $scratch" >&2 ;;
  esac
}
trap cleanup EXIT INT TERM

readonly device_source="$scratch/device-arm64.c"
readonly simulator_arm64_source="$scratch/simulator-arm64.c"
readonly simulator_x86_64_source="$scratch/simulator-x86_64.c"
readonly device_requires_false_source="$scratch/device-arm64-requires-false.c"
readonly simulator_arm64_requires_false_source="$scratch/simulator-arm64-requires-false.c"
readonly simulator_x86_64_requires_false_source="$scratch/simulator-x86_64-requires-false.c"
readonly device_identity_max_source="$scratch/device-arm64-identity-max.c"
readonly simulator_arm64_identity_max_source="$scratch/simulator-arm64-identity-max.c"
readonly simulator_x86_64_identity_max_source="$scratch/simulator-x86_64-identity-max.c"
readonly device_checked_source="$scratch/device-arm64-checked-add-overflow.c"
readonly simulator_arm64_checked_source="$scratch/simulator-arm64-checked-add-overflow.c"
readonly simulator_x86_64_checked_source="$scratch/simulator-x86_64-checked-add-overflow.c"
readonly device_ensures_source="$scratch/device-arm64-ensures-false.c"
readonly simulator_arm64_ensures_source="$scratch/simulator-arm64-ensures-false.c"
readonly simulator_x86_64_ensures_source="$scratch/simulator-x86_64-ensures-false.c"
cargo run --locked -p semaprax-native-host \
  --features unstable-apple-swift-harness \
  --bin private-apple-swift-v1-fixture -- \
  "$device_source" "$simulator_arm64_source" "$simulator_x86_64_source" \
  "$device_requires_false_source" "$simulator_arm64_requires_false_source" \
  "$simulator_x86_64_requires_false_source" \
  "$device_identity_max_source" "$simulator_arm64_identity_max_source" \
  "$simulator_x86_64_identity_max_source" \
  "$device_checked_source" "$simulator_arm64_checked_source" "$simulator_x86_64_checked_source" \
  "$device_ensures_source" "$simulator_arm64_ensures_source" "$simulator_x86_64_ensures_source"
for source in "$device_source" "$simulator_arm64_source" "$simulator_x86_64_source" \
  "$device_requires_false_source" "$simulator_arm64_requires_false_source" \
  "$simulator_x86_64_requires_false_source" \
  "$device_identity_max_source" "$simulator_arm64_identity_max_source" \
  "$simulator_x86_64_identity_max_source" \
  "$device_checked_source" "$simulator_arm64_checked_source" "$simulator_x86_64_checked_source" \
  "$device_ensures_source" "$simulator_arm64_ensures_source" "$simulator_x86_64_ensures_source"; do test -s "$source"; done

for target in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
  cargo check --locked -p semaprax-native-host --features unstable-apple-swift-harness \
    --target "$target" --lib
  cargo rustc --locked -p semaprax-native-host --features unstable-apple-swift-harness \
    --target "$target" --release --lib --crate-type staticlib
done

readonly device_host="target/aarch64-apple-ios/release/libsemaprax_native_host.a"
readonly simulator_arm64_host="target/aarch64-apple-ios-sim/release/libsemaprax_native_host.a"
readonly simulator_x86_64_host="target/x86_64-apple-ios/release/libsemaprax_native_host.a"
for archive in "$device_host" "$simulator_arm64_host" "$simulator_x86_64_host"; do test -s "$archive"; done

SEMAPRAX_IOS_SWIFT_DEVICE_SOURCE="$device_source" \
SEMAPRAX_IOS_SWIFT_SIM_ARM64_SOURCE="$simulator_arm64_source" \
SEMAPRAX_IOS_SWIFT_SIM_X86_64_SOURCE="$simulator_x86_64_source" \
SEMAPRAX_IOS_SWIFT_DEVICE_REQUIRES_FALSE_SOURCE="$device_requires_false_source" \
SEMAPRAX_IOS_SWIFT_SIM_ARM64_REQUIRES_FALSE_SOURCE="$simulator_arm64_requires_false_source" \
SEMAPRAX_IOS_SWIFT_SIM_X86_64_REQUIRES_FALSE_SOURCE="$simulator_x86_64_requires_false_source" \
SEMAPRAX_IOS_SWIFT_DEVICE_IDENTITY_MAX_SOURCE="$device_identity_max_source" \
SEMAPRAX_IOS_SWIFT_SIM_ARM64_IDENTITY_MAX_SOURCE="$simulator_arm64_identity_max_source" \
SEMAPRAX_IOS_SWIFT_SIM_X86_64_IDENTITY_MAX_SOURCE="$simulator_x86_64_identity_max_source" \
SEMAPRAX_IOS_SWIFT_DEVICE_CHECKED_ADD_OVERFLOW_SOURCE="$device_checked_source" \
SEMAPRAX_IOS_SWIFT_SIM_ARM64_CHECKED_ADD_OVERFLOW_SOURCE="$simulator_arm64_checked_source" \
SEMAPRAX_IOS_SWIFT_SIM_X86_64_CHECKED_ADD_OVERFLOW_SOURCE="$simulator_x86_64_checked_source" \
SEMAPRAX_IOS_SWIFT_DEVICE_ENSURES_FALSE_SOURCE="$device_ensures_source" \
SEMAPRAX_IOS_SWIFT_SIM_ARM64_ENSURES_FALSE_SOURCE="$simulator_arm64_ensures_source" \
SEMAPRAX_IOS_SWIFT_SIM_X86_64_ENSURES_FALSE_SOURCE="$simulator_x86_64_ensures_source" \
SEMAPRAX_IOS_SWIFT_DEVICE_HOST="$device_host" \
SEMAPRAX_IOS_SWIFT_SIM_ARM64_HOST="$simulator_arm64_host" \
SEMAPRAX_IOS_SWIFT_SIM_X86_64_HOST="$simulator_x86_64_host" \
  platform-tests/ios-swift/package.sh

readonly output_root="platform-tests/ios-swift/build"
readonly result_file="semaprax-ios-swift-v1.txt"
device_line="$(xcrun simctl list devices available | sed -n '/iPhone.*(Shutdown)/{p;q;}')"
booted_udid="$(sed -E 's/.*\(([0-9A-Fa-f-]{36})\).*/\1/' <<<"$device_line")"
if [[ ! "$booted_udid" =~ ^[0-9A-Fa-f-]{36}$ ]]; then
  echo "no available shutdown iPhone Simulator was found" >&2
  booted_udid=""
  exit 1
fi
xcrun simctl boot "$booted_udid"
xcrun simctl bootstatus "$booted_udid" -b

run_app() {
  local app="$1"
  local bundle="$2"
  local expected="$3"
  xcrun simctl uninstall "$booted_udid" "$bundle" >/dev/null 2>&1 || true
  xcrun simctl install "$booted_udid" "$app"
  xcrun simctl launch --terminate-running-process "$booted_udid" "$bundle" >/dev/null
  local container=""
  local result=""
  local attempt=0
  while [[ "$attempt" -lt 120 ]]; do
    container="$(xcrun simctl get_app_container "$booted_udid" "$bundle" data 2>/dev/null || true)"
    if [[ -n "$container" && -f "$container/Documents/$result_file" ]]; then
      result="$(tr -d '\r\n' <"$container/Documents/$result_file")"
      break
    fi
    sleep 1
    attempt=$((attempt + 1))
  done
  if [[ "$result" != "$expected" ]]; then
    echo "private Swift app result is absent or not exact: $bundle" >&2
    printf '%s\n' "$result" >&2
    exit 1
  fi
  xcrun simctl terminate "$booted_udid" "$bundle" >/dev/null 2>&1 || true
  xcrun simctl uninstall "$booted_udid" "$bundle" >/dev/null
  printf '%s\n' "$expected"
}

run_app "$output_root/SemapraxPrivateSwift-Onone.app" dev.semaprax.private.swift.o0 "$expected_o0"
run_app "$output_root/SemapraxPrivateSwift-O.app" dev.semaprax.private.swift.o2 "$expected_o2"
