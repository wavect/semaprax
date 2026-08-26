#!/usr/bin/env bash
set -euo pipefail

readonly android_ndk_version="27.2.12479018"
readonly android_api_level="35"
readonly android_minimum_api="28"
readonly package_name="dev.semaprax.runtime"
readonly instrumentation_name="dev.semaprax.instrumentation.ContractInstrumentation"
readonly expected_result="SEMAPRAX_ANDROID_JNI_V1_OK api=35 abi=x86_64 o0=explicit o2=cleaner handle=0001000001000001 declared=0000006b00000007 unexpected=0000004500000001 finalizers=1:13,0:11 publication=no-owned allocations=0 handles=0 rf=1 om=1 ca=1 ef=1"

for command in adb cargo file find gradle grep mktemp realpath sed tr; do
  if ! which "$command" >/dev/null 2>&1; then
    echo "required Android JNI application tool is unavailable: $command" >&2
    exit 1
  fi
done
if ! gradle --version | grep -E '^Gradle 9[.]' >/dev/null; then
  echo "runner Gradle is not the source-locked major version 9" >&2
  exit 1
fi
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
readonly llvm_readelf="$ndk_bin/llvm-readelf"
for tool in "$x86_clang" "$arm64_clang" "$llvm_readelf"; do
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

scratch="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/semaprax-android-jni-v3.XXXXXX")"
cleanup() {
  case "$scratch" in
    "${RUNNER_TEMP:-${TMPDIR:-/tmp}}"/semaprax-android-jni-v3.*)
      rm -rf -- "$scratch"
      ;;
    *)
      echo "refusing to remove unexpected Android JNI scratch path: $scratch" >&2
      ;;
  esac
}
trap cleanup EXIT INT TERM

readonly x86_discard_source="$scratch/x86-discard.c"
readonly arm64_discard_source="$scratch/arm64-discard.c"
readonly x86_requires_false_source="$scratch/x86-requires-false.c"
readonly arm64_requires_false_source="$scratch/arm64-requires-false.c"
readonly x86_identity_max_source="$scratch/x86-identity-max.c"
readonly arm64_identity_max_source="$scratch/arm64-identity-max.c"
readonly x86_checked_source="$scratch/x86-checked-add-overflow.c"
readonly arm64_checked_source="$scratch/arm64-checked-add-overflow.c"
readonly x86_ensures_source="$scratch/x86-ensures-false.c"
readonly arm64_ensures_source="$scratch/arm64-ensures-false.c"
readonly x86_jni_source="$scratch/x86-jni.c"
readonly arm64_jni_source="$scratch/arm64-jni.c"
cargo run --locked -p semaprax-native-host \
  --features unstable-android-jni-harness \
  --bin private-android-jni-v3-fixture -- \
  "$x86_discard_source" "$arm64_discard_source" \
  "$x86_requires_false_source" "$arm64_requires_false_source" \
  "$x86_identity_max_source" "$arm64_identity_max_source" \
  "$x86_checked_source" "$arm64_checked_source" \
  "$x86_ensures_source" "$arm64_ensures_source" \
  "$x86_jni_source" "$arm64_jni_source"
for source in \
  "$x86_discard_source" "$arm64_discard_source" \
  "$x86_requires_false_source" "$arm64_requires_false_source" \
  "$x86_identity_max_source" "$arm64_identity_max_source" \
  "$x86_checked_source" "$arm64_checked_source" \
  "$x86_ensures_source" "$arm64_ensures_source" \
  "$x86_jni_source" "$arm64_jni_source"; do
  test -s "$source"
done

export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$x86_clang"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$arm64_clang"
for target in x86_64-linux-android aarch64-linux-android; do
  cargo check --locked -p semaprax-native-host --target "$target" --all-targets \
    --features unstable-android-jni-harness
  cargo rustc --locked -p semaprax-native-host \
    --target "$target" --release \
    --features unstable-android-jni-harness \
    --lib --crate-type staticlib
done

