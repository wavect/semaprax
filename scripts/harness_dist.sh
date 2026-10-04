#!/bin/sh
# Build and smoke-test the distributable harness tarball (HN-07). Local only:
# nothing is published, downloaded or installed outside a temp directory.
#
#   scripts/harness_dist.sh build --binary <semaprax-harness> --out <dir> [--full <semaprax>]
#   scripts/harness_dist.sh smoke --tarball <file.tar.gz> --compiler <semaprax> \
#       --rtk <abs> --graft <abs> --node <abs> --python <abs>
#
# `build` packs the binary (its adapter assets are embedded), LICENSE, the
# HARNESS-* docs, a machine-readable support catalog and the smoke fixtures.
# `semaprax-full` is included only when a prebuilt one is passed with --full.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cmd=${1:-}; [ $# -gt 0 ] && shift

die() { echo "harness_dist: $*" >&2; exit 1; }

catalog() { # emit the support catalog from the shipped descriptors and skill catalog
  python3 - "$root" <<'PY'
import json, sys, os
root = sys.argv[1]
base = os.path.join(root, "packages/semaprax-harness-adapters")
providers = []
for d in ["graft", "graphify", "rtk"]:
    p = json.load(open(os.path.join(base, d, "harness-provider.json")))
    providers.append({
        "provider_id": p["provider"]["id"], "capabilities": [c["kind"] for c in p["capabilities"]],
        "runtime": p["adapter"]["runtime"], "platforms": p["platforms"],
        "upstream": {k: p["upstream"][k] for k in ("name", "package", "repository", "versions")},
        "tested": p["support"]["tested"],
    })
skills = json.load(open(os.path.join(base, "skills/catalog.json")))
print(json.dumps({
    "schema": "semaprax.harness-support-catalog.v1",
    "providers": sorted(providers, key=lambda x: x["provider_id"]),
    "official_skills": sorted(s["id"] for s in skills["skills"]),
    "skill_presets": sorted(skills["presets"]),
    "note": "Tested platforms are listed per provider; anything else is untested and setup refuses to choose it.",
}, indent=2, sort_keys=True))
PY
}

build() {
  binary=; out=; full=
  while [ $# -gt 0 ]; do case $1 in
    --binary) binary=$2; shift 2;; --out) out=$2; shift 2;; --full) full=$2; shift 2;;
    *) die "unknown option $1";; esac; done
  [ -x "$binary" ] || die "--binary must be an executable semaprax-harness"
  [ -n "$out" ] || die "--out is required"
  ver=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/crates/semaprax-harness/Cargo.toml" | head -1)
  name="semaprax-harness-$ver-$(uname -s | tr A-Z a-z)-$(uname -m)"
  stage=$out/$name
  rm -rf "$stage"; mkdir -p "$stage/bin" "$stage/docs" "$stage/share/smoke"
  cp "$binary" "$stage/bin/semaprax-harness"
  if [ -n "$full" ]; then [ -x "$full" ] || die "--full must be executable"; cp "$full" "$stage/bin/semaprax"; fi
  cp "$root/LICENSE" "$stage/LICENSE"
  cp "$root"/docs/HARNESS-*.md "$stage/docs/"
  catalog > "$stage/share/support-catalog.json"
  fx=$root/crates/semaprax-harness/tests/fixtures/workflow
  cp -R "$fx/downstream" "$stage/share/smoke/project"
  cp "$fx/task.json" "$stage/share/smoke/task.json"
  cp "$fx/proposals/valid.json" "$stage/share/smoke/valid-proposal.json"
  (cd "$stage/bin" && shasum -a 256 *) > "$stage/share/BINARIES.sha256"
  tar -C "$out" -czf "$out/$name.tar.gz" "$name"
  (cd "$out" && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
  echo "$out/$name.tar.gz"
}

