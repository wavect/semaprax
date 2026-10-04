# Harness updates v1 (HN-05)

Status: additive development-harness specification (HN-05); local macOS aarch64 evidence only.

Audience: toolchain contributors, harness adapter authors and maintainers of the curated catalog.

Keeps curated skills and external adapter packages current without making any run depend on a
floating upstream. Implementation: `crates/semaprax-harness/src/updates/`; verb `semaprax harness updates`.
Diagnostics use letter `U` (`SPX-HPU001`..`016`). Identity rules are those of
[HARNESS-ARTIFACT-IDENTITY-V1](HARNESS-ARTIFACT-IDENTITY-V1.md); the skill profile is
[HARNESS-SKILLS-V1](HARNESS-SKILLS-V1.md).

## Sources and channels

A source is `{id, kind: skill|adapter, repo, subpath?, channel, head_branch, files?}`. Catalog skills are
seeded from `catalog.json` (active revision = the embedded one); other skills and adapter packages are
registered with `updates add`. Skills and adapters use one resolver, one pipeline and one state file; only
the format validator differs (`SKILL.md` profile vs `harness-provider.json` descriptor). There is no
brand-specific branch. An empty `files` list means the whole subtree under `subpath`.

| Channel | Resolves to |
|---|---|
| `latest-stable` | highest `X.Y.Z` release that is neither draft nor pre-release |
| `range:<req>` | highest stable release satisfying comparators (`>=,>,<=,<,=,^,~`, comma separated) |
| `commit:<40 hex>` | that commit |
| `head[:branch]` | the branch tip at check time, **resolved once to a commit and recorded** (`resolved_head`) |

Annotated tags are peeled to the underlying commit. Every content read (tree, blob) is addressed by commit
or blob sha; the only request that names a mutable branch is the one resolution call. A recorded tag
(active or pending) that now points at a different commit is `SPX-HPU003` (moved tag) and is never staged.

## Transport

The harness has no remote HTTP client (`host/http.rs` is loopback-only). Remote reads use a pluggable
`Fetcher`:

- `GitHubCliFetcher`: the host's already-authenticated GitHub CLI at an explicit absolute path
  (`updates approve-policy --gh /abs/gh`, or `--gh`). Cleared environment with a short allow-list from the
  explicit `Environment`, null stdin, bounded output, wall-clock timeout (policy `timeout_ms`), read-only
  `gh api` GETs, `https://github.com/<owner>/<repo>` origins only (`SPX-HPU016`; relative `gh` is `SPX-HPU015`).
  Blobs are fetched raw by git blob sha and verified twice: git SHA-1 object id and size from the tree, then
  sha256 of the bytes into the artifact inventory. A hash proves byte identity, not publisher authenticity;
  the origin repository and its tag/commit are the trust anchor, and no upstream installation hook, script
  or package manager is ever executed while fetching.
- `MemoryFetcher` / `DirectoryFetcher`: simulated upstream (tests, offline demos, `--fixture-dir`), logging
  every request.

## Pipeline

resolve -> stage (tree, bounded blobs, truncation/hash checks, path and entry-type checks) -> extract into the
content-addressed store (`<home>/artifacts/<hex>`) -> validate (skill: HN-03 `SKILL.md` profile and bundle
smoke; adapter: strict `Descriptor::parse` and entry-in-closure) -> optional caller gate -> diff against the
active revision -> decision. Hard failures (`SPX-HPU003` moved tag, `004` changed identity, `005` truncated or
mismatching bytes, `006` escaping path/symlink/submodule, `007` validation or gate failure, `011` revoked) are
recorded as `rejected` with a bounded message; the active revision never changes and the incompatible latest
stays visible in `status`. Soft findings become **review reasons**: `changed-publisher`, `changed-license`,
`widened-permissions:<x>` (new `allowed-tools`, `hooks`, descriptor permissions), `new-executable:<path>`,
`changed-executable:<path>`, `adapter-code-changed`, `initial-install`.

