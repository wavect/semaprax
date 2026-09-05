#!/usr/bin/env bash
set -euo pipefail

readonly android_ndk_version="27.2.12479018"
readonly android_api_level="35"
readonly android_minimum_api="28"

for command in adb cargo file find grep mktemp sed tr; do
  if ! which "$command" >/dev/null 2>&1; then
    echo "required Android emulator tool is unavailable: $command" >&2
    exit 1
  fi
done

if [[ -z "${ANDROID_SDK_ROOT:-}" ]]; then
  echo "ANDROID_SDK_ROOT is required" >&2
  exit 1
fi
readonly ndk_root="$ANDROID_SDK_ROOT/ndk/$android_ndk_version"
if [[ ! -d "$ndk_root" ]]; then
  echo "required pinned Android NDK is unavailable: $ndk_root" >&2
  exit 1
fi

mapfile -t ndk_prebuilts < <(find "$ndk_root/toolchains/llvm/prebuilt" -mindepth 1 -maxdepth 1 -type d -print)
if [[ "${#ndk_prebuilts[@]}" -ne 1 ]]; then
  echo "expected exactly one Android NDK host prebuilt" >&2
  exit 1
fi
readonly ndk_bin="${ndk_prebuilts[0]}/bin"
readonly x86_clang="$ndk_bin/x86_64-linux-android${android_minimum_api}-clang"
readonly arm64_clang="$ndk_bin/aarch64-linux-android${android_minimum_api}-clang"
readonly llvm_ar="$ndk_bin/llvm-ar"
readonly llvm_readelf="$ndk_bin/llvm-readelf"
for tool in "$x86_clang" "$arm64_clang" "$llvm_ar" "$llvm_readelf"; do
  if [[ ! -x "$tool" ]]; then
    echo "required pinned Android NDK tool is unavailable: $tool" >&2
    exit 1
  fi
done

if [[ "$(adb get-state | tr -d '\r')" != "device" ]]; then
  echo "an online Android emulator is required" >&2
  exit 1
fi
if [[ "$(adb shell getprop ro.build.version.sdk | tr -d '\r')" != "$android_api_level" ]]; then
  echo "Android emulator API level does not match the pinned runtime" >&2
  exit 1
fi
if [[ "$(adb shell getprop ro.product.cpu.abi | tr -d '\r')" != "x86_64" ]]; then
  echo "Android emulator primary ABI is not x86_64" >&2
  exit 1
fi
case "$(adb shell uname -m | tr -d '\r')" in
  x86_64) ;;
  *) echo "Android emulator kernel architecture is not x86_64" >&2; exit 1 ;;
esac

scratch="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/semaprax-android-v3.XXXXXX")"
remote_scratch=""
cleanup() {
  if [[ "$remote_scratch" =~ ^/data/local/tmp/semaprax-android-v3\.[A-Za-z0-9]+$ ]]; then
    adb shell rm -rf -- "$remote_scratch" >/dev/null 2>&1 || true
  fi
  case "$scratch" in
    "${RUNNER_TEMP:-${TMPDIR:-/tmp}}"/semaprax-android-v3.*)
      rm -rf -- "$scratch"
      ;;
    *)
      echo "refusing to remove unexpected Android scratch path: $scratch" >&2
      ;;
  esac
}
trap cleanup EXIT INT TERM

x86_provider_source="$scratch/semaprax-android-v3-x86-provider.c"
arm64_provider_source="$scratch/semaprax-android-v3-arm64-provider.c"
runner_source="$scratch/semaprax-android-v3-runner.c"
cargo run --locked -p semaprax-native-host \
  --features unstable-android-emulator-harness \
  --bin private-android-emulator-v3-fixture -- \
  "$x86_provider_source" "$arm64_provider_source" "$runner_source"
test -s "$x86_provider_source"
test -s "$arm64_provider_source"
test -s "$runner_source"

export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$x86_clang"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$arm64_clang"
export CC_x86_64_linux_android="$x86_clang"
export CC_aarch64_linux_android="$arm64_clang"
export AR_x86_64_linux_android="$llvm_ar"
export AR_aarch64_linux_android="$llvm_ar"

for target in x86_64-linux-android aarch64-linux-android; do
  cargo check --locked -p semaprax-native-loader --target "$target" --all-targets
  cargo check --locked -p semaprax-native-host --target "$target" --all-targets \
    --features unstable-android-emulator-harness
  loader_tree="$(cargo tree --locked -p semaprax-native-loader --target "$target" -e normal)"
  host_tree="$(cargo tree --locked -p semaprax-native-host --target "$target" -e normal)"
  if ! grep -F 'libloading v0.9.0' <<<"$loader_tree" >/dev/null \
    || ! grep -F 'libloading v0.9.0' <<<"$host_tree" >/dev/null; then
    echo "Android dynamic loader/host target lost its exact libloading dependency: $target" >&2
    exit 1
  fi
