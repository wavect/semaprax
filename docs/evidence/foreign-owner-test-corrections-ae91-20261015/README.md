# Foreign owner controls: ae91 corrections

Exact immediate source: ae91fa963ecfe85f4cc830cda85fff2f5bdbe094.
The complete wrong test sources and both actual95-round failure logs are retained.
No production behavior changes, new test bodies, Cargo/build/test/Clippy execution
or ROOT edits occurred. Source-only inverse proof is not a passing runtime receipt.

The held closure owner test called current_metrics outside measure. The complete
foreign/function/budget/nested lifecycle, including actual paid acquisition and
all local Drop, now runs within one measure. Its original sticky/no-foreign-query
assertions are unchanged. The expired-owner lifecycle also observes acquisition
through final Drop. Both observations require no invalid accounting, no live
bytes and allocated==freed. No observer starts after a constructor it is meant
to count, and no fee/allowance is exempted.

The typed declaration control's child scope was correctly untouched, its AST
unchanged, and its receiver sticky Detached after restoring ACTIVE. Its final
clean-originating-ledger assertion contradicted ClassifierLedger::current,
which explicitly records a zero-byte refusal and sets the HELD originating
ledger overflowed on a foreign owner. The test now requires that sticky
originating refusal with exact requested0, original remaining, floor0 and
stageNone. All original child-used0/no-first-refusal, allocation0, detached retry
and AST equality checks remain. A separate fresh unperturbed full declaration
rewrite retains the original !refused && first.is_none success assertion,
compares the complete result with the independent old rewrite, and observes
all cloned AST/receiver backing through Drop.

Both complete immediate sources are archived with full-source inverses. Existing
source consumers must authenticate the new combined current stage before older
immutable views. The weighted source lane owns those shared helper/view updates;
this patch does not rewrite its files or any historical runtime JSON. No claim
is made that the isolated patch alone has already passed older source guards.
