//! Logical authority is rebuilt; only exact invocation-paid backing survives.
use super::*;
use crate::bounded_output::{active_remaining, with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
use crate::workspace_graph::index_authored;
const PROVIDER: &str = r#"module private.provider;
@id("payload") record Payload { @id("payload.text") text:string, }
@id("root") variant Root {
 @id("root.ok") Ok { @id("root.payload") payload:Payload, },
 @id("root.no") No { @id("root.code") code:i64, @id("root.offset") offset:usize, @id("root.field") field:i64, },
}
@id("read") fn read(input:borrow Slice<u8>)->Root {Root::No {code:7,offset:0usize,field:0}}
"#;
const CALLER: &str = r#"module private.caller;
use type @id("root") from private.provider as Root;
use function @id("read") from private.provider as read;
@id("main") fn main()->i64 {let bytes=[0u8];let value=read(array_as_slice(bytes));match own value {Root::No {code,offset,field}=>code,Root::Ok {payload}=>0,}}
"#;
const EXTRA: &str = r#"module private.extra;
@id("extra.token")
resource Token {
    @id("extra.token.drop")
    drop import "extra.token.finalize";
}
@id("extra.host")
interface TokenHost
    permits { extra.token.release }
{
    @id("extra.token.finalize")
    import fn finalize(token: own Token) -> unit
        effects { extra.token.release }
        failure infallible
        consumes token always;
}
@id("extra.helper")
fn helper() -> i64
{
    0
}
"#;
const CLASSES: &str = r#"module private.classes;
@id("classes.counter")
class Counter {
    @id("classes.counter.value")
    value: i64,

    @id("classes.counter.get")
    fn get(self: Counter) -> i64
{
        self.value
    }
}
@id("classes.helper")
fn helper() -> i64 { 0 }
"#;
fn programs(classes: bool) -> Vec<Program> {
    [CALLER, PROVIDER, if classes { CLASSES } else { EXTRA }]
        .iter()
        .enumerate()
        .map(|(n, s)| crate::parse(s, format!("authority-{n}.spx")).unwrap())
        .collect()
}

#[test]
fn sequential_leases_repeat_complete_old_roots_work_queries_and_current_callbacks() {
    for classes in [false, true] {
        let mut programs = programs(classes);
        let second = |source: &str| {
            source
                .replace("private.caller", "private.second.caller")
                .replace("private.provider", "private.second.provider")
                .replace("\"payload", "\"second.payload")
                .replace("\"root", "\"second.root")
                .replace("\"read\"", "\"second.read\"")
                .replace("\"main\"", "\"second.main\"")
        };
        programs.push(crate::parse(&second(CALLER), "second-caller.spx").unwrap());
        programs.push(crate::parse(&second(PROVIDER), "second-provider.spx").unwrap());
        let authored = index_authored(&programs).unwrap();
        let old = Invocation::new(&programs, &authored);
        let owner = ScratchInvocation::new(&programs, &authored).unwrap();
        for index in [0, 3, 2, 3, 0, 1, 0, 3] {
            let saved = old
                .derive_bound(&programs[index], &programs, &authored)
                .unwrap();
            let lease = owner
                .lease_bound(&programs[index], &programs, &authored)
                .unwrap();
            assert_eq!(&*lease.roots, &saved.roots);
            for (actual, previous) in lease.roots.iter().zip(&saved.roots) {
                assert!(std::ptr::eq(actual.0, previous.0));
                assert!(std::ptr::eq(actual.1, previous.1));
            }
            assert_eq!(lease.remaining.get(), saved.remaining.get());
            for (id, module) in [
                ("payload", Some("private.provider")),
                ("second.payload", Some("private.second.provider")),
                ("payload", None),
                ("root", Some("private.provider")),
            ] {
                assert_eq!(
                    lease.view().unwrap().contains(id, module),
                    saved.contains(id, module)
                );
                assert_eq!(lease.remaining.get(), saved.remaining.get());
            }
            let mut current_calls = 0;
            let mut previous_calls = 0;
            assert_eq!(
                lease.view().unwrap().validate_signatures(|_, _| {
                    current_calls += 1;
                    true
                }),
                saved.validate_signatures(|_, _| {
                    previous_calls += 1;
                    true
                })
            );
            assert_eq!(current_calls, previous_calls);
            assert_eq!(lease.remaining.get(), saved.remaining.get());
            // Semantic rejection is fresh on every callback and never poisons derivation.
            assert_eq!(
                lease.view().unwrap().validate_signatures(|_, _| false),
                saved.validate_signatures(|_, _| false)
            );
            assert_eq!(lease.remaining.get(), saved.remaining.get());
            drop(lease);
            assert!(owner.roots.borrow().is_empty());
            assert!(owner.base.state.borrow().frames.is_empty());
            assert!(!owner.base.state.borrow().failed);
        }
    }
}

#[test]
fn roots_backing_is_paid_once_exact_short_floor_sticky_and_drop_are_observed() {
    let programs = programs(false);
    let authored = index_authored(&programs).unwrap();
    let count: usize = programs.iter().map(|p| p.types.len()).sum();
    let frames = DEPTH * size_of::<Frame<'_>>();
    let roots = count * size_of::<(&str, &str)>();
    let run = || {
        let owner = ScratchInvocation::new(&programs, &authored).ok_or_else(refusal)?;
        for n in 0..16 {
            let lease = owner.lease_bound(&programs[0], &programs, &authored)?;
            let observed = current_metrics();
            assert!(!observed.invalid);
            assert_eq!(
                observed.allocated,
                frames + roots,
                "reuse {n}: {observed:?}"
            );
            assert_eq!(observed.live, frames + roots);
            assert_eq!(observed.allocations, 2);
            assert_eq!(observed.reallocations, 0);
            drop(lease);
            assert_eq!(owner.roots.borrow().capacity(), count);
            assert_eq!(current_metrics().live, frames + roots);
        }
        Ok::<_, Vec<Diagnostic>>(())
    };
    let ((answer, metrics), refused, exact) = with_limit_usage(usize::MAX, || measure(run));
    answer.unwrap();
    assert!(!refused && !metrics.invalid);
    assert_eq!(metrics.allocated, frames + roots);
    assert_eq!(metrics.highwater, frames + roots);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.freed, metrics.allocated);
    assert!(metrics.allocated <= exact);
    let ((answer, metrics), refused, paid) = with_limit_usage(exact, || measure(run));
    answer.unwrap();
    assert!(!refused && !metrics.invalid);
    assert_eq!(paid, exact);
    assert_eq!(metrics.live, 0);
    // Complete independently executed previous constructor retains 16 root acquisitions.
    let ((previous, previous_metrics), refused, previous_paid) =
        with_limit_usage(usize::MAX, || {
            measure(|| {
                let old = Invocation::new(&programs, &authored);
                for _ in 0..16 {
                    drop(old.derive_bound(&programs[0], &programs, &authored)?);
                }
                Ok::<_, Vec<Diagnostic>>(())
            })
        });
    previous.unwrap();
    assert!(!refused && !previous_metrics.invalid);
    assert_eq!(previous_paid, frames + 16 * roots);
    assert_eq!(previous_metrics.allocated, previous_paid);
    assert_eq!(previous_metrics.allocations, 17);
    assert_eq!(previous_metrics.live, 0);
    assert_eq!(previous_metrics.freed, previous_metrics.allocated);
    for floor in [0, 1] {
        let ((answer, observed), refused, paid, first) =
            with_limit_usage_refusal(exact - 1 + floor, || {
                measure(|| {
                    assert!(crate::bounded_output::set_active_floor(floor));
                    let owner = ScratchInvocation::new(&programs, &authored).unwrap();
                    let mut accepted = 0;
                    loop {
                        match owner.lease_bound(&programs[0], &programs, &authored) {
                            Ok(lease) => {
                                accepted += 1;
                                drop(lease);
                            }
                            Err(error) => {
                                assert_eq!(accepted, 15);
                                assert_eq!(error[0].code, "SPX-G171");
                                let remaining = active_remaining();
                                assert!(owner
                                    .lease_bound(&programs[0], &programs, &authored)
                                    .is_err());
                                assert_eq!(active_remaining(), remaining);
                                assert!(owner.roots.borrow().is_empty());
                                assert!(owner.base.state.borrow().frames.is_empty());
                                break;
                            }
                        }
                    }
                    Ok::<_, Vec<Diagnostic>>(())
                })
            });
        answer.unwrap();
        assert!(refused && first.is_some());
        assert!(!observed.invalid);
        assert!(observed.allocated <= paid, "{observed:?}, paid={paid}");
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        assert_eq!(observed.reallocations, 0);
        let first = first.unwrap();
        assert_eq!(first.requested, lease_controls().unwrap());
        assert_eq!(first.remaining, lease_controls().unwrap() - 1 + floor);
        assert_eq!(first.floor, floor);
    }
    let ((owner, observed), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| ScratchInvocation::new(&programs, &authored))
    });
    assert!(owner.is_none() && refused);
    assert_eq!(paid, 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.live, 0);
    assert!(first.unwrap().requested > 0);
}

