# Source-only846 diagnostic writer candidate

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Base7a87b33115c0cd45b14cac64d7b37e5742503159; isolated private branch codex/opt846-cli-diagnostic-writer-20261015. No ROOT mutation, Cargo/build/Rust tests/Clippy or target executable ran.845 implementation was stopped without source edits; its precise initial-funding/context blocker was posted at issue845 comment6198428875.825/845 remain OPEN.

Actual adoption: source_execution::report_all locks stdout for JSON or stderr for human output and writes the same ordered diagnostic lines through diagnostic::writer. Empty reports retain the original no-stream/no-output route. report() still returns1; a failed writer never replaces selected compiler status. Existing WorkspaceReportFailure::Display uses the same human writer for already-owned reports; its owner selection, opaque fallback, framing and report borrowing stay byteexact otherwise. No new constructor channel, snapshot API or unused alternate endpoint is introduced.

Unbounded write_json borrows code/severity/message/path/span/help and writes their exact original JSON order/nulls/escapes without owning quoted fields/location/full JSON Strings. Diagnostic::json, quote_json, human Display and every original test body remain byteexact. Active bounded output intentionally calls the whole old json receiver: it charges path/help before code/severity/message and can retain partial escapes/floor refusals. Reordering those logical writes would change the contract. That unfavorable route retains all original String allocations and receipts.

Output remains caller-owned ordinary IO. New writer controls, stream locks, IO error ownership and caller buffering are not certified prepaid compiler output; no zero-budget report or825 funding claim. This lane removes avoidable intermediate Strings where the real ordinary CLI already writes unbounded output. It does not donate another invocation's budget, raise a cap or simulate a payer. Writer failure may leave partial diagnostic stream bytes, as ordinary IO can; no authoritative artifact/project publication occurs here.

Complete original Diagnostic production source is independently compiled cfg(test) in before_writer.rs (every original constructor/json/quote/Display path). Five new library bodies cover all fields/hostile Unicode/control/quotes/nulls/order/human/newlines; every bounded byte/floor/sticky retry; whole diagnostic/budget/renderer/Drop allocations with the fixed test caller sink and unfavorable bounded case; first IO error; and full source inverses/new-owner joins. One new existing cli_check_routing_v1 body invokes the actual binary, compares complete old-json/human bytes for parser and HIR findings, checks streams/status/source bytes, and onUnix deterministically refuses its selected output descriptor while retaining compiler status1. All these Rust bodies are AUTHORED UNEXECUTED; no measured gain is claimed.

New owners writer.rs, writer/tests.rs and before_writer.rs join each of all three source audits exactly once; no recorded file grows. Ten complete current→7a87 rows preserve old source bodies. The new source gate enters diagnostic846_input, initially identity; a future combined844 assembly must add a full final→this named view there independently of inherited guards. Protected current-to-7a87.json must remain immutable at checkpoint.

The historical831 Python proof predates parent's7a87 CHANGELOG entry. parent-changelog-static-view.json authenticates ONLY that complete7a87→31 document for STATIC old-proof replay; actual runtime source guards and846's own full source test retain the real7a87 domain. No old script/JSON/archive is changed or rehashed. All prior source proofs are replayed in their complete authenticated domain.

Bounded command:

    python3 docs/evidence/diagnostic-writer-846/verify.py

The full old-json compatibility API, all original receiver/test bodies, ten full inverse rows and three new audit owners are checked. Actual unit/CLI controls and scoped Clippy belong to the consolidated future batch. pending-selectors.json names six new bodies plus minimal original compatibility/typed-rendering controls, including the existing CLI-owned harness rather than a new top-level test file. CLI integration links the current actualcore executable; existing raw-Cargo artifact receipt workflow applies. No pin changed; any moved producer wire still requires actual regeneration and exact consumers.
