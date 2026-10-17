# Replay work refusal capture

Status: test diagnostic capture authored; owning regression and CI pending.

Audience: compiler maintainers and reviewers.

The unchanged production-core retry regression currently reports SPX-H006 for
`stub.wide` during status identity directory work. That phase name is shared
by several receivers, so it does not establish which implementation refused.
Normal nonlexical replay uses the retained status receiver, whose larger sets
use the exclusive `HeldPrefix` builder. The original schema receiver is test
only. No particular receiver is blamed by this evidence packet.

Test builds now retain one plain record at the existing ordinary work-exhaustion
boundary: static caller location, requested and remaining work, status count,
first status identifier byte length and payer kind. A one-shot accessor clears
that thread's record. The existing identity retry regression clears it before
the failing ordinary build and prints its fields before its original unwrap.
Every assertion, fixture-size selection, retry count, builder limit, work limit
and production diagnostic remains. Production builds omit the record, hook,
caller tracking and accessor. The record owns no HIR, text or heap backing;
it is never used by a validation or budget decision and carries no authority.

Four complete predecessor sources have full hashes and exact inverses. Earlier
custody packets compose those inverses; the original archives and assertions
remain. This capture does not fix or qualify the status-directory work refusal,
any 64 MiB construction refusal, or whole current-head CI. The full original
regression and complete quality/Lean/backend gates remain required.

Local checks executed the complete charge hook and record transport on Rust
1.88 and 1.98 with explicit successful preflight/oracle and diagnostic/HIR
carrier adapters. All four payer labels, caller location, exact/exhausted work,
one-shot clearing and zero heap allocation passed. This component check is not
a full compiler run. All twenty complete source-custody assertion bodies and
the complete no-authority boundary assertion passed separately. Local compiler
process trees stayed below the unchanged 200 MiB guard.
