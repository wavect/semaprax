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

IMAGE must be an already-pulled, immutable linux/amd64 toolchain image.  PATH
must be a Linux x86_64 Cargo registry/cache directory containing every locked
RI-13 dependency.  The runner never pulls an image or permits network access.

--plan is the default and has no filesystem or container side effects.  --run
requires a nonexistent evidence path, creates a detached checked-out clone
there, and removes the container after the command stops.
EOF
}

mode=plan
image=
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
    *@sha256:*) ;;
    *) echo "--image must name an immutable digest (IMAGE@sha256:DIGEST)" >&2; exit 64 ;;
esac

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
  image: $image
  Linux Cargo cache: $cargo_home
  evidence path: $evidence

--run will create a detached clean clone at $workspace, then run:
  container run --arch amd64 --rosetta --rm --init --network none \\
    --read-only --tmpfs /tmp --tmpfs /work --mount source=$workspace,target=/repo \\
    --mount source=$evidence,target=/evidence --mount source=$cargo_home,target=/cargo-home,readonly \\
    $image bash /repo/scripts/ri13-linux-x86_64-evidence-inner.sh

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
[ ! -e "$evidence" ] || {
    echo "evidence path already exists (fresh evidence requires a new path): $evidence" >&2
    exit 1
}

mkdir -p "$evidence"
trap 'rm -rf "$evidence"' HUP INT TERM
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
    --env CARGO_HOME=/cargo-home \
    --env CARGO_NET_OFFLINE=true \
    --env CARGO_BUILD_JOBS=1 \
    --env CARGO_INCREMENTAL=0 \
    --env CARGO_PROFILE_DEV_DEBUG=0 \
    --env HOME=/tmp \
    --env CLANG=/usr/bin/clang \
    "$image" bash /repo/scripts/ri13-linux-x86_64-evidence-inner.sh

trap - HUP INT TERM
printf '%s\n' "RI-13 Linux x86_64 evidence retained at $evidence"
