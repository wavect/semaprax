# Finite PLAN acquisition at the complete e24 integration

Base: `e24e99787e8dec6cf5aed2930ff909cedc187f97`.

This source-only repair retains the original six-row PLAN packet, every complete
original/inverse, the root archive/inverse and prior source manifest byte-for-byte.
A distinct sealed packet records six finite successor chains:

`actual current -> complete e24 checkpoint -> complete 3af PLAN checkpoint -> original c874 input`

The first two links have exact complete after/before SHA-256 bindings and
bounded descending UTF-8 byte-span inverses. The original independent six-row
inverse performs the final link. The complete e24 and 3af text is retained per
source; original c874 text remains in the unchanged earlier packet. The live
physical source must match the new endpoint before any supplied staged value
can be accepted. Whole current source, e24, 3af and c874 are the only accepted
stages in the explicitly selected old namespace. Wrong/truncated/duplicated,
foreign and drifted physical inputs fail closed.

`acquire_expected` is called only from the existing expected-input guard. It
selects a source only when that guard's immutable expected SHA is exactly the
original six-row c874 target. Current/raw and older expected namespaces return
unchanged. The old `acquire`/restore implementation remains independently used
by the complete legacy control after physical acquisition. Direct
`compile35f_input` has no expected SHA and none of its seven source keys belongs
to this six-row PLAN domain; its complete original c874 body is restored rather
than normalizing unrelated inputs before its key check. All singleton,
supplemental, alias-fixture and original compile/custody code elsewhere in the
shared helper remains intact. This is no universal current-source normalizer.

The actual HIR, workspace and native raw source carriers are exactly their e24
bytes. They retain all original entries, all nine PLAN owners and all main21
matching/helper/namespace occurrences. None is projected to c874 for its own
raw audit. The unchanged 043 additive-fragment parser reconstructs all three
complete original a82e guards. Required=maximum uses actual original scope
occurrences: PLAN owners once in every carrier, main21 matching and its test
2/1/3 in HIR/workspace/native and incoming21 namespace once in each. All c874
required/maximum entries stay unchanged. Native stays exactly 1,500 lines.
Current full raw binding records are refreshed against those actual unchanged
e24 carriers, with their entire prior binding records retained under `before/`.

`bind_stages.py` binds only this separate stage. The old `bind_sources.py`
receives an explicit delegation prefix; its complete old body and an independent
whole preimage remain intact, and no prior packet is rewritten. The new current
manifest binds the nine actual Rust owners separately from their old manifest.
The source packets retain the same 2 MiB cap; the new packet is below it.

Executed verification: bounded Python reconstruction of all six chains,
complete raw-three/043 union checks and 137 hostile source cases; rustfmt on the
changed test-only helper; `git diff --check`. The receipt is `proof.json`.
No Cargo, build, rustc, Rust test, language runtime or application gate ran.
These source checks establish neither type correctness nor application fit.

Any later main/Selected integration must add its own complete-current-to-this-
checkpoint edge before this stage. Do not regenerate this fixed e24 packet from
a moving checkout or replace its endpoint hashes alone.
