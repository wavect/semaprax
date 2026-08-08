#!/usr/bin/env bash
set -euo pipefail

expected_toolchain="nightly-2026-07-16"
expected_rustc_commit="d0babd8b6b05ef9bb65d42f928cef4129d64cf65"
target="x86_64-unknown-linux-gnu"
required_env="SEMAPRAX_REQUIRE_RUST_HOST_ASAN"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Rust-host ASan evidence requires Linux" >&2
  exit 1
fi
if [[ "${SEMAPRAX_REQUIRE_RUST_HOST_ASAN-}" != "1" ]]; then
  echo "$required_env must be exactly 1" >&2
  exit 1
fi
if [[ "${SEMAPRAX_RUST_HOST_ASAN_TOOLCHAIN-}" != "$expected_toolchain" ]]; then
  echo "SEMAPRAX_RUST_HOST_ASAN_TOOLCHAIN must be $expected_toolchain" >&2
  exit 1
fi
for command_name in rustup clang-18 nm; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Rust-host ASan evidence requires $command_name" >&2
    exit 1
  fi
done

if ! rustup run "$expected_toolchain" rustc -vV | grep -Eq '^release: .*-nightly$'; then
  echo "Rust-host ASan evidence requires a nightly rustc" >&2
  exit 1
fi
if ! rustup run "$expected_toolchain" rustc -vV | grep -q "^commit-hash: $expected_rustc_commit$"; then
  echo "Rust-host ASan rustc commit does not match the audited nightly" >&2
  exit 1
fi
if ! rustup run "$expected_toolchain" cargo -vV | grep -Eq '^release: .*-nightly$'; then
  echo "Rust-host ASan evidence requires Cargo from the pinned nightly" >&2
  exit 1
fi
if ! clang-18 --version | grep -Eq 'clang version 18\.'; then
  echo "Rust-host ASan evidence requires the audited Clang 18 major" >&2
  exit 1
fi
if [[ "${RUSTFLAGS-}" != *"-Zsanitizer=address"* ]]; then
  echo "RUSTFLAGS must enable -Zsanitizer=address" >&2
  exit 1
fi
if [[ "${RUSTFLAGS-}" != *"-Zexternal-clangrt"* ]] \
  || [[ "${RUSTFLAGS-}" != *"-Clinker=clang-18"* ]] \
  || [[ "${RUSTFLAGS-}" != *"-Clink-arg=-fsanitize=address"* ]]; then
  echo "RUSTFLAGS must select one external Clang ASan runtime" >&2
  exit 1
fi
if [[ "${RUSTFLAGS-}" != *"force-frame-pointers=yes"* ]]; then
  echo "RUSTFLAGS must preserve frame pointers" >&2
  exit 1
fi
if [[ ":${ASAN_OPTIONS-}:" != *":halt_on_error=1:"* ]]; then
  echo "ASAN_OPTIONS must contain halt_on_error=1" >&2
  exit 1
fi
if [[ ":${ASAN_OPTIONS-}:" != *":abort_on_error=1:"* ]]; then
  echo "ASAN_OPTIONS must contain abort_on_error=1" >&2
  exit 1
fi
if [[ ":${ASAN_OPTIONS-}:" != *":detect_leaks=1:"* ]]; then
  echo "ASAN_OPTIONS must contain detect_leaks=1" >&2
  exit 1
fi

work_dir="$(mktemp -d /tmp/semaprax-rust-host-asan.XXXXXX)"
cleanup() {
  case "$work_dir" in
    /tmp/semaprax-rust-host-asan.*) ;;
    *)
      echo "refusing to clean unexpected Rust-host ASan path: $work_dir" >&2
      return 1
      ;;
  esac
  if [[ ! -d "$work_dir" ]] || [[ -L "$work_dir" ]]; then
    echo "refusing to clean invalid Rust-host ASan directory: $work_dir" >&2
    return 1
  fi
  rm -rf -- "$work_dir"
}
trap cleanup EXIT

probe_dir="$work_dir/probe"
target_dir="$work_dir/target"
mkdir "$probe_dir" "$target_dir"
probe_source="$probe_dir/activation.rs"
probe_binary="$probe_dir/activation"
build_log="$probe_dir/host-build.log"

cat >"$probe_source" <<'RUST'
#![feature(cfg_sanitize)]

#[cfg(not(sanitize = "address"))]
compile_error!("Rust AddressSanitizer cfg is not active");

fn main() {
    let bytes = Box::new([1_u8, 2, 3, 4]);
    let dangling = bytes.as_ptr();
    drop(bytes);
    unsafe {
        std::hint::black_box(dangling.read_volatile());
    }
}
RUST

rustup run "$expected_toolchain" rustc \
  -Zsanitizer=address \
  -Zexternal-clangrt \
  -Clinker=clang-18 \
  -Clink-arg=-fsanitize=address \
  -Cforce-frame-pointers=yes \
  --target "$target" \
  "$probe_source" \
  -o "$probe_binary"
nm "$probe_binary" >"$probe_dir/activation.nm"
if ! grep -q '__asan_' "$probe_dir/activation.nm"; then
  echo "nightly activation probe has no defined or unresolved AddressSanitizer symbols" >&2
  exit 1
fi
if "$probe_binary" >"$probe_dir/activation.out" 2>&1; then
  echo "intentional Rust use-after-free escaped AddressSanitizer" >&2
  exit 1
fi
if ! grep -q 'ERROR: AddressSanitizer: heap-use-after-free' "$probe_dir/activation.out"; then
  echo "intentional Rust fault did not produce the required ASan diagnostic" >&2
  sed -n '1,120p' "$probe_dir/activation.out" >&2
  exit 1
fi

export CARGO_TARGET_DIR="$target_dir"
export CARGO_INCREMENTAL=0

rustup run "$expected_toolchain" cargo test -Zbuild-std \
  --target "$target" \
  --locked \
  -p semaprax-native-host \
  --test runtime_callable_host \
  --no-run \
  -vv 2>&1 | tee "$build_log"

if ! awk '
  /--crate-name semaprax_native_host/ && /-Zsanitizer=address/ && /-Zexternal-clangrt/ { found = 1 }
  END { exit(found ? 0 : 1) }
' "$build_log"; then
  echo "semaprax-native-host rustc command was not ASan-instrumented" >&2
  exit 1
fi

host_test_binary="$(find "$target_dir/$target/debug/deps" -maxdepth 1 -type f -name 'runtime_callable_host-*' -perm -111 -print -quit)"
if [[ -z "$host_test_binary" ]]; then
  echo "ASan-instrumented native-host test executable was not produced" >&2
  exit 1
fi
nm "$host_test_binary" >"$probe_dir/host-test.nm"
if ! grep -q '__asan_' "$probe_dir/host-test.nm"; then
  echo "native-host test executable has no defined or unresolved ASan symbols" >&2
  exit 1
fi

rustup run "$expected_toolchain" cargo test -Zbuild-std \
  --target "$target" \
  --locked \
  -p semaprax-native-host \
  --test runtime_callable_host

rustup run "$expected_toolchain" cargo test -Zbuild-std \
  --target "$target" \
  --locked \
  -p semaprax-native-host \
  --test runtime_callable_corpus \
  authoritative_corpus_executes_through_generated_callable_host_at_o0_and_o2 \
  -- --exact
