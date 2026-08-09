#!/usr/bin/env bash
set -euo pipefail

readonly minimum_ios_version="15.0"
readonly project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"

for command in codesign file grep lipo mktemp nm otool plutil sed swiftc vtool xcodebuild xcrun; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "required private Swift packaging tool is unavailable: $command" >&2
    exit 1
  fi
done
for variable in \
  SEMAPRAX_IOS_SWIFT_DEVICE_SOURCE \
  SEMAPRAX_IOS_SWIFT_SIM_ARM64_SOURCE \
  SEMAPRAX_IOS_SWIFT_SIM_X86_64_SOURCE \
  SEMAPRAX_IOS_SWIFT_DEVICE_HOST \
  SEMAPRAX_IOS_SWIFT_SIM_ARM64_HOST \
  SEMAPRAX_IOS_SWIFT_SIM_X86_64_HOST
do
  if [[ -z "${!variable:-}" ]]; then
    echo "required generated Swift packaging input is absent: $variable" >&2
    exit 1
  fi
done

readonly device_sdk="$(xcrun --sdk iphoneos --show-sdk-path)"
readonly simulator_sdk="$(xcrun --sdk iphonesimulator --show-sdk-path)"
test -d "$device_sdk"
test -d "$simulator_sdk"
readonly device_target="arm64-apple-ios${minimum_ios_version}"
readonly simulator_arm64_target="arm64-apple-ios${minimum_ios_version}-simulator"
readonly simulator_x86_target="x86_64-apple-ios${minimum_ios_version}-simulator"

readonly work="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/semaprax-ios-swift-package.XXXXXX")"
cleanup() {
  case "$work" in
    "${RUNNER_TEMP:-${TMPDIR:-/tmp}}"/semaprax-ios-swift-package.*) rm -rf -- "$work" ;;
    *) echo "refusing to remove unexpected private Swift packaging path: $work" >&2 ;;
  esac
}
trap cleanup EXIT INT TERM

copy_input() {
  local input="$1"
  local output="$2"
  if [[ ! -f "$input" || -L "$input" ]]; then
    echo "private Swift packaging input is absent or linked: $input" >&2
    exit 1
  fi
  cp "$input" "$output"
}
copy_input "$SEMAPRAX_IOS_SWIFT_DEVICE_SOURCE" "$work/device.c"
copy_input "$SEMAPRAX_IOS_SWIFT_SIM_ARM64_SOURCE" "$work/simulator-arm64.c"
copy_input "$SEMAPRAX_IOS_SWIFT_SIM_X86_64_SOURCE" "$work/simulator-x86_64.c"
copy_input "$SEMAPRAX_IOS_SWIFT_DEVICE_HOST" "$work/host-device.a"
copy_input "$SEMAPRAX_IOS_SWIFT_SIM_ARM64_HOST" "$work/host-simulator-arm64.a"
copy_input "$SEMAPRAX_IOS_SWIFT_SIM_X86_64_HOST" "$work/host-simulator-x86_64.a"

compile_fixture() {
  local sdk="$1"
  local target="$2"
  local optimization="$3"
  local source="$4"
  local output="$5"
  xcrun --sdk "$sdk" clang -target "$target" -isysroot "$(xcrun --sdk "$sdk" --show-sdk-path)" \
    -std=c11 "-O$optimization" -fvisibility=hidden \
    -Wall -Wextra -Werror -pedantic -c "$source" -o "$output"
}

compile_fixture iphoneos "$device_target" 2 "$work/device.c" "$work/device-o2.o"
compile_fixture iphonesimulator "$simulator_arm64_target" 0 "$work/simulator-arm64.c" "$work/simulator-arm64-o0.o"
compile_fixture iphonesimulator "$simulator_arm64_target" 2 "$work/simulator-arm64.c" "$work/simulator-arm64-o2.o"
compile_fixture iphonesimulator "$simulator_x86_target" 2 "$work/simulator-x86_64.c" "$work/simulator-x86_64-o2.o"

