//! Missing OPT-729 capacity and live descendant-loan receiving controls.
use super::*;
use semaprax::hir::{self, PlaceProjection};

const CAPACITY: &str = include_str!("../../fixtures/owned-nested-outcome-capacity.spx");
const LOANS: &str = include_str!("../../fixtures/owned-nested-outcome-loans.spx");

fn qualify(source: &str, label: &str) {
    let root = directory(label);
    let path = root.join("app.spx");
    std::fs::write(&path, source).unwrap();
    let ast = semaprax::check(source, &path).unwrap();
    let canonical = semaprax::format::canonical(&ast);
    let reparsed = semaprax::check(&canonical, &path).unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    let graph = semaprax::graph::to_json(&ast).unwrap();
    semaprax::graph::verify_json(&ast, &graph).unwrap();
    for _ in 0..3 {
        let result =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        interpreter::verify_envelope(&result.envelope).unwrap();
        assert!(result.returned, "{label}: {}", result.envelope);
        let wire: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        assert_eq!(wire["payload"]["outcome"]["value"], "42", "{label}");
    }
    // Preserve the original repeated O0/O2 allocation/authority/Drop probe and
    // strict Wasm host rather than introducing a lighter boundary-only oracle.
    super::super::owned_collection_outcome::run_native(&ast, &root);
    super::super::owned_collection_outcome::run_strict_wasm(&ast, &root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_outcome_empty_and_full_scalar_vec_capacity_preserve_every_element_and_drop() {
    // Scalar Vec's existing exact bound is 8192. Keep owning String/Bytes
    // siblings while filling it, without exceeding String live-owner limits.
    for (capacity, count) in [(0, 0), (8192, 0), (8192, 8192)] {
        let main = format!(
            "@id(\"app.main\") fn main()->i64 {{\nlet value=make({capacity}usize,{count}usize);\nlet before=inspect(value,{capacity}usize,{count}usize);\nlet moved=forward(value);\nlet after=inspect(moved,{capacity}usize,{count}usize);\nlet consumed=finish(moved,{capacity}usize,{count}usize);\nif before==42 && after==42 {{consumed}}else{{0}}\n}}\n"
        );
        let prefix = CAPACITY.split_once("@id(\"app.main\")").unwrap().0;
        qualify(
            &format!("{prefix}{main}"),
            &format!("capacity-{capacity}-{count}"),
        );
    }
}

fn loan_source(vector: bool) -> String {
    if vector {
        assert_eq!(
            LOANS.matches("string_as_str(payload.config.label)").count(),
            1
        );
        LOANS.replace(
            "string_as_str(payload.config.label)",
            "vec_field<Row>(payload.items,0usize,\"label\")",
        )
    } else {
        LOANS.to_owned()
    }
}

#[test]
fn nested_outcome_released_string_and_projected_vec_loans_allow_payload_transfer() {
    for vector in [false, true] {
        let source = loan_source(vector);
        let ast = semaprax::check(&source, "released-nested-outcome.spx").unwrap();
        let program = hir::resolve(&ast).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "loans.read-then-move")
            .unwrap();
        let expected: &[&str] = if vector {
            &["loans.payload.items"]
        } else {
            &["loans.payload.config", "loans.config.label"]
        };
        assert!(
            function.loan_plan.loans.iter().any(|loan| {
                loan.origin.projections.len() == expected.len()
                && loan.origin.projections.iter().zip(expected).all(|(actual, expected)| {
                    matches!(actual, PlaceProjection::Field(id) if id.as_str() == *expected)
                })
            }),
            "the receiving control must carry the actual complete projected loan"
        );
        qualify(
            &source,
            if vector {
                "released-vector-loan"
            } else {
                "released-string-loan"
            },
        );
    }
}

#[test]
fn nested_outcome_live_string_and_projected_vec_loans_refuse_payload_transfer() {
    for vector in [false, true] {
        let released = loan_source(vector);
        // The exact same view/path is admitted once its last use precedes the
        // move. Only moving that use after construction makes the loan live.
        semaprax::check(&released, "released-nested-outcome.spx").unwrap();
        let before = "let length = str_len_bytes(view); let moved = finish(Outcome::Ready { value: payload });";
        let after = "let moved = finish(Outcome::Ready { value: payload }); let length = str_len_bytes(view);";
        assert_eq!(released.matches(before).count(), 1);
        let live = released.replacen(before, after, 1);
        let errors = semaprax::check(&live, "live-nested-outcome.spx").unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "SPX-T265"),
            "vector={vector}: {errors:?}"
        );
    }
}
