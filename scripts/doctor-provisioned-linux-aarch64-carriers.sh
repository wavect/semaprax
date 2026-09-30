#!/usr/bin/env bash
# Provision the real AArch64 Clang, Node and Rust carriers for the AArch64
# offline-doctor lifecycle probe, then write the bundle, selector and
# independent expected details that its two real-distribution fixtures read.
#
# THIS IS NOT PART OF scripts/doctor-provisioned-linux-gate.py. That gate
# admits x86-64 only. This script feeds
# scripts/doctor-provisioned-linux-aarch64-local-lifecycle.sh, and nothing it
# produces is x86-64 evidence, a signed release, or a WP-05 promotion. See
# docs/DOCTOR-PROVISIONED-LINUX-AARCH64-V1.md.
#
# It mirrors the x86-64 workflow's acquisition: the same Clang 17.0.6
# `clang-17` binary (from LLVM's official aarch64-linux-gnu archive), Node
# v22.23.2 and Rust 1.88.0. The two archives are pinned by SHA-256; Rust comes
# from rustup, which verifies its own components. Each tool is staged into a
# private 0700 tree with link count 1 and mode 755, and must run before it is
# bundled. The expected details are read from the staged tools, unconfined,
# so the fixtures compare the confined answers against an independent oracle.
#
#   scripts/doctor-provisioned-linux-aarch64-carriers.sh <output-directory>
#
# This is the only network-using step. It writes <output-directory>/carriers.env
# (KEY=VALUE lines) for the offline lifecycle driver. Any missing tool, digest
# mismatch or unrunnable carrier is a failure, not a skip.

set -o errexit
set -o nounset
set -o pipefail

readonly CLANG_RELEASE=17.0.6
readonly CLANG_ARCHIVE="clang+llvm-${CLANG_RELEASE}-aarch64-linux-gnu.tar.xz"
readonly CLANG_SHA256=6dd62762285326f223f40b8e4f2864b5c372de3f7de0731cb7cd55ca5287b75a
readonly NODE_RELEASE=v22.23.2
readonly NODE_ARCHIVE="node-${NODE_RELEASE}-linux-arm64.tar.xz"
readonly NODE_SHA256=fff4078c5def658577f92c88db7db3bc0072924bfb93fe52c1e744a54e94abb8
readonly RUST_DISTRIBUTION=1.88.0
readonly SELECTOR=real-distributions

fail() {
	printf 'error: %s\n' "$1" >&2
	printf 'This is a failure, not a skip.\n' >&2
	exit 1
}

[ "$#" -eq 1 ] || fail "usage: $0 <output-directory>"
[ "$(uname -s)" = Linux ] || fail "host system is '$(uname -s)', not 'Linux'"
case "$(uname -m)" in
aarch64 | arm64) ;;
*) fail "host architecture is '$(uname -m)', not AArch64" ;;
esac
for tool in curl tar xz sha256sum python3 rustup realpath; do
	command -v "${tool}" >/dev/null 2>&1 || fail "${tool} is not installed"
done

readonly REPOSITORY="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p -- "$1"
readonly OUTPUT="$(realpath -- "$1")"
readonly DOWNLOADS="${OUTPUT}/downloads"
readonly DIST="${OUTPUT}/distributions"
readonly CARRIER="${OUTPUT}/carrier"

acquire() {
	local url="$1" archive="$2" digest="$3"
	[ -f "${DOWNLOADS}/${archive}" ] || curl -fsSL -o "${DOWNLOADS}/${archive}" "${url}"
	printf '%s  %s\n' "${digest}" "${DOWNLOADS}/${archive}" | sha256sum -c --quiet - ||
		fail "${archive} does not match its pinned SHA-256"
}

mkdir -p -- "${DOWNLOADS}"
acquire "https://github.com/llvm/llvm-project/releases/download/llvmorg-${CLANG_RELEASE}/${CLANG_ARCHIVE}" \
	"${CLANG_ARCHIVE}" "${CLANG_SHA256}"