readonly native_dir="$scratch/native"
mkdir -p "$native_dir"
readonly packaged_provider_o0="$native_dir/libsemaprax_provider_o0.so"
readonly packaged_provider_o2="$native_dir/libsemaprax_provider_o2.so"
readonly packaged_provider_rf_o0="$native_dir/libsemaprax_provider_rf_o0.so"
readonly packaged_provider_rf_o2="$native_dir/libsemaprax_provider_rf_o2.so"
readonly packaged_provider_om_o0="$native_dir/libsemaprax_provider_om_o0.so"
readonly packaged_provider_om_o2="$native_dir/libsemaprax_provider_om_o2.so"
readonly packaged_provider_ca_o0="$native_dir/libsemaprax_provider_ca_o0.so"
readonly packaged_provider_ca_o2="$native_dir/libsemaprax_provider_ca_o2.so"
readonly packaged_provider_ef_o0="$native_dir/libsemaprax_provider_ef_o0.so"
readonly packaged_provider_ef_o2="$native_dir/libsemaprax_provider_ef_o2.so"
readonly packaged_jni="$native_dir/libsemaprax_jni.so"
readonly x86_host="target/x86_64-linux-android/release/libsemaprax_native_host.a"
readonly arm64_host="target/aarch64-linux-android/release/libsemaprax_native_host.a"
test -s "$x86_host"
test -s "$arm64_host"
readonly export_map="$scratch/jni.exports"
printf '%s\n' '{ global: JNI_OnLoad; local: *; };' >"$export_map"

for optimization in 0 2; do
  "$x86_clang" -std=c11 "-O$optimization" -fPIC -shared \
    -Wall -Wextra -Werror -pedantic \
    "$x86_discard_source" -o "$native_dir/libsemaprax_provider_o$optimization.so"
  "$x86_clang" -std=c11 "-O$optimization" -fPIC -shared \
    -Wall -Wextra -Werror -pedantic \
    "$x86_requires_false_source" -o "$native_dir/libsemaprax_provider_rf_o$optimization.so"
  "$x86_clang" -std=c11 "-O$optimization" -fPIC -shared \
    -Wall -Wextra -Werror -pedantic \
    "$x86_identity_max_source" -o "$native_dir/libsemaprax_provider_om_o$optimization.so"
  "$x86_clang" -std=c11 "-O$optimization" -fPIC -shared \
    -Wall -Wextra -Werror -pedantic \
    "$x86_checked_source" -o "$native_dir/libsemaprax_provider_ca_o$optimization.so"
  "$x86_clang" -std=c11 "-O$optimization" -fPIC -shared \
    -Wall -Wextra -Werror -pedantic \
    "$x86_ensures_source" -o "$native_dir/libsemaprax_provider_ef_o$optimization.so"
  "$arm64_clang" -std=c11 "-O$optimization" -fPIC -shared \
    -Wall -Wextra -Werror -pedantic \
    "$arm64_discard_source" -o "$scratch/libsemaprax_provider_arm64_o$optimization.so"
  "$arm64_clang" -std=c11 "-O$optimization" -fPIC -shared \
    -Wall -Wextra -Werror -pedantic \
    "$arm64_requires_false_source" -o "$scratch/libsemaprax_provider_rf_arm64_o$optimization.so"
  "$arm64_clang" -std=c11 "-O$optimization" -fPIC -shared \
    -Wall -Wextra -Werror -pedantic \
    "$arm64_identity_max_source" -o "$scratch/libsemaprax_provider_om_arm64_o$optimization.so"
  "$arm64_clang" -std=c11 "-O$optimization" -fPIC -shared \
    -Wall -Wextra -Werror -pedantic \
    "$arm64_checked_source" -o "$scratch/libsemaprax_provider_ca_arm64_o$optimization.so"
  "$arm64_clang" -std=c11 "-O$optimization" -fPIC -shared \
    -Wall -Wextra -Werror -pedantic \
    "$arm64_ensures_source" -o "$scratch/libsemaprax_provider_ef_arm64_o$optimization.so"
done
test -s "$packaged_provider_o0"
test -s "$packaged_provider_o2"
test -s "$packaged_provider_rf_o0"
test -s "$packaged_provider_rf_o2"
test -s "$packaged_provider_om_o0"
test -s "$packaged_provider_om_o2"
test -s "$packaged_provider_ca_o0"
test -s "$packaged_provider_ca_o2"
test -s "$packaged_provider_ef_o0"
test -s "$packaged_provider_ef_o2"