#[test]
fn absent_explicit_zero_types_and_ordinary_fallback_keep_unfavorable_controls_visible() {
    // Keep the original imports/declarations-free source as an exact parser refusal.
    let diagnostic = crate::parse("module empty;", "empty.spx").unwrap_err();
    assert_eq!(diagnostic.code, "SPX-P101");
    assert_eq!(diagnostic.severity, crate::diagnostic::Severity::Error);
    assert_eq!(
        diagnostic.message,
        "a module must declare at least one function"
    );
    assert_eq!(diagnostic.path.as_deref(), Some("empty.spx"));
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start, span.end, span.line, span.column)),
        Some((13, 13, 1, 14))
    );
    assert_eq!(diagnostic.help, None);
    for source in [
        "module empty; @id(\"main\") fn main()->i64 {0}",
        "module empty; @id(\"empty.helper\") fn helper()->i64 {0}",
    ] {
        let programs = vec![crate::parse(source, "empty.spx").unwrap()];
        let authored = index_authored(&programs).unwrap();
        let ((answer, observed), refused, paid) = with_limit_usage(usize::MAX, || {
            measure(|| {
                let owner = ScratchInvocation::new(&programs, &authored).unwrap();
                for _ in 0..2 {
                    let lease = owner.lease_bound(&programs[0], &programs, &authored)?;
                    assert!(lease.roots.is_empty());
                }
                assert_eq!(owner.roots.borrow().capacity(), 0);
                assert_eq!(owner.base.state.borrow().frames.capacity(), 0);
                Ok::<_, Vec<Diagnostic>>(())
            })
        });
        answer.unwrap();
        assert!(!refused && !observed.invalid);
        assert_eq!(observed.allocated, 0);
        assert!(
            paid > 0,
            "new owner/lease headers are a real tiny-case cost"
        );
        let old = PrivateTypes::derive(&programs[0], &programs, &authored).unwrap();
        let (new, refused, ordinary_paid) = with_limit_usage(usize::MAX, || {
            Current::derive(None, &programs[0], &programs, &authored)
        });
        let new = new.unwrap();
        assert!(!refused);
        assert_eq!(new.contains("absent", None), old.contains("absent", None));
        assert!(
            ordinary_paid > 0,
            "ordinary fallback pays its widened receiving carrier"
        );
    }
    // Explicit-only import proves absence before any roots or DFS backing.
    let mut programs = programs(false);
    programs[0]
        .module_uses
        .retain(|u| u.kind == ModuleUseKind::Function);
    programs[1].functions[0].return_type = crate::ast::Type::I64;
    let authored = index_authored(&programs).unwrap();
    let owner = ScratchInvocation::new(&programs, &authored).unwrap();
    let lease = owner
        .lease_bound(&programs[0], &programs, &authored)
        .unwrap();
    assert!(lease.roots.is_empty());
    assert_eq!(lease.roots.capacity(), 0);
    assert_eq!(lease.state.frames.capacity(), 0);
}

