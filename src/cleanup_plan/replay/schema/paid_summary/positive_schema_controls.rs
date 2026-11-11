//! Positive selection controls for the exact ordinary schema predecessor.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;

fn literal(file: &'static str, name: &str) -> &'static str {
    let declaration = format!("const {name}: &str = r#\"");
    file.split_once(declaration.as_str())
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0
}

const BUFFER_TEXT_VECTOR: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/language/owned_byte_buffer_v1/cleanup_renewal.rs"
));
const ITERATORS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/workspace/canonical_revision/owned_iterators.rs"
));
const RECORD_RENEWAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/language/while_loops/record_borrow_renewal.spx"
));
const RECORD_UPDATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/cleanup_plan/replay/record_destructure/update/paid.rs"
));
const LOOP: &str = r#"module schema.iterator_loop;
@id("main") fn main()->i64 {
 let input=vec_push<i64>(vec_with_capacity<i64>(1usize),1);
 let mut total=0;
 for own item in vec_into_iter<i64>(input) {total=total+item;0}
 total
}
"#;
const RENEWAL: &str = r#"module schema.iterator_renewal;
@id("main") fn main()->i64 {
 let input=vec_push<i64>(vec_with_capacity<i64>(1usize),1);
 let mut output=vec_with_capacity<i64>(1usize);
 for own item in vec_into_iter<i64>(input) { if item>0 {output=vec_push<i64>(output,item);0}else{0} }
 if vec_len<i64>(output)==1usize {1}else{0}
}
"#;

#[test]
fn all_nine_ordinary_selectors_reach_their_positive_schema_with_the_held_payer() {
    let buffer: &[(&str, &str)] = &[
        ("legacy.vec", CLEANUP_PLAN_SCHEMA_V15),
        ("legacy.string", CLEANUP_PLAN_SCHEMA_V16),
        ("buffer.main", CLEANUP_PLAN_SCHEMA_V17),
    ];
    let subjects: [(&str, &[(&str, &str)]); 7] = [
        (literal(BUFFER_TEXT_VECTOR, "COMPOSED"), buffer),
        (
            literal(ITERATORS, "SOURCE"),
            &[("fixture.main", CLEANUP_PLAN_SCHEMA_V13)],
        ),
        (
            literal(ITERATORS, "RECORD_SOURCE"),
            &[("fixture.main", CLEANUP_PLAN_SCHEMA_V13)],
        ),
        (RECORD_RENEWAL, &[("matcher.run", CLEANUP_PLAN_SCHEMA_V12)]),
        (
            literal(RECORD_UPDATE, "SOURCE"),
            &[("update.copy", CLEANUP_PLAN_SCHEMA_V9)],
        ),
        (LOOP, &[("main", CLEANUP_PLAN_SCHEMA_V11)]),
        (RENEWAL, &[("main", CLEANUP_PLAN_SCHEMA_V12)]),
    ];
    for (source, expected) in subjects {
        let parsed = crate::check(source, "positive-ordinary-schema.spx").unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        for (id, schema) in expected {
            let function = program
                .functions
                .iter()
                .find(|f| f.id.as_str() == *id)
                .unwrap();
            assert_eq!(function.cleanup_plan.schema, *schema);
            let mut forged = function.clone();
            forged.cleanup_plan.schema = "untrusted.attached.schema";
            for subject in [function, &forged] {
                let old = with_limit_usage_refusal(usize::MAX, || {
                    let mut budget = ReplayBudget::new();
                    assert!(budget.pays_builder());
                    before_selectors::selected_with_budget(&program, subject, None, &mut budget)
                });
                let new = with_limit_usage_refusal(usize::MAX, || {
                    let mut budget = ReplayBudget::new();
                    assert!(budget.pays_builder());
                    selected_with_budget(&program, subject, None, &mut budget)
                });
                assert_eq!(old.0, Ok(*schema), "{id}");
                assert_eq!(new.0, old.0, "{id}");
                assert!(!old.1 && old.3.is_none() && !new.1 && new.3.is_none());
            }
        }
    }
}
