#!/bin/sh
# Prepare target-authentic RI-13 Rust API envelopes in an offline Linux guest.
# This script never edits the committed Darwin fixtures.
set -eu

usage() {
    cat <<'EOF'
Usage:
  scripts/ri13-linux-x86_64-index-prepare.sh --plan \
    --image IMAGE@sha256:DIGEST --cargo-home PATH --nightly-toolchain PATH \
    --output FRESH-ABSOLUTE-PATH [--repo PATH]
  scripts/ri13-linux-x86_64-index-prepare.sh --run \
    --image IMAGE@sha256:DIGEST --cargo-home PATH --nightly-toolchain PATH \
    --output FRESH-ABSOLUTE-PATH [--repo PATH]

The supplied Cargo home must already contain the locked crates.io archives and
unpacked sources for regex 1.13.1 and url 2.5.8.  The supplied nightly
toolchain must be an already-installed Linux x86_64 nightly-2026-10-02
toolchain.  Both are mounted read-only.  --run performs two locked, offline
Cargo rustdoc captures and writes the raw JSON, canonical extractor envelopes,
and a digest receipt below OUTPUT.  It never downloads an image, crate, or
toolchain.
EOF
}

mode=plan
image=
image_tag=
image_digest=
cargo_home=
nightly_toolchain=
output=
repo=$(pwd -P)

while [ "$#" -gt 0 ]; do
    case "$1" in
        --plan) mode=plan ;;
        --run) mode=run ;;
        --image) shift; image=${1-} ;;
        --cargo-home) shift; cargo_home=${1-} ;;
        --nightly-toolchain) shift; nightly_toolchain=${1-} ;;
        --output) shift; output=${1-} ;;
        --repo) shift; repo=${1-} ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; exit 64 ;;
    esac
    shift
done

[ -n "$image" ] || { echo "--image is required" >&2; exit 64; }
[ -n "$cargo_home" ] || { echo "--cargo-home is required" >&2; exit 64; }
[ -n "$nightly_toolchain" ] || { echo "--nightly-toolchain is required" >&2; exit 64; }
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

repo=$(cd "$repo" && pwd -P)
cargo_home=$(cd "$cargo_home" && pwd -P)
nightly_toolchain=$(cd "$nightly_toolchain" && pwd -P)
case "$output" in
    /*) ;;
    *) echo "--output must be absolute" >&2; exit 64 ;;
esac
[ ! -e "$output" ] || { echo "--output must be a fresh path" >&2; exit 1; }
[ -z "$(git -C "$repo" status --porcelain)" ] || {
    echo "the source checkout must be clean" >&2
    exit 1
}
revision=$(git -C "$repo" rev-parse HEAD)

python3 - "$cargo_home" "$nightly_toolchain" <<'PY'
import hashlib
from pathlib import Path
import sys

cargo_home = Path(sys.argv[1])
nightly = Path(sys.argv[2])
expected = {
    "regex-1.13.1": "f020237b6c8eed93db2e2cb53c00c60a8e1bc73da7d073199a1180401450218d",
    "url-2.5.8": "ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed",
}
for binary in ("cargo", "rustc", "rustdoc"):
    path = nightly / "bin" / binary
    if not path.is_file() or path.is_symlink():
        raise SystemExit(f"--nightly-toolchain must provide regular bin/{binary}")
for package, digest in expected.items():
    archives = list(cargo_home.glob(f"registry/cache/*/{package}.crate"))
    sources = list(cargo_home.glob(f"registry/src/*/{package}"))
    if len(archives) != 1 or not archives[0].is_file():
        raise SystemExit(f"Cargo home must contain exactly one {package}.crate archive")
    if len(sources) != 1 or not sources[0].is_dir() or sources[0].is_symlink():
        raise SystemExit(f"Cargo home must contain exactly one regular unpacked {package} source")
    observed = hashlib.sha256(archives[0].read_bytes()).hexdigest()
    if observed != digest:
        raise SystemExit(f"{package}.crate checksum differs from the committed lock")
PY

