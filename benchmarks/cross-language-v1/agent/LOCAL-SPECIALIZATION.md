# Local held-out control execution

Status: experimental, opt-in implementation for #326. The transport, orchestration,
receipt audit and refusal paths have synthetic local tests; a genuine Ollama model
run and the Darwin native sandbox execution have not been demonstrated by this
change. It is not issue-closure, model-performance or independent-review evidence.
Audience: the experiment operator, independent reviewer and held-out data custodian.

## Scope

`specialization_local.py` wraps the **existing** `specialization_protocol.build_plan`
and frozen `run.py` scorer. It does not make the original `not_authorized` protocol
executable, modify `LiveTransport`, or create a second specialization protocol.
The 81 original slots remain: `base`, `guided`, `constrained`, nine task IDs and
three repetitions. The later iterative-repair task's nine extra slots stay outside
this study; the prior accounting command retains their classification.

The default proposed local model is `qwen2.5-coder:3b`. Preparation binds the full
installed manifest digest, complete `/api/show` metadata digest and daemon version;
execution checks them before and after each generation. A mutable tag by itself
is insufficient. No command pulls a model, installs a package, reads an API key or
creates a hosted job. No training, task-specific tuning, tools or adapted arm is
introduced. There is no promotion of a comparison adapter: #322's official
TypeScript-only comparison support decision remains unchanged.

The current frozen Semaprax discount port and its written fractional oracle still
disagree. This implementation does **not** patch that oracle or run its nine cells.
They remain `unexecuted: unresolved_fractional_discount_oracle`. Thus at most 72
cells can produce generations in this exact profile. Adding or fixing an oracle
requires reviewed successor inputs, not a changed file under an old digest.

The original fixture oracle digest and the pinned current scorer differ. The
prepared plan records both and requires review of the change. It does not inherit
approval from a historical fixture, TypeScript execution, or an installed compiler.
All 298 comparison-source files and predecessor wire profiles remain unchanged.

## Control design and bounded authority

| Item | Local execution choice |
| --- | --- |
| Model | Explicit installed Qwen2-family GGUF completion model; default `qwen2.5-coder:3b`; full digest required |
| Model connection | Only literal `http://127.0.0.1:<port>`; default port 11434; no DNS, proxy lookup, credentials, redirects, cloud model references, pulls or installation hooks |
| Adaptation | None; generic versioned language guidance only for `guided` and `constrained` |
| Prompt construction | Public requirements, candidate interfaces, manifest and unchanged public scaffolds; no candidate implementations, EQUIVALENCE documents, README/AGENTS text or hidden trees |
| Constrained control | Same prompt as `guided`; additionally a closed JSON Schema for exactly the permitted candidate files |
| Sampling | Temperature 0.2; top-p 0.95; repeat number as seed, identical across controls; maximum 2,048 output tokens; 8,192 context tokens |
| Original ceilings | Unchanged 32,000 input / 16,000 output / 48,000 total tokens and maximum two retries; this implementation uses **zero automatic retries** and one generation per cell |
| Prompt admission | Conservative raw Qwen2 byte-BPE reservation, including explicit ChatML framing and 32 spare tokens; refuse before generation if it cannot fit the token/context ceilings |
| HTTP bounds | 128 KiB request, response and decoded canonical response; 120-second absolute deadline per request, including slow headers; no streaming or compressed-body expansion |
| Scoring bounds | Existing 120-second process-group deadline, existing per-stream output limits, 512 MiB maximum compiler snapshot, existing source/result limits; no increased archive limits |
| Cost | Zero incremental provider charges; electricity, hardware and operator/reviewer cost are **not measured** |
| Native execution | Existing Darwin arm64 / macOS 26.5.1 build 25F80 host check; explicitly supplied compiler bytes copied privately and hash-checked; no fallback on another host |
| Native isolation | An inverted `sandbox-exec` profile retains Darwin's required anonymous VM setup, then denies network, forks, non-tool execution, external reads and external writes; only the exact compiler may execute, only the current public or hidden phase is writable, and protected system libraries are readable |

