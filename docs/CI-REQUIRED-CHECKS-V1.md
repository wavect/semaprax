# Required CI checks v1

Audience: repository maintainers and administrators.

Status: **proposal, not applied.** The workflow-side half -- an aggregate gate
that cannot report success on a failed, skipped, cancelled, missing, or
foreign-commit shard -- is implemented and locally tested in this repository.
The repository-side half -- the ruleset that makes that gate a *required* check
-- is written out below as an exact, reviewable API request and has **not** been
executed. Creating or reviewing this document changes no repository setting,
ruleset, membership, credential, or branch permission. Nothing here is enforced
until a maintainer with administration authority applies it and records the
read-back evidence in [Applying the proposal](#applying-the-proposal).

## Observed configuration

Read-only requery on 2026-09-05T17:07:56Z, against `wavect/semaprax`
(organization-owned, public, default branch `main`):

```sh
gh api repos/wavect/semaprax/branches/main --jq '{protected, protection_url}'
gh api 'repos/wavect/semaprax/rulesets?includes_parents=true'
gh api repos/wavect/semaprax/branches/main/protection
```

| Query | Result |
| --- | --- |
| `branches/main` | `{"protected": false, "protection_url": ".../branches/main/protection"}` |
| `rulesets?includes_parents=true` | `[]` -- no repository or inherited ruleset |
| `branches/main/protection` | HTTP 404 `Branch not protected` |

`main` therefore admits any push its actor's write permission allows, including
a force-push or a branch deletion, with no check result consulted. This matches
the audit observation and is a configuration fact at the timestamp above, not a
property of the source tree. Requery before acting on it.

### Requery 2026-09-12T09:07:42Z

```sh
gh api repos/wavect/semaprax/branches/main --jq '{protected, protection_url}'
gh api 'repos/wavect/semaprax/rulesets?includes_parents=true'
gh api repos/wavect/semaprax/branches/main/protection
```

| Query | Result |
| --- | --- |
| `branches/main` | `{"protected": false, ...}` |
| `rulesets?includes_parents=true` | `[]` -- still no repository or inherited ruleset |
| `branches/main/protection` | HTTP 404 `Branch not protected` |

No drift from the 2026-09-05 baseline on protection state: `main` is still
unprotected, seven days into this issue's audit baseline.

**The cancellation streak is longer and more recent than the 2026-09-05
figure above, and is still open.** Requerying
`gh run list --branch main --workflow CI --limit 100 --json databaseId,status,conclusion,createdAt,updatedAt`
at the timestamp in this heading finds, of the last 100 `CI` runs on `main`:
98 `cancelled`, 1 `success`, 1 still `queued` (the push at the head of `main`
at requery time). The single `success` is the run created at
`2026-09-11T11:35:45Z` and completed at `2026-09-11T13:08:39Z`, for the commit
titled "fix(tests): stop two copies of one native helper sharing scratch
paths". Every one of the 89 pushes to `main` after that commit and before this
requery -- from `2026-09-11T13:12:21Z` through `2026-09-12T09:00:26Z` -- was
`cancelled`, plus the one `queued` run in flight at requery time that this
streak's own pattern says is very unlikely to complete either. That is
**19 hours 55 minutes** (13:08:39Z to 09:07:42Z) during which `main` produced
*no hosted CI verdict of either color* -- not the roughly thirteen hours
observed earlier in this same backlog's execution, because the push cadence
that causes the cancellations has continued at the same rate since the
2026-09-05 baseline was written. The mechanism is exactly the one already
named below: `concurrency.cancel-in-progress: true` grouped by `github.ref`
in `.github/workflows/ci.yml`, applied to `refs/heads/main` as to every other
ref, racing against a push cadence measured in minutes.

This is independent evidence for, not a change to, the design conclusion two
paragraphs below: a required-status-check rule on `main` is unsatisfiable
under the current push habit regardless of which check is named, because the
ref that would carry the required result is also the ref whose in-progress
run every subsequent push kills. See
[Prerequisite](#prerequisite) for the concurrency-scoping change this
implies, which is a `.github/workflows/ci.yml` edit and therefore outside
this document's authority to make.

### Why a naive rule would fail here

Two measured facts constrain the design more than the endpoints above do.

**Main almost never reaches a completed CI verdict.** Of the last 100 `CI` runs
on `main`, 97 were `cancelled`, 2 were `failure`, 1 was still running, and none
succeeded
(`gh run list --limit 100 --branch main --workflow CI --json conclusion,status`).
`.github/workflows/ci.yml` sets `concurrency.cancel-in-progress: true` grouped
by `github.ref`, and agents push to `main` from many worktrees faster than the
matrix completes, so each push cancels its predecessor. A cancellation is not a
passing and not a failing compiler result; it is an absent one. So `main` today
holds almost no commit with a completed verdict of any kind, and a rule turned
on against the current habit -- commit in a worktree, push straight to `main` --
would reject essentially every push, because the required result would have to
exist before the push that would produce it. The rule is only satisfiable if the
result is earned somewhere else first, which is what the next section is about.
(This has since gotten worse, not better: see
[Requery 2026-09-12T09:07:42Z](#requery-2026-09-12t090742z) for a measured
89-run, ~20-hour cancellation streak with zero completed verdicts.)

**The aggregate that existed was green in the failure case.** The `release-gate`
job aggregated all sixteen blockers under `if: ${{ success() }}`. That
expression leaves the job *skipped* whenever a blocker did not succeed, and
GitHub counts a check run whose conclusion is `skipped` as a **satisfied**
required status check. Requiring `Release gate` in that form would have produced
a green required check precisely when a shard failed. That job is fixed in this
change; see [The aggregate gate](#the-aggregate-gate).

## Operating model this proposal preserves

SEMAPRAX is developed by many agents working in parallel worktrees that commit
**directly to `main`**; [AGENTS.md](https://github.com/wavect/semaprax/blob/main/AGENTS.md) assumes a shared checkout and
sibling sessions rather than a review queue, and the remote carries dozens of
`agent/*` and `codex/*` branches from that workflow.

**Direct pushes to `main` remain allowed under this proposal.** No pull request
is required, no review is required, and no approval gate is introduced. What
changes is only that a commit must already carry a passing aggregate CI result
*before* it reaches `main`, which the existing branch-per-agent habit already
produces. Concretely, an agent's routine becomes:

1. commit on its `agent/*` branch and push that branch;
2. let `CI` run to completion on that exact commit;
3. fast-forward the same commit onto `main`.

The commit SHA does not change between steps 2 and 3, so the check result
already recorded against it satisfies the rule. This is the reason the proposal
uses a ruleset's required-status-check rule rather than a pull-request rule: the
former admits a direct push of an already-verified commit, the latter does not.

Requiring a check that can only run *after* the push, on the other hand, is
unsatisfiable by construction: the check cannot exist for a commit that the rule
refuses to admit. Any policy that requires checks on `main` implies the
branch-first routine above. That cost is stated here rather than discovered
after the rule is switched on.

## Current check-context inventory

The names below are what CI actually publishes, read from the live check runs on
`main` at the timestamp above and cross-checked against the workflow's matrix
expansion. A ruleset matches a check by this exact string, so the inventory is
recorded rather than paraphrased. The `verify-build` row is an authored
addition awaiting hosted execution.

```sh
gh api repos/wavect/semaprax/commits/main/check-runs \
  --paginate --jq '.check_runs[].name' | sort -u
```

| Job | Published context names | Count |
| --- | --- | --- |
| `public-generic-ownership-milestone` | `Public generic ownership milestone (ubuntu-latest \| macos-latest \| windows-latest)` | 3 |
| `std-library-depth` | `STD-08 bundled library depth` | 1 |
| `release-claim-reconcile` | `Release claim reconciliation` | 1 |
| `supply-chain` | `Dependency policy` | 1 |
| `component-runtime-v3` | `Private Wasmtime Component result runtime` | 1 |
| `wasm-scalar-exports-browser-v1` | `Public Wasm Scalar Exports v1 Chromium` | 1 |
| `project-product-acceptance-v1` | `Project Product Acceptance v1 (ubuntu-24.04 \| macos-15 \| windows-2025)` | 3 |
| `project-v1` | `Project Manifest v1 (ubuntu-24.04 \| macos-15 \| windows-2025)` | 3 |
| `native-rust-sdk-v1` | `Public Native Rust SDK v1 (ubuntu-latest \| macos-latest \| windows-latest)` | 3 |
| `verify` | `Rust ubuntu-latest`, `Rust macos-latest`, `Rust windows-latest` | 3 |
| `verify-build` | `Rust build ubuntu-latest`, `Rust build macos-latest`, `Rust build windows-latest` | 3 |
| `verify-tests` | `Rust tests <os> (unit \| integration-0 \| integration-1 \| integration-2)` over the three hosts | 12 |
| `desktop-native-product` | `Private desktop + native UI product (windows-2025 \| macos-15)` | 2 |
| `ios-static-cross-check` | `Private iOS static loader + host runtime` | 1 |
| `ios-swift-app-cross-check` | `Private Swift/iOS application + XCFramework runtime` | 1 |
| `android-emulator-cross-check` | `Private Android dynamic loader + host runtime` | 1 |
| `android-jni-app-cross-check` | `Private Android JNI/Kotlin application runtime (x86_64 \| arm64-v8a)` | 2 |
| `callable-host-sanitizers` | `Callable host ASan + UBSan` | 1 |
| `rust-host-address-sanitizer` | `Rust host ASan (nightly-2026-07-16)` | 1 |
| `msrv` | `Rust 1.88 minimum (unit \| integration-0 \| integration-1 \| integration-2)` | 4 |
| `release-gate` | **`Release gate`** | 1 |

The authored workflow additionally includes the three AGENT-06 client contexts
and the GEN-05B closure context. With `verify-build`, it declares 52 blocking
contexts plus the aggregate; the new build, library-depth, and public generic
ownership milestone contexts await hosted execution.
`release-artifacts`
(`Release artifact (<target>)`) and `publish-release` (`Publish tag release`)
run only on `refs/tags/v*` and are not candidates for a branch rule. The `Docs`
workflow adds `Build book` and, on `main` pushes only, `Deploy to GitHub Pages`.

## The aggregate gate

`.github/workflows/ci.yml` shards across twenty-two blocking jobs whose names and
matrix legs change often. Pinning twenty-plus expanded context names into a ruleset
would make every sharding change a repository-administration change. The
proposal requires exactly one context instead: **`Release gate`**, the job that
already aggregates every release blocker.

An aggregate is only worth requiring if it cannot be satisfied vacuously. The
`release-gate` job now:

- runs under `if: ${{ always() }}`, so it produces a real conclusion instead of
  the `skipped` conclusion that GitHub scores as a pass;
- passes `${{ toJSON(needs) }}` to `scripts/ci-required-checks.py` through the
  environment, and fails unless **every** upstream entry has
  `result == "success"` -- `failure`, `skipped`, and `cancelled` are all
  rejected by name;
- passes `--min-jobs 22`, so an accidentally emptied or narrowed `needs:` list
  cannot pass vacuously on `{}`;
- checks out the repository and compares `git rev-parse HEAD` against
  `${{ github.sha }}`, so a verdict cannot be attributed to another commit.

The `needs` context reaches the script through the environment, never through
the shell word list, so an upstream job name cannot be spliced into the command.
The script grants no filesystem, network, or token authority beyond reading that
variable and its own arguments.

`always()` has one visible consequence: it also runs when the workflow run
itself is cancelled, so a run superseded by `cancel-in-progress` now publishes a
red `Release gate` instead of publishing nothing. That is the honest report --
the superseded commit was never fully verified -- and it is the behaviour a
required check needs, since a skipped or absent aggregate is the thing that must
not read as success. Release semantics are unchanged: `release-artifacts` still
`needs: release-gate`, so a failing gate skips the tag jobs exactly as an
unsatisfied `success()` used to.

### Binding publication itself to the exact-tag gate

The aggregate above answers "did every blocker pass," which matters to
`main`'s required-status-check rule. Two further properties matter to the tag
workflow specifically, addressing [issue
167](https://github.com/wavect/semaprax/issues/167): that `release-artifacts`
and `publish-release` cannot run without `release-gate` having succeeded *in
the same run*, and that neither job can silently act on a commit other than
the one the gate verified.

- `release-artifacts`'s `if:` now spells `success()` out explicitly --
  `if: ${{ success() && startsWith(github.ref, 'refs/tags/v') }}` -- rather
  than relying on the fact that GitHub Actions ANDs a bare, status-function-free
  `if:` with an implicit `success()` today. That insertion is real GitHub
  behaviour, but it is undocumented enough that this workflow should not read
  as depending on it silently; the explicit form says the same thing and is
  what the contract test below pins.
- `release-artifacts` and `publish-release` each gained a step, immediately
  after checkout, that asserts `git rev-parse HEAD` equals `$GITHUB_SHA`. The
  gate already proves this once, in its own job
  (`--sha "${{ github.sha }}" --head-sha "$(git rev-parse HEAD)"`); GitHub
  Actions keeps `github.sha` constant across every job in one run, so the two
  downstream jobs re-deriving the same fact locally is redundant under normal
  operation. It stops being redundant the moment either job's checkout gains a
  `ref:` override in a later edit -- exactly the kind of drift a required
  check must catch rather than assume away -- and it makes "the gate's commit
  binding is dropped" a local, reviewable test failure at the job that
  actually builds the archive or calls `gh release create`, not only inside
  `release-gate`.

Publication takes no other route: `publish-release` is the only `gh release
create` invocation in this repository, it needs both `release-gate` and
`release-artifacts` directly, and its `if:` already carried an explicit
`success()`.

### Evidence

| Property | Evidence |
| --- | --- |
| Failed, skipped, cancelled, malformed, or absent `result` is rejected | `tests/offline_package/ci_release_gate.rs::aggregate_gate_rejects_failed_skipped_cancelled_missing_and_foreign_results` drives `scripts/ci-required-checks.py` over synthetic `needs` contexts |
| A missing shard, or an emptied `needs:`, cannot pass | same test, `--min-jobs` cases |
| A verdict from another commit is rejected | same test, checked-out-versus-reported commit cases |
| A new CI job cannot silently escape the aggregate | `tests/offline_package/ci_release_gate.rs::every_job_that_is_not_a_tag_only_release_step_is_a_release_blocker` derives the job inventory from the workflow and compares it to the gate's `needs` |
| The workflow keeps the fail-closed shape | `tests/offline_package/ci_release_gate.rs::release_gate_fails_closed_over_the_complete_blocker_set`, which now forbids `success()` in that job |
| `release-artifacts` cannot run, or build an archive under a commit other than the one the gate verified, without `release-gate` succeeding on the exact tag commit | `tests/offline_package/release_workflow.rs::tag_artifacts_are_exact_blocking_children_of_the_release_gate` pins the explicit `success()` guard, the `needs: release-gate` edge, and the `git rev-parse HEAD` == `$GITHUB_SHA` step |
| `publish-release` cannot call `gh release create`, or do so under a commit other than the one the gate verified, without `release-gate` and `release-artifacts` both succeeding | `tests/offline_package/release_workflow.rs::publication_waits_for_all_artifacts_and_owns_the_only_write_authority` pins both `needs` entries, the `success()` guard, and the same commit-binding step |

Run them with:

```sh
cargo test --locked --test offline_package ci_release_gate
```

The module depends only on `std`, `python3`, and the two files it reads, so it
also compiles and runs standalone when a full workspace build is not affordable:

```sh
sed 's#env!("CARGO_MANIFEST_DIR")#"'"$PWD"'"#g' \
  tests/offline_package/ci_release_gate.rs > /tmp/gate.rs
rustc --edition 2021 --test -o /tmp/gate /tmp/gate.rs && /tmp/gate --test-threads 1
```

Each case has a negative control: reverting the gate's guard, deleting one entry
from its `needs`, adding a CI job that is not wired into the gate, and
short-circuiting `failures()` to accept everything each fail exactly one of the
tests above and nothing else.

This is deterministic local evidence about the gate's decision logic. It is not
hosted evidence that a required rule blocked a real push; that is
[what remains unapplied](#what-remains-unapplied).

### Deliberately not required: `Build book`

`.github/workflows/docs.yml` produces the only other check contexts on `main`:
`Build book`, and `Deploy to GitHub Pages`, which is a `main`-push-only
deployment and is not a verification result at all. `Build book` is **not**
proposed as a required context, because its `push` trigger is filtered to
`branches: [main]`. A commit verified on an `agent/*` branch would therefore
never carry a `Build book` result, and the rule would be unsatisfiable for
exactly the route this proposal preserves.
Making it required means first widening that trigger to every branch, which adds
a book build to every branch push (the pinned mdBook installation is cached).
That trade is a
maintainer decision and is deliberately left open rather than made here.
Documentation regressions are already caught locally by `tests/documentation.rs`
per [Quality gates](QUALITY-GATES.md#documentation-changes).

## Proposed rule

A repository ruleset named `main change integrity`, targeting `refs/heads/main`.

| Field | Value | Rationale |
| --- | --- | --- |
| `target` | `branch`, include `~DEFAULT_BRANCH` | `main` only; agent branches stay unconstrained |
| `enforcement` | `active` | the `evaluate` dry-run mode requires GitHub Enterprise Cloud; confirm the plan before relying on it |
| `rules[].type` | `deletion` | `main` must not be deletable |
| `rules[].type` | `non_fast_forward` | a force-push silently discards a sibling agent's commit; this is the single highest-value rule and costs the workflow nothing |
| `rules[].type` | `required_status_checks` | admits only a commit already carrying a green aggregate |
| `required_status_checks[]` | `Release gate`, `integration_id: 15368` | the one aggregate context; `15368` is the GitHub Actions app, confirmed from live check runs, and pinning it stops another app from claiming the name |
| `strict_required_status_checks_policy` | `false` | requiring branch-freshness would serialize parallel agents against each other |
| `do_not_enforce_on_create` | `false` | not applicable to an existing branch |
| `bypass_actors` | organization admin (`actor_id: 1`, `actor_type: OrganizationAdmin`, `bypass_mode: always`) | the recovery path in the next section |

No `pull_request` rule, no `required_signatures`, no `creation`, no
`required_linear_history`, no `commit_message_pattern`. Each of those would
change the operating model rather than protect it.

### Staging

The two halves are separable, and the first is free. A maintainer who is not yet
ready to adopt the branch-first routine can apply `deletion` and
`non_fast_forward` alone: they impose no check requirement, block no push that
CI would have blocked, and still close the force-push and deletion holes that
`main.protected=false` leaves open today. Add `required_status_checks` in a
second edit once the branch-first routine is in use and, if the maintainers want
`main` runs to complete, once `concurrency.cancel-in-progress` no longer cancels
`main` (see [Prerequisite](#prerequisite)).

### Prerequisite

`required_status_checks` evaluates the check result already recorded against the
pushed commit, so a cancelled `main` run does not itself block the push: under
the branch-first routine above, the commit's passing result was recorded
against its `agent/*` ref, whose `concurrency` group (keyed on `github.ref`) no
other agent's push shares, so that run is never cancelled by a sibling agent
working on a different branch. But while 98 of the last 100 `main`-ref CI runs
are cancelled -- see [Requery 2026-09-12T09:07:42Z](#requery-2026-09-12t090742z)
for the exact streak, 89 consecutive cancellations spanning almost 20 hours
with zero completed verdict of either color -- `main` carries no completed
verdict of its own once the fast-forwarded commit's push re-triggers `CI` on
`refs/heads/main` itself. That absence does not block a merge, but it does
block every consumer that reads "the latest completed run on `main`" -- the
GitHub commit-status UI, `docs/CI-REQUIRED-CHECKS-V1.md`'s own audit script
once it exists, and a human skimming `main` for the project's actual health --
and it means post-push evidence for a released or claimed commit has to be
re-run by hand.

**Recommended fix: scope `cancel-in-progress` to non-default refs, not a merge
queue.** Maintainers who want `main` runs to finish should change
`.github/workflows/ci.yml`'s `concurrency` block to
`cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}` (or an equivalent
`group`/`cancel-in-progress` pair that only ever cancels non-`main` refs). This
is a `.github/workflows/ci.yml` edit and is out of this document's authority to
make; it is recorded here as the exact change, for a maintainer or the
integrating coordinator to apply. A GitHub merge queue is **not** recommended
as a substitute: it requires branch protection with pull requests enabled and
serializes merges one at a time through the queue, which conflicts with the
operating model this proposal explicitly preserves --
[direct pushes from many parallel agent worktrees](#operating-model-this-proposal-preserves)
with no pull request required. A queue would either throttle this backlog's
merge throughput to one commit at a time, defeating the reason the many-worktree
model exists, or, if agents kept fast-forwarding around it, leave the same
`concurrency`-driven cancellation in place for the direct pushes that still
happen. The ref-scoped `cancel-in-progress` change achieves the same end
(`main` gets a completed verdict) without changing who may push or how, and
that is why it is the recommendation. It changes CI compute cost materially
(no push to `main` cancels a prior one, so overlapping `main` runs queue up
back-to-back rather than being killed) and is left to the maintainers; it is
not required for the required-status-check rule itself to function, and it is
not changed by this proposal.

## Bypass and emergency recovery

**Who may bypass.** Organization administrators of `wavect`, through the
`bypass_actors` entry above, with `bypass_mode: always`. No bot, app, token, or
`write`-role principal is granted bypass. Agents are not bypass principals: an
agent that cannot get a green aggregate must fix the change, not route around
the rule.

**Recovery without broadly disabling verification.** In order of preference:

1. **Push the fix through the ordinary route.** Commit the repair on an
   `agent/*` branch, let `Release gate` go green on that exact commit, then
   fast-forward `main`. This is the normal path and needs no authority at all.
2. **Bypass one push.** An organization administrator pushes the specific repair
   commit directly. The ruleset stays `active` for everyone else and for every
   other commit; only that one push is exempt, and the bypass is recorded as a
   rule suite
   (`gh api 'repos/wavect/semaprax/rulesets/rule-suites?rule_suite_result=bypass'`).
   Prefer this over any rule edit.
3. **Narrow the rule, never the ruleset.** If the aggregate itself is broken --
   a runner outage, an action yanked upstream -- remove only the
   `required_status_checks` rule from the ruleset, leaving `deletion` and
   `non_fast_forward` `active`. Force-push and deletion protection must not be
   collateral damage of a CI outage.
4. **Last resort.** Setting `enforcement` to `disabled` removes every protection
   at once and must be paired, in the same session, with an issue recording who
   disabled it, why, and the commit at which it is restored.

Steps 3 and 4 are administration actions; record each in the changelog with the
restoring commit. Deleting the ruleset is never the recovery step -- it discards
the configuration and its history together.

## Applying the proposal

These commands mutate repository configuration. They require administration
authority and are **not** run by preparing this document.

Create the ruleset:

```sh
gh api --method POST repos/wavect/semaprax/rulesets \
  --input - <<'JSON'
{
  "name": "main change integrity",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [
    { "actor_id": 1, "actor_type": "OrganizationAdmin", "bypass_mode": "always" }
  ],
  "conditions": {
    "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": false,
        "do_not_enforce_on_create": false,
        "required_status_checks": [
          { "context": "Release gate", "integration_id": 15368 }
        ]
      }
    }
  ]
}
JSON
```

For the staged variant, send the same body with the `required_status_checks`
rule omitted, and add it later with `--method PUT` on
`repos/wavect/semaprax/rulesets/<id>`.

The equivalent UI route is **Settings -> Rules -> Rulesets -> New branch
ruleset**, targeting the default branch, with *Restrict deletions*, *Block force
pushes*, and *Require status checks to pass* -> *Release gate*, and
*Bypass list* -> *Organization admin* -> *Always*.

### Read-back

Confirm the applied rule, and record the output in this section together with
the commit and timestamp:

```sh
gh api 'repos/wavect/semaprax/rulesets?includes_parents=true' \
  --jq '.[] | {id, name, target, enforcement}'
gh api repos/wavect/semaprax/rulesets/<id> \
  --jq '{name, enforcement, bypass_actors, conditions, rules}'
gh api repos/wavect/semaprax/rules/branches/main \
  --jq '[.[] | {type, ruleset_id}]'
```

The third command is the decisive one: it reports the rules GitHub actually
evaluates for `main`, rather than the rules a ruleset declares. Note that
`gh api repos/wavect/semaprax/branches/main --jq .protected` reports the legacy
branch-protection flag and may still read `false` under a ruleset; it is not the
read-back for this proposal.

### Behavioural test after authorization

Read-back proves the rule exists, not that it blocks. On a scratch branch, push
a commit that makes one release blocker fail deliberately, let `CI` finish, and
attempt the fast-forward onto `main`. The push must be rejected, and
`Release gate` must be **red** rather than skipped. Repeat with a commit whose
run was cancelled: the push must also be rejected, since a cancelled run leaves
no `success` conclusion. Record both attempts, with run links, in this section.

## What remains unapplied

Unapplied, and requiring administration authority the preparing agent does not
have and must not acquire:

- the ruleset itself -- `main` is unprotected and the ruleset inventory is empty
  as of the timestamp in [Observed configuration](#observed-configuration);
- read-back evidence of the resulting rule;
- the behavioural test that a deliberately failing required job actually
  prevents the update route.

Applied and locally verified in this repository:

- the fail-closed aggregate gate and its script;
- the local tests that a failed, skipped, cancelled, missing, or foreign-commit
  shard cannot appear as aggregate success.

Until the first list is done, this repository has an aggregate check that
reports the truth and no rule that consults it.
