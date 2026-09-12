# Limited-agent tutorial: inspect, review, test, publish

Status: worked walkthrough with locally executed, exact-subject command
output for `inspect`/`review`/`test`; `publish` is documented and cited to
existing hosted-evidenced regressions rather than executed inline here (see
"What this tutorial does not run" below).

Audience: agent and integration authors wiring a *limited* agent — one that
never needs `source_write` or `candidate_only` authority — against the
public CLI.

[Agent Skill Bundle v1](AGENT-SKILL-BUNDLE-V1.md) defines `PUBLIC_WORKFLOW`:
ten verbs, each stamped with exactly one of five authority classes
(`read_only`, `candidate_only`, `source_write`, `test_execute`,
`publication`). This tutorial follows one useful subset of that workflow —
`inspect` → `review` → `test` → `publish` — chosen because it never needs
`source_write` (`apply`/`repair`) or `candidate_only` (`propose`/`rebase`):
a bot that only reads, reviews someone else's proposed patch, runs the
project's tests, and publishes an already-approved result never needs write
access to source at all. That is what "limited agent" means here: fewer
authority classes than the full ten-verb workflow, not a smaller CLI.

| Step | Verb | Authority class | Wraps |
| --- | --- | --- | --- |
| 1 | `inspect` | `read_only` | `semaprax graph` |
| 2 | `review` | `read_only` | `semaprax review` |
| 3 | `test` | `test_execute` | `semaprax test` |
| 4 | `publish` | `publication` | `semaprax project-candidate-git-publish` |

Every command below is the same top-level `semaprax` command
`PUBLIC_WORKFLOW` names; nothing here is a new command surface, matching
[Agent Skill Bundle v1](AGENT-SKILL-BUNDLE-V1.md#the-public-semantic-workflow).

## 1. `inspect` — read the semantic graph (`read_only`)

Start from the committed example used throughout this repository's docs:

```sh
semaprax graph examples/meaning.spx
```

Executed against exact subject `46c50ac45850c6a1b4fe97182574610021f784f2` on
Darwin arm64, this prints the module's `semaprax.graph.v10` document,
beginning:

```json
{"schema":"semaprax.graph.v10","revision":"sha256:42aeae2650d15b1e44b8fd6d8a7ce6018d61f43e0e7988a58da2426b2f0c1657","prelude":{"schema":"semaprax.prelude.v1","digest":"sha256:d37bad7e3911669bbf2c66b25c8b31d5c2e36eb181cc54fdc86c3a49a8fb9c5e"},...
```

`inspect` opens no file for writing and starts no session; it is a pure read
of the checked graph. A limited agent can call this on any `.spx` file it can
read, with no other authority.

## 2. `review` — preview a proposed change (`read_only`)

The committed [`examples/rename.spatch`](../examples/rename.spatch) is the
same three-line patch [`examples/README.md`](../examples/README.md#semantic-change-input)
uses for `impact`; `review` accepts the identical `<file> <patch.spatch>`
shape. As committed, its `base` line names a placeholder
(`GRAPH_REVISION`) so a stale copy never accidentally reviews clean: running
`review` against the committed file fails closed instead of silently
reviewing the wrong base:

```sh
semaprax review examples/meaning.spx examples/rename.spatch
```

```text
error[SPX-G409]: stale semantic patch: expected graph GRAPH_REVISION, current graph sha256:42aeae2650d15b1e44b8fd6d8a7ce6018d61f43e0e7988a58da2426b2f0c1657
  help: regenerate the patch against the current semantic graph
```

Substituting the real revision `check` reports (into a working copy — never
edit the committed fixture; `examples/README.md` explains why) and rerunning
produces a `semaprax.semantic-review.v1` document. Executed for the same
exact subject, its `sections` classify the rename as
`bounded_no_change`/`change` per section (behavior unchanged, API identity
renamed but stable, no new effects, ownership and cleanup unchanged), and its
embedded `semaprax.semantic-impact.v1` evidence names exactly one affected
call site (`app.main`). Its `nonclaims` array says explicitly what this is
not: no proof-carrying patch, no authenticated provenance, no lock/stage/
apply/commit authority, and no test or target execution — `review` only
previews.

## 3. `test` — run the project's tests (`test_execute`)

`test` accepts a manifest as well as a standalone file. Against the committed
multi-file example:

```sh
semaprax test examples/calculator-project/semaprax.toml
```

Executed for the same exact subject, this prints:

```text
project tests passed
```

`test_execute` is exactly that authority and nothing more: it runs the
project's own declared test module through the development path and reports
pass/fail. It does not write source, stage a candidate, or publish anything.

## 4. `publish` — land an already-approved result (`publication`)

`publish` wraps `project-candidate-git-publish`:

```text
semaprax project-candidate-git-publish <manifest> <capsule.json> <approved-candidate-digest> <host-policy.json>
```

Unlike the first three steps, this command cannot run against a bare `.spx`
file and a hand-edited patch: `capsule.json` is a complete-candidate recovery
capsule, and `<approved-candidate-digest>` must match it exactly — both come
from the managed-workspace protocol (`workspace/open`, candidate
construction, `candidate/source-review`, capsule export), not from this
tutorial's simple file-plus-patch shape. [Candidate Git publication
CLI v1](CANDIDATE-GIT-PUBLICATION-CLI-V1.md) is the exact reference for the
command and the bounded `host-policy.json` shape (repository, ref, base
commit, author identity, message, and process limits — no ambient identity,
clock, or signing service). A limited agent's `publication` authority is
exactly this one command against an already-produced capsule and an
already-external approval of its digest; it is not authority to construct or
approve the candidate itself.

This tutorial does not fabricate that capsule inline: producing one requires
the full managed-workspace sequence documented in [Project graph-operational
Git workflow v1](PROJECT-GRAPH-OPERATIONAL-GIT-WORKFLOW-V1.md), whose twelve
connected steps — from opening an authenticated workspace image through the
real restricted Git subprocess adapter's compare-and-swap ref update — are
exercised by `tests/project_graph_operational_git_workflow_v1.rs` and its
runner, [`scripts/graph-operational-evidence.py`](GRAPH-OPERATIONAL-EXECUTION-EVIDENCE-V1.md).
That fixture's regression evidence is recorded as **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md); this tutorial cites it
rather than re-deriving a new capsule end to end, so it does not claim to
have executed `publish` itself.

## What this tutorial does not run

`inspect`, `review`, and `test` above were executed locally, against the
exact subject named in each step, on Darwin arm64 — that is local evidence
for one host and one commit, not a hosted or cross-platform claim. `publish`
is documented, not executed here; its cited fixture's own hosted status is
stated above rather than repeated as this tutorial's own. None of the four
steps grants filesystem, network, or process authority beyond what its own
command already has; a limited agent combining exactly these four verbs gets
`read_only` + `test_execute` + `publication` and nothing else — never
`source_write` or `candidate_only`.
