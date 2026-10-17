# Keep one paid comparison state per expression lookup

Status: focused component and source checks passed; full HIR and current-head CI qualification pending.

Audience: compiler maintainers and reviewers.

Earlier hosted graph-building failures exhausted the unchanged 64 MiB account
in expression-fact comparison controls. Each visited node constructed a fallible
comparison result and returned a tuple, charging their complete carriers again.
This change removes those repeated constructions from the ordinary builder's
lookup on every nonempty arena.

One fully funded comparison state now belongs to the complete immutable search.
Its existing fields receive candidate bytes, certified prefixes, ordering and
work counts. The original endpoint and AVL algorithm still compares complete
byte keys, including lengths, NULs and Unicode. There is no cached lookup result,
new index, normalization, pointer shortcut or alias-dependent charge. The full
old lookup controls remain independently funded; two complete new state layouts
are paid before initialization. All original comparison/bookkeeping work remains,
plus eight explicit initialization units per lookup. No quota, refund or limit
changes. Empty arenas, unobserved replay, selected reservations and independent
oracle payers retain the entire original lookup route.

Three new owning component controls pass on Rust 1.88 and 1.98, with focused
Clippy. They cover empty/small/threshold/wide trees, different tree shapes, every
borrowed result reference, hits/misses/endpoints, complete opaque byte keys,
original work events, unchanged legacy/selected work, exact/short/zero byte
frontiers, reserved floors, sticky refusals and final Drop. Detached equal query
strings retain exactly the same bill and work. Removing only state payment or
reversing only the unequal-byte comparator fails the controls on both toolchains.
A fourth real-HIR call-fact test is authored and awaits hosted execution.

The component compiles the complete original/new prefix module, actual routing
body and Node/Arena declarations. HIR, arena construction and non-executed fact
payloads have explicit adapters. Budget paying/work/refusal methods, diagnostics,
classifier ledger and System observer are actual implementations. Arena/query
construction occurs outside observation. It does not qualify full HIR execution,
the real application fixtures, or complete graph-building budgets.

For 520 queries over the focused 257-key fixture, cumulative charges fall from
1,138,803 to 225,163 bytes. Work rises from 712,585 to 716,745, exactly eight units
per query. Actual heap allocation is unchanged at 523 bytes; both lifetimes end
with zero live bytes. The improvement reduces cumulative builder accounting;
it is not a process-RAM or whole-application measurement.

The complete current SourceReflows suite and original Oracle-a controller pass,
as do the three original arena/prefix/dispatch source controllers in separate
small binaries. All probes retain the unchanged 200 MiB guard. These checks
preserve full current/predecessor hashes and ten complete archives. The old
prefix module and all five original owning tests remain verbatim. Only the
historical cursor reader uses its authenticated complete predecessor; every
original assertion remains. Its entire modified source has its own inverse.
The original three-row prefix packet, all fourteen receiver proofs and every
prior formatting, ordinary-setup, status-prefix and continuation proof remain.
Semantic changes are separate from the unchanged formatting registry.
