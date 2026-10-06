#!/usr/bin/env sh
# Build, package and smoke-test a Linux release archive inside an Ubuntu 22.04
# container so the produced binaries link against glibc 2.35, the documented
# runtime baseline, instead of the newer glibc of the CI host.
#
#   scripts/package-release-linux-container.sh [--dry-run] TAG COMMIT TARGET OUTPUT_ROOT
#
# TARGET must be the Linux target of the machine running this script
# (x86_64-unknown-linux-gnu on an x86_64 host, aarch64-unknown-linux-gnu on an
# arm64 host); the container runs natively, never under emulation. OUTPUT_ROOT
# must lie inside the checkout, which is mounted into the container. The work
# is done by scripts/package-release.sh, which also refuses any binary that
# needs a newer glibc and runs the archive smoke on the container's glibc.
#
# --dry-run validates the arguments, prints the exact container invocation and
# the in-container script, and runs nothing.
set -eu

fail() {
    echo "release container package rejected: $1" >&2
    exit 2
}

# Must equal the `toolchain:` pinned for release-artifacts in ci.yml.
RUST_TOOLCHAIN=1.97.1
IMAGE=${SEMAPRAX_RELEASE_CONTAINER_IMAGE:-ubuntu:22.04}

dry_run=0
if [ "${1:-}" = --dry-run ]; then
    dry_run=1
    shift
fi
[ "$#" -eq 4 ] || fail "expected [--dry-run] TAG COMMIT TARGET OUTPUT_ROOT"
tag=$1
commit=$2
target=$3
output_root=$4

[ -f scripts/package-release.sh ] || fail "run from the repository root"
checkout=$(pwd -P) || fail "working directory cannot be resolved"

case "$target" in
    x86_64-unknown-linux-gnu) rustup_triple=x86_64-unknown-linux-gnu; expected_arch=x86_64 ;;
    aarch64-unknown-linux-gnu) rustup_triple=aarch64-unknown-linux-gnu; expected_arch=aarch64 ;;
    *) fail "unsupported Linux release target" ;;
esac

host_arch=$(uname -m)
[ "$host_arch" = arm64 ] && host_arch=aarch64
[ "$host_arch" = "$expected_arch" ] || fail "host architecture $host_arch cannot natively build $target"

[ -n "$output_root" ] || fail "output root must not be empty"
case "$output_root" in
    /*) absolute_output=$output_root ;;
    *) absolute_output="$checkout/$output_root" ;;
esac
case "$absolute_output" in
    "$checkout"/*) ;;
    *) fail "output root must be inside the checkout so the container can write it" ;;
esac
output_relative=${absolute_output#"$checkout"/}
case "/$output_relative/" in
    */../*) fail "output root must not contain .. components" ;;
esac

# Runs inside the container as root. Everything it needs is installed here;
# nothing from the host except the mounted checkout is trusted or reused.
# shellcheck disable=SC2016 # the script is deliberately expanded by the container's shell, not this one
inner='
set -eu
tag=$1
commit=$2
target=$3
output=$4
rustup_triple=$5
toolchain=$6
host_uid=$7
host_gid=$8
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install --yes --no-install-recommends build-essential ca-certificates cmake curl git pkg-config
export RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
mkdir /tmp/rustup-init
cd /tmp/rustup-init
base=https://static.rust-lang.org/rustup/dist/$rustup_triple
curl --proto "=https" --tlsv1.2 --fail --silent --show-error --location --output rustup-init "$base/rustup-init"
curl --proto "=https" --tlsv1.2 --fail --silent --show-error --location --output rustup-init.sha256 "$base/rustup-init.sha256"
expected=$(cut -d " " -f 1 rustup-init.sha256)
actual=$(sha256sum rustup-init | cut -d " " -f 1)
[ "$expected" = "$actual" ] || { echo "rustup-init digest mismatch" >&2; exit 2; }
chmod 0755 rustup-init
./rustup-init -y --no-modify-path --profile minimal --default-toolchain "$toolchain"
export PATH=/opt/cargo/bin:$PATH
rustc --version | grep -q "^rustc $toolchain " || { echo "container rustc is not $toolchain" >&2; exit 2; }
ldd --version | head -n 1
cd /work
status=0
sh scripts/package-release.sh "$tag" "$commit" "$target" "$output" || status=$?
# Hand the root-created output back to the invoking user, even after a failure.
chown -R "$host_uid:$host_gid" "$output" 2>/dev/null || true
exit "$status"
'

if [ "$dry_run" -eq 1 ]; then
    printf '%s\n' "docker run --rm" \
        "  --volume $checkout:/work" \
        "  $IMAGE" \
        "  sh -c <in-container script> sh $tag $commit $target $output_relative $rustup_triple $RUST_TOOLCHAIN $(id -u) $(id -g)"
    printf '%s\n' '--- in-container script ---' "$inner"
    exit 0
fi

command -v docker >/dev/null 2>&1 || fail "docker is required"
mkdir -p "$absolute_output"
docker run --rm \
    --volume "$checkout:/work" \
    "$IMAGE" \
    sh -c "$inner" sh "$tag" "$commit" "$target" "$output_relative" "$rustup_triple" "$RUST_TOOLCHAIN" "$(id -u)" "$(id -g)" >&2

archive="$absolute_output/semaprax-$tag-$target.tar.gz"
[ -f "$archive" ] || fail "container run finished without producing $archive"
printf '%s\n' "$archive"
