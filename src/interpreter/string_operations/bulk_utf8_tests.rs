use super::*;

fn evaluate(body: &str, budget: Utf8MaterializationBudget) -> (Result<Value, Flow>, (u64, u64)) {
    let source=format!("module test.bulk;@id(\"copy\") fn copy(input:borrow Slice<u8>)->string{{string_from_utf8(input)}}@id(\"app.main\") fn main()->i64{{{body}}}");
    let program = hir::resolve(&crate::check(&source, "bulk.spx").unwrap()).unwrap();
    let entry = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let admitted = program
        .functions
        .iter()
        .map(|f| (f.id.as_str(), f))
        .collect();
    let (result, _, _, usage) = evaluate_resolved_entry_with_utf8_budget(
        entry,
        &[],
        &admitted,
        &program,
        100_000,
        false,
        budget,
    );
    (result, usage)
}

#[test]
fn bulk_utf8_one_materialization_checks_slice_extent_and_both_fixed_caps() {
    let body="let raw=[255u8,65u8,0u8,195u8,169u8,255u8];let text=copy(byte_range(array_as_slice(raw),1usize,5usize));str_len_bytes(string_as_str(text))";
    for (remaining_count, remaining_bytes, accepted) in [(1, 4, true), (0, 4, false), (1, 3, false)]
    {
        let initial = (
            MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS - remaining_count,
            MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES - remaining_bytes,
        );
        let (result, usage) = evaluate(
            body,
            Utf8MaterializationBudget::Fixed {
                used_materializations: initial.0,
                used_bytes: initial.1,
            },
        );
        if accepted {
            assert!(matches!(result, Ok(Value::Int(4))), "{result:?}");
            assert_eq!(usage, (initial.0 + 1, initial.1 + 4));
        } else {
            assert!(
                matches!(result, Err(Flow::Utf8MaterializationLimitExceeded { .. })),
                "{result:?}"
            );
            assert_eq!(usage, initial);
        }
    }
}

#[test]
fn bulk_utf8_rejection_precedes_materialization_and_helpers_share_the_meter() {
    let initial = Utf8MaterializationBudget::Fixed {
        used_materializations: MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS,
        used_bytes: MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES,
    };
    let (result, usage) = evaluate(
        "let raw=[237u8,160u8,128u8];let text=copy(array_as_slice(raw));0",
        initial,
    );
    assert!(
        matches!(result, Err(Flow::Failure(_))),
        "malformed UTF-8 is a checked conversion before capacity: {result:?}"
    );
    assert_eq!(usage, initial.usage());
    let (result, usage) = evaluate(
        "let raw=[65u8];let first=copy(array_as_slice(raw));let second=copy(array_as_slice(raw));0",
        Utf8MaterializationBudget::Fixed {
            used_materializations: MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS - 1,
            used_bytes: 0,
        },
    );
    assert!(matches!(
        result,
        Err(Flow::Utf8MaterializationLimitExceeded {
            attempted_materializations: 4097,
            ..
        })
    ));
    assert_eq!(usage, (MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS, 1));
    let (result,usage)=evaluate("let raw=[0u8];let empty=copy(byte_range(array_as_slice(raw),0usize,0usize));str_len_bytes(string_as_str(empty))",Utf8MaterializationBudget::fixed());
    assert!(matches!(result, Ok(Value::Int(0))));
    assert_eq!(usage, (1, 0));
}
