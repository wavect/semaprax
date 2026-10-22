# Fixed Main44 entry custody

Status: partial CI repair evidence; qualification is limited to the checks recorded below.

Audience: compiler maintainers and CI reviewers.

The original 26-key owner and authenticated immutable inverse table remain
unchanged. Each sealed entry is reconstructed from its complete original source
by reversing every original inverse step. All 26 entries then replay every
original step back to the exact complete original bytes, with unique matches and
all original SHA checks intact.

Current source comes from the existing original compiled INPUTS table. The
bounded no-follow ingress authenticates physical current bytes before reading a
sealed entry and calling the original complete projector. Both relocated vector
source keys resolve to their actual existing compiled physical files. The
supplied text must be compiled current or the exact sealed original entry.

The getters reuse existing modules and source acquisitions. No old key, class,
role, inverse, old source archive, source cap or diagnostic is removed. The
original whole source-domain test, including every original EOF drift control,
passes for the final candidate. Source-harness Clippy also passes. This is
private test infrastructure, not production or hosted qualification.
