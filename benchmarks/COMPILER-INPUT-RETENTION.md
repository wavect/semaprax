# Selected compiler input retention

The Codex LogLens, ShiftSim and TeamDesk adapters supply a runner-owned compiler
proxy during SEMAPRAX authoring. Acceptance and compiler qualification continue
to use the actual pinned binary. TypeScript launches and prompts are unchanged.
The proxy path/hash and the actual compiler path, source commit and binary hash
are separate fields in the trial row. The proxy is never reported as a compiler
artifact. Its two Python sources are part of each immutable harness inventory.

For the closed `json-codec` CLI grammar, a runner-side mailbox broker retains
the selected Project manifest and `--source` bytes before acknowledging the
request. It uses `compiler_output_provenance.capture_inputs`, no-follow regular
file reads and exclusive evidence directories outside the authoring workspace.
The broker enumerates and reads its mailbox through the directory handle held
since creation; replacing the mailbox path cannot redirect those accesses.
It also holds the original workspace directory handle and opens every candidate
directory component without following symlinks. Input capture receives that
authorized handle rather than reopening a mutable candidate pathname.
Those snapshots survive installation of a mixed schema/helper output over the
authored schema. Successful reported invocations also retain the declared output
file separately. Failed compiler invocations retain their proxy-reported status.
The compiler inherits the original agent sandbox, environment, streams, working
directory and process group; the broker cannot execute it or any candidate code.
Non-codec and malformed CLI arguments delegate directly to the actual compiler.

This selection is **not a complete Project input closure**: imported sources,
packages and other inputs are not inferred. Independently reading live files
does not lock them or prove which exact bytes the compiler read. Status is
reported by the proxy, not independently observed by the runner. Mailbox writers
can request captures; a capture proves retained bytes, not compiler execution.
Direct invocations of the actual binary are not observed. Complete capture
coverage, independent execution proof, repeat-output proof and generated-file
classification therefore remain unavailable. Existing webapp output provenance
validation keeps its closed webapp grammar and rejects mixed/overlapping output.

Mixed schema/helper files are not wholly compiler-authored. Retained authored
input bytes and observed output bytes remain separate; no token subtraction is
performed, and separately tokenized segments are not treated as additive. This
does not turn a final source proxy into cumulative model authorship. Unknown
fixed-context tokens and actual billing remain null, and these retention receipts
are ineligible for generated-authorship savings claims.

Per attempt, capture reads are bounded to 16 KiB per mailbox request, 128 requests,
8 MiB per selection and 64 MiB of retained input/output bytes. These are evidence
limits, not language or input acceptance limits. Capture failure preserves the
ordinary compiler execution, paid attempt and independent acceptance; it records
missing/unavailable evidence instead of supplying authorship credit. The runner
stops and joins the broker and removes its temporary workspace mailbox before
workspace guards and source counting, including when authoring times out or
raises. Existing compiler binding checks still fail closed on binary drift.
Capture acknowledgement waits have a 30-second deadline. Their elapsed
nanoseconds are reported separately as proxy-reported harness overhead, with no
independent timing or language-cost claim. Capture failures do not add compiler
stdout/stderr warnings or extra instructions to the authoring prompt. Best-effort
bounded mailbox measurement messages retain failures where the channel remains
available; no observed request never establishes complete capture coverage.

Owning regression harness: `benchmarks/cli-tokens-v1/test_live_campaign.py`.
Actual sandbox/proxy transport admission and the grouped harness regression run
remain required before a live campaign; a source-only commit is not that gate.
