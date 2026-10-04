#!/bin/sh
# Plan or run the bounded RI-13 Linux x86_64 evidence fixture with Apple Container.
# The guest is x86_64 Linux under Rosetta on Apple silicon; this is a
# portability/reproducibility receipt and carries no native-performance claim.
#
# The default is --plan: it creates no clone, target directory, container, image
# pull, or build.  --run makes a fresh detached clone below the evidence path and
# invokes one removed-on-exit container with networking disabled.
set -eu

usage() {
    cat <<'EOF'
Usage:
  scripts/ri13-linux-x86_64-evidence.sh --plan --image IMAGE@sha256:DIGEST \
    --cargo-home PATH [--repo PATH] [--evidence PATH]
  scripts/ri13-linux-x86_64-evidence.sh --run --image IMAGE@sha256:DIGEST \
    --cargo-home PATH [--repo PATH] [--evidence PATH]

IMAGE must be an already-pulled local tag with an immutable digest suffix
(TAG@sha256:DIGEST).  The local TAG is inspected and the exact digest is
required before running.  PATH
must be a Linux x86_64 Cargo registry/cache directory containing every locked
RI-13 dependency.  The runner never pulls an image or permits network access.

--plan is the default and has no filesystem or container side effects.  --run
requires a nonexistent evidence path, creates a detached checked-out clone
there, and removes the container after the command stops.
EOF
}

mode=plan
image=
image_tag=
image_digest=
cargo_home=
repo=$(pwd -P)
evidence=

while [ "$#" -gt 0 ]; do
    case "$1" in
        --plan) mode=plan ;;
        --run) mode=run ;;
        --image)
            shift
            image=${1-}
            ;;
        --cargo-home)
            shift
            cargo_home=${1-}
            ;;
        --repo)
            shift
            repo=${1-}
            ;;
        --evidence)
            shift
            evidence=${1-}
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage >&2
            exit 64
            ;;
    esac
    shift
done

[ -n "$image" ] || { echo "--image is required" >&2; exit 64; }
[ -n "$cargo_home" ] || { echo "--cargo-home is required" >&2; exit 64; }
case "$image" in
    *@sha256:*)
        image_tag=${image%@sha256:*}
        image_digest=${image##*@}
        ;;
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
revision=$(git -C "$repo" rev-parse HEAD)
[ -z "$(git -C "$repo" status --porcelain)" ] || {
    echo "the source checkout must be clean" >&2
    exit 1
}
if [ -z "$evidence" ]; then
    evidence="$repo/evidence/ri13-linux-x86_64-${revision}"
fi
case "$evidence" in
    /*) ;;
    *) evidence="$repo/$evidence" ;;
esac
cargo_home=$(cd "$cargo_home" 2>/dev/null && pwd -P) || {
    echo "--cargo-home must name an existing directory" >&2
    exit 1
}

workspace="$evidence/worktree"
target="$evidence/target"

print_plan() {
    cat <<EOF
RI-13 Linux x86_64 evidence plan
  source revision: $revision
  source checkout: $repo
  image tag: $image_tag
  image digest: $image_digest
  Linux Cargo cache: $cargo_home
  evidence path: $evidence

--run will create a detached clean clone at $workspace, then run:
  container run --arch amd64 --rosetta --rm --init --network none \\
    --memory 4G --read-only --tmpfs /tmp --tmpfs /work \\
    --mount type=bind,source=$workspace,target=/repo \\
    --mount type=bind,source=$evidence,target=/evidence \\
    --mount type=bind,source=$cargo_home,target=/cargo-home,readonly \\
    $image_tag bash /repo/scripts/ri13-linux-x86_64-evidence-inner.sh

The inner command rejects a non-Linux/non-x86_64 guest or a revision mismatch,
runs the exact M1/M2/M3 prepare/consumer stages plus the linked Project check,
and writes one output receipt with per-file digests below the evidence path.
EOF
}

if [ "$mode" = plan ]; then
    print_plan
    exit 0
fi

command -v container >/dev/null || {
    echo "Apple Container CLI is required for --run" >&2
    exit 1
}
[ -n "$image_tag" ] || { echo "--image tag is required" >&2; exit 64; }
[ ! -e "$evidence" ] || {
    echo "evidence path already exists (fresh evidence requires a new path): $evidence" >&2
    exit 1
}

inspect_json=$(container image inspect "$image_tag") || {
    echo "local image tag is unavailable: $image_tag" >&2
    exit 1
}
actual_digest=$(printf '%s' "$inspect_json" | python3 -c '
import json
import sys
try:
    rows = json.load(sys.stdin)
    digest = rows[0]["configuration"]["descriptor"]["digest"]
except (IndexError, KeyError, TypeError, json.JSONDecodeError) as error:
    raise SystemExit(f"local image inspect did not expose an OCI descriptor digest: {error}")
print(digest)
') || exit 1
[ "$actual_digest" = "$image_digest" ] || {
    echo "local image digest mismatch for $image_tag: expected $image_digest, got $actual_digest" >&2
    exit 1
}

mkdir -p "$evidence"
trap 'echo "RI-13 Linux x86_64 evidence preserved after interruption: $evidence" >&2' HUP INT TERM
git clone --no-local --no-checkout "$repo" "$workspace"
git -C "$workspace" checkout --detach "$revision"
[ -z "$(git -C "$workspace" status --porcelain)" ] || {
    echo "detached evidence clone is unexpectedly dirty" >&2
    exit 1
}
mkdir -p "$target"
printf '%s\n' "$revision" > "$evidence/revision"

container run --arch amd64 --rosetta --rm --init --network none \
    --memory 4G --read-only --tmpfs /tmp --tmpfs /work \
    --mount "type=bind,source=$workspace,target=/repo" \
    --mount "type=bind,source=$evidence,target=/evidence" \
    --mount "type=bind,source=$cargo_home,target=/cargo-home,readonly" \
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
    --env CLANG=/usr/bin/clang \
    "$image_tag" bash /repo/scripts/ri13-linux-x86_64-evidence-inner.sh

printf '%s\n' "RI-13 Linux x86_64 evidence retained at $evidence"
