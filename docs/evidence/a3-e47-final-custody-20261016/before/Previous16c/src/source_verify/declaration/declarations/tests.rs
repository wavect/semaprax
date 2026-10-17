//! Regression coverage for the SPX-T268 `check_byte_data_declarations`
//! widening: a direct owned `string` case field (Owned String Variant v1)
//! and a drop-free Copy-closed nested-record case field (Copy Aggregate
//! Variant Payload v1, `is_admitted_copy_aggregate_variant_field`) may now
//! be declared as sibling case fields of the same variant. Before this
//! widening, `check_byte_data_declarations`'s `has_direct_string` branch
//! consulted only `owned_byte_record_copy_field_is_admitted` (direct Copy
//! scalars), so a variant combining the two -- each independently admitted
//! on its own -- was refused outright.

use std::path::Path;

fn codes(source: &str) -> Vec<&'static str> {
    let program = crate::parse(source, Path::new("declarations-t268-sibling.spx")).unwrap();
    crate::verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn owned_string_variant_admits_a_drop_free_copy_aggregate_sibling_case_field() {
    let source = r#"
module test.owned_string_variant_copy_aggregate_sibling;
@id("test.inner")
record Inner {
    @id("test.inner.x")
    x: i64,
}
@id("test.payload")
variant Payload {
    @id("test.payload.text")
    Text {
        @id("test.payload.text.value")
        value: string,
    },
    @id("test.payload.wrapped")
    Wrapped {
        @id("test.payload.wrapped.value")
        value: Inner,
    },
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert_eq!(codes(source), Vec::<&str>::new());
}

#[test]
fn owned_string_variant_still_refuses_a_sibling_field_that_itself_needs_drop() {
    // `Inner` here reaches an owned `Bytes` leaf, so it stays outside Copy
    // Aggregate Variant Payload v1 (which requires the referenced record to
    // need no drop at all) -- the widening above must not also admit this
    // shape. SPX-T268 must still fire, exactly as it does for this same
    // nested-owned-leaf shape without a co-declared `string` field
    // (`copy_aggregate_variant_payload_rejects_a_nested_owned_leaf` in
    // `tests/language/variants_semantics.rs`).
    let source = r#"
module test.owned_string_variant_owned_leaf_sibling;
@id("test.inner")
record Inner {
    @id("test.inner.data")
    data: Bytes,
}
@id("test.payload")
variant Payload {
    @id("test.payload.text")
    Text {
        @id("test.payload.text.value")
        value: string,
    },
    @id("test.payload.wrapped")
    Wrapped {
        @id("test.payload.wrapped.value")
        value: Inner,
    },
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let got = codes(source);
    assert!(got.contains(&"SPX-T268"), "{got:?}");
}

#[test]
fn owned_string_variant_still_refuses_a_class_sibling_case_field() {
    // Class Inheritance v1 is excluded from Copy Aggregate Variant Payload
    // v1 (upcast slicing has no meaning inside a tagged union), so a class
    // sibling must still be refused even alongside a direct `string` case
    // field.
    let source = r#"
module test.owned_string_variant_class_sibling;
@id("test.inner")
class Inner {
    @id("test.inner.x")
    x: i64,
}
@id("test.payload")
variant Payload {
    @id("test.payload.text")
    Text {
        @id("test.payload.text.value")
        value: string,
    },
    @id("test.payload.wrapped")
    Wrapped {
        @id("test.payload.wrapped.value")
        value: Inner,
    },
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let got = codes(source);
    assert!(got.contains(&"SPX-T268"), "{got:?}");
}

/// Cross-classifier agreement test for issue #261: adding Copy Aggregate
/// Variant Payload v1 to the owned-string variant profile required the same
/// disjunct in five places, at two abstraction levels -- `Type` before
/// resolution (sites 1 and 2) and `ResolvedType` after resolution (sites 3
/// through 5). This drives one admitted profile and one profile that is not
/// admitted through the whole pipeline: `crate::verify::verify` (sites 1 and
/// 2, `check_byte_data_declarations` and `TypeTable::
/// is_flat_owned_string_variant`), `crate::hir::resolve` (site 3,
/// `is_admitted_owned_string_variant` via `resolver_admits_owned_variant`),
/// `crate::hir::validate` (site 4, `hir/validation.rs`'s variant profile
/// gate), and `crate::interpreter::evaluate_resolved_zero_arg_i64` (site 5,
/// `Interpreter::value_has_type`, which the function validates again
/// internally before executing). A regression at any single site turns into
/// one failure here instead of five unrelated bug reports at five different
/// layers; each site was reverted by hand to confirm this (see the issue
/// report for the exact commands and output).
#[test]
fn all_five_owned_string_variant_classifiers_agree_on_the_copy_aggregate_profile() {
    fn compile_and_run(source: &str) -> Result<i64, String> {
        let program = crate::parse(source, Path::new("t261-classifier-agreement.spx"))
            .map_err(|error| format!("parse: {error:?}"))?;
        let errors: Vec<_> = crate::verify::verify(&program)
            .into_iter()
            .filter(|diagnostic| diagnostic.severity.is_error())
            .collect();
        if !errors.is_empty() {
            return Err(format!("verify (sites 1+2): {errors:?}"));
        }
        let resolved = crate::hir::resolve(&program)
            .map_err(|errors| format!("resolve (site 3): {errors:?}"))?;
        crate::hir::validate(&resolved).map_err(|error| format!("validate (site 4): {error:?}"))?;
        let evaluation =
            crate::interpreter::evaluate_resolved_zero_arg_i64(&resolved, "app.main", 1_000)
                .map_err(|errors| format!("evaluate (site 5): {errors:?}"))?;
        match evaluation.outcome {
            crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(value) => Ok(value),
            other => Err(format!("non-return outcome: {other:?}")),
        }
    }

    let admitted = r#"
module test.t261_admitted;
@id("test.t261.inner")
record Inner {
    @id("test.t261.inner.x")
    x: i64,
}
@id("test.t261.payload")
variant Payload {
    @id("test.t261.payload.text")
    Text {
        @id("test.t261.payload.text.value")
        value: string,
    },
    @id("test.t261.payload.wrapped")
    Wrapped {
        @id("test.t261.payload.wrapped.value")
        value: Inner,
    },
}
@id("test.t261.relay")
fn relay(payload: own Payload) -> i64 {
    match own payload {
        Payload::Text { value } => 1,
        Payload::Wrapped { value } => 2,
    }
}
@id("app.main")
fn main() -> i64 {
    relay(Payload::Wrapped { value: Inner { x: 7 } })
}
"#;
    assert_eq!(
        compile_and_run(admitted),
        Ok(2),
        "an owned-string variant with a drop-free Copy-aggregate sibling must be admitted \
         end to end by all five classifiers"
    );

    // Not admitted: `Inner` here is a `class`, and Class Inheritance v1 is
    // deliberately excluded from Copy Aggregate Variant Payload v1 (upcast
    // slicing has no meaning inside a tagged union). It reaches no owned
    // `Bytes`, so only the copy-aggregate disjunct -- not the separate
    // owned-Bytes-nesting check -- decides this case; every site that can
    // see the shape before execution must refuse it, so this program never
    // reaches resolution.
    let not_admitted = r#"
module test.t261_not_admitted;
@id("test.t261.inner2")
class Inner {
    @id("test.t261.inner2.x")
    x: i64,
}
@id("test.t261.payload2")
variant Payload {
    @id("test.t261.payload2.text")
    Text {
        @id("test.t261.payload2.text.value")
        value: string,
    },
    @id("test.t261.payload2.wrapped")
    Wrapped {
        @id("test.t261.payload2.wrapped.value")
        value: Inner,
    },
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    match compile_and_run(not_admitted) {
        Err(message) => assert!(
            message.contains("SPX-T268"),
            "expected the shared declaration gate to refuse a class sibling: {message}"
        ),
        Ok(value) => panic!("class sibling must not be admitted, got {value}"),
    }
}
