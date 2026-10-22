# Separate final PLAN source ingress

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

This source-only candidate adds one finite acquisition before the complete
`787d9810480209532fa645b8cb97f2585ce5f426` PLAN stage. The six literal inputs
are the existing `CURRENT` owners. Original 787 six-row stages, old six-row
packet, root inverse/archive, original manifest and 787 current manifest remain
byte-exact. The existing `legacy_checkpoint` changes only its physical/current
acquisition binding; all its prior predicates, stage inverses and domain
selection remain. Only exact original c874 expected hashes reach this ingress.
Current/raw and older expected namespaces remain unchanged in `acquire_expected`.

Before accepting any supplied stage, the new reader authenticates all six
complete compiled and physical current sources, their retained complete current
archives, and complete inverses to independent Git787 archives. The reader uses
one no-follow regular handle, exact length/EOF/SHA/UTF-8 and post-handle/path
identity. Packet authentication precedes parsing. Its 2 MiB cap is unchanged.
Raw source carriers are read as physical raw sources and retain their original
043 fragment/count checks; none is projected for its own raw audit.

The initial current packet binds the known Git787 checkpoint. It is intentionally
stale after later joins until the parent performs the separate final bind.
`ordinary_plan_inputs.rs` is outside the six source paths, avoiding a hash cycle
when the new current packet length/SHA pins are updated. The new nine-owner
manifest is independent of the immutable 787 manifest.

The guarded new recipe is the ONLY new final binder. Do not execute the old
`bind_stages.py` or its delegated `bind_sources.py` on a moving checkout.
The new binder requires the actual full 40-character HEAD and all six source
bytes and the other eight PLAN owners equal to that Git HEAD. It rejects
unexpected helper code changes, recipe/template drift, modified old packets,
changed Git787 archives, malformed prior current metadata, symlinks/nonregular
files, overbound data and source/HEAD races. Only its two new helper pins, new
current packet, new current owner manifest and new receipt are rewritten;
complete current sources and prior current packets are retained by content
address. No old table or endpoint is retargeted.

After review and final source integration, the parent may explicitly invoke:

```
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 /opt/homebrew/bin/python3 docs/evidence/ordinary-plan-final-ingress-20261017/bind-current-inputs.py --repo /Volumes/Semaprax/worktrees/next-large-integration-20261017 --expected-head FINAL_FULL40_HEAD --write
```

This is a command template, not a recorded execution. Replace the placeholder
with the actual current full HEAD. Omit `--write` for the later fixed-point
check. Commit the binding outputs and perform the parent's existing common
source/authority refresh after these Rust pin writes. A later source change
requires another explicit reviewed HEAD; no runtime receipt may be relabeled.

Pure source controls (no binder main) are available through
`verify-source.py --expected-head FULL40`; `--output` accepts only an absolute
path under `/Volumes/Semaprax/provenance`. They verify all six exact chains,
source-helper restoration to Git787, all frozen artifacts, raw-three original
043 predicates, current owner hashes, physical-reader refusal and synthetic
nonidentity UTF-8 inverses. The authored Rust selector is
`source_reflows_merge::ordinary_plan_inputs::final_ingress_six_physical_owners_preserve_frozen_stages_and_expected_dispatch`.
No Cargo, rustc, rustfmt, Rust test, binder main, authority generation or
application gate was executed for this candidate. Type correctness and final
application support remain unqualified.
