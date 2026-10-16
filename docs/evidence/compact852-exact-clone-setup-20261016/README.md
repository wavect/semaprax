# OPT852 exact compact-owner clone setup

Source-only NEXT candidate from `e9dd415b563412b160fd4737102b9cb5437b1008`.
The actual historical a82e owner247 failed at the `floor source` setup clone,
before any of the three intended compact-constructor/comparison floor attacks.
`actual247-before.log` preserves that unmodified receipt. No Rust execution was
performed here; later target outcomes remain pending, including the sticky and
foreign-owner assertions that the historical run never reached.

`ReplayBudget::new` starts with zero skeleton allowance. `skeleton_clone` always
charges one skeleton unit before reserving clone backing/materializing. The
fixture prepared builder refusal storage but did not admit the owned setup clone.
The later `work source` clone had the same source-visible omission. The foreign
block already uses an explicit oracle skeleton allowance and is unchanged.

The repair calls existing `reserve_skeleton` for exactly one setup clone, or two
only for the comparison attack that creates two independently owned operands.
It subtracts those units from the unchanged global work allowance; it does not
raise a ceiling, set a free allowance, pay a donor, or alter production code. New
checks assert the setup spends exactly those units and leaves no skeleton units.
The work-first attack still explicitly zeros both original/current comparison
allowances before selecting the original work failure. Every old phase, floor,
first requested/remaining/floor/site, all six Diagnostic fields, zero-byte sticky
retry, foreign no-debit receipt, restored-owner retry and complete Drop assertion
is preserved, along with all three old target branches and the complete test
module. Target byte caps remain exactly their original values.

The original zero-funded setup is also executed as an explicit negative control
inside the same owner, on the original real residual source and phase. It must
select the complete recorded six-field work Diagnostic, spend/materialize no
skeleton owner, retain zero allowance, return the original empty sticky retry,
and produce no physical floor receipt. The added setup admission therefore does
not make zero-funded cloning legal or change the tested production frontier.

The complete old 454-line test module (all59 original assertions) and changelog
are preserved byte for byte. `source-bindings.json` gives additive whole-file
recipes and both full hashes. The verifier authenticates both complete forward
and reverse relations to exact base Git, rejects six empty/foreign/duplicate
whole inputs, and binds ten unchanged production/custody inputs. No new Rust
module or relocation was introduced. All three existing raw inventory files and
the complete original source-custody reader remain byte exact; native inventory
stays1500 lines. Tests module is501 lines. Old source tables/receipts are untouched.

Source context came first from retained `graft/src/cleanup_plan/replay/factored/
oracle_order.md`, replay and physical-budget cards. The issue's owning invariant
is RFC0003 sticky failure/work/order, with cleanup completion/quality rows and
architecture module-size/source-boundary rules unchanged. No syntax, backend,
public ABI, authority, feature status, application-fit or efficiency claim changes.

Bounded source check:

```sh
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 /opt/homebrew/bin/python3 docs/evidence/compact852-exact-clone-setup-20261016/verify-source.py
```

Literal minimum executable owner remains:

```json
{"package":"semaprax","kind":"lib","selector":"cleanup_plan::replay::factored::oracle_order::tests::compact_staged_receiving_keeps_floor_and_foreign_owner_first_refusal"}
```

This is actual344 historical247/current focused105. No new test selector or role
is added. The unchanged full compact order/receiving owners remain relevant to
OPT852 closure under the parent's existing scoped qualification, rather than
being credited by this source check.

After a future joined integration, preserve this private checkpoint and compose
any overlapping test/changelog bytes explicitly. The changed full tests input
must receive authenticated current-source endpoint binding before its historical
readers: Main043 first, Main44 second, overall a3/e47 authority last. Do not rewrite
this archive, original tables, or historical receipts to fit joined source. This
candidate stays separate from OPT858 and the parent's frozen qualification.
