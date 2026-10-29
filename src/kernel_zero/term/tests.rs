use super::*;
use crate::kernel_zero::{eval::eval_program, reify::BoundTranslation, value::Value};
use crate::test_allocation_observer::measure;

// Saved linear lookup, independent of both current entry points.
fn old_lookup<'a>(program: &'a KernelProgram, id: &DeclarationId) -> Option<&'a KernelFn> {
    program.functions.iter().find(|function| &function.id == id)
}

fn function(id: &str, value: i64) -> KernelFn {
    KernelFn {
        id: DeclarationId::new(id),
        params: Vec::new(),
        return_type: KernelType::I64,
        body: Term::Int(value),
    }
}

#[test]
fn borrowed_entry_preserves_exact_current_pointer_and_first_duplicate_without_allocation() {
    let program = KernelProgram {
        functions: vec![
            function("", 0),
            function("entry.prefix", 1),
            function("entry.prefix.long", 2),
            function("entry.prefix", 3),
            function("entrée", 4),
        ],
    };
    let foreign = program.clone();
    for text in [
        "",
        "entry.prefix",
        "entry.prefix.long",
        "entrée",
        "missing",
        "entry",
        "ENTRY.PREFIX",
    ] {
        let id = DeclarationId::new(text);
        let old = old_lookup(&program, &id);
        let new = program.function_text(text);
        assert_eq!(old.map(std::ptr::from_ref), new.map(std::ptr::from_ref));
        assert_eq!(
            program.function(&id).map(std::ptr::from_ref),
            new.map(std::ptr::from_ref)
        );
        if let Some(current) = new {
            assert!(!std::ptr::eq(current, foreign.function_text(text).unwrap()));
        }
    }
    assert!(std::ptr::eq(
        program.function_text("entry.prefix").unwrap(),
        &program.functions[1]
    ));
    let text = "entry.prefix.long";
    let (_, old) = measure(|| {
        for _ in 0..1000 {
            let id = DeclarationId::new(text);
            assert!(std::ptr::eq(
                old_lookup(&program, &id).unwrap(),
                &program.functions[2]
            ));
        }
    });
    assert!(!old.invalid);
    // a3d51efe8 made `DeclarationId::new` move its temporary String into one
    // shared `Arc<str>` backing: two exact requests per saved lookup.
    let backing = crate::hir::DeclarationId::shared_backing_bytes(text.len()).unwrap();
    assert_eq!(old.allocations, 2000);
    assert_eq!(old.reallocations, 0);
    assert_eq!(old.allocated, 1000 * (text.len() + backing));
    assert_eq!(old.freed, old.allocated);
    assert_eq!(old.live, 0);
    let (_, new) = measure(|| {
        for _ in 0..1000 {
            assert!(std::ptr::eq(
                program.function_text(text).unwrap(),
                &program.functions[2]
            ));
        }
    });
    assert_eq!(new, Default::default());
}

#[test]
fn borrowed_renderer_entry_keeps_authenticated_program_and_actual_evaluator_results() {
    let source = crate::kernel_zero::canonical_int_renderer::SOURCE;
    let entry = DeclarationId::new("format.render-byte");
    let binding = BoundTranslation::derive(source, &entry).unwrap();
    let program = binding.replay(source, &entry).unwrap();
    for value in [i64::MIN, -10, -1, 0, 1, 10, i64::MAX] {
        let expected = value.to_string();
        let id = DeclarationId::new("format.render-length");
        let args = [Value::Int(value)];
        let old = eval_program(program, old_lookup(program, &id).unwrap(), &args);
        let new = eval_program(program, program.function_text(id.as_str()).unwrap(), &args);
        assert_eq!(new, old);
        assert_eq!(new, Ok(Value::Int(expected.len() as i64)));
        for (index, byte) in expected.bytes().enumerate() {
            let args = [Value::Int(value), Value::Int(index as i64)];
            let old = eval_program(program, old_lookup(program, &entry).unwrap(), &args);
            let new = eval_program(
                program,
                program.function_text(entry.as_str()).unwrap(),
                &args,
            );
            assert_eq!(new, old);
            assert_eq!(new, Ok(Value::Int(i64::from(byte))));
        }
    }
    // Borrowed discovery does not replace the exact-source renewal proof.
    let changed = source.replace("decimal_digits(value) + if", "decimal_digits(value) - if");
    assert!(binding.replay(&changed, &entry).is_err());
}
