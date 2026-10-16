# Compact staged-result observations

Status: private component evidence; full HIR and hosted fit qualification pending.

Audience: compiler maintainers and allocation reviewers.

The complete hosted JSON group at `0ab1bb6cc` passed seven cases and refused
four under the unchanged 67,108,864-byte builder limit. Two first refusals were
248-byte observation cells; another was a flat observation-history backing.

The private observation enum previously embedded the largest canonical staged
result in every variant. `compact_stage` keeps body fields inline and stores
residual sources in a separately acquired owner. The public cleanup source,
identities, concrete types, paths, work events, and materialization events stay
intact. Derived value order and explicit Debug formatting retain the canonical
source's order and text. Every unknown-source constructor prepays its new
receiving layouts, and each residual prepays its complete canonical payload
before allocation. There is no credit, refund, raised limit, or new authority.

On the measured 64-bit source shapes, the observation becomes 120 bytes rather
than 232, and its Rc backing becomes 136 rather than 248 bytes. Constructor
controls and residual backing add separate costs; an isolated staged source
can therefore cost more. Application fit requires the full hosted gates.

`before.rs` compiles the complete original plan enumerator from the existing
`fc722eea9f308a5ccae38965e33eeee30f426173c6fea9cb8a364238049ee2e8`
archive, its original observation enum, and its flat observation header and
traits. `lexical_before.rs` compiles the complete original lexical marker
producer. Only explicit visibility changes adapt these bodies to their test
namespace. The plan producer always creates a `None` DAG prefix; comparisons
require that on both receivers. This packet does not claim a second old DAG
implementation. Typed gates check the old header/path size and alignment.

`original-pieces.json` authenticates all seven original bodies and the exact
compiled adaptations. `inverse.json` authenticates every byte of seven current
receiving files, reconstructs their entire `8fb7b0255` predecessors, and keeps
the complete previous branch-clone tests and their assertions. Both live source
inventories include all five new modules. The original formatting registry and
all earlier source archives remain unchanged.

Six actual source/digest tests passed locally, including the unchanged whole
enumerator/clone/push source test, full-file inverses, and input mutation guards.
Compiler peak was 169,280 KiB under the 200 MiB process-tree stop guard.

Constructor probes passed on Rust 1.88 and 1.97 with compiler peaks of 174,064
and 193,312 KiB. They compile the actual constructor, Debug/equivalence logic,
complete type/source clone census, Rc conversion and push, physical growth,
bounded-output ledger, diagnostic and allocation observer. Thin budget and
derived identity-trait adapters delimit the probe: it covers three source forms,
nested owned backing, ordering, ordinary/pretty formatting, legacy and paid
routes, zero/exact/short funding, work/materialization preservation, and complete
Drop. It does not qualify full HIR, the full clone cursor, or oracle integration.
Those real-type gates are authored in the owning library harness for CI.
