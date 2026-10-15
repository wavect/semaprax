# Bounded review disposition

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

One actual tools-disabled Sonnet dispatch reviewed 9,847 exact source bytes. It completed in 82.1386 seconds, exit 0, peak process-group RSS 322,879,488 bytes; provider list cost $0.118096. No confirmed blocker was found in the pasted transitions. This is source-only review, not compiler, runtime, physical funding, or full owner qualification evidence. The complete raw verdict and process receipt are retained without rewriting.

Reviewed production commit: 5d9e6b12551ffd94a99db59f432a7ac4e57b6d26.

The six omitted-context uncertainties resolve as follows:

1. DeclarationId is CompilerStatic or Arc<str> (src/hir/ids.rs:28..35); Clone allocates no backing. ResolvedType leaf arms have no Vec/Box allocation; owning Nominal/Function arms are rebuilt explicitly (nodes.rs:145 onward). The old nominal ID/backing owner remains responsible for its backing; this receiver pays its new inline carriers.
2. The TypeGuard charges price the actual transient guard initializers, distinct from the simultaneous Vec<Frame> backing initialized by grow. They do not replace the Frame slot charge. The constructor contract requires both carriers; no cap or existing forecast term is discounted. These are size_of-derived types rather than an observed-allocation margin.
3. nominal_paid_values::grow grows geometrically (max needed, twice old capacity, four), pays the whole actual replacement Vec backing/header/result before allocation, then moves old entries. It is not exact incremental reallocation and does not perform quadratic one-element capacity replacement.
4. Storage::receive at flags/paid/storage.rs:75..87 returns Some for every Ok value; it never discards an Ok root due to prior state. children calls Storage::grow before substitute, and grow returns None before callback when Storage already stopped. ConstructorPayer likewise short-circuits its first raw E. substituted only returns borrowed references and booleans; its error path owns no partial recursive root.
5. nominal_rollback.rs:120..142 transfers the root into PendingType without allocation (only an Arc clone), and Drop calls nominal_payload::dispose. dispose at nominal_payload.rs:271..297 iteratively moves children into already-acquired scratch and asserts capacity before each push; it does not allocate. The new expanded-tree census pays/grows that shared scratch before recursive output construction. Source rollback initialization primes its lock separately. The owning controls must still execute physical post-refusal and final-Drop checks; this review does not establish those actual results.
6. This is the constructor's existing byte-only callback boundary. It grants no classifier/replay work allowance and does not adapt an unrelated work receiver to success. The existing Payer cfg(test) default is unchanged; explicit oracle-work accounting is not claimed for this new constructor. Existing complete old/current constructor, every-acquisition refusal, exact/short/zero, HIR/wire/diagnostic and Drop controls are to qualify the combined producer. No additional success evidence is inferred from this review.

Template errors retain the old H006 Diagnostic::io message, Error severity, absent path/span/help. The source-order validation walk precedes the disposal-order walk so foreign-owner/missing-parameter selection matches monomorphize.rs:29..65. Full typed raw E is retained in Option<E>, taken before Storage is dropped, and returned in IdentityIndexError::Identity; the static internal Diagnostic sentinel is never its public replacement.
