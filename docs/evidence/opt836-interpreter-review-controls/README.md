# OPT836 interpreter review controls

Audience: integration maintainers and reviewers.

Status: authored, all three exact pending test names are unexecuted.

This additive fix binds complete immediate-prior source at
`6a3f309d101f84ad8a6673233eec474b83877156`, exact new bytes, and a
zero-context forward/inverse patch (`git apply --unidiff-zero`). No old test or
independent oracle is changed. The existing OPT836 implementation manifest is
historical evidence for its named checkpoint; this manifest binds this fix only.

Checked source creates the vector/record/String/Bytes through ordinary runtime
receivers. Tests introduce only hostile Arc aliases or generation exhaustion at
the private runtime boundary. Unique-store accounting includes actual metadata
controls and complete construction-through-Drop. The backend lane separately
owns public interpreter execution of its shared source corpus.

No Cargo, build, test, Clippy, benchmark or Claude command ran. Global historical
metadata remains deliberately uncomposed until parent integration.
