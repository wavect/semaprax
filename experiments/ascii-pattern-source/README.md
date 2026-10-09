# Private ASCII-pattern source experiment

This private experiment contains source, independent review fixtures, and four passing language-harness witnesses on the OPT-702–704 baseline. The current OPT-706 source revision is unchecked. It has no package manifest, catalog entry, target admission, performance result, or completion claim. The engine is [ascii.spx](ascii.spx); [DRAFT.md](DRAFT.md) records its exact grammar, carrier layout, and reason/offset domains. The separate [oracle](fixtures/README.md) enumerates count vectors and does not imitate engine traversal or predict its work meter.

The source carrier needs only OPT-702: one `Bytes` field, one `usize` field, and an independent whole named borrowed input on exact whole-record renewal. Its buffer stays exactly 3,072 bytes; `position` stays zero. Pattern/input views never enter the carrier. The parser uses flat Copy variants and scalar local counters; classes use a temporary bitmap in bytes 2984–3015. Every renewing source call is pure and nongeneric. No new language, regex runtime, nominal ABI, stored loan, or source allocation site is proposed inside compile/match.

Static caller-profile review uses the exact classifiers: `loop_calls::ast_copy_variant` and `resolved_match_scrutinee_admitted`
inspect every direct case field. `Token` and `Quantifier` have only `usize` payloads and no type arguments; the source oracle,
resolver, and HIR loop validator therefore admit their result and match shapes. This does not admit arbitrary Copy aggregates.
Each renewing helper takes one whole owned `Matcher`, returns that same explicit nongeneric record, and declares no effects.
The fixture's initial `bytes_zeroed` is the existing controlled allocation builtin outside the loop; no invented `alloc` effect
is declared.

The following costs are pinned by unrolled source helpers, not runtime measurements:

| Operation | Logical work |
| --- | ---: |
| `load2/4/8` | 2/4/8 byte reads |
| `store1/2/4/8` | 1/2/4/8 byte writes |
| `store_atom`, `frame`, `capture_result` | 8/8/16 byte writes |
| `clear_candidate`, `compiled_header`, `packet` | 32/64/32 byte writes |
| Pattern dispatch | 1 control iteration + 1 read |
| Escape token | 1–4 actual reads, preflighted individually |
| Decimal quantifier iteration | 1 control + 1 read; EOF iteration costs 1 control |
| Class member bit update/complement/copy | 1 control + 1 read + 1 write |
| Class equality iteration | 1 control + 2 reads |
| Full-match table validation | 128 + 9×atoms + 5×captures |
| Forward atom dispatch | 1 control + 8 metadata reads |
| Literal/any/class predicate | 2/2/3 |
| Backtrack dispatch | 1 control + 2 minimum + 4 start + 4 count reads |
| Successful count reduction | 4 writes |
| Match finalization `F(c)` | 32 + 29×captures |
| Successful compile finalization | 64 header + 32 packet writes |

Compile first invalidates the old ready byte. Therefore its checked minimum work limit is 33; matching retains the minimum 32. Compile failures expose no old compiled authority. Invalid source packets use status 3/reason 1 and pattern offsets; malformed table packets use status 3/reason 2 and absolute table offsets. Resource reason 6 is reserved and rejected. Read-only result observers use ordinary AST fuel and never add to `work_used`.

The greedy engine explores complete count vectors in descending lexicographic order. With `k` adjacent variable atoms and `n` input bytes, the count partitions can grow on the order of the binomial coefficient `(n+k choose k)`. Work exhaustion stays a resource refusal. The 262,144-unit maximum limits performed byte/control events, and fixed helpers make ordinary source work bounded in terms of those events. It does **not** pin the multiplier in interpreter AST steps. A generic 65,536-byte class scan still costs 196,608 logical units plus validation, frame, and packet work; its ordinary fuel remains a major usability constraint. Selected exact bitmap modes use the cheaper source path below.

The OPT-706 source candidate borrows storage once per fixed-width load and fuses contiguous writes with the existing `bytes_set5` operation. It keeps the 3,072-byte carrier, packet layout, grammar and limits. Long literal runs and exact singleton/complement/all-byte or CR/LF, NUL/LF, token (exclude bytes 0..32 and 127), and path (also exclude quote/backslash) classes use 48-byte direct-read chunks; arbitrary classes retain the original predicate. Any-byte runs use their checked extent directly. Bitmap mode derivation reads every membership bit and stores no authority. Terminal and required-disjoint-follower pruning skip only count vectors that cannot accept. See [the candidate cost and proof section](DRAFT.md#opt-706-source-cost-and-greedy-pruning-candidate).

Selected successful scans cost `49*floor(n/48)+2*(n%48)` logical units, plus 32 class reads where used: a 65,536-byte run costs 66,917 scan units. A conservative hand estimate allocates 660 ordinary AST steps per complete 48-byte exclusion chunk and below roughly 904,000 for a complete 65 KiB scan. Those counts are pending HIR/execution and exclude compile, validation, result observers and caller work. Compile a fixed pattern once and reuse the returned Matcher, which retains its table after every match packet. `capture_bounds` returns a flat Copy pair after one complete packet validation; the existing start/end observers retain their predicates while avoiding redundant recursive validations. All invocation work must fit the unchanged 1,000,000 ordinary fuel and 262,144 logical limit. Generic classes and ambiguous count vectors still have no long-input acceptance guarantee.

Interpreter byte updates transfer the owner before `bytes_set`/`bytes_set5`; `Arc::make_mut` provides the update. Read-only helper calls end before the next update. Source-level buffer allocation remains absent inside compile/match, but interpreter record/call metadata still allocate. The new source has no payload-copy or peak-memory measurement. Its fused writers check exact carrier shape before access and retain the existing physical byte-write charges.

The minimal remaining sequence is:

1. Finish the OPT-706 source review and independent grammar/49-obligation/long-input fixtures under the exact choices in `DRAFT.md`.
2. After the whole shared source batch is ready, execute the oracle and the four existing-language-harness witnesses, fix diagnostics/cost failures without raising fuel, and run the final required formatting/clippy batch.
3. Pin exact helper AST costs and run the unchanged 49-obligation corpus, long valid/nonmatching records, hostile adjacent repeats, and ambiguous accepted suffixes under ordinary fuel and declared work limits.
4. Consider package admission only if the useful corpus succeeds with exact interpreter/C11/Wasm parity and settlement. If ordinary fuel fails, report the exact helper/profile obstacle before proposing a separately reviewed change.

Focused selectors used in the combined OPT-702–704 batch:

```sh
python3 experiments/ascii-pattern-source/fixtures/oracle.py
cargo test --locked -p semaprax --test language private_ascii_pattern_
cargo clippy --locked -p semaprax --lib --bin semaprax -- -D warnings
```

[Compiled witnesses](compiled-witnesses.json) pin full carrier bytes by zero-fill plus ordered segments and SHA-256. In particular, `(a*)(a*)` on `aa` matches with work 293 at limit 293; limit 292 refuses before capture publication and reports work 235. Empty-match and zero-repeat witnesses pin sentinel handling; hostile adjacent repeats require a work refusal. The four focused witnesses and independent oracle passed on the earlier baseline. This does not establish the unchanged 49-obligation corpus, long-record fuel, or package admission. Verification and fixture repairs are recorded in [the batch receipt](../../benchmarks/opt-batch-verification-v1/opt702-704-minimal-verification.json).