The localhost client **does not sandbox an independently running Ollama daemon**.
The operator must provision and isolate that daemon, disable Ollama cloud features
and unwanted egress, and bind its actual model origin/runtime in the preflight
review. An API-reported manifest digest is not a measurement of every weight byte.
Pre/post metadata checks also do not exclude a hostile administrator temporarily
substituting a daemon/model and restoring it between checks. Exclusive trusted
local custody is an explicit prerequisite, not something a JSON receipt proves.
The same-user operator and protected OS runtime are trusted; file modes are not
an administrator-resistant security boundary.

Ollama's documented local-mode configuration is `OLLAMA_NO_CLOUD=1` or
`disable_ollama_cloud` in its configuration. The client rejects cloud-shaped model
metadata but cannot verify how an existing server was launched. The independent
review must retain physical no-egress and native sandbox negative-control evidence
for the exact installation. No such physical evidence is produced by mock tests.

Before the first generation, all eight eligible reference candidates must pass
both public and hidden scoring inside the native sandbox. This checks that the
scorer/host can execute those tasks. These records are explicitly **not** model
trials. The model never receives the reference bodies or hidden scoring output,
and it gets no automatic repair attempt after a failed candidate.

The source projection addresses a concrete legacy-prompt risk: `prompts.py`
includes complete EQUIVALENCE documents, which contain hidden-vector/mutant
information. The new runner does not call that builder. This does not retroactively
claim that legacy prompts or previously exposed material were leakage-free.

## Prepare the exact local inputs

Run from the repository root on the reviewed host. The compiler path must refer
to the intended built Semaprax executable. The daemon and the selected model must
already be provisioned. There is deliberately no global-install or model-download
step in this command. Use canonical absolute paths, including `/private/...`
rather than a symlinked `/tmp` on macOS.

```sh
mkdir -m 700 ./specialization-326-evidence
EVIDENCE="$(cd ./specialization-326-evidence && pwd -P)"
COMPILER="$(python3 -c 'import pathlib; print(pathlib.Path("target/debug/semaprax").resolve())')"

python3 benchmarks/cross-language-v1/agent/specialization_local.py prepare \
  --compiler "$COMPILER" \
  --endpoint http://127.0.0.1:11434 \
  --model qwen2.5-coder:3b \
  --operator "Kevin Riedl" \
  --output "$EVIDENCE/plan.json"
```

Preparation makes metadata requests only. It emits a canonical plan, its SHA-256,
and `plan.review-template.json`, explicitly marked **pending**. It does not ask
for or obtain credentials and does not call `/api/generate`. Missing compiler,
missing model, unsupported metadata or changed source fails closed.

The independent reviewer must inspect the actual inputs, origin evidence,
no-egress/native-sandbox controls, public prompt projection, unchanged split,
explicit oracle discrepancy and data custody. They must record evidence for all
six checks, their identity, independence and date. An operator cannot approve their
own review. The implementation author cannot stand in for the reviewer. Generating,
filling or hashing a template without performing the review does not satisfy this
requirement. Reviewer identity/evidence are trusted operator-supplied records, not
cryptographically authenticated by this program.

Retain the completed review as `preflight-review.json`. Obtain its expected digest
from that independently reviewed record; do not replace the pin with the hash of
an arbitrary file merely to bypass a mismatch.

If the user has explicitly waived independent human review for this issue, add
`--review-waiver` to `prepare`, complete the resulting operator-attestation
record, and add `--accept-review-waiver` to `run`. That distinct record binds the
same six checks and plan digest, but reports an operator technical attestation
with `independent_human_review: waived_by_user`; it never names or invents an
independent reviewer or data custodian.

After a complete run, the waiver path emits a separate summary-digest-bound
post-run operator-attestation template. Its receipt audit, control comparison
with uncertainty, and leakage/data-custody checks must all carry retained
evidence references. `finalize` accepts only that completed exact record and
only when all 72 eligible generations and nine explicit oracle exclusions are
accounted for. The closure record remains an operator technical review under
the waiver; it is not an independent-human review claim.

## Execute only after real preflight review

