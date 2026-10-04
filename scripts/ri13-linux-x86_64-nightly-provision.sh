#!/bin/sh
# Provision the exact Rustdoc extractor without trusting rustup or a moving tag.
# This script does not invoke Cargo or the RI-13 application gate.
set -eu

usage() {
    cat <<'EOF'
Usage:
  scripts/ri13-linux-x86_64-nightly-provision.sh --plan \
    --image IMAGE@sha256:DIGEST --cargo-home PATH --output FRESH-ABSOLUTE-PATH
  scripts/ri13-linux-x86_64-nightly-provision.sh --run \
    --image IMAGE@sha256:DIGEST --cargo-home PATH --output FRESH-ABSOLUTE-PATH

--run starts one disposable Linux x86_64 guest with network access only to
download the named Rust distribution artifacts.  It verifies the pinned
nightly manifest and every artifact before installing cargo, rustc/rustdoc,
and the Linux x86_64 standard library below OUTPUT/toolchain.  The supplied
Cargo home is mounted read-only and checked for the two locked RI-13 sources;
Cargo itself is never executed.
EOF
}

mode=plan
image=
image_tag=
image_digest=
cargo_home=
output=

while [ "$#" -gt 0 ]; do
    case "$1" in
        --plan) mode=plan ;;
        --run) mode=run ;;
        --image) shift; image=${1-} ;;
        --cargo-home) shift; cargo_home=${1-} ;;
        --output) shift; output=${1-} ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; exit 64 ;;
    esac
    shift
done

[ -n "$image" ] || { echo "--image is required" >&2; exit 64; }
[ -n "$cargo_home" ] || { echo "--cargo-home is required" >&2; exit 64; }
[ -n "$output" ] || { echo "--output is required" >&2; exit 64; }
case "$image" in
    *@sha256:*) image_tag=${image%@sha256:*}; image_digest=${image##*@} ;;
    *) echo "--image must name an immutable digest (IMAGE@sha256:DIGEST)" >&2; exit 64 ;;
esac
[ -n "$image_tag" ] || { echo "--image must include a local tag before @sha256" >&2; exit 64; }
python3 - "$image_digest" <<'PY'
import re
import sys
if re.fullmatch(r"sha256:[0-9a-f]{64}", sys.argv[1]) is None:
    raise SystemExit("--image digest must be sha256: followed by 64 lowercase hex digits")
PY

cargo_home=$(cd "$cargo_home" && pwd -P)
script_dir=$(cd "$(dirname "$0")" && pwd -P)
case "$output" in
    /*) ;;
    *) echo "--output must be absolute" >&2; exit 64 ;;
esac
[ ! -e "$output" ] || { echo "--output must be a fresh path" >&2; exit 1; }

python3 - "$cargo_home" <<'PY'
import hashlib
from pathlib import Path
import sys

root = Path(sys.argv[1])
expected = {
    "regex-1.13.1": "f020237b6c8eed93db2e2cb53c00c60a8e1bc73da7d073199a1180401450218d",
    "url-2.5.8": "ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed",
}
for package, digest in expected.items():
    archives = list(root.glob(f"registry/cache/*/{package}.crate"))
    sources = list(root.glob(f"registry/src/*/{package}"))
    if len(archives) != 1 or not archives[0].is_file():
        raise SystemExit(f"Cargo home must contain exactly one {package}.crate archive")
    if len(sources) != 1 or not sources[0].is_dir() or sources[0].is_symlink():
        raise SystemExit(f"Cargo home must contain exactly one regular unpacked {package} source")
    if hashlib.sha256(archives[0].read_bytes()).hexdigest() != digest:
        raise SystemExit(f"{package}.crate checksum differs from the committed lock")
PY

print_plan() {
    cat <<EOF
RI-13 exact Linux x86_64 nightly extractor provision plan
  image tag: $image_tag
  image digest: $image_digest
  Cargo cache (read-only): $cargo_home
  output: $output
  nightly date: 2026-10-02
  manifest SHA-256: 50abfdf8df57de84ff7b3188b6cad2a9681a4165518712c9e504948bf896e2b3

--run starts one removed-on-exit Linux x86_64 guest with 2 GiB RAM and guest
network only for the three checksum-pinned Rust distribution downloads.  It
does not invoke Cargo, rustdoc extraction, or the M1/M2/M3 evidence gate.
EOF
}

if [ "$mode" = plan ]; then
    print_plan
    exit 0
fi

command -v container >/dev/null || { echo "Apple Container CLI is required for --run" >&2; exit 1; }
inspect_json=$(container image inspect "$image_tag") || {
    echo "local image tag is unavailable: $image_tag" >&2
    exit 1
}
actual_digest=$(printf '%s' "$inspect_json" | python3 -c '
import json
import sys
try:
    print(json.load(sys.stdin)[0]["configuration"]["descriptor"]["digest"])
except (IndexError, KeyError, TypeError, json.JSONDecodeError) as error:
    raise SystemExit(f"local image inspect did not expose an OCI descriptor digest: {error}")
') || exit 1
[ "$actual_digest" = "$image_digest" ] || {
    echo "local image digest mismatch for $image_tag: expected $image_digest, got $actual_digest" >&2
    exit 1
}

mkdir "$output"
trap 'echo "RI-13 nightly provision retained after interruption: $output" >&2' HUP INT TERM
container run --arch amd64 --rosetta --rm --init \
    --memory 2G --read-only --tmpfs /tmp --tmpfs /work \
    --mount "type=bind,source=$script_dir,target=/provision-scripts,readonly" \
    --mount "type=bind,source=$cargo_home,target=/cargo-home,readonly" \
    --mount "type=bind,source=$output,target=/output" \
    --env "RI13_IMAGE_TAG=$image_tag" \
    --env "RI13_IMAGE_DIGEST=$image_digest" \
    "$image_tag" bash /provision-scripts/ri13-linux-x86_64-nightly-provision-inner.sh

test -s "$output/receipt.json" || {
    echo "nightly provision did not produce a receipt; inspect $output/provision-failure.txt" >&2
    exit 1
}
for executable in cargo rustc rustdoc; do
    test -f "$output/toolchain/bin/$executable" || {
        echo "nightly provision did not produce regular bin/$executable" >&2
        exit 1
    }
done
