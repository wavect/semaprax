# Seven consumer checks at d8fad

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Prepared source is exact d8fadfb7cd5ece54504cda542575e5a154d3657c; its six live
Rust changes equal reviewed 1cc exactly. All original 145 actual rows and 25
failures remain bound and disclosed. Nothing relabels those failures PASS.
Original six execution drivers are copied byte-exact. No compiler/runtime was
run during preparation. Prior successful production Clippy is reused under the
explicit parent-approved production-equivalence scope; no Clippy rerun is added.

Run only after the preparation receipt and all driver files exist:

```sh
ROUND=/Volumes/Semaprax/lane-prompts/pin-consumer-d8fad-seven-20261015
python3 /Volumes/Semaprax/provenance/focused-build-pressure-wrapper-v4-20261015.py --round "$ROUND" --rss-limit-gib 7
# Only after actual successful complete build:
SEMAPRAX_CHECK_ROUND="$ROUND" python3 "$ROUND/drivers/run-owning-selected.py"
```

The existing planner emits three commands: unit --no-run, ordinary production
lib build, then offline_package and workspace --no-run in one command. Jobs1,
pinned Rust1.97.1 and existing CGU64/profile/7GiB guardian remain unchanged.
Runtime uses the original driver's 8GiB test guard and seven exact bodies.
No owner was renamed and no ignored/full-suite/wildcard selection is added.

Historical early guardian invocation without prepared drivers was an
infrastructure preflight failure before any build/body; keep it separately.
Fresh actual build and selected receipts must be retained without overwrites.
