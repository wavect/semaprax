# Catalog23 owned-data application cohort

This is a new independent application/cohort, not a later ShiftSim or LogLens
measurement. SPEC, corpus and independent oracle are exact copies of the
previous source-only catalog contract in
`opt723-owned-runtime-adoption-20261010/catalog`. All 23 original obligations and
expected output bytes are retained: multikey ordering, decoded unique ASCII
identifiers, prefix selection, whole-input validation, restocking, metrics,
invalid diagnostics and unrestricted raw JSON whitespace. No prior application
acceptance, paid run or efficiency result is reused.
Identifier comparison is unsigned decoded ASCII bytes in this closed cohort.
If a future cohort admits arbitrary Unicode, JavaScript's default UTF-16 sort
cannot stand in for UTF-8 byte order: use an explicit byte comparator such as
Node's Buffer.compare and a new versioned oracle/corpus. This cohort has no
astral/BMP acceptance or general Unicode-codec claim.

The two arms are SEMAPRAX Project v30 owned-data and idiomatic TypeScript on
Node, both pinned to gpt-6.1-sol/medium, 1800 seconds and at least five attempts.
Only the public SPEC is seeded. Both author their implementation and tests.
Neither receives reference or generated application source. TypeScript must use
the same strong dependency-only TypeScript 5.9.3/@types-node 22.20.5 bootstrap
as the existing CLI/ShiftSim baseline; all package/config/application files remain
authored. No SEMAPRAX record representation constraint applies to TypeScript.
Its build produces `dist/catalog.mjs` and may use additional local compiled
modules. The harness independently invokes the pinned tsc with the authored tsconfig
and an external output directory; arbitrary authored build/run wrappers do not
select the accepted executable. It retains the complete regular emitted module
closure, executes it with the pinned Node binary and checks it again after
acceptance, metrics and archive. The 16 MiB/4096-file artifact retention ceiling limits compiler output,
never raw application input. Node built-in APIs remain available.

Before any paid calibration or trial, the harness requires a fresh closed
reviewed SEMAPRAX reference candidate, exact compiler-source/binary/profile,
manifest and native binary, and all original23 acceptance rows in a hashed build
receipt. Qualification builds/checks with the real compiler and accepts its
native binary rather than an authored wrapper. Failed qualification retains
its result/log fields but gives no dispatch eligibility. Every live attempt
then faces the same 23 hidden cases. Source, native/Node compiled artifacts,
bootstrap dependencies and workspace boundaries are checked throughout.
Generated-source authoring uses the existing bounded input retention broker;
its selected snapshots are explicitly not a complete input closure and do not
grant authority or pretend mixed files are wholly generated.

Per-arm full prompt bytes/hash are fixed harness/task context, reported separately
from authored/generated source. Context tokens, actual billing and unknown
measurements remain null. Empty-task calibration is diagnostic and never
subtracted from raw usage. Actual live and qualification results are pending;
there is no tokenizer upper-bound or cost-saving claim.

Pending commands after a verified native compiler is available:

```sh
python3 benchmarks/catalog-tokens-v1/codex_campaign.py qualify \
  --compiler-source-ref <verified-source> --semaprax-bin <verified-compiler> \
  --candidate <reviewed-closed-v30-reference> --output <new-external-qualification> \
  --authoring-profile semaprax-project-v30-owned-data-v1

python3 benchmarks/catalog-tokens-v1/codex_campaign.py plan \
  --base-ref <verified-source> --compiler-source-ref <verified-source> \
  --semaprax-bin <verified-compiler> --qualification-evidence <fresh-evidence-json> \
  --authoring-profile semaprax-project-v30-owned-data-v1 --artifacts <new-external-campaign> \
  --typescript-bootstrap-receipt <strong-tooling-receipt> --node-binary <verified-node> \
  --npm-binary <verified-npm> --trials-per-arm 5 --model gpt-6.1-sol --effort medium
```

The collection-record cohort is a separate v31 choice. Use a reviewed v31
reference candidate and qualify it independently; qualification and planning
must name the same profile. The byte-frozen SPEC retains its historical v30
profile clause, which this cohort supersedes only for Project route/setup. Its
entire 23-case functional contract and source requirements remain unchanged,
including runtime items with owned String identifiers and scalar marks, plus
ordered publication fragments with owned String labels and Bytes payloads.
The v31 prompt states this route-only qualification
explicitly. Model, effort, timeout, attempt count, and the strong TypeScript
baseline remain the same as the v30 cohort. Its route is
`language-command-io.collection-record.v1` and its campaign schema is
`semaprax.catalog-codex-campaign.v2`.

```sh
python3 benchmarks/catalog-tokens-v1/codex_campaign.py qualify \
  --compiler-source-ref <verified-source> --semaprax-bin <verified-compiler> \
  --candidate <reviewed-closed-v31-reference> --output <new-external-v31-qualification> \
  --authoring-profile semaprax-project-v31-collection-record-v1

python3 benchmarks/catalog-tokens-v1/codex_campaign.py plan \
  --base-ref <verified-source> --compiler-source-ref <verified-source> \
  --semaprax-bin <verified-compiler> --qualification-evidence <fresh-v31-evidence-json> \
  --authoring-profile semaprax-project-v31-collection-record-v1 \
  --artifacts <new-external-v31-campaign> \
  --typescript-bootstrap-receipt <strong-tooling-receipt> --node-binary <verified-node> \
  --npm-binary <verified-npm> --trials-per-arm 5 --model gpt-6.1-sol --effort medium
```

`plan` has no model dispatch; `run` additionally requires the explicit
`--acknowledge-paid-attempts` flag. The owning mocked tests are
`test_codex_campaign.CatalogCampaignTests`; production build, Python checks,
real qualification, paired runtime and paid measurements remain pending.

The nested-outcome successor is an explicit third choice:
`semaprax-project-v32-nested-outcome-v1`, campaign schema
`semaprax.catalog-codex-campaign.v3`, Project schema `semaprax.project.v32`,
and profile `language-command-io.nested-outcome.v1`. Both qualification and
plan/run must select it explicitly. It requires a new reviewed application
and fresh source/compiler/native-command binding through all original 23
cases; v30/v31 evidence cannot authorize it. The functional SPEC, corpus,
oracle, TypeScript prompt and baseline, model/settings, minimum five trials
per arm and separate fixed-context accounting stay unchanged. Only the
historical Project route/setup clause is superseded. This harness addition
is pending compiler and application qualification; it establishes no token,
cost or acceptance gain and dispatches no model runs by itself. ShiftSim uses its own explicitly selected round 5 and independent all15
qualification; Catalog evidence cannot authorize a ShiftSim campaign.