#[test]
fn foreign_bindings_overlap_current_work_and_unwind_cannot_reuse_stale_authority() {
    let programs = programs(false);
    let authored = index_authored(&programs).unwrap();
    for mutation in 0..4 {
        let mut wrong = index_authored(&programs).unwrap();
        let entry = wrong.get_mut("payload").unwrap();
        match mutation {
            0 => entry.path = "foreign.spx",
            1 => entry.module = "foreign",
            2 => entry.explicit = false,
            _ => entry.kind = crate::workspace_graph::AuthoredKind::Other,
        }
        assert_eq!(wrong.len(), authored.len());
        let owner = ScratchInvocation::new(&programs, &wrong).unwrap();
        assert!(owner.lease_bound(&programs[0], &programs, &wrong).is_err());
        assert!(owner.base.state.borrow().failed);
        assert!(owner.roots.borrow().is_empty());
        assert!(owner.base.state.borrow().frames.is_empty());
        assert!(owner.lease_bound(&programs[0], &programs, &wrong).is_err());
    }
    for which in 0..3 {
        let owner = ScratchInvocation::new(&programs, &authored).unwrap();
        let copied_programs = programs.clone();
        let copied_map = index_authored(&programs).unwrap();
        let foreign = programs[0].clone();
        let answer = match which {
            0 => owner.lease_bound(&foreign, &programs, &authored),
            1 => owner.lease_bound(&programs[0], &copied_programs, &authored),
            _ => owner.lease_bound(&programs[0], &programs, &copied_map),
        };
        assert!(answer.is_err());
        drop(answer);
        assert!(owner.base.state.borrow().failed);
        assert!(owner
            .lease_bound(&programs[0], &programs, &authored)
            .is_err());
    }
    let owner = ScratchInvocation::new(&programs, &authored).unwrap();
    let old = Invocation::new(&programs, &authored)
        .derive(&programs[0])
        .unwrap();
    let lease = owner
        .lease_bound(&programs[0], &programs, &authored)
        .unwrap();
    assert!(owner
        .lease_bound(&programs[0], &programs, &authored)
        .is_err());
    assert!(lease
        .view()
        .unwrap()
        .contains("payload", Some("private.provider"))
        .unwrap());
    old.remaining.set(0);
    lease.remaining.set(0);
    assert_eq!(
        lease
            .view()
            .unwrap()
            .contains("payload", Some("private.provider")),
        old.contains("payload", Some("private.provider"))
    );
    assert_eq!(lease.remaining.get(), old.remaining.get());
    assert_eq!(
        lease.view().unwrap().validate_signatures(|_, _| true),
        old.validate_signatures(|_, _| true)
    );
    drop(lease);
    assert!(
        !owner.base.state.borrow().failed,
        "consumer WORK failure is not derivation poisoning"
    );
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _lease = owner
            .lease_bound(&programs[0], &programs, &authored)
            .unwrap();
        panic!("consumer unwind");
    }));
    assert!(panicked.is_err());
    assert!(owner.roots.borrow().is_empty());
    assert!(owner.base.state.borrow().frames.is_empty());
    drop(
        owner
            .lease_bound(&programs[0], &programs, &authored)
            .unwrap(),
    );
    let (_, refused, _) = with_limit_usage(usize::MAX, || {
        let owner = ScratchInvocation::new(&programs, &authored).unwrap();
        let lease = owner
            .lease_bound(&programs[0], &programs, &authored)
            .unwrap();
        let (rejected, _, _) = with_limit_usage(usize::MAX, || lease.view().is_err());
        assert!(
            rejected,
            "an equal-limit foreign ledger supplies no authority"
        );
        assert!(lease
            .view()
            .unwrap()
            .contains("payload", Some("private.provider"))
            .unwrap());
        drop(lease);
        let (_, _, _) = with_limit_usage(usize::MAX, || {
            assert!(owner
                .lease_bound(&programs[0], &programs, &authored)
                .is_err())
        });
        drop(
            owner
                .lease_bound(&programs[0], &programs, &authored)
                .unwrap(),
        );
        crate::bounded_output::refuse_active_required(0);
        assert!(owner
            .lease_bound(&programs[0], &programs, &authored)
            .is_err());
    });
    assert!(refused);
}