smoke() {
  tarball=; compiler=; rtk=; graft=; node=; python=
  while [ $# -gt 0 ]; do case $1 in
    --tarball) tarball=$2; shift 2;; --compiler) compiler=$2; shift 2;;
    --rtk) rtk=$2; shift 2;; --graft) graft=$2; shift 2;;
    --node) node=$2; shift 2;; --python) python=$2; shift 2;;
    *) die "unknown option $1";; esac; done
  for v in tarball compiler rtk graft node python; do eval "[ -n \"\$$v\" ]" || die "--$v is required"; done
  # Everything lives outside the checkout, with an empty HOME and a scrubbed environment.
  work=$(mktemp -d "${TMPDIR:-/tmp}/harness-dist-smoke.XXXXXX")
  trap 'rm -rf "$work"' EXIT
  mkdir -p "$work/empty-home" "$work/cwd"
  tar -C "$work" -xzf "$tarball"
  dist=$(ls -d "$work"/semaprax-harness-*/ | head -1); dist=${dist%/}
  [ ! -e "$dist/packages" ] || die "the tarball must not contain the packages/ checkout tree"
  H() { (cd "$work/cwd" && env -i PATH=/usr/bin:/bin HOME="$work/empty-home" TMPDIR="$work" \
      SEMAPRAX_HARNESS_HOME="$work/state" "$dist/bin/semaprax-harness" "$@"); }
  ok() { echo "ok: $*"; }

  H --help | grep -q ' setup' || die "help does not list setup"; ok "help lists setup"
  grep -q '"semaprax.harness-support-catalog.v1"' "$dist/share/support-catalog.json" || die "no catalog"
  ok "support catalog present"
  H skills list | grep -q '^ponytail ' && H skills list | grep -q '^caveman ' || die "default skills not discovered"
  ok "default skills discovered (ponytail, caveman)"

  tools="--tool rtk=$rtk --tool graft=$graft --tool node=$node --tool python=$python"
  proj_native=$work/proj-native; proj_eff=$work/proj-efficient
  for p in "$proj_native" "$proj_eff"; do cp -R "$dist/share/smoke/project" "$p"; (cd "$p" && git init -q); done

  H setup --project "$proj_eff" --preset local-efficient $tools --dry-run >/dev/null
  [ ! -e "$work/state" ] && [ ! -e "$proj_eff/semaprax.harness.toml" ] || die "dry-run changed state"
  ok "dry-run changes nothing"
  H setup --project "$proj_eff" --preset local-efficient $tools --yes | tee "$work/setup1.txt" | grep '^changed:'
  grep -q 'adopted and trusted org.nanonets/graft-context' "$work/setup1.txt" || die "graft not adopted"
  grep -q 'adopted and trusted ai.rtk/rtk-command-view' "$work/setup1.txt" || die "rtk not adopted"
  H setup --project "$proj_eff" --preset local-efficient $tools --yes | grep -q 'setup was already complete' || die "second setup is not a no-op"
  ok "setup adopted RTK + Graft once; repeat is a no-op"
  H status --project "$proj_eff" > "$work/status.txt"
  grep -q 'context.repository  selected    org.nanonets/graft-context' "$work/status.txt" || die "graft not selected"
  grep -q 'command.view        selected    ai.rtk/rtk-command-view' "$work/status.txt" || die "rtk not selected"
  ok "status selects Graft and RTK"
  grep -q "/state/artifacts/" "$work/state/installations.json" || die "adapters are not served from the store"
  ok "adapters run from the content-addressed store under the harness home"

  # Native-only task on a project pinned to the builtin providers.
  H setup --project "$proj_native" --preset native $tools --yes >/dev/null
  H run "$proj_native" --compiler "$compiler" --task "$dist/share/smoke/task.json" \
    --proposal "$dist/share/smoke/valid-proposal.json" --json > "$work/run.json"
  grep -q '"status":"approved-candidate-ready"' "$work/run.json" || die "native-only run did not reach an approved candidate"
  grep -q 'semaprax/native-context' "$work/run.json" || die "run did not use the native context provider"
  ok "native-only run reached approved-candidate-ready"

  # The adopted runtime serves run with the Graft provider: no HARNESS_NODE/PYTHON and no --node.
  H run "$proj_eff" --compiler "$compiler" --task "$dist/share/smoke/task.json" \
    --proposal "$dist/share/smoke/valid-proposal.json" --json > "$work/run-graft.json"
  grep -q '"status":"approved-candidate-ready"' "$work/run-graft.json" || die "graft-backed run failed"
  grep -q 'org.nanonets/graft-context' "$work/run-graft.json" || die "run did not use graft"
  ok "run with adopted Graft + RTK reached approved-candidate-ready without HARNESS_* variables"
  echo "SMOKE PASSED ($dist)"
}

case $cmd in build) build "$@";; smoke) smoke "$@";; *) die "usage: harness_dist.sh build|smoke ...";; esac