done

cargo rustc --locked -p semaprax-native-host \
  --target x86_64-linux-android \
  --release \
  --features unstable-android-emulator-harness \
  --lib --crate-type staticlib

host_archive="target/x86_64-linux-android/release/libsemaprax_native_host.a"
test -s "$host_archive"

runner="$scratch/semaprax-android-v3-runner"
"$x86_clang" \
  -std=c11 -O2 -fPIE -pie -Wall -Wextra -Werror -pedantic \
  "$runner_source" "$host_archive" -pthread -ldl -llog -lm \
  -o "$runner"

for optimization in 0 2; do
  provider="$scratch/libsemaprax_android_v3_O$optimization.so"
  "$x86_clang" \
    -std=c11 "-O$optimization" -fPIC -shared \
    -Wall -Wextra -Werror -pedantic \
    "$x86_provider_source" -o "$provider"
done

arm64_provider="$scratch/libsemaprax_android_v3_arm64.so"
"$arm64_clang" \
  -std=c11 -O2 -fPIC -shared \
  -Wall -Wextra -Werror -pedantic \
  "$arm64_provider_source" -o "$arm64_provider"

for artifact in \
  "$runner" \
  "$scratch/libsemaprax_android_v3_O0.so" \
  "$scratch/libsemaprax_android_v3_O2.so"
do
  file "$artifact" | grep -F 'ELF 64-bit LSB' >/dev/null
  "$llvm_readelf" -h "$artifact" | grep -F 'Machine:' | grep -F 'Advanced Micro Devices X86-64' >/dev/null
  dynamic="$($llvm_readelf -d "$artifact")"
  if grep -E 'RPATH|RUNPATH|libloading|semaprax_native|AI-Lang|@rpath|target/' <<<"$dynamic" >/dev/null; then
    echo "Android evidence artifact contains a forbidden dynamic dependency or search path" >&2
    exit 1
  fi
done
file "$arm64_provider" | grep -F 'ELF 64-bit LSB' >/dev/null
"$llvm_readelf" -h "$arm64_provider" | grep -F 'Machine:' | grep -F 'AArch64' >/dev/null
arm64_dynamic="$($llvm_readelf -d "$arm64_provider")"
if grep -E 'RPATH|RUNPATH|libloading|semaprax_native|AI-Lang|@rpath|target/' <<<"$arm64_dynamic" >/dev/null; then
  echo "Android arm64 provider contains a forbidden dynamic dependency or search path" >&2
  exit 1
fi

remote_scratch="$(adb shell mktemp -d /data/local/tmp/semaprax-android-v3.XXXXXX | tr -d '\r')"
if [[ ! "$remote_scratch" =~ ^/data/local/tmp/semaprax-android-v3\.[A-Za-z0-9]+$ ]]; then
  echo "Android emulator returned an unexpected scratch path" >&2
  remote_scratch=""
  exit 1
fi
adb push "$runner" "$remote_scratch/runner" >/dev/null
adb push "$scratch/libsemaprax_android_v3_O0.so" "$remote_scratch/provider-O0.so" >/dev/null
adb push "$scratch/libsemaprax_android_v3_O2.so" "$remote_scratch/provider-O2.so" >/dev/null
adb shell chmod 700 "$remote_scratch/runner"
adb shell chmod 600 "$remote_scratch/provider-O0.so" "$remote_scratch/provider-O2.so"

for optimization in 0 2; do
  provider="$(adb shell realpath "$remote_scratch/provider-O$optimization.so" | tr -d '\r')"
  if [[ "$provider" != "$remote_scratch/provider-O$optimization.so" ]]; then
    echo "Android provider path did not resolve to the exact pushed image" >&2
    exit 1
  fi
  marker="$remote_scratch/finalizers-O$optimization"
  expected="SEMAPRAX_ANDROID_EMULATOR_V3_OK O$optimization target=x86_64-android finalizers=1:13,0:11 publication=no-owned allocations=0"
  output="$(adb shell "SEMAPRAX_ANDROID_V3_MARKER='$marker' '$remote_scratch/runner' '$provider' 'O$optimization'" | tr -d '\r')"
  grep -Fx "$expected" <<<"$output" >/dev/null
  finalizers="$(adb shell cat "$marker" | tr -d '\r')"
  if [[ "$finalizers" != $'1:13\n0:11' ]]; then
    echo "Android physical finalizer evidence is not exact at O$optimization" >&2
    exit 1
  fi
done
