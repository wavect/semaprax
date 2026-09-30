//! Kernel selection and admission refusals reached from real checked source.
//! Each fixture differs from an admitted kernel in exactly the construct
//! named by the test.

use crate::compute_profile::classifier::{
    EFFECT_OUTSIDE_CLOSED_VOCABULARY, FLOATING_POINT_NOT_ADMITTED_IN_V1, KERNEL_BODY_NOT_PURE,
    PARAMETER_COUNT_OUT_OF_BOUNDS, TYPE_OUTSIDE_KERNEL_VOCABULARY,
};

use super::*;

const MAP: KernelShape = KernelShape::ElementwiseMap { workgroup_size: 64 };

fn module(declarations: &str) -> String {
    format!(
        "module test.compute_admission;\n\n{declarations}\n\n\
         @id(\"app.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
    )
}

fn load(declarations: &str, shape: KernelShape) -> Result<KernelArtifact, ComputeRefusal> {
    let program = resolve(&module(declarations));
    session().load_kernel(&program, "k.subject", shape)
}

fn refusal_code(declarations: &str, shape: KernelShape) -> &'static str {
    load(declarations, shape)
        .expect_err("the fixture must be refused")
        .code()
}

#[test]
fn baseline_kernel_is_admitted_and_bound_to_its_declaration() {
    let artifact = load(
        "@id(\"k.subject\")\nfn subject(x: i64) -> i64\n{\n    x + 1\n}",
        MAP,
    )
    .unwrap();
    assert_eq!(artifact.declaration(), "k.subject");
    assert_eq!(artifact.shape(), MAP);
}

#[test]
fn absent_automatic_or_mis_shaped_declarations_refuse_selection() {
    let program = resolve(KERNELS);
    assert_eq!(
        session()
            .load_kernel(&program, "k.missing", MAP)
            .unwrap_err()
            .code(),
        KERNEL_SELECTION_REFUSED
    );
    // No explicit @id: an automatic identity is not a persistent binding.
    let automatic = resolve(&module("fn subject(x: i64) -> i64\n{\n    x\n}"));
    let automatic_id = automatic
        .functions
        .iter()
        .find(|function| function.name == "subject")
        .unwrap()
        .id
        .as_str()
        .to_owned();
    assert_eq!(
        session()
            .load_kernel(&automatic, &automatic_id, MAP)
            .unwrap_err()
            .code(),
        KERNEL_SELECTION_REFUSED
    );
    // A fold needs `fn(acc: T, element: U) -> T`.
    for declarations in [
        "@id(\"k.subject\")\nfn subject(acc: i64, x: i64) -> bool\n{\n    acc < x\n}",
        "@id(\"k.subject\")\nfn subject(x: i64) -> i64\n{\n    x\n}",
    ] {
        assert_eq!(
            refusal_code(declarations, KernelShape::SequentialFold),
            KERNEL_SELECTION_REFUSED
        );
    }
}

#[test]
fn declared_host_effects_refuse_with_gc001() {
    let source = module(
        "@id(\"k.subject\")\nfn subject(x: i64) -> i64\n    uses { clock.read }\n{\n    x\n}",
    )
    .replace(
        "module test.compute_admission;",
        "module test.compute_admission;\n\npermit { clock.read }",
    );
    let refusal = session()
        .load_kernel(&resolve(&source), "k.subject", MAP)
        .unwrap_err();
    assert_eq!(refusal.code(), EFFECT_OUTSIDE_CLOSED_VOCABULARY);
    assert!(refusal.diagnostic().message.contains("clock.read"));
}

