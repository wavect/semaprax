# OPT842 test-key compatibility correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only successor to8025cd196. The focused compiler reported E0277 because
the existing before_alias::same_closure_state test adapter tried to look up an
old Rc<Vec> key in the new Rc<SmallWeighted> dictionary.

The adapter still compares complete signature dictionaries and equal state
counts, then compares every ordered old/current state slice and its exact ID.
Both dictionaries use content lexicographic order, so equal cardinality plus
every zipped state/ID pair is equivalent to the old all-key lookup. No projected
collection, key owner or allocation is introduced. The current behavioral
receiver stays current; the entire complete old producer and every byte before
this existing adapter function remain unchanged.

The full immediate file is archived and restored exactly by the new one-row
source inverse. All earlier evidence packets remain immutable. Final combined
metadata must authenticate this new source stage before prior bindings.
No new test selector: existing epsilon-alias callers of same_closure_state own
the comparison. No Cargo, build, test, Clippy, Claude, push or ROOT edit occurred.
Executable qualification remains pending.
