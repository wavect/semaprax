# Per-core provider-proof storage custody

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source-only, no Cargo/build/test execution. Based on the corrected ordinal owner at 71417722e. Prerequisite old default/wrapper helpers are the cleanup packet 6e2659d53, locally cherry-picked as 2ea958adc; cherry-pick only this packet's final commit after that cleanup packet.

One `CoreProviderProofs` is constructed inside the actual resolved-core budget after the unchanged input-carrier reservation. It owns only exact immutable source references, captured budget Rc identity, Vec/Box backing, paid-carrier state and poison. Each logical forecast/constructor phase checks the exact authored-map and full program-slice identity, captured current payer, and sticky reserve0 before consuming backing. It clears every memo row and fixed cursor frame and creates a fresh original WORK scope. Provider/body/source membership and signature checks still run in their original order. Within-phase memo semantics are unchanged. SourceBacking also retains failure and rejects reuse after failure. Any early return, failed proof or unwind returns storage through ordinary Drop and poisons the core owner. Each new resolved-core attempt starts empty; pre-core and standalone calls remain fresh.

The complete owner and additional guard carrier are prepaid once. Moving backing does not subtract the simultaneous empty owner header. Original ProviderProofs/SourceProofs first-use carrier and full geometric replacement/Box charges remain unchanged; actual reused capacity causes no new acquisition. No forecast formula, HIR upper bound, cap, source/profile admission, refund, credit, global payer reset or truth-cache bypass changes.

Five independently compiled complete old receivers preserve the entire SourceProofs, provider, import facade, forecaster and constructor sources. The only lexical bindings select the complete saved SourceProofs, saved default constructor and saved wrapper classifiers, plus additive test observations. `whole-source-inverses.json` restores all 17 entire prior files using literal unique inverse hooks; original SHA/length and exact byte equality are protected. Workspace projection and interop byte-writer/verifier inventories retain all previous members and join the relevant new sources. HIR production validation remains complete without mixing unrelated test-only receivers into its authority domain.

Six new owning bodies and twelve complete-original bodies are listed in `pending-selectors.json`, alongside nine retained current owners and the existing pure projection boundary. New controls cover width/capacity 1/2/17/33, transitive depth 8/32/96, original per-phase WORK/verdicts and actual callback replay, exact/one-short/zero/sticky payment, immutable source and Rc substitution, failed backing, incomplete Drop, first-refusal receipt parity, complete forecast fields, constructor/HIR/identity parity and actual allocation/complete Drop observations. Assertions are pending execution; none are proof of app fit, RSS reduction or backend qualification.

The bounded tool-free Claude source author/review terminated normally with no resource refusal: 271.70 seconds, process-group RSS peak 357,122,048 bytes, list price $0.5058775, invoice unknown. Its header subtraction was rejected locally and corrected. Receipt and review-result hashes are in `review-receipt.json`.

Parent integration owns moving-root inverse composition, fresh compiler/registration qualification, actual measurement pins and publication. Preserve all existing Catalog/service failures and original source archives.
