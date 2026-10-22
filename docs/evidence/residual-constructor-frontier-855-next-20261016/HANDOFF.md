# #855 compact residual receiving correction — source only

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Direct Main7c6a2a801f70525f31a2cd49fe64b59ab32f75f8. No Cargo/test/build or RAM claim.
All current repository calls use sanctioned `hir::resolved_residual_type!`; no
shipping `try_new(Type)` owned forwarding parameter remains. A fixed zero-owned-
argument helper admits before lexical Some/Self/Box construction. Original
resolve/cache Type producers run first unchanged; monomorphize keeps substitute
at the original final-field position before admission, moving the old producer
result into a field-local instead of an extra owned constructor parameter.
Earlier fields/work/error order are unchanged.

Actual bank: one Type heap backing + Box::new's actual Type parameter; two
Option<Self> initializer/receiving; two Self initializer/receiving; TWO Box
returned/tuple-field headers independently of Self; one reservation usize; two
bools (required reservation result + new zero-arg helper return). No eliminated
try_new parameter fee, temporary, observation-derived deficit or arbitrary
padding. Extra Box field and bool fees may be unfavorable; no savings claim.
Initial reservation's own cold ABI remains ordinary unreceived caller control,
not whole #825 constructor/report funding. Nested Type backing/work/caps unchanged.

Raw public tuple field is permitted only as untrusted public HIR model data,
like other public Expr/Kind fields. Macro export/reexport works across the
native-interop crate without naming private core budget/expr modules. No new
validation, source, cache, cleanup, backend or capability authority: hir::validate
and existing independent gates remain mandatory. Parser-independent external
fixtures stay parser-independent; no frontend setup or changed observer window.
Owning versioned contract: docs/HIR-RESIDUAL-CONSTRUCTION-V1.md.

Complete original Main expr_nodes.rs (812 lines, including old constructor,
opaque wrapper and ALL nested borrowed/residual tests) is independently compiled
as hir::residual_constructor_before. Its exact Main Codec implementation is
cfg(test) for that old nominal wrapper, never routed through modern constructor.
The existing pre-Main residual_payload_before remains untouched. All original
four current Main test assertions survive, with only constructor expressions and
old compound producer locals adapted. A fifth current test checks old producer
versus new argument order, floor refusal, current nested scope and restoration
of first sticky refusal. Complete original source files/inverses are archived.

verify-source.py PASS10 complete whole source current→Main7c6 inverses,
byte-exact entire old model/nested tests/old nominal codec, absent current owned
constructor, unchanged cache/backend/validation/capacity/language controls.
This is source-only and does not prove runtime/compiler success. Private model
recursive module/privacy and public macro forwarding were independently read;
no concrete blocker remained. Module sizes stay <=1500.

New source owner: src/hir/residual_type/before.rs only. Canonical compositor owns
its all-three raw audit joins and new authority metadata; no shared helper/audit
was edited here. Main12 reader must receive an independently authenticated
final→Main7c6 whole source view before original immutable inverse predicates.
The component current-to-main7c6.json has10 rows/25,147B; a final→855 component
view is separate from other components/oldMain domains. Do not overwrite any
original Main12/Main7/ours old5/Main9 table or old/archive bytes. Parent can wrap
this small source inverse in its fixed runtime final-custody inventory.

pending-selectors.json contains five current residual controls, four independent
Main predecessor controls, and the existing native-interop HIR fingerprint
owner (two exact direct constructor fixtures). These are finite existing
harnesses; no standalone binary. Full current test execution remains pending.