"$x86_clang" -std=c11 -O2 -fPIC -shared \
  -Wall -Wextra -Werror -pedantic -fvisibility=hidden \
  "$x86_jni_source" "$x86_host" \
  -Wl,--no-undefined -Wl,--fatal-warnings -Wl,-z,relro -Wl,-z,now \
  -Wl,--version-script="$export_map" \
  -pthread -ldl -llog -lm -o "$packaged_jni"
"$arm64_clang" -std=c11 -O2 -fPIC -shared \
  -Wall -Wextra -Werror -pedantic -fvisibility=hidden \
  "$arm64_jni_source" "$arm64_host" \
  -Wl,--no-undefined -Wl,--fatal-warnings -Wl,-z,relro -Wl,-z,now \
  -Wl,--version-script="$export_map" \
  -pthread -ldl -llog -lm -o "$scratch/libsemaprax_jni_arm64.so"

for artifact in "$native_dir"/*.so; do
  file "$artifact" | grep -F 'ELF 64-bit LSB' >/dev/null
  "$llvm_readelf" -h "$artifact" | grep -F 'Machine:' | grep -F 'Advanced Micro Devices X86-64' >/dev/null
  dynamic="$($llvm_readelf -d "$artifact")"
  if grep -E 'RPATH|RUNPATH|AI-Lang|target/' <<<"$dynamic" >/dev/null; then
    echo "Android JNI x86_64 artifact contains a forbidden path" >&2
    exit 1
  fi
done
for artifact in "$scratch"/libsemaprax_*_arm64*.so; do
  file "$artifact" | grep -F 'ELF 64-bit LSB' >/dev/null
  "$llvm_readelf" -h "$artifact" | grep -F 'Machine:' | grep -F 'AArch64' >/dev/null
  dynamic="$($llvm_readelf -d "$artifact")"
  if grep -E 'RPATH|RUNPATH|AI-Lang|target/' <<<"$dynamic" >/dev/null; then
    echo "Android JNI arm64 artifact contains a forbidden path" >&2
    exit 1
  fi
done
exports="$($llvm_readelf --dyn-syms "$packaged_jni")"
if ! grep -F 'JNI_OnLoad' <<<"$exports" >/dev/null; then
  echo "Android JNI shim does not export JNI_OnLoad" >&2
  exit 1
fi
mapfile -t defined_exports < <(
  "$llvm_readelf" --dyn-syms --wide "$packaged_jni" |
    awk '$7 != "UND" && $5 == "GLOBAL" && $8 != "" { print $8 }' |
    LC_ALL=C sort -u
)
if [[ "${defined_exports[*]}" != "JNI_OnLoad" ]]; then
  echo "Android JNI shim exported-symbol allowlist is not exact" >&2
  printf '%s\n' "${defined_exports[@]}" >&2
  exit 1
fi
mapfile -t jni_needed < <(
  "$llvm_readelf" -d "$packaged_jni" |
    sed -n 's/.*Shared library: \[\([^]]*\)\].*/\1/p' |
    LC_ALL=C sort -u
)
for needed in "${jni_needed[@]}"; do
  case "$needed" in
    libc.so|libdl.so|liblog.so|libm.so) ;;
    *)
      echo "Android JNI shim has an unexpected dynamic dependency: $needed" >&2
      exit 1
      ;;
  esac
done

SEMAPRAX_ANDROID_JNI_NATIVE_DIR="$native_dir" \
  gradle --offline --no-daemon --console=plain --stacktrace \
  -p platform-tests/android-jni check
readonly apk="platform-tests/android-jni/build/outputs/semaprax-android-jni.apk"
test -s "$apk"

if adb shell pm path "$package_name" | grep -F 'package:' >/dev/null; then
  adb uninstall "$package_name" >/dev/null
fi
adb install --no-streaming "$apk" >/dev/null
instrumentation_output="$(adb shell am instrument -w \
  "$package_name/$instrumentation_name" | tr -d '\r')"
if ! grep -F 'INSTRUMENTATION_CODE: -1' <<<"$instrumentation_output" >/dev/null; then
  echo "$instrumentation_output" >&2
  echo "Android JNI instrumentation did not report success" >&2
  exit 1
fi
result="$(adb shell run-as "$package_name" cat files/semaprax-android-jni-v1.txt | tr -d '\r')"
if [[ "$result" != "$expected_result" ]]; then
  echo "Android JNI app-private result is not exact" >&2
  exit 1
fi
printf '%s\n' "$expected_result"