Set `PLAN_SHA256` to the preparation receipt and `REVIEW_SHA256` to the approved
review's independently supplied digest. Both use `sha256:` followed by 64 hex digits.

```sh
python3 benchmarks/cross-language-v1/agent/specialization_local.py run \
  --plan "$EVIDENCE/plan.json" --plan-sha256 "$PLAN_SHA256" \
  --review "$EVIDENCE/preflight-review.json" --review-sha256 "$REVIEW_SHA256" \
  --output "$EVIDENCE/run-01"
```

Run rechecks the complete prepared input record and installed model metadata. It
refuses missing approval, changed source/parameters, changed identities or an
existing output directory. It never silently resumes or repeats a run.

Malformed/truncated candidate JSON is a measured failed model attempt with actual
returned usage; it is not repaired or replaced with a reference answer. Ambiguous
HTTP failure, model identity drift or scoring-authority failure stops subsequent
calls. Unknown usage/correctness stays `null`; the remaining cells are explicitly
unexecuted. Validated response/usage is retained even when later scoring fails.

Ctrl-C during model work is recorded conservatively as an ambiguous attempt when
sending may have started. During a bounded compiler process, SIGINT is deferred
until the existing process-group runner finishes collecting it, so a caught
interrupt does not orphan the candidate process. Forced termination or storage
failure can leave incomplete immutable receipts; an absent terminal record is
not evidence that an intended model call did not execute. Do not rerun it silently.
The audit refuses incomplete result sets.

## Retain and audit results

`run-01` is private and contains the plan, preflight review, separate non-model
reference checks, per-cell intents, actual JSON request/reply records, isolated
scorer receipts, all 81 terminal cell records and `summary.json`. Writes are
exclusive and no-follow; process output is preserved as bounded base64 bytes.
Intent records explicitly are **not** execution receipts. A dispatch count includes
calls whose sending began but whose delivery/outcome is uncertain; separately
counted validated model responses distinguish that case.

The summary keeps the 27-cell denominator for each control and gives coverage
bounds with missing cells included. Paired descriptive differences use complete
three-repeat task clusters only; a 2,000-resample fixed-seed cluster bootstrap is
reported for more than one complete cluster. It is not a population-performance,
equivalence, statistical-power, causal or language-superiority claim. Frozen order,
small corpus, repeated tasks, unexecuted cells and local runtime variability remain
limitations. No observed correctness yields a null observed rate, not zero success.

Run also emits a **pending** post-run review template. An offline checker helps
the actual independent reviewer replay hashes, full denominators, comparisons,
reported usage and the exact model-visible public projection:

```sh
python3 benchmarks/cross-language-v1/agent/specialization_local.py audit \
  --run "$EVIDENCE/run-01" \
  --summary-sha256 "$SUMMARY_SHA256" \
  --output "$EVIDENCE/run-01-audit.json"
```

Use the original summary receipt digest from the run custodian. Audit never contacts
Ollama or executes candidate code. It cannot establish from self-consistent files
alone that a model/process genuinely ran, authenticate reviewer identities, or
replace independent comparison/leakage review. It therefore retains
`actual_execution_independently_attested: false` and `issue_closable: false`.
Final review must bind the original actual run artifacts and explicitly decide
how the unavailable cells affect acceptance. Neither an audit pass nor all-green
unit tests authorizes closing #326.

## Owning checks

```sh
python3 -m unittest discover -s benchmarks/cross-language-v1/agent/tests \
  -p 'test_*local*.py' -v
```

These tests deliberately use a synthetic loopback daemon, fixture review records
and fake process/scoring execution where needed. They exercise HTTP protocol,
source projection, frozen scorer staging, failure retention, integrity and control
refusals. They are not model-quality, hardware, physical sandbox or independent-
review evidence. Non-POSIX hosts run the portable transport checks; tests that
require no-follow acquisition are explicitly skipped without claiming that profile.

## Primary API references

- [Ollama API contract](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [Ollama local/cloud configuration and trust boundary](https://github.com/ollama/ollama/blob/main/docs/faq.mdx)

These explain the implemented API/configuration surface. They are not origin
attestations for the operator's actual installed daemon or model.
