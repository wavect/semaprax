# Independent installed-artifact repair journey v1

Status: **preparation only; unexecuted.**

Audience: the operator authorized to complete issue #335 and the reviewer who
must distinguish its evidence from checkout, fixture, or archive-smoke work.

## Purpose

This worksheet is the execution record shape for the remaining independent
installed-artifact journey. It does not select an artifact, install a binary,
invoke a provider, run a check, or create a repair candidate. Filling it from
an offline fixture or a binary compiled from the current checkout does not
satisfy the journey.

The required product result has three parts:

1. an independently obtained distributable artifact is verified and installed
   into a clean workspace outside the source checkout;
2. that installed artifact performs a genuine multi-module build and a
   failing-project repair loop with the admitted live provider profile; and
3. the retained checkpoint is interrupted and recovered while an unrelated
   project still has its recorded behavior.

The source-live repair CLI is intentionally a source-immutable candidate
preview route. It does not publish a candidate or grant a candidate-test,
source-write, Git, or publication capability. A run record must retain that
fact instead of reporting a preview as an applied repair.

## Authorities required before execution

The following authorities are outside this worksheet and must be obtained by
the operator at the point they are needed:

| Authority | Required use |
| --- | --- |
| Release/distribution selection and download | obtain one exact independently produced artifact and its published digest material |
| Host filesystem/process authority | create the clean workspace, install root, checkpoint, and scratch paths, then run the selected binary and independent checks |
| Provider/account authorization | make the real request with the fixed `opencode/muse-spark-1.3-contributor-free` profile admitted by the selected source Agent deployment |
| Candidate-test authority | perform the independent correctness check if the selected integration exposes the capability-bearing embedding route; ordinary CLI v2 deliberately has none |

The provider profile being free does not make a provider request an offline
fixture. Credentials, raw provider responses, and secrets are excluded from
the retained record.

## Artifact admission record

Create a new evidence directory outside both the source checkout and the
installed workspace. Retain a canonical `artifact.json` with these values
before unpacking:

```text
artifact acquisition URL or immutable locator
archive filename, byte length, and SHA-256
SHA256SUMS filename, byte length, and SHA-256
selected host target
release version and commit claimed by the archive manifest
archive manifest digest and exact inventory result
absolute installed binary path and SHA-256
installed binary `--version` and `version --json` stdout/stderr/exit status
```

The archive checksum establishes an integrity comparison only. It does not
establish publisher identity, provenance, signing, or that the artifact
implements this journey. Before provider work, record the installed binary's
scoped help for `source-live` and `source-live repair`; if either required
route is unavailable, stop with an honest incompatibility result. Do not
replace the artifact with a binary built from the current checkout.

## Clean workspace and control projects

Use a new parent outside the checkout with a new `HOME`, install root,
workspace, checkpoint directory, and provider scratch directory. Capture their
absolute paths only after redacting user-specific path components if the
evidence will leave the operator's machine.

Prepare two independent multi-module projects:

| Project | Required initial observation | Required final observation |
| --- | --- | --- |
| Repair subject | independent build/check result showing the intended failure | independent build/check result after the retained repair outcome |
| Unrelated control | recorded successful build/check/run result before repair work | the same successful result after interruption and recovery |

Record every command as argv, working directory, selected binary digest,
environment allowlist, stdout digest, stderr digest, exit status, start/end
timestamps, and output-file digests. The environment allowlist must omit
credentials and must state how the provider credential was supplied without
recording it.

## Live repair and recovery ledger

Build the repair configuration only against the installed artifact's admitted
`semaprax.source-live-cli.repair-config.v2` route. Record the configuration
bytes and SHA-256, its checked source/ProgramRoot/proposal-schema bindings, the
selected provider profile, and the exact OpenCode executable snapshot digest.
Use a new empty absolute scratch directory for each invocation as required by
the CLI contract.

Retain these events in order:

1. the failing subject's independent correctness result;
2. the exact `repair run` argv and its bounded receipt or selected failure;
3. a deliberately documented interruption point after a durable checkpoint is
   present, with no claim that an unobserved external request was not sent;
4. the exact `repair resume` argv and receipt, including its retained
   model-attempt projection and replay/uncertainty state;
5. the repair subject's final independent correctness result and the unrelated
   control's repeated result.

If a checkpoint is terminal, resume is a read-only replay and must not be
described as a second model dispatch. If recovery reports uncertainty, refusal,
or a provider failure, retain it as the result; do not retry until it looks
successful. The journal binds causal shape but does not prove freshness or
exactly-once external delivery.

## Review checklist

The final capsule is sufficient for issue #335 only when it contains all of
the following, each bound to the actual host and revision where it occurred:

- artifact identity, verification commands, inventory result, and installed
  binary identity;
- clean-workspace proof and evidence that no checkout-built binary substituted
  for the installed one;
- before/after independent correctness results for the actual multi-module
  repair subject, including an honest failure if repair did not succeed;
- the selected live provider/model identity and retained usage observations,
  with missing usage represented as missing rather than zero;
- the retained config, checkpoint/journal identifiers, run/resume receipts,
  interruption point, and recovery outcome; and
- the unrelated control's before/after observations.

Local completion is enough. A hosted CI receipt is not required. Conversely,
the checkout installed-toolchain test, the offline repair demonstration, a
fixture provider, and archive-smoke tooling are preparation or regression
evidence; none is an independent installed-artifact journey result.

## Related contracts

- [Installation and archive identity](INSTALL.md)
- [Source live CLI route and recovery semantics](SOURCE-LIVE-CLI-V1.md)
- [Live repair smoke authority boundary](LIVE-REPAIR-SMOKE-V1.md)
- [Checkout-installed journey regression](INSTALLED-JOURNEY-TEST.md)
- [Unpacked archive product gate](RELEASE-PROCESS.md#explicit-unpacked-product-acceptance)