combine_archive() {
  xcrun libtool -static -o "$3" "$1" "$2"
  test -s "$3"
}
combine_archive "$work/device-o2.o" "$work/host-device.a" "$work/lib-device.a"
combine_archive "$work/simulator-arm64-o0.o" "$work/host-simulator-arm64.a" "$work/lib-simulator-arm64-o0.a"
combine_archive "$work/simulator-arm64-o2.o" "$work/host-simulator-arm64.a" "$work/lib-simulator-arm64-o2.a"
combine_archive "$work/simulator-x86_64-o2.o" "$work/host-simulator-x86_64.a" "$work/lib-simulator-x86_64-o2.a"
xcrun lipo -create "$work/lib-simulator-arm64-o2.a" "$work/lib-simulator-x86_64-o2.a" \
  -output "$work/lib-simulator-universal.a"

test "$(xcrun lipo -archs "$work/lib-device.a")" = "arm64"
test "$(xcrun lipo -archs "$work/lib-simulator-arm64-o0.a")" = "arm64"
test "$(xcrun lipo -archs "$work/lib-simulator-universal.a")" = "x86_64 arm64" || \
  test "$(xcrun lipo -archs "$work/lib-simulator-universal.a")" = "arm64 x86_64"
vtool -show-build "$work/simulator-arm64-o2.o" | grep -F 'platform IOSSIMULATOR' >/dev/null
vtool -show-build "$work/device-o2.o" | grep -F 'platform IOS' >/dev/null

mkdir -p "$work/xc-device/Headers" "$work/xc-simulator/Headers"
cp "$project_root/include/SemapraxPrivateSwift.h" "$work/xc-device/Headers/"
cp "$project_root/include/module.modulemap" "$work/xc-device/Headers/"
cp "$project_root/include/SemapraxPrivateSwift.h" "$work/xc-simulator/Headers/"
cp "$project_root/include/module.modulemap" "$work/xc-simulator/Headers/"
cp "$work/lib-device.a" "$work/xc-device/libSemapraxPrivateSwift.a"
cp "$work/lib-simulator-universal.a" "$work/xc-simulator/libSemapraxPrivateSwift.a"

xcodebuild -create-xcframework \
  -library "$work/xc-device/libSemapraxPrivateSwift.a" -headers "$work/xc-device/Headers" \
  -library "$work/xc-simulator/libSemapraxPrivateSwift.a" -headers "$work/xc-simulator/Headers" \
  -output "$work/SemapraxPrivateSwift.xcframework" >/dev/null
readonly xc_plist="$work/SemapraxPrivateSwift.xcframework/Info.plist"
test -f "$xc_plist"
plutil -convert xml1 -o "$work/xc-info.xml" "$xc_plist"
grep -F '<key>SupportedPlatform</key>' "$work/xc-info.xml" >/dev/null
grep -F '<key>SupportedPlatformVariant</key>' "$work/xc-info.xml" >/dev/null
grep -F '<string>ios</string>' "$work/xc-info.xml" >/dev/null
grep -F '<string>simulator</string>' "$work/xc-info.xml" >/dev/null
grep -F '<string>arm64</string>' "$work/xc-info.xml" >/dev/null
grep -F '<string>x86_64</string>' "$work/xc-info.xml" >/dev/null

cat >"$work/app.exports" <<'EOF'
_main
_spx_private_apple_swift_fixture_v1_open
_spx_private_apple_swift_v1_adopt_pair
_spx_private_apple_swift_v1_close_runtime
_spx_private_apple_swift_v1_consume
EOF
expected_app_exports='_spx_private_apple_swift_fixture_v1_open
_spx_private_apple_swift_v1_adopt_pair
_spx_private_apple_swift_v1_close_runtime
_spx_private_apple_swift_v1_consume'