Decision: no reasons and an approved policy with `auto_content` and kind `skill` -> activate; anything else is
`pending` with its reasons (`auto-update-not-approved` when only the policy is missing). Adapters never
auto-activate (executable code is not updated on semver alone), and activation of an adapter publishes its
closure snapshot and records it; re-trust of the adopted installation stays with `adopt`/`trust`.

## Activation, sessions, rollback

Activation is one atomic write of `<home>/updates/state.json` (`semaprax.updates-state.v1`) after the snapshot
re-validates. Active sessions keep the revision they locked and keep loading it from the immutable store;
`session_pin(ctx, id)` and `effective_set(home)` (the embedded catalog with activated skills overlaid through
`OfficialSet::with_revision`) give a session started now the new revision. Up to 5 previous revisions are
retained. `rollback` restores the newest retained non-revoked revision and *holds* the rolled-back commit:
it is re-staged as pending with `rolled-back-hold` and never re-activated automatically. User overrides,
skill modes and locally derived skills live outside the state and are never read or written by update,
rollback or revoke.

Revocation (`updates revoke`, or the fetcher's `revoked`) falls back to a safe previous revision, or leaves the
source `unavailable` (the catalog skill is withdrawn from the effective set). A revoked revision is refused by
`apply` (`SPX-HPU011`) and never reactivated.

## Policy, frozen, offline

`updates approve-policy [--auto-content] [--ttl-secs N] [--timeout-ms N] [--gh PATH]` records one onboarding
approval of routine checks and compatible content-only updates. The only network paths are explicit
`updates check|propose|maintain`; `maintain` is the session-start hook entry: it runs only if the policy is
approved and the TTL expired, never fails the session, and converts any failure to a bounded notice
(<= 200 characters, `status`). Nothing on compile/check calls the module and nothing runs mid-invocation.

`--offline` makes zero requests and reports cached state; staged candidates can still be applied. `--frozen`
implies offline and additionally refuses `apply` and `rollback` (`SPX-HPU010`), so locked artifacts reproduce.
Outage, rate limit or timeout during a check leaves state untouched and adds the notice.

## Verbs

`check [id...]`, `status [id...]`, `apply <id> [--approve]` (review reasons need `--approve`, `SPX-HPU008`),
`rollback <id>` (`SPX-HPU014` when nothing is retained), `revoke <id> <commit|digest>`, `approve-policy`,
`add <id> --kind skill|adapter --repo URL [--subpath P] [--channel C] [--branch B] [--file rel=upstream]`,
`propose [id...]`, `maintain`; flags `--json --frozen --offline --gh --fixture-dir`.

## Maintainer automation

`updates propose` resolves each catalog skill's channel, stages the candidate into a throwaway store with the
same pipeline and prints `semaprax.catalog-proposal.v1`: status `current|proposed|failed-conformance|unresolved`,
proposed tag/commit/bundle digest/per-file blob and sha256 (the fields of `catalog.json`), the diff, review
reasons and conformance results. It writes nothing to the catalog or the user's state and merges nothing; a
maintainer edits `catalog.json` and the embedded snapshots by hand.

## Diagnostics

001 bad source/channel/argument; 002 channel did not resolve; 003 moved tag; 004 changed identity; 005 truncated,
oversize or hash-mismatching bytes; 006 escaping path, symlink, submodule or bound; 007 validation, smoke or gate
failure; 008 explicit review required; 009 upstream unavailable (offline, outage, rate limit, timeout); 010 frozen
or no fetcher; 011 revoked; 013 state corrupt or no harness home; 014 nothing to roll back to; 015 invalid `gh`
executable; 016 unsupported origin.

## Evidence and limits

Fixture tests cover every acceptance criterion; one provisioned test (`SEMAPRAX_GH`) resolves Ponytail and
Caveman `latest-stable` with the real `gh` and reproduces their embedded bundle digests byte-for-byte from the
real blobs. Limits: GitHub only; releases (not bare tags) define stable; publisher authenticity beyond origin
and tag is not proven (no signature verification in v1); the `DefaultSkills` constructors still use the
embedded set until the workflow adopts `effective_set`; revocation lists from a real origin are not read
(`GitHubCliFetcher::revoked` is empty), only local `revoke` and fixtures.
