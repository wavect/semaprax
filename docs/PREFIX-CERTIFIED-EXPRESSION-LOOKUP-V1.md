# Prefix-certified expression lookup v1

Audience: compiler contributors, tool authors, and reviewers.

Status: private source checkpoint; owning runtime qualification pending.

The ordinary expression-fact arena keeps its complete current insertion order,
AVL links, full opaque keys and borrowed fact payloads. Its original complete
lookup remains independently compiled for controls. The production lookup uses
byte-prefix certificates from actual comparisons with ordered lower and upper
keys. Every key between two byte-lexicographic bounds shares their common prefix;
if the query matches both bounds through that prefix, the next comparison need
only read the remaining bytes. No identity spelling, parsed structural ID, hash,
source-profile shortcut or cached semantic verdict establishes that certificate.

For trees of at least sixteen nodes, paid comparisons with the existing minimum
and a paid right-child walk to the maximum seed both bounds. This is a strategy
threshold independent of any observed fixture debit. Outside/equal endpoints are
resolved by those full comparisons. Smaller trees start with zero certificates.
All key operations use UTF-8 bytes, preserving ordinary string order including
embedded NUL, Unicode and keys ending within a longer key. The immutable producer
owns every link; no authoritative plan vector is sorted or repaired.

Each actual byte-suffix comparison retains the old one-unit-per-byte-pair upper
charge, including the end comparison. Node access retains three units, paid
before its read. Ten additional units cover bracket bookkeeping, comparator
fields and scalar return fields; four initial control writes are paid separately.
Only repeated prefix traversals that the new algorithm does not execute disappear.
The 32-million work ceiling and all ordinary/selected quotas remain unchanged.

Every lookup prepays the full LookupControl and both complete return/receiving
Result layouts. Each comparator prepays its complete Comparison, Result and
receiving pair layouts. No old local or Directory header is discounted. These are
conservative cumulative byte debits for initialized controls; the algorithm adds
no heap allocation, retained index, field, Rc or cache. The existing payer and
sticky physical/work refusal remain authoritative. This packet makes no complete
pipeline funding or runtime recovery claim; control payments can increase byte
usage even when work decreases.

Owning controls compare full old lookup hits/misses and exact fact object pointers,
opaque endpoints and long prefixes, ascending/reverse insertion, actual per-byte
rates, work exact/short/zero, active sticky refusal, full new carrier exact/short/
zero payment and complete Drop. Full original sources and child joins have exact
inverses. Runtime recovery, current-head application gates and all source custody
checks remain required before qualification.
