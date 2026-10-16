use super::fixture::{agent_program, agent_texts, agents, module_name, saved_original};
use super::*;
use crate::ast::AgentDeclaration;
use crate::test_allocation_observer::measure;

#[derive(Debug, Eq, PartialEq)]
struct Stop(usize);

fn with_text(declaration: &AgentDeclaration, text: &str) -> AgentDeclaration {
    let mut changed = declaration.clone();
    changed.stable_id = text.to_owned();
    for role in &mut changed.types {
        role.stable_id = text.to_owned();
    }
    for operation in &mut changed.operations {
        operation.stable_id = text.to_owned();
    }
    if let Some(binding) = &mut changed.model_wait {
        binding.helper_id = text.to_owned();
    }
    changed
}

#[test]
fn callback_receives_borrowed_source_text_in_original_order_and_matches_saved_builder() {
    let program = agent_program();
    assert!(!agents(&program).is_empty());
    let module = module_name(&program);
    for declaration in agents(&program) {
        let mut seen = Vec::new();
        let built = resolve_agent_declaration_with(declaration, module, |text| {
            seen.push((text.as_ptr(), text.len()));
            Ok::<_, Stop>(DeclarationId::new(text))
        })
        .unwrap();
        // Pointer equality proves the exact borrowed source str, not a clone.
        let expected: Vec<_> = agent_texts(declaration)
            .iter()
            .map(|text| (text.as_ptr(), text.len()))
            .collect();
        assert_eq!(seen, expected);
        assert_eq!(built, saved_original(declaration, module));
    }
}

#[test]
fn arbitrary_nonempty_nul_unicode_and_duplicate_text_matches_saved_builder() {
    let program = agent_program();
    let module = module_name(&program);
    for declaration in agents(&program) {
        for text in ["", "nul\0id", "\u{e9}\u{2713}.agent", "duplicate"] {
            let changed = with_text(declaration, text);
            let built = resolve_agent_declaration_with(&changed, module, |text| {
                Ok::<_, Stop>(DeclarationId::new(text))
            })
            .unwrap();
            let saved = saved_original(&changed, module);
            assert_eq!(built, saved);
            assert_eq!(built.stable_id.as_str().as_bytes(), text.as_bytes());
        }
    }
}

#[test]
fn identity_refusal_is_returned_unchanged_and_stops_later_requests() {
    let program = agent_program();
    let module = module_name(&program);
    for declaration in agents(&program) {
        let count = agent_texts(declaration).len();
        for stop in 0..count {
            let mut seen = 0;
            let (result, observed) = measure(|| {
                resolve_agent_declaration_with(declaration, module, |text| {
                    let at = seen;
                    seen += 1;
                    if at == stop {
                        Err(Stop(at))
                    } else {
                        Ok(DeclarationId::new(text))
                    }
                })
            });
            assert_eq!(result.err(), Some(Stop(stop)));
            assert_eq!(seen, stop + 1);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
            assert!(!observed.invalid);
        }
    }
}

#[test]
fn borrowed_payment_removes_exactly_one_temporary_string_per_identity() {
    let program = agent_program();
    let module = module_name(&program);
    for declaration in agents(&program) {
        let texts = agent_texts(declaration);
        let nonempty = texts.iter().filter(|text| !text.is_empty()).count();
        let bytes: usize = texts.iter().map(|text| text.len()).sum();
        let ((), old) = measure(|| drop(saved_original(declaration, module)));
        let (((), new), failed, _) = crate::bounded_output::with_limit_usage(fixture::BIG, || {
            let mut owner =
                crate::bounded_output::identity_reservation::IdentityReservation::acquire()
                    .unwrap();
            measure(|| {
                drop(
                    resolve_agent_declaration_with(declaration, module, |text| {
                        DeclarationId::paid_borrowed(text, |bytes| owner.reserve(bytes))
                    })
                    .ok()
                    .unwrap(),
                )
            })
        });
        assert!(!failed);
        assert_eq!(old.allocations, new.allocations + nonempty);
        assert_eq!(old.allocated, new.allocated + bytes);
        assert_eq!((old.live, new.live), (0, 0));
        assert!(!old.invalid && !new.invalid);
    }
}

#[test]
fn complete_original_missing_entrypoint_diagnostic_precedes_every_agent_request() {
    let mut program = agent_program();
    program.functions.retain(|function| function.name != "main");
    let original = fixture::original_resolve(&program).err().unwrap();
    let declarations = crate::hir::DeclarationIndex::from_verified_with_prelude(
        &program,
        crate::prelude::declarations_for_program(&program),
    )
    .unwrap();
    let mut seen = 0;
    let current = crate::hir::Resolver {
        program: &program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(&program),
        declarations,
        reuse: None,
        function_work: crate::hir::FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    }
    .resolve_with_agent_identities(|text| {
        seen += 1;
        Ok::<_, Stop>(DeclarationId::new(text))
    });
    match current {
        Err(AgentIdentityFailure::Semantic(error)) => assert_eq!(error, original),
        _ => panic!("missing main must select the complete original diagnostic"),
    }
    assert_eq!(seen, 0);
}