#[test]
fn incomplete_partial_walk_and_overflow_layout_never_publish_roots() {
    let programs = programs(false);
    let authored = index_authored(&programs).unwrap();
    let owner = ScratchInvocation::new(&programs, &authored).unwrap();
    drop(
        owner
            .lease_bound(&programs[0], &programs, &authored)
            .unwrap(),
    );
    // Calibrate the complete actual walk, then deny its last work unit. The
    // real guard is already installed while partial DFS/root state exists.
    let mut remaining = WORK;
    {
        let mut state = owner.base.state.borrow_mut();
        let mut roots = owner.roots.borrow_mut();
        let State {
            held,
            count,
            frames,
            ..
        } = &mut *state;
        owner
            .fill(
                &programs[0],
                held.as_ref().unwrap(),
                *count,
                &mut remaining,
                frames,
                &mut roots,
            )
            .unwrap();
        roots.clear();
        frames.clear();
    }
    let needed = WORK - remaining;
    let mut lease = Lease {
        state: owner.base.state.borrow_mut(),
        roots: owner.roots.borrow_mut(),
        remaining: Cell::new(needed - 1),
        binding: &owner.binding,
        complete: false,
    };
    let State {
        held,
        count,
        frames,
        ..
    } = &mut *lease.state;
    let mut work = needed - 1;
    assert!(owner
        .fill(
            &programs[0],
            held.as_ref().unwrap(),
            *count,
            &mut work,
            frames,
            &mut lease.roots
        )
        .is_err());
    assert!(
        !lease.roots.is_empty(),
        "fixture must actually reach partial roots before refusing"
    );
    drop(lease);
    assert!(owner.roots.borrow().is_empty());
    assert!(owner.base.state.borrow().frames.is_empty());
    assert!(owner.base.state.borrow().failed);
    assert!(owner
        .lease_bound(&programs[0], &programs, &authored)
        .is_err());
    let owner = ScratchInvocation::new(&programs, &authored).unwrap();
    drop(
        owner
            .lease_bound(&programs[0], &programs, &authored)
            .unwrap(),
    );
    let mut roots = Vec::new();
    let mut frames = Vec::new();
    let mut work = WORK;
    let state = owner.base.state.borrow();
    assert!(owner
        .fill(
            &programs[0],
            state.held.as_ref().unwrap(),
            usize::MAX,
            &mut work,
            &mut frames,
            &mut roots
        )
        .is_err());
    assert!(roots.is_empty() && frames.is_empty());
}

