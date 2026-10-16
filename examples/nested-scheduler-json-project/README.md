# Independent nested scheduler request specimen

Source-only preparation for OPT729. This is a pure, capability-free
`owned-data-api.v1` example with no public exports, command transport or claim
of full scheduler/ShiftSim application acceptance. Existing scheduling business
requirements and all historical cohorts remain unchanged.

The ordinary nested request decoder owns a ScheduleRequest containing nested
configuration metadata and one bounded jobs Vec. Its proper Ready outcome owns
the whole request independently of input Bytes retired by read_fixture(). Error
contains only code/offset/field; no dummy record is constructed. No generated
helper body is guessed or committed. Explicit bounds stay 16 decoded UTF-8 bytes
per String and eight jobs, through bounded-nested-request.v1.

The consumer validates Unicode/NUL configuration, server count, both ordered job
identifiers/arrival/service fields and the preemption flag. It uses the existing
record/Vec carriers and rooted String views. It does not implement scheduling
policy, identifier uniqueness or server-count domain rules that this specimen
has not specified. Schema-valid empty Strings/arrays remain valid.

Keep the bootstrap app/tests until deriving and replaying the complete schema
replacement against the same bootstrap. After installation, select
app.consumer.spx and tests.consumer.spx. The authored 13 source tests cover the
detached sample, empty/exact capacities, over-capacity, duplicate/missing/unknown
keys, wrong kind, numeric range, input budget and grammar errors. Exact schema
error code/offset/field expectations are derived from the existing owning spec;
they have not executed. The grammar witness checks code1/field0 and does not
invent a scanner offset; the retained independent order corpus owns its exact
scanner offsets and three-backend replay.

The enclosing packet's deferred stage_and_check_scheduler.py retains actual CLI
outputs and failures. It is an operator convenience for source tests/interpreter,
not a replacement for the existing strict native/Wasm/physical owners. The
separate proposed-integration test child reuses the complete existing direct
order fixture's three-backend qualifier and additionally executes all13 source
tests. No runtime, token, performance or billing gain is claimed.
