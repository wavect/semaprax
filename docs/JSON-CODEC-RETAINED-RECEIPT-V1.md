# JSON codec retained-input receipt v1

Audience: compiler contributors and reviewers.

Status: source candidate; all executable qualification pending. This does not close OPT606 or establish measured token, cost, turn, memory or wall-time gains.

The additive single-codec route is:

```text
semaprax json-codec <project> --source <declared-module-path> --type <record-id> --output <new-receipt.json> [ordinary profile and bounds] --retained-receipt
```

`--retained-receipt` occurs exactly once. It selects a JSON artifact rather than a `.spx` replacement. The original single-codec, plan and suitability forms are unchanged. Mixing this flag with `--plan` or `--explain`, duplicate flags, unknown selectors and invalid bounds refuses before Project authority. Ordinary profile spellings and bounds are parsed by the existing single-codec parser. This new receipt route uses the existing codec-plan admission bounds (1 selected entry, at most64 resolved sources, at most2 MiB authored/candidate source, at most8 MiB inner bundle). It does not enlarge an existing route.

## Authority, exact originals and closure

`project::with_authenticated_json_codec_receipt(manifest, entries)` acquires only the existing `with_authenticated_project` authority. The CLI supplies one entry; the library accepts the ordinary bounded plan entry set. Strict-law policy, aliases, symlinks, duplicate physical files, source admission, original diagnostic selection and final held-input checks stay on their ordinary routes. The receipt grants no authority.

Before derivation or publication, each existing held handle is rechecked, read under its original bound, compared to its original full length/SHA256, and rechecked again. Captured bytes are read only from the retained handle; ordinary path/identity rechecks remain unchanged. The retained inventory consists of the exact original UTF-8 manifest, every manifest-declared source in manifest order, and every local dependency-subject envelope in manifest order. Role and canonical relative path distinguish each entry. It includes unchanged sources, test/entry sources, manifest comments and original dependency envelopes, not merely selected schema text or canonical reconstructed TOML. All original retained bytes have separate byte counts and SHA256. The input-retention ceiling is16 MiB total; exceeding it refuses this new route. Existing ordinary route bounds remain unchanged.

The complete inner `semaprax.json-codec-bundle.v1` additionally preserves all resolved admitted source bytes, including compiler-bundled standard dependencies and authenticated local-subject sources, separately from candidate sources. Existing bundled dependency resolution and Subject-v3 replay remain the authority; receipt contents are not interpreted as trusted HIR or new dependencies. The raw held inventory describes actual filesystem inputs; the resolved source inventory describes compiler-admitted sources. Neither is substituted for the other.

## Actual compiler derivation and repeat

The schema is `semaprax.json-codec-retained-receipt.v1`. The receipt contains:

- `compiler_package_version`, an ordinary compatibility version; `compiler_binary_sha256` is null.
- `held_inputs`, the complete original held bytes, roles, paths, byte lengths and SHA256 strings.
- `bundle`, the complete existing checked bundle, including exact original/candidate Project revisions, resolved original sources, complete candidate sources, and per-entry profile, bounds, schema/output digests and newly generated identities.
- `derivations`, exactly two records with ordinals1/2, original Project revision, exact compact inner-bundle byte length and SHA256.
- `repeat_kind = same-process.same-retained-revision.v1` and `independent_execution = false`.
- `publication_policy = explicit-new-artifact.no-overwrite.v1`, describing the publication protocol rather than asserting a publication already succeeded.

Both derivations actually invoke the ordinary complete codec-plan generator against the same retained `Arc<ProjectRevision>`. The second output is not copied from the first. Both complete bundle structs and serialized bytes must match. A failed second derivation or byte disagreement returns no receipt. Both passes perform ordinary original/candidate validation; all held inputs are rechecked around the request and again before returning from the existing authenticated Project operation.

The inner-bundle hash covers exactly `serde_json::to_vec(bundle)` without a terminal LF. Existing source digests cover the exact UTF-8 source bytes. The outer receipt is compact serde JSON followed by one LF in CLI publication. Its serialized bound is64 MiB, checked through a nonallocating bounded writer before a publication buffer is constructed. There is no embedded digest of the outer receipt itself; a runner may authenticate its complete published bytes separately.

`verify_authenticated_json_codec_receipt` acquires fresh ordinary Project authority, retains the current complete original inputs and actually rederives both outputs. It compares the entire receipt. An internally self-consistent rehashed forgery, altered original bytes, changed inventory/profile/output, missing repeat or invented binary/independent-execution claim is not accepted by digest equality alone. Deserialization remains data-only and denies unknown fields.

## Publication and accounting limits

The complete receipt is encoded before the existing private-file/write/sync/exclusive-hard-link publisher is called. Exactly one explicit new artifact is published. Existing destinations and racing creators refuse; partial source-plus-sidecar publication is avoided. Source and manifest files are unchanged. Publication success is the command's actual successful return and artifact, not a claim generated before the link succeeds. No native code, subprocess, extra compiler executable or provider is executed.

The receipt establishes a compiler-owned same-revision repeated derivation protocol, not independent-process execution or a trusted executable fingerprint. A host requiring exact compiler identity must separately authenticate the actual in-sandbox compiler binary and bind this artifact to that invocation. It must retain raw invocation, model/accounting and resolved-context information independently. It must not fill the null binary field with an unverified claim or reinterpret the package version as provenance. Missing exact runtime binding or independent-execution prerequisites still refuse scored OPT606 completeness.

Original input and mixed candidate output remain separate inventories. Candidate source contains original schema plus generated helpers. Whole-file classification cannot erase original authorship. Never estimate generated helper tokens by subtracting independently tokenized strings; no NET/GPT-token or billing claim is created here. Frozen benchmark v1 refusal and accounting policy are unchanged; host adoption is a separate versioned replay/qualification step.

## Pending owning checks

Focused source-authored owners cover full original byte retention, held content/identity replacement refusal, resolved bundled closure, ordinary plan equivalence, actual repeat bindings, full original diagnostic equality, rehashed forgery rejection, exact grammar, one-file no-overwrite publication, no publication on invalid derivation, and unchanged original source mode. Their names and registration are recorded in the evidence packet. Cargo, runtime checks and repository gates have not been executed for this source candidate.

## Optional stdout publication correlation

The additive `--receipt-stdout-seal <nonce>` form, usable only with exactly one `--retained-receipt`, is specified in [JSON Codec Publication Seal v1](JSON-CODEC-PUBLICATION-SEAL-V1.md). It emits a bounded stdout record after successful exclusive publication of the identical JSON-plus-LF artifact. It creates no second artifact and grants no execution authentication. Existing forms without that flag retain their original output and diagnostic behavior. Qualification of the additive form remains pending.