print_plan() {
    cat <<EOF
RI-13 Linux x86_64 Rust API index preparation plan
  source revision: $revision
  source checkout: $repo
  image tag: $image_tag
  image digest: $image_digest
  Cargo cache: $cargo_home
  nightly toolchain: $nightly_toolchain
  output: $output

--run will start one named Linux x86_64 guest with no network,
capture regex 1.13.1 and url 2.5.8 Rustdoc JSON with the supplied pinned
nightly, convert each capture through the repository converter, and retain
raw JSON, SHA-256 receipt data, and the guest's stdout/stderr below $output.
The runner waits up to 30 minutes for the guest to stop, retains its inspect
record, then deletes the guest.
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
printf '%s\n' "$revision" > "$output/revision"
container_name="ri13-linux-index-$$"
container_started=false

cleanup_container() {
    status=$?
    trap - EXIT HUP INT TERM
    if [ "$container_started" = true ]; then
        # Keep diagnostics even when the caller interrupts a detached guest.
        container logs "$container_name" > "$output/container.log" 2>&1 || true
        container inspect "$container_name" > "$output/container-inspect.json" 2>&1 || true
        container delete --force "$container_name" > "$output/container-delete.log" 2>&1 || true
    fi
    exit "$status"
}
trap cleanup_container EXIT HUP INT TERM

container run --detach --name "$container_name" --arch amd64 --rosetta --init --network none \
    --memory 6G --read-only --tmpfs /tmp --tmpfs /work \
    --mount "type=bind,source=$repo,target=/repo,readonly" \
    --mount "type=bind,source=$cargo_home,target=/cargo-home,readonly" \
    --mount "type=bind,source=$nightly_toolchain,target=/nightly-toolchain,readonly" \
    --mount "type=bind,source=$output,target=/output" \
    --workdir /repo \
    --env "RI13_EXPECTED_REVISION=$revision" \
    --env "RI13_IMAGE_TAG=$image_tag" \
    --env "RI13_IMAGE_DIGEST=$image_digest" \
    --env CARGO_HOME=/cargo-home \
    --env CARGO_NET_OFFLINE=true \
    --env CARGO_BUILD_JOBS=1 \
    --env CARGO_INCREMENTAL=0 \
    --env CARGO_PROFILE_DEV_DEBUG=0 \
    --env HOME=/tmp \
    "$image_tag" bash /repo/scripts/ri13-linux-x86_64-index-prepare-inner.sh \
    > "$output/container-launch.log" 2>&1
container_started=true

# Apple Container's `logs --follow` can return before a guest has stopped.
# Poll inspect state first so cleanup cannot terminate a live extraction.
deadline=$(( $(date +%s) + 1800 ))
while :; do
    inspect_json=$(container inspect "$container_name") || {
        status=$?
        echo "could not inspect the guest (status $status)" >&2
        exit "$status"
    }
    state=$(printf '%s' "$inspect_json" | python3 -c '
import json
import sys
try:
    print(json.load(sys.stdin)[0]["status"]["state"])
except (IndexError, KeyError, TypeError, json.JSONDecodeError) as error:
    raise SystemExit(f"container inspect did not expose guest state: {error}")
') || exit 1
    case "$state" in
        running|created)
            if [ "$(date +%s)" -ge "$deadline" ]; then
                printf '%s\n' "guest remained $state for 1800 seconds" > "$output/container-timeout.txt"
                exit 124
            fi
            sleep 1
            ;;
        *) break ;;
    esac
done
printf '%s\n' "$inspect_json" > "$output/container-inspect.json"
container logs "$container_name" > "$output/container.log" 2>&1 || {
    status=$?
    echo "could not retain the stopped guest log (status $status)" >&2
    exit "$status"
}
container delete "$container_name" > "$output/container-delete.log" 2>&1 || {
    status=$?
    echo "could not delete the stopped guest (status $status)" >&2
    exit "$status"
}
container_started=false

# A log stream does not communicate the guest's exit status. The inner runner
# writes this receipt only after both raw captures and both typed envelopes pass
# its target/package assertions.
[ -f "$output/receipt.json" ] || {
    echo "guest exited without a completed RI-13 Linux preparation receipt" >&2
    exit 1
}
