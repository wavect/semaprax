#!/usr/bin/env bash
set -euo pipefail

readonly package_name="dev.semaprax.runtime"
readonly compile_api="35"
readonly build_tools_version="35.0.0"
readonly minimum_api="28"

for command in find java keytool kotlinc mktemp readlink realpath unzip zip; do
  if ! which "$command" >/dev/null 2>&1; then
    echo "required Android JNI packaging tool is unavailable: $command" >&2
    exit 1
  fi
done

if [[ -z "${ANDROID_SDK_ROOT:-}" ]]; then
  echo "ANDROID_SDK_ROOT is required" >&2
  exit 1
fi
if [[ -z "${SEMAPRAX_ANDROID_JNI_NATIVE_DIR:-}" ]]; then
  echo "SEMAPRAX_ANDROID_JNI_NATIVE_DIR is required" >&2
  exit 1
fi

readonly project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly sdk_build_tools="$ANDROID_SDK_ROOT/build-tools/$build_tools_version"
readonly android_jar="$ANDROID_SDK_ROOT/platforms/android-$compile_api/android.jar"
for tool in aapt2 apksigner d8 zipalign; do
  if [[ ! -x "$sdk_build_tools/$tool" ]]; then
    echo "required pinned Android build tool is unavailable: $sdk_build_tools/$tool" >&2
    exit 1
  fi
done
if [[ ! -f "$android_jar" ]]; then
  echo "required pinned Android platform is unavailable: $android_jar" >&2
  exit 1
fi

readonly native_dir="$(realpath "${SEMAPRAX_ANDROID_JNI_NATIVE_DIR:?}")"
if [[ ! -d "$native_dir" ]]; then
  echo "generated Android JNI native directory is unavailable: $native_dir" >&2
  exit 1
fi
readonly native_names=(
  libsemaprax_jni.so
  libsemaprax_provider_o0.so
  libsemaprax_provider_o2.so
  libsemaprax_provider_rf_o0.so
  libsemaprax_provider_rf_o2.so
  libsemaprax_provider_om_o0.so
  libsemaprax_provider_om_o2.so
)
mapfile -t native_files < <(find "$native_dir" -mindepth 1 -maxdepth 1 -type f -name '*.so' -print | LC_ALL=C sort)
if [[ "${#native_files[@]}" -ne "${#native_names[@]}" ]]; then
  echo "generated Android JNI directory must contain exactly seven shared libraries" >&2
  exit 1
fi
for name in "${native_names[@]}"; do
  file="$native_dir/$name"
  if [[ ! -f "$file" || -L "$file" || "$(realpath "$file")" != "$file" ]]; then
    echo "generated Android JNI artifact is absent, linked, or non-canonical: $file" >&2
    exit 1
  fi
done

readonly kotlinc_path="$(readlink -f "$(which kotlinc)")"
readonly kotlin_home="$(cd "$(dirname "$kotlinc_path")/.." && pwd -P)"
readonly kotlin_stdlib="$kotlin_home/lib/kotlin-stdlib.jar"
readonly kotlin_stdlib_jdk7="$kotlin_home/lib/kotlin-stdlib-jdk7.jar"
readonly kotlin_stdlib_jdk8="$kotlin_home/lib/kotlin-stdlib-jdk8.jar"
for library in "$kotlin_stdlib" "$kotlin_stdlib_jdk7" "$kotlin_stdlib_jdk8"; do
  if [[ ! -f "$library" ]]; then
    echo "runner Kotlin distribution is incomplete: $library" >&2
    exit 1
  fi
done
if ! kotlinc -version 2>&1 | grep -E 'kotlinc-jvm 2[.]' >/dev/null; then
  echo "runner Kotlin compiler is not the source-locked major version 2" >&2
  exit 1
fi

readonly output_root="$project_root/build/outputs"
mkdir -p "$output_root"
work="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/semaprax-android-jni-package.XXXXXX")"
cleanup() {
  case "$work" in
    "${RUNNER_TEMP:-${TMPDIR:-/tmp}}"/semaprax-android-jni-package.*)
      rm -rf -- "$work"
      ;;
    *)
      echo "refusing to remove unexpected Android JNI packaging path: $work" >&2
      ;;
  esac
}
trap cleanup EXIT INT TERM