swift_sources=("$project_root"/Sources/*.swift)
if [[ "${#swift_sources[@]}" -lt 5 ]]; then
  echo "private Swift source set is incomplete" >&2
  exit 1
fi

build_executable() {
  local sdk="$1"
  local target="$2"
  local optimization="$3"
  local mode_define="$4"
  local archive="$5"
  local output="$6"
  xcrun --sdk "$sdk" swiftc \
    -target "$target" -sdk "$(xcrun --sdk "$sdk" --show-sdk-path)" \
    -module-name SemapraxPrivateSwiftContractApp \
    -swift-version 6 -strict-concurrency=complete -warnings-as-errors -parse-as-library \
    "-$optimization" -D "$mode_define" \
    -I "$project_root/include" \
    "${swift_sources[@]}" "$archive" \
    -framework Foundation -framework UIKit -framework Security -liconv -lm \
    -Xlinker -dead_strip -Xlinker -exported_symbols_list -Xlinker "$work/app.exports" -o "$output"
}

# Compile and inspect a real device-target executable, without claiming device execution.
build_executable iphoneos "$device_target" O "SEMAPRAX_DEINIT" \
  "$work/lib-device.a" "$work/device-compile-proof"
file "$work/device-compile-proof" | grep -F 'Mach-O 64-bit executable arm64' >/dev/null
vtool -show-build "$work/device-compile-proof" | grep -F 'platform IOS' >/dev/null

make_app() {
  local optimization="$1"
  local mode="$2"
  local define="$3"
  local archive="$4"
  local bundle="$5"
  local app="$work/SemapraxPrivateSwift-$optimization.app"
  mkdir -p "$app"
  build_executable iphonesimulator "$simulator_arm64_target" "$optimization" "$define" \
    "$archive" "$app/SemapraxPrivateSwift"
  sed "s/__SEMAPRAX_BUNDLE_IDENTIFIER__/$bundle/g" "$project_root/Info.plist.in" >"$app/Info.plist"
  plutil -lint "$app/Info.plist" >/dev/null
  file "$app/SemapraxPrivateSwift" | grep -F 'Mach-O 64-bit executable arm64' >/dev/null
  vtool -show-build "$app/SemapraxPrivateSwift" | grep -F 'platform IOSSIMULATOR' >/dev/null
  otool -hv "$app/SemapraxPrivateSwift" | grep -E 'ARM64|arm64' >/dev/null
  linked_images="$(otool -L "$app/SemapraxPrivateSwift")"
  if grep -E 'AI-Lang|target/|@loader_path|@executable_path|libloading' <<<"$linked_images" >/dev/null; then
    echo "private Swift app linked a forbidden image or source path" >&2
    exit 1
  fi
  while IFS= read -r image; do
    case "$image" in
      /System/Library/*|/usr/lib/*|@rpath/libswift*) ;;
      *) echo "private Swift app has an unexpected dependency: $image" >&2; exit 1 ;;
    esac
  done < <(sed -n '2,$s/^[[:space:]]*\([^[:space:]]*\).*/\1/p' <<<"$linked_images")
  private_exports="$(nm -gjU "$app/SemapraxPrivateSwift" | grep '^_spx_private_apple_swift' | LC_ALL=C sort -u)"
  if [[ "$private_exports" != "$expected_app_exports" ]]; then
    echo "private Swift app export allowlist changed" >&2
    exit 1
  fi
  codesign --force --sign - --timestamp=none "$app"
  codesign --verify --strict "$app"
  printf '%s\n' "$app"
}

make_app Onone explicit SEMAPRAX_EXPLICIT \
  "$work/lib-simulator-arm64-o0.a" dev.semaprax.private.swift.o0
readonly app_o0="$work/SemapraxPrivateSwift-Onone.app"
make_app O deinit SEMAPRAX_DEINIT \
  "$work/lib-simulator-arm64-o2.a" dev.semaprax.private.swift.o2
readonly app_o2="$work/SemapraxPrivateSwift-O.app"

readonly output_root="$project_root/build"
if [[ -e "$output_root" && ( ! -d "$output_root" || -L "$output_root" ) ]]; then
  echo "refusing to replace unexpected private Swift output root" >&2
  exit 1
fi
rm -rf -- "$output_root"
mkdir -p "$output_root"
cp -R "$work/SemapraxPrivateSwift.xcframework" "$output_root/"
cp -R "$app_o0" "$output_root/"
cp -R "$app_o2" "$output_root/"

printf '%s\n' "$output_root"