#[test]
fn parameter_count_bound_counts_the_output_buffer() {
    let params = |count: usize| {
        (0..count)
            .map(|index| format!("p{index}: i64"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let admitted = format!(
        "@id(\"k.subject\")\nfn subject({}) -> i64\n{{\n    p0\n}}",
        params(7)
    );
    load(&admitted, MAP).expect("seven inputs plus the output is the bound");
    let refused = format!(
        "@id(\"k.subject\")\nfn subject({}) -> i64\n{{\n    p0\n}}",
        params(8)
    );
    assert_eq!(refusal_code(&refused, MAP), PARAMETER_COUNT_OUT_OF_BOUNDS);
}

#[test]
fn types_outside_the_kernel_vocabulary_refuse_with_gc004() {
    for declarations in [
        "@id(\"k.subject\")\nfn subject(x: f64) -> i64\n{\n    0\n}",
        "@id(\"k.subject\")\nfn subject(x: char) -> i64\n{\n    0\n}",
        "@id(\"k.subject\")\nfn subject(x: i64) -> f32\n{\n    1.5f32\n}",
    ] {
        assert_eq!(
            refusal_code(declarations, MAP),
            TYPE_OUTSIDE_KERNEL_VOCABULARY,
            "{declarations}"
        );
    }
}

#[test]
fn floating_point_body_operations_refuse_with_gc010() {
    let declarations =
        "@id(\"k.subject\")\nfn subject(x: i64) -> i64\n{\n    let half = 0.5;\n    \
                        if half < 1.0 { x } else { 0 }\n}";
    let refusal = load(declarations, MAP).unwrap_err();
    assert_eq!(refusal.code(), FLOATING_POINT_NOT_ADMITTED_IN_V1);
    assert!(refusal.diagnostic().message.contains("floating point"));
}

#[test]
fn constructs_outside_the_kernel_body_vocabulary_refuse_with_gc012() {
    for declarations in [
        // A call out of the kernel body.
        "@id(\"k.helper\")\nfn helper(x: i64) -> i64\n{\n    x\n}\n\n\
         @id(\"k.subject\")\nfn subject(x: i64) -> i64\n{\n    helper(x)\n}",
        // Mutation.
        "@id(\"k.subject\")\nfn subject(x: i64) -> i64\n{\n    let mut y = x;\n    y = y + 1;\n    y\n}",
        // A loop.
        "@id(\"k.subject\")\nfn subject(x: i64) -> i64\n{\n    let mut y = x;\n    \
         while y > 0 {\n        y = y - 1;\n        y > 0\n    }\n    y\n}",
        // A match.
        "@id(\"k.subject\")\nfn subject(x: i64) -> i64\n{\n    match x { 0 => 1, _ => x, }\n}",
        // A contract.
        "@id(\"k.subject\")\nfn subject(x: i64) -> i64\n    requires x > 0\n{\n    x\n}",
    ] {
        assert_eq!(
            refusal_code(declarations, MAP),
            KERNEL_BODY_NOT_PURE,
            "{declarations}"
        );
    }
}

#[test]
fn kernel_body_depth_bound_refuses_with_gc020() {
    let chain = |depth: usize| {
        format!(
            "@id(\"k.subject\")\nfn subject(x: i64) -> i64\n{{\n    x{}\n}}",
            " + 1".repeat(depth)
        )
    };
    load(&chain(MAX_KERNEL_IR_DEPTH - 2), MAP).expect("within the depth bound");
    assert_eq!(
        refusal_code(&chain(MAX_KERNEL_IR_DEPTH + 1), MAP),
        KERNEL_BODY_BOUND_EXCEEDED
    );
}

#[test]
fn every_new_code_is_a_three_digit_gc_code_with_a_rendered_diagnostic() {
    let refusals = [
        ComputeRefusal::KernelSelection {
            detail: String::new(),
        },
        ComputeRefusal::StaleHandle {
            detail: String::new(),
        },
        ComputeRefusal::BufferReleased { buffer: 0 },
        ComputeRefusal::OutOfBounds {
            detail: String::new(),
        },
        ComputeRefusal::ElementTypeMismatch {
            detail: String::new(),
        },
        ComputeRefusal::FailureAlreadySelected {
            failure: SessionFailure::Cancelled {
                completed_invocations: 0,
            },
        },
        ComputeRefusal::KernelBodyBoundExceeded {
            detail: String::new(),
        },
        ComputeRefusal::EffectNotGranted {
            effect: crate::compute_profile::classifier::DeviceEffect::DeviceCopyOut,
        },
    ];
    let codes: Vec<&str> = refusals.iter().map(ComputeRefusal::code).collect();
    assert_eq!(
        codes,
        [
            "SPX-GC014",
            "SPX-GC015",
            "SPX-GC016",
            "SPX-GC017",
            "SPX-GC018",
            "SPX-GC019",
            "SPX-GC020",
            "SPX-GC021"
        ]
    );
    for refusal in &refusals {
        let diagnostic = refusal.diagnostic();
        assert_eq!(diagnostic.code, refusal.code());
        assert!(diagnostic.message.contains(CPU_REFERENCE_SCHEMA));
    }
}