#[test]
fn all_four_complete_consumers_match_independent_previous_receivers_and_six_diagnostic_fields() {
    use crate::workspace_graph::{self as wg, validation};
    let mut programs = programs(false);
    programs.truncate(2);
    let authored = index_authored(&programs).unwrap();
    let mut before_compiler = BTreeMap::new();
    let mut current_compiler = BTreeMap::new();
    for _ in 0..2 {
        assert_eq!(
            validation::before_roots::insert_expected_compiler_declaration(
                &mut before_compiler,
                "compiler.control",
                crate::hir::DeclarationKind::Function,
                None
            ),
            validation::insert_expected_compiler_declaration(
                &mut current_compiler,
                "compiler.control",
                crate::hir::DeclarationKind::Function,
                None
            )
        );
        assert_eq!(before_compiler, current_compiler);
    }
    let original: Vec<(String, wg::ResolvedModuleInput)> = programs
        .iter()
        .map(|p| {
            let synthetic =
                wg::expected_projection::synthetic_program(p, &authored, &programs).unwrap();
            (
                p.module.clone(),
                crate::hir::resolve(&synthetic).unwrap().into(),
            )
        })
        .collect();
    let mut previous_ordinary =
        validation::before_roots::WorkspaceValidationIndex::new(&programs).unwrap();
    let mut current_ordinary = validation::WorkspaceValidationIndex::new(&programs).unwrap();
    for (module, resolved) in &original {
        assert_eq!(
            current_ordinary.record_module(module, resolved, &programs),
            previous_ordinary.record_module(module, resolved, &programs)
        );
    }
    assert_eq!(
        current_ordinary.validate_stub_signatures(&programs),
        previous_ordinary.validate_stub_signatures(&programs)
    );
    assert_eq!(
        current_ordinary.finish(&programs),
        previous_ordinary.finish(&programs)
    );
    for mutation in 0..5 {
        let mut modules = original.clone();
        if mutation != 0 {
            let payload = modules[0]
                .1
                .types
                .iter_mut()
                .find(|t| t.id.as_str() == "payload")
                .unwrap();
            if mutation == 1 {
                payload.id = crate::hir::DeclarationId::new("stale.payload");
            } else if mutation == 2 {
                modules[0].1.types.retain(|t| t.id.as_str() != "payload");
            } else {
                let crate::hir::ResolvedTypeDeclarationKind::Record { fields } = &mut payload.kind
                else {
                    unreachable!()
                };
                if mutation == 3 {
                    fields[0].ty = crate::hir::ResolvedType::Bytes;
                } else {
                    fields[0].id = crate::hir::DeclarationId::new("foreign.field");
                }
            }
        }
        for use_owner in [false, true] {
            let old = Invocation::new(&programs, &authored);
            let current = ScratchInvocation::new(&programs, &authored).unwrap();
            let old_owner = use_owner.then_some(&old);
            let current_owner = use_owner.then_some(&current);
            assert_eq!(
                wg::reconstruct_workspace_declaration_facts_with_authority(
                    &modules,
                    &programs,
                    &authored,
                    current_owner
                ),
                wg::before_private_roots::reconstruct_workspace_declaration_facts_with_authority(
                    &modules, &programs, &authored, old_owner
                )
            );
            if !use_owner {
                assert_eq!(
                    validation::before_roots::validate_stub_signatures(&programs, &modules),
                    validation::validate_stub_signatures(&programs, &modules)
                );
            }
            assert_eq!(
                validation::validate_stub_signatures_authored(
                    &programs,
                    &modules,
                    &authored,
                    current_owner
                ),
                validation::before_roots::validate_stub_signatures_authored(
                    &programs, &modules, &authored, old_owner
                )
            );
            let mut previous =
                validation::before_roots::WorkspaceValidationIndex::new(&programs).unwrap();
            let mut receiving = validation::WorkspaceValidationIndex::new(&programs).unwrap();
            let mut recorded = true;
            for (module, resolved) in &modules {
                let before = previous
                    .record_module_authored(module, resolved, &programs, &authored, old_owner);
                let after = receiving.record_module_authored(
                    module,
                    resolved,
                    &programs,
                    &authored,
                    current_owner,
                );
                assert_eq!(
                    after, before,
                    "complete declaration receiver, mutation {mutation}"
                );
                if before.is_err() {
                    recorded = false;
                    break;
                }
            }
            if recorded {
                if !use_owner {
                    assert_eq!(
                        receiving.validate_stub_signatures(&programs),
                        previous.validate_stub_signatures(&programs)
                    );
                }
                assert_eq!(
                    receiving.validate_stub_signatures_authored(
                        &programs,
                        &authored,
                        current_owner
                    ),
                    previous.validate_stub_signatures_authored(&programs, &authored, old_owner)
                );
                assert_eq!(receiving.finish(&programs), previous.finish(&programs));
            }
            assert!(current.roots.borrow().is_empty());
            assert!(current.base.state.borrow().frames.is_empty());
        }
    }
}
