# Branch-local closed owners (#831), source-only

Base e93f7169240520ad18613d38f3c51795ae696bfc. No ROOT changes, Cargo,
compiler, Rust tests, Node or Clippy execution. This candidate removes a real
constructor; it does not establish strict #831 acceptance or recover an app.

`successors_with_receiving` previously constructed and received the large
`Closed::{Borrowed,Owned}` enum for every state. The selected borrowed branch
now retains its existing Rc in a branch-local owner and returns a borrowed view;
the general branch initializes its OrderedWeights owner directly and returns a
borrowed view. The other owner remains uninitialized and never constructs a
value. No Option/Result/heap/shared wrapper replaces the removed enum.

Payment precedes the actual branch construction: borrowed uses two Rc values
and two reference values; general keeps both original OrderedWeights values and
two reference values. The original positive reserve Result stays executed, as
do closure authentication/held slots, all route arithmetic, work, ordering,
checked additions, overflow, backing, signatures, state IDs and Drop. Complete
source proof compares those algorithm bodies exactly modulo local variable
renaming. The complete old Closed enum/methods are cfg(test), still used by the
independently compiled historical receivers. No old receiving fee formula or
complete original receiver is edited.

There is no universal savings assertion. The old envelope costs
`2*size_of::<Closed>() + size_of::<&OrderedWeights>()`; the borrowed replacement
costs `2*size_of::<Rc<OrderedWeights>>() + 2*size_of::<&OrderedWeights>()`.
The general replacement keeps `2*size_of::<OrderedWeights>()` and pays its two
references. The expected fixed reduction may be much smaller than the observed
strict gap; only the unchanged actual whole-owner test can decide acceptance.

Actual historical strict width0 at7a87: old10829/current12518 reserved;
allocated1517/1453, peak1501/1453, allocations12/12, full Drop. Its failure log
is archived here unchanged. No current tuple has been measured. The strict
`current_reserved < original_reserved` assertion remains unchanged, including
all widths0..4, not replaced by a favorable immediate baseline. #831 stays OPEN
unless every required strict/physical/work/auth/diagnostic owner actually passes.

Four complete current→e93f source rows include production canonicalizer,
ordered_weights, shared test-only source helper and receiving tests. All old
runtime JSONs, source proof scripts/tables/archives remain immutable. The new
protected runtime table `current-to-e93f.json` must remain immutable during future
composition; independently project final→this candidate before the existing
combined_ready_input fronts. Do not serially rewrite old tables or guessed hashes.
The new current guard authenticates all four full current inputs and four full
immediate originals. All old receiving test bodies are byteexact prefixes.
No new Rust module/file exists; each existing owner was already joined in all
three audit inventories, and source line caps remain below1500.

Run bounded metadata only:

```
python3 docs/evidence/branch-closed-receiving-831-20261015/verify.py
```

`pending-selectors.json` names exactly one new source guard and seven existing
whole receiver/source/work/physical owners. Retain the original retry and
normalizer owning app controls and disclose their actual outcomes in the final
batch; this lane does not run them, alter caps, or claim token/app/RSS gains.
Parent's old e93f122 resource refusal remains zero-body history, not a compiler
or runtime result. Parent will consolidate qualification with #845/future work.
