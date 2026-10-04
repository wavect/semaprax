#!/bin/sh
# Build and smoke-test the distributable harness tarball (HN-07). Local only:
# nothing is published, downloaded or installed outside a temp directory.
#
#   scripts/harness_dist.sh build --binary <semaprax-harness> --out <dir> [--full <semaprax>] [--platform <os-arch>]
#   scripts/harness_dist.sh smoke-core --tarball <file.tar.gz> [--compiler <semaprax>]
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

sha256() { if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$@"; else sha256sum "$@"; fi; }

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
  binary=; out=; full=; platform=
  while [ $# -gt 0 ]; do case $1 in
    --binary) binary=$2; shift 2;; --out) out=$2; shift 2;; --full) full=$2; shift 2;; --platform) platform=$2; shift 2;;
    *) die "unknown option $1";; esac; done
  [ -x "$binary" ] || die "--binary must be an executable semaprax-harness"
  [ -n "$out" ] || die "--out is required"
  ver=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/crates/semaprax-harness/Cargo.toml" | head -1)
  [ -n "$platform" ] || platform="$(uname -s | tr A-Z a-z)-$(uname -m)"
  name="semaprax-harness-$ver-$platform"
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
  (cd "$stage/bin" && sha256 *) > "$stage/share/BINARIES.sha256"
  tar -C "$out" -czf "$out/$name.tar.gz" "$name"
  (cd "$out" && sha256 "$name.tar.gz" > "$name.tar.gz.sha256")
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

# Core smoke: portable cells that need no RTK, Graft, node or python. Every cell
# is reported pass, fail or untested (with the missing dependency). Only a run
# with every cell passing prints SMOKE PASSED; an untested cell is exit 3.
smoke_core() {
  tarball=; compiler=
  while [ $# -gt 0 ]; do case $1 in
    --tarball) tarball=$2; shift 2;; --compiler) compiler=$2; shift 2;;
    *) die "unknown option $1";; esac; done
  [ -n "$tarball" ] || die "--tarball is required"
  pass=0; fail=0; untested=0
  cell() { # cell <name> pass|fail|untested <detail>
    echo "CELL $1 $2 $3"
    case $2 in pass) pass=$((pass+1));; fail) fail=$((fail+1));; *) untested=$((untested+1));; esac
  }
  work=$(mktemp -d "${TMPDIR:-/tmp}/harness-dist-core.XXXXXX")
  trap 'rm -rf "$work"' EXIT
  mkdir -p "$work/empty-home" "$work/cwd" "$work/a"
  echo "PLATFORM $(uname -s | tr A-Z a-z)-$(uname -m)"
  tar -C "$work/a" -xzf "$tarball" || die "cannot extract $tarball"
  dist=$(ls -d "$work"/a/semaprax-harness-*/ | head -1); dist=${dist%/}
  H() { (cd "$work/cwd" && env -i PATH=/nonexistent HOME="$work/empty-home" TMPDIR="$work" \
      SEMAPRAX_HARNESS_HOME="$STATE" "$dist/bin/semaprax-harness" "$@"); }

  STATE=$work/state1
  if [ -x "$dist/bin/semaprax-harness" ] && [ ! -e "$dist/packages" ] \
     && H --help 2>&1 | grep -q ' setup' && grep -q '"semaprax.harness-support-catalog.v1"' "$dist/share/support-catalog.json"; then
    cell fresh-install pass "extracted outside the checkout, empty HOME, PATH=/nonexistent"
  else cell fresh-install fail "binary, catalog or help missing"; fi

  if H skills list | grep -q '^ponytail ' && H skills list | grep -q '^caveman ' \
     && H skills load ponytail | grep -q 'BEGIN SKILL name="ponytail"' \
     && H skills load caveman | grep -q 'BEGIN SKILL name="caveman"'; then
    cell default-skills-offline pass "list and load of the official skills with no network and no tools on PATH"
  else cell default-skills-offline fail "official skills not served from the binary"; fi

  proj=$work/proj; cp -R "$dist/share/smoke/project" "$proj"
  if H setup --project "$proj" --preset native --yes >"$work/setup1.txt" 2>&1; then
    # Offline reuse: a repeat finds everything current and rewrites nothing.
    before=$(cd "$STATE" && find . -type f | sort | while read -r f; do sha256 "$f"; done | sha256 | cut -d' ' -f1)
    if H setup --project "$proj" --preset native --yes 2>&1 | grep -q 'setup was already complete' \
       && [ "$before" = "$(cd "$STATE" && find . -type f | sort | while read -r f; do sha256 "$f"; done | sha256 | cut -d' ' -f1)" ]; then
      cell offline-reuse pass "repeat setup is a no-op and the harness home is byte-identical (no network is attempted by design; this run does not deny it)"
    else cell offline-reuse fail "repeat setup changed state or was not a no-op"; fi
  else cell offline-reuse fail "$(head -c 300 "$work/setup1.txt")"; fi

  # Relocation: move the distribution, then set up from the new location into a fresh home.
  mkdir -p "$work/moved"; old=$dist; mv "$dist" "$work/moved/relocated"; dist=$work/moved/relocated
  STATE=$work/state2
  proj2=$work/proj2; cp -R "$dist/share/smoke/project" "$proj2"
  if H skills load ponytail | grep -q 'BEGIN SKILL' && H setup --project "$proj2" --preset native --yes >/dev/null 2>&1 \
     && ! grep -rqF "$old" "$STATE" 2>/dev/null && ! grep -rqF "$old" "$proj2/semaprax.harness.toml" 2>/dev/null; then
    cell relocated-asset-paths pass "moved distribution runs and records no path of its former location"
  else cell relocated-asset-paths fail "relocated distribution failed or recorded its old path"; fi

  # Native-only task: needs a semaprax compiler built for this platform and git.
  STATE=$work/state3
  if [ -z "$compiler" ]; then
    cell native-task untested "no semaprax compiler for $(uname -s)-$(uname -m) was supplied (--compiler)"
  elif ! command -v git >/dev/null 2>&1; then
    cell native-task untested "git is not installed on this host"
  else
    proj3=$work/proj3; cp -R "$dist/share/smoke/project" "$proj3"; (cd "$proj3" && git init -q)
    H setup --project "$proj3" --preset native --yes >/dev/null 2>&1
    if H run "$proj3" --compiler "$compiler" --task "$dist/share/smoke/task.json" \
         --proposal "$dist/share/smoke/valid-proposal.json" --json 2>/dev/null | grep -q '"status":"approved-candidate-ready"'; then
      cell native-task pass "native-only run reached approved-candidate-ready"
    else cell native-task fail "native-only run did not reach an approved candidate"; fi
  fi

  echo "SUMMARY pass=$pass fail=$fail untested=$untested"
  if [ "$fail" -gt 0 ]; then echo "SMOKE FAILED"; exit 1; fi
  if [ "$untested" -gt 0 ]; then echo "SMOKE INCOMPLETE ($untested untested)"; exit 3; fi
  echo "SMOKE PASSED"
}

case $cmd in build) build "$@";; smoke-core) smoke_core "$@";; smoke) smoke "$@";; *) die "usage: harness_dist.sh build|smoke|smoke-core ...";; esac
