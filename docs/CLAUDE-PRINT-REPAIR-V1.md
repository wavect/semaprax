# Claude print repair v1

Status: private Unix host route, with local fixture gates. No live repair,
hosted, billing, or production evidence is claimed by this implementation.

The additive `semaprax.source-live-cli.repair-config.v3` has exactly the V2
configuration key set. It selects the native Claude print-JSON profile only
with explicit host operands:

```text
semaprax-full source-live repair run|resume CONFIG CHECKPOINT --claude ABS --scratch EMPTY_ABS [--pause-after-settled]
```

`repair-tested` admits the same operands with its existing separately granted
candidate-test capability. V1 fixture and V2 OpenCode documents retain their
previous meanings. A V3 document with `--opencode`, or a V2 document with
`--claude`, refuses before dispatch. The source deployment must independently
admit provider `anthropic` and exact model `claude-haiku-4-5`. Neither a config
nor provider output grants model, candidate-test, source-write, or publication
authority.

The host binds canonical executable and scratch paths and the complete image
snapshot into the adapter identity. Claude's native executable has a separate
256 MiB ceiling; OpenCode retains its 160 MiB ceiling. The shared staging
machinery reauthenticates frozen bytes before execution. `bounded_capture.rs`
owns both hosts' nonblocking stdout drain, output ceiling, deadline,
cancellation, process-group termination and child reap. Reads continue while
the child runs, including for output larger than pipe capacity. A continuously
writing child is checked for cancellation and deadline within the drain loop.
Non-Unix native Claude dispatch refuses before spawning.

One adapter instance starts one CLI process with `--print --output-format json`,
`--tools ''`, `--no-session-persistence`, `--safe-mode`, `--restricted`,
`--strict-mcp-config`, `--permission-prompts none`, `--prompt-suggestions false`,
and the exact model. Adapter version 1.0.4 supplies a system prompt bounded to
4096 bytes, replacing dynamic workspace system context. Its response guidance
is derived from the same compiled proposal schema used by the decoder: the
exact envelope prefix includes the agent identity and schema digest, with
stable field IDs, declaration order and decimal-string integer encoding
explained. It supplies no field values or repair answer. The frozen canonical
source-adapter request contains the schema but not its digest and remains the
sole user prompt; proposal bytes are never repaired or appended by the transport.
The native deadline is at most 90 seconds per invocation, capped by the
remaining repair-config deadline at host construction; shorter limits still
apply. The source deployment must separately admit sufficient cumulative elapsed
time. OpenCode retains its 30-second per-call bound. The request is at most 64 KiB,
and captured output at most 1 MiB. The checked deployment's narrower response
and cumulative budgets still apply.

The child gets a cleared environment with explicit host HOME for the operator's
existing subscription authentication and a bounded ASCII login identity from
USER, explicitly mapped to child USER and LOGNAME, plus system PATH, scratch TMPDIR, disabled
updates and nonessential traffic, and safe mode. The nonessential-traffic control follows the
[CLI environment reference](https://code.claude.com/docs/en/env-vars). It inherits no API-key,
endpoint, cloud-provider or proxy environment values. Known system managed
settings files, fragment directories and managed MCP files cause refusal.
[Managed policy](https://code.claude.com/docs/en/managed-settings) still applies
in safe/restricted modes. [Server-managed policy](https://code.claude.com/docs/en/server-managed-settings)
can still apply to Team/Enterprise subscription accounts; the operator
selects a trusted CLI host and the adapter does not authenticate remote policy
or claim that these flags isolate an account from its organizational policy. These CLI controls are not an OS sandbox,
billing proof, an assertion that no internal transport retries occur, or proof
of exclusive physical network execution. They authorize the selected CLI's
ordinary subscription authentication; credentials are not copied into prompts
or receipts. Tests use local shell fixtures and never execute the real CLI.

The observed CLI 2.1.286 JSON envelope must report result/success, no error,
one turn, end_turn/completed, result index and queued-turn count zero, no
permission denials, no spawned subagents, and exactly one `modelUsage` row for
`claude-haiku-4-5` with matching canonical model, firstParty provider and no
web search requests. A nonempty string result and integer input/output usage
are required. The result text must itself be a single JSON string encoding the
complete proposal document. A second admitted framing has exactly three opening
backticks, `json`, LF, one JSON string, LF and three closing backticks, with no
bytes before or after that fence. Other labels, prose, nested fences and
trailing content refuse.
Decoding this explicit transport framing preserves
exact document bytes, including the provider-authored escaped final LF. Bare
objects and extra JSON content refuse. The adapter never appends a newline,
trims, repairs or canonicalizes the decoded document; the existing strict
compiler proposal decoder receives those exact bytes. Usage preserves the
provider's `input_tokens` and `output_tokens`; cost remains unknown. These fields are self-reports, not
independent provider identity or billing evidence. No OpenCode events, export,
session receipt or claimed zero cost are synthesized.

V3 receipts use `semaprax.source-live-cli.repair-receipt.v3` and retain the
existing checked prerequisites, journal binding, model attempts, candidate
review and explicit authority limits. Terminal resume replays without creating
an adapter or candidate. The optional post-settlement barrier reuses the
ordinary acknowledged checkpoint and exact retained marker; resume authenticates
them before removing the marker. Evidence cannot authorize a new model call.

Focused gates are the existing toolchain library harness selectors
`claude_host`, `source_live_cli::repair::tests::claude`, and preservation selector
`opencode_host`. Full and hosted quality evidence remain separate requirements.

## Login-identity correction

A local CLI 2.1.286 authentication-status diagnostic found that the cleared
environment without USER could not find the operator's existing keychain
login. Restoring USER alone made the same local auth-status command report a
login; LOGNAME alone did not. The host now retains validated login metadata
as USER/LOGNAME while keeping all credential, endpoint and provider environment
variables cleared. The executable fixture checks these exact environment
properties without reading credentials or making a model call.

## Observed fenced-string framing

A single private diagnostic call (diagnostic08, excluded from qualifying repair
evidence) observed a successful exact-model native envelope whose result was
one JSON string inside a literal `json` Markdown fence. The decoded string
contained the required final LF. Adapter 1.0.4 admits this exact second transport
framing; it does not add LF, change proposal values, relax compiler decoding,
or reinterpret ordinary receipts. The diagnostic wrapper is disabled and is
not part of the native adapter path.
