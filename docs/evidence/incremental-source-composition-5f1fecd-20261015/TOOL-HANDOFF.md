# Successor source-custody tools — preparation checkpoint

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

These tools are authored for the pending merged source after main88884. They
have not composed or independently verified that final source. Source-only
Python syntax checking is not execution evidence. No Rust, Cargo, build, test,
Clippy, Node or executable discovery is part of this lane.

The private earlier pass at `ea0be15f4dea6f3db36a031e17988e9fa105dbf5` produced
97 packets / 868 rows and all36 Source36 assertions passed, with a 16,768,430-byte
Git-cache peak. That is a historical preparation observation, not a final merged
proof. Its generated outputs remain in the private worktree, outside this
source-ready tool checkpoint.

`compose.py` defaults to ALL90 previous active packets / 795 rows, including all
three additions that the old d39 compositor required on its command line. It
then adds complete 839 initial/result-fee and 838 checkpoint/amortized stage
packets (all66 full commit patch paths), the two-warning packet, the earlier
Source36 successor's inverse/four views, and all three reviewed MAIN Source36
packets. The838 MAIN literal adapter adds its single helper inverse; its four runtime
views and local metadata history remain protected inputs. Expected default
inventory is101 packets / 904 rows; only a completed
final invocation can establish the actual counts. Additional packets may be
explicitly supplied; no default packet is removed.

The immutable stage originals, seeds and complete patches are prepared here.
`prepare_next_packets.py` refuses to overwrite any changed immutable prepared
bytes. Only active source-inverse manifests are generated/recomposed later.
Neither these tools nor the new Source36 paths recursively compose local history
JSON. All prior proof/script/archive bytes, hook lists, full independent old
receivers and original capacity bounds remain mandatory.

`prepare_receiver_bindings.py` requires all53 full independent receivers and both
unchanged original manifests. A changed current receiver needs a complete
current-to-immutable-old join with the exact original expected SHA. The joined
verifier reports byte-unchanged and joined receivers separately; it cannot claim
53 unchanged when MAIN formatting changed some of them.

`prepare_starter_bindings.py` begins with all38 current d39 starter joins and
preserves every prior inverse. `verify_joined.py` independently checks their
complete32+6 archived patch paths and all66 839/838 patch paths by reverse
application through private Git indexes. Its Git object cache is bounded at
16 MiB / 128 entries. Archives are checked one source at a time.

After the parent supplies the final merged source basis, use a clean private
worktree with that exact HEAD and these tools; preserve earlier generated
outputs separately. Replace FINAL_BASIS with the supplied full40hex revision:

```sh
python3 docs/evidence/incremental-source-composition-5f1fecd-20261015/prepare_next_packets.py
python3 docs/evidence/incremental-source-composition-5f1fecd-20261015/prepare_starter_bindings.py --basis FINAL_BASIS
python3 docs/evidence/incremental-source-composition-5f1fecd-20261015/prepare_receiver_bindings.py --basis FINAL_BASIS
python3 docs/evidence/incremental-source-composition-5f1fecd-20261015/compose.py --basis FINAL_BASIS
python3 docs/evidence/incremental-source-composition-5f1fecd-20261015/compose.py --verify-only --basis FINAL_BASIS
python3 docs/evidence/incremental-source-composition-5f1fecd-20261015/verify_joined.py --basis FINAL_BASIS
```

The default outboard runner is the reviewed
`docs/evidence/source36-main88884-merge-20261015/verify_source36.py`; its original
36 inverses plus71 MAIN reflow/presence assertions remain required. Its three new
JSON packets authenticate complete current source before the unchanged old
checks. No docscript or CI guard is rewritten or weakened by these tools.

This metadata lane adds no Rust test registration selectors. Keep the separate
parent owner-selection inventory and every existing 838/839/original source
selector. The new Source36 presence/projection owning selector comes from the
reviewer's source adapter. No application recovery, 838/839 completion, runtime,
backend, budget-fit, or live-model efficiency claim follows from metadata proof.

Protected-domain correction after the actual2b5 checkpoint: the first compositor
rewrote an intermediate Source36 runtime manifest for physical current source,
then its original runner correctly refused the mismatched domain. The corrected
new tool writes derived active manifests under `derived-packets/`, retaining the
logical original packet identities and all old hooks. The nine actual Source36
packet files, MAIN71 and presence JSON, the838 runtime views and local metadata
histories remain byte-exact. `--protected-runtime-input` extends this explicitly.
The historical failed and corrected2b5 source-only runs are recorded once under
`execution-history/2b5cae-checkpoint/`; they are not the final successor proof.
No generated active packet manifests or current proof outputs are included in
this correction-only commit. Final composition must be rerun at the supplied
source basis after the parent applies the tool correction.