mapfile -t kotlin_sources < <(find "$project_root/src" -type f -name '*.kt' -print | LC_ALL=C sort)
if [[ "${#kotlin_sources[@]}" -lt 7 ]]; then
  echo "private Android JNI Kotlin source set is incomplete" >&2
  exit 1
fi

readonly classes_jar="$work/classes.jar"
kotlinc \
  -Werror \
  -jvm-target 11 \
  -language-version 2.2 \
  -api-version 2.2 \
  -no-reflect \
  -classpath "$android_jar:$kotlin_stdlib:$kotlin_stdlib_jdk7:$kotlin_stdlib_jdk8" \
  -d "$classes_jar" \
  "${kotlin_sources[@]}"
test -s "$classes_jar"

mkdir -p "$work/dex"
"$sdk_build_tools/d8" \
  --min-api "$minimum_api" \
  --lib "$android_jar" \
  --output "$work/dex" \
  "$classes_jar" "$kotlin_stdlib" "$kotlin_stdlib_jdk7" "$kotlin_stdlib_jdk8"
test -s "$work/dex/classes.dex"

readonly base_apk="$work/base.apk"
"$sdk_build_tools/aapt2" link \
  -I "$android_jar" \
  --manifest "$project_root/AndroidManifest.xml" \
  --min-sdk-version "$minimum_api" \
  --target-sdk-version "$compile_api" \
  --version-code 1 \
  --version-name 0.0.0-private \
  -o "$base_apk"
test -s "$base_apk"

mkdir -p "$work/payload/lib/x86_64"
cp "$work/dex/classes.dex" "$work/payload/classes.dex"
for name in "${native_names[@]}"; do
  cp "$native_dir/$name" "$work/payload/lib/x86_64/$name"
done
(
  cd "$work/payload"
  zip -q -X -0 "$base_apk" classes.dex \
    lib/x86_64/libsemaprax_jni.so \
    lib/x86_64/libsemaprax_provider_o0.so \
    lib/x86_64/libsemaprax_provider_o2.so \
    lib/x86_64/libsemaprax_provider_rf_o0.so \
    lib/x86_64/libsemaprax_provider_rf_o2.so \
    lib/x86_64/libsemaprax_provider_om_o0.so \
    lib/x86_64/libsemaprax_provider_om_o2.so
)

readonly aligned_apk="$work/aligned.apk"
"$sdk_build_tools/zipalign" -P 16 -f 4 "$base_apk" "$aligned_apk"

readonly keystore="$work/private-debug.keystore"
keytool -genkeypair -noprompt \
  -keystore "$keystore" \
  -storepass semaprax-private \
  -keypass semaprax-private \
  -alias semaprax-private \
  -keyalg RSA -keysize 2048 -validity 1 \
  -dname "CN=SEMAPRAX private Android JNI evidence" >/dev/null 2>&1

readonly output_apk="$output_root/semaprax-android-jni.apk"
"$sdk_build_tools/apksigner" sign \
  --ks "$keystore" \
  --ks-key-alias semaprax-private \
  --ks-pass pass:semaprax-private \
  --key-pass pass:semaprax-private \
  --out "$output_apk" \
  "$aligned_apk"
"$sdk_build_tools/zipalign" -c -P 16 4 "$output_apk"
"$sdk_build_tools/apksigner" verify --verbose "$output_apk" >/dev/null

mapfile -t packaged_native < <(unzip -Z1 "$output_apk" | grep '^lib/' | LC_ALL=C sort)
readonly expected_native=(
  lib/x86_64/libsemaprax_jni.so
  lib/x86_64/libsemaprax_provider_o0.so
  lib/x86_64/libsemaprax_provider_o2.so
  lib/x86_64/libsemaprax_provider_rf_o0.so
  lib/x86_64/libsemaprax_provider_rf_o2.so
  lib/x86_64/libsemaprax_provider_om_o0.so
  lib/x86_64/libsemaprax_provider_om_o2.so
)
if [[ "${packaged_native[*]}" != "${expected_native[*]}" ]]; then
  echo "APK native library inventory is not exact" >&2
  exit 1
fi

printf '%s\n' "$output_apk"