acquire "https://nodejs.org/dist/${NODE_RELEASE}/${NODE_ARCHIVE}" "${NODE_ARCHIVE}" "${NODE_SHA256}"

rm -rf -- "${DIST}" "${CARRIER}"
mkdir -m 700 -p -- "${DIST}/clang/bin" "${DIST}/node/bin" "${DIST}/rust/bin" "${CARRIER}"
tar -xJf "${DOWNLOADS}/${CLANG_ARCHIVE}" -C "${DIST}/clang/bin" --strip-components=2 \
	--wildcards "*/bin/clang-17"
mv -- "${DIST}/clang/bin/clang-17" "${DIST}/clang/bin/clang"
tar -xJf "${DOWNLOADS}/${NODE_ARCHIVE}" -C "${DIST}/node/bin" --strip-components=2 \
	"node-${NODE_RELEASE}-linux-arm64/bin/node"
rustup toolchain install "${RUST_DISTRIBUTION}" --profile minimal --no-self-update >/dev/null
rustc_source="$(rustup which --toolchain "${RUST_DISTRIBUTION}" rustc)"
# rustup hard-links rustc between toolchains; a copy gives a private inode.
# rustc finds its driver through RUNPATH $ORIGIN/../lib, so carry that
# directory beside it; the packager relocates what the loader must find.
cp --no-preserve=links,mode -- "${rustc_source}" "${DIST}/rust/bin/rustc"
cp -R --no-preserve=links,mode -- "$(dirname -- "${rustc_source}")/../lib" "${DIST}/rust/lib"
chmod 700 "${DIST}/clang" "${DIST}/node" "${DIST}/rust"
for staged in "${DIST}/clang/bin/clang" "${DIST}/node/bin/node" "${DIST}/rust/bin/rustc"; do
	chmod 755 -- "${staged}"
	[ "$(stat -c %h -- "${staged}")" = 1 ] || fail "${staged} has more than one link"
	"${staged}" --version >/dev/null || fail "${staged} does not execute unconfined"
done

clang_line="$("${DIST}/clang/bin/clang" --version | head -1)"
node_line="$("${DIST}/node/bin/node" --version | head -1)"
rust_line="$("${DIST}/rust/bin/rustc" --version | head -1)"

python3 "${REPOSITORY}/scripts/doctor-provisioned-linux-bundle.py" \
	--selector "${SELECTOR}" --architecture aarch64 \
	--clang "${DIST}/clang/bin/clang" --node "${DIST}/node/bin/node" --rustc "${DIST}/rust/bin/rustc" \
	--closure --plan >&2
python3 "${REPOSITORY}/scripts/doctor-provisioned-linux-bundle.py" \
	--selector "${SELECTOR}" --architecture aarch64 \
	--clang "${DIST}/clang/bin/clang" --node "${DIST}/node/bin/node" --rustc "${DIST}/rust/bin/rustc" \
	--closure --target all --bundle "${CARRIER}/bundle.bin" >&2
chmod 600 -- "${CARRIER}/bundle.bin"

{
	printf 'SEMAPRAX_DOCTOR_REAL_BUNDLE=%s\n' "${CARRIER}/bundle.bin"
	printf 'SEMAPRAX_DOCTOR_REAL_SELECTOR=%s\n' "${SELECTOR}"
	printf 'SEMAPRAX_DOCTOR_EXPECTED_CLANG_DETAIL=%s (%s)\n' "$(realpath -- "${DIST}/clang/bin/clang")" "${clang_line}"
	printf 'SEMAPRAX_DOCTOR_EXPECTED_NODE_DETAIL=%s\n' "${node_line}"
	printf 'SEMAPRAX_DOCTOR_EXPECTED_RUST_DETAIL=%s\n' "${rust_line}"
} >"${OUTPUT}/carriers.env"
cat -- "${OUTPUT}/carriers.env"
