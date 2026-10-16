# Private metered String storage

Status: private implementation; full compiler and backend qualification pending.

Audience: compiler contributors and reviewers.

The complete production sources at main `b4523cc7c` are archived with their
SHA-256 identities. Exact inverse edits restore both whole originals. The
aggregate receiver changes only its existing storage-selection condition;
source/HIR, descriptor, static-text and stage admission remain in place.

The private Agent Stage Semantic Work build already holds a thread-local
metering selection after stage admission. It may contain local String owners
without a String result carrier. That build now selects the existing inline
UTF-8 runtime. The emitted-function census remains the original source of
storage demand, including body and contract expressions. Unmetered builds keep
their explicit UTF-8-profile requirement and complete original refusal.
Public v9/v10 descriptors, source admission and transport types are unchanged.

The focused scope control checks the explicit public choice, private scope,
foreign thread, nested refusal, return and unwind restoration. The new actual
HIR/emitter gate requires deterministic, validator-accepted Wasm containing the
local literal and meter, while checking complete frozen refusals before and
after the scope. The unchanged Agent-stage lexical fixture remains the required
interpreter/native O0/O2/Core-Wasm fuel, cleanup order and reentry gate. Source
and scope controls do not qualify that full runtime path or broader support.
