//! CPU reference vs the ordinary reference interpreter on the same checked
//! kernel body, plus the negative-control mutant.

use std::path::Path;

use crate::ast::BinaryOp;
use crate::interpreter::{self, ArgumentValue, InterpreterOptions};

use super::super::kernel_ir::KernelExpr;
use super::super::session::{run_map, RunOutcome};
use super::*;

/// One scalar result from the interpreter: a value, or the stable
/// `semaprax.status.v1` code of the checked failure it selected.
type Expected = Result<Scalar, u32>;

fn render(value: Scalar) -> String {
    match value {
        Scalar::I64(value) => value.to_string(),
        Scalar::I32(value) => format!("{value}i32"),
        Scalar::U8(value) => format!("{value}u8"),
        Scalar::Usize(value) => format!("{value}usize"),
        Scalar::Bool(value) => value.to_string(),
    }
}

fn interpret(path: &Path, declaration: &str, arguments: &[Scalar]) -> Expected {
    let arguments: Vec<String> = arguments.iter().copied().map(render).collect();
    let interpretation = interpreter::interpret(
        path,
        declaration,
        &arguments,
        &InterpreterOptions::default(),
    )
    .expect("the interpreter admits every differential kernel");
    let envelope: serde_json::Value =
        serde_json::from_str(&interpretation.envelope).expect("envelope is JSON");
    let outcome = &envelope["payload"]["outcome"];
    match outcome["kind"].as_str() {
        Some("returned") => {
            let text = outcome["value"].as_str().expect("returned value text");
            Ok(
                match interpreter::parse_argument(text).expect("canonical value") {
                    ArgumentValue::Int(value) => Scalar::I64(value),
                    ArgumentValue::Int32(value) => Scalar::I32(value),
                    ArgumentValue::Uint8(value) => Scalar::U8(value),
                    ArgumentValue::Usize(value) => Scalar::Usize(value),
                    ArgumentValue::Bool(value) => Scalar::Bool(value),
                    other => panic!("non-kernel interpreter result {other:?}"),
                },
            )
        }
        Some("failed") => Err(outcome["status"]["code"].as_u64().expect("status code") as u32),
        other => panic!("unexpected interpreter outcome {other:?}"),
    }
}

/// Compare one CPU-reference run against the interpreter's per-invocation
/// results. A completed run must equal every value; a failed run must name
/// the lowest failing ordinal and the same status code.
fn compare(cpu: &RunOutcome, expected: &[Expected]) -> Result<(), String> {
    let first_failure = expected.iter().position(Result::is_err);
    match (cpu, first_failure) {
        (RunOutcome::Completed(values), None) => {
            let expected: Vec<Scalar> = expected.iter().map(|value| value.unwrap()).collect();
            if *values == expected {
                Ok(())
            } else {
                Err(format!(
                    "values differ: cpu {values:?}, interpreter {expected:?}"
                ))
            }
        }
        (RunOutcome::Status { invocation, status }, Some(index)) => {
            let code = expected[index].unwrap_err();
            if *invocation == index && status.code() == code {
                Ok(())
            } else {
                Err(format!(
                    "failure differs: cpu {status:?} at {invocation}, interpreter {code} at {index}"
                ))
            }
        }
        (cpu, first_failure) => Err(format!(
            "outcome differs: cpu {cpu:?}, interpreter first failure {first_failure:?}"
        )),
    }
}

/// Run `declaration` as a map through a real session, returning the
/// normalized CPU outcome.
fn cpu_map(
    program: &ResolvedProgram,
    declaration: &str,
    result: ScalarKind,
    columns: &[Vec<Scalar>],
) -> RunOutcome {
    let mut session = session();
    let artifact = session
        .load_kernel(
            program,
            declaration,
            KernelShape::ElementwiseMap { workgroup_size: 4 },
        )
        .expect("kernel admitted");
    let len = columns[0].len();
    let inputs: Vec<BufferHandle> = columns
        .iter()
        .map(|column| {
            let handle = session.alloc(column[0].kind(), len).unwrap();
            session.upload(handle, 0, column).unwrap();
            handle
        })
        .collect();
    let output = session.alloc(result, len).unwrap();
    let outcome = session
        .dispatch_map(
            program,
            &artifact,
            &inputs,
            output,
            DispatchControl::default(),
        )
        .expect("dispatch admitted");
    let normalized = match outcome {
        DispatchOutcome::Completed { invocations } => {
            assert_eq!(invocations, len);
            RunOutcome::Completed(session.download(output, 0, len).unwrap())
        }
        DispatchOutcome::Failed(SessionFailure::KernelStatus {
            invocation, status, ..
        }) => RunOutcome::Status { invocation, status },
        DispatchOutcome::Failed(other) => panic!("uninjected failure {other:?}"),
    };
    let settlement = session.settle();
    assert_eq!(settlement.releases.len(), columns.len() + 1);
    normalized
}

fn expected_map(path: &Path, declaration: &str, columns: &[Vec<Scalar>]) -> Vec<Expected> {
    (0..columns[0].len())
        .map(|index| {
            let arguments: Vec<Scalar> = columns.iter().map(|column| column[index]).collect();
            interpret(path, declaration, &arguments)
        })
        .collect()
}

fn differential_map(declaration: &str, result: ScalarKind, columns: Vec<Vec<Scalar>>) {
    let program = resolve(KERNELS);
    let source = write_source(KERNELS);
    let expected = expected_map(source.path(), declaration, &columns);
    let cpu = cpu_map(&program, declaration, result, &columns);
    compare(&cpu, &expected).unwrap();
}

#[test]
fn i64_map_with_let_and_if_matches_the_interpreter() {
    differential_map(
        "k.affine",
        ScalarKind::I64,
        vec![
            i64s(&[0, 1, -7, 40, 1_000_000, -3, 12, 5, 9]),
            i64s(&[0, 5, -30, 100, 7, -9, 36, 15, -1]),
        ],
    );
}

#[test]
fn i64_map_overflow_selects_the_same_lowest_failing_invocation() {
    // Invocations 3 and 6 both overflow `x * 3`; the lowest ordinal wins.
    differential_map(
        "k.affine",
        ScalarKind::I64,
        vec![
            i64s(&[1, 2, 3, i64::MAX, 5, 6, i64::MIN, 8]),
            i64s(&[0, 0, 0, 0, 0, 0, 0, 0]),
        ],
    );
}

#[test]
fn i64_division_by_zero_and_min_over_minus_one_match_the_interpreter() {
    let columns = |y: i64| vec![i64s(&[10, -9, i64::MIN, 4]), i64s(&[3, 2, y, 0])];
    differential_map("k.ratio", ScalarKind::I64, columns(-1));
    differential_map("k.ratio", ScalarKind::I64, columns(1));
}

#[test]
fn i64_remainder_normal_by_zero_and_min_over_minus_one_match_the_interpreter() {
    differential_map(
        "k.rem",
        ScalarKind::I64,
        vec![i64s(&[10, -9, 7, 0, -1]), i64s(&[3, 2, -3, 5, 1])],
    );
    // `MIN % -1` at invocation 1 and `5 % 0` at invocation 2: the lowest
    // failing ordinal wins with `RemainderOverflow`.
    differential_map(
        "k.rem",
        ScalarKind::I64,
        vec![i64s(&[4, i64::MIN, 5]), i64s(&[2, -1, 0])],
    );
    // Remainder by zero alone selects `RemainderByZero` at invocation 0.
    differential_map("k.rem", ScalarKind::I64, vec![i64s(&[1]), i64s(&[0])]);
}

#[test]
fn usize_remainder_normal_and_by_zero_match_the_interpreter() {
    differential_map(
        "k.rem_usize",
        ScalarKind::Usize,
        vec![
            [10u64, 7, 0, u64::MAX].map(Scalar::Usize).to_vec(),
            [3u64, 7, 5, 6].map(Scalar::Usize).to_vec(),
        ],
    );
    differential_map(
        "k.rem_usize",
        ScalarKind::Usize,
        vec![
            [9u64, 1].map(Scalar::Usize).to_vec(),
            [4u64, 0].map(Scalar::Usize).to_vec(),
        ],
    );
}

#[test]
fn unary_negation_normal_and_overflow_match_the_interpreter() {
    differential_map("k.neg", ScalarKind::I64, vec![i64s(&[0, 1, -7, 42])]);
    differential_map("k.neg", ScalarKind::I64, vec![i64s(&[3, i64::MIN, 5])]);
    differential_map(
        "k.neg32",
        ScalarKind::I32,
        vec![[0, 9, -12].map(Scalar::I32).to_vec()],
    );
    differential_map(
        "k.neg32",
        ScalarKind::I32,
        vec![vec![Scalar::I32(1), Scalar::I32(i32::MIN)]],
    );
}

#[test]
fn i32_u8_and_usize_maps_match_the_interpreter() {
    differential_map(
        "k.halve32",
        ScalarKind::I32,
        vec![[0, 7, -8, i32::MAX, -i32::MAX, 3].map(Scalar::I32).to_vec()],
    );
    differential_map(
        "k.halve32",
        ScalarKind::I32,
        vec![vec![Scalar::I32(4), Scalar::I32(i32::MIN), Scalar::I32(1)]],
    );
    differential_map(
        "k.byte_mix",
        ScalarKind::U8,
        vec![
            [0u8, 3, 100, 127].map(Scalar::U8).to_vec(),
            [6u8, 13, 255, 0].map(Scalar::U8).to_vec(),
        ],
    );
    differential_map(
        "k.byte_mix",
        ScalarKind::U8,
        vec![
            [1u8, 128, 2].map(Scalar::U8).to_vec(),
            [0u8, 0, 0].map(Scalar::U8).to_vec(),
        ],
    );
    differential_map(
        "k.index_scale",
        ScalarKind::Usize,
        vec![[1u64, 2, 1 << 40, 0, 5].map(Scalar::Usize).to_vec()],
    );
}

#[test]
fn mixed_bool_map_with_lazy_operators_matches_the_interpreter() {
    differential_map(
        "k.flag",
        ScalarKind::Bool,
        vec![
            i64s(&[0, 1, 0, -4, 9]),
            [true, true, false, false, true].map(Scalar::Bool).to_vec(),
        ],
    );
}

fn differential_fold(declaration: &str, initial: Scalar, elements: Vec<Scalar>) {
    let program = resolve(KERNELS);
    let source = write_source(KERNELS);
    let mut accumulator = initial;
    let mut expected = Ok(());
    for (index, element) in elements.iter().enumerate() {
        match interpret(source.path(), declaration, &[accumulator, *element]) {
            Ok(value) => accumulator = value,
            Err(code) => {
                expected = Err((index, code));
                break;
            }
        }
    }

    let mut session = session();
    let artifact = session
        .load_kernel(&program, declaration, KernelShape::SequentialFold)
        .unwrap();
    let input = session.alloc(elements[0].kind(), elements.len()).unwrap();
    session.upload(input, 0, &elements).unwrap();
    let output = session.alloc(initial.kind(), 1).unwrap();
    let outcome = session
        .dispatch_fold(
            &program,
            &artifact,
            initial,
            input,
            output,
            DispatchControl::default(),
        )
        .unwrap();
    match (outcome, expected) {
        (DispatchOutcome::Completed { invocations }, Ok(())) => {
            assert_eq!(invocations, elements.len());
            assert_eq!(session.download(output, 0, 1).unwrap(), vec![accumulator]);
        }
        (
            DispatchOutcome::Failed(SessionFailure::KernelStatus {
                invocation, status, ..
            }),
            Err((index, code)),
        ) => {
            assert_eq!(invocation, index);
            assert_eq!(status.code(), code);
        }
        (outcome, expected) => panic!("fold differs: cpu {outcome:?}, interpreter {expected:?}"),
    }
}

#[test]
fn sequential_folds_match_left_to_right_interpreter_folds() {
    differential_fold("k.sum", Scalar::I64(5), i64s(&[1, -2, 30, 400, -5000]));
    differential_fold("k.sum", Scalar::I64(i64::MAX - 10), i64s(&[4, 6, 1, -100]));
    differential_fold(
        "k.count_small",
        Scalar::Usize(0),
        [1u8, 200, 9, 10, 0, 255].map(Scalar::U8).to_vec(),
    );
}

fn swap_first_add_for_sub(expr: &mut KernelExpr) -> bool {
    match expr {
        KernelExpr::Binary { op, left, right } => {
            if *op == BinaryOp::Add {
                *op = BinaryOp::Sub;
                return true;
            }
            swap_first_add_for_sub(left) || swap_first_add_for_sub(right)
        }
        KernelExpr::Neg(inner) | KernelExpr::Not(inner) => swap_first_add_for_sub(inner),
        KernelExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            swap_first_add_for_sub(condition)
                || swap_first_add_for_sub(then_branch)
                || swap_first_add_for_sub(else_branch)
        }
        KernelExpr::Block { lets, tail } => {
            lets.iter_mut()
                .any(|(_, value)| swap_first_add_for_sub(value))
                || swap_first_add_for_sub(tail)
        }
        KernelExpr::Slot(_) | KernelExpr::Literal(_) => false,
    }
}

/// Negative control: the same comparison that accepts every run above must
/// reject a CPU reference whose lowered body has one operator mutated. If
/// this passes while the mutant agrees, the differential oracle is vacuous.
#[test]
fn negative_control_mutated_kernel_is_rejected_by_the_differential_oracle() {
    let program = resolve(KERNELS);
    let source = write_source(KERNELS);
    let columns = vec![i64s(&[0, 1, -7, 40]), i64s(&[0, 5, -30, 100])];
    let expected = expected_map(source.path(), "k.affine", &columns);

    let mut session = session();
    let mut artifact = session
        .load_kernel(
            &program,
            "k.affine",
            KernelShape::ElementwiseMap { workgroup_size: 4 },
        )
        .unwrap();
    let slices: Vec<&[Scalar]> = columns.iter().map(Vec::as_slice).collect();
    let faithful = run_map(artifact.ir(), &slices, 4, DispatchControl::default());
    compare(&faithful, &expected).expect("the faithful lowering agrees");

    assert!(swap_first_add_for_sub(&mut artifact.ir_mut().body));
    let mutant = run_map(artifact.ir(), &slices, 4, DispatchControl::default());
    let rejection = compare(&mutant, &expected).expect_err("the oracle must reject the mutant");
    assert!(rejection.contains("values differ"), "{rejection}");

    // The tampered artifact is also refused by the session itself.
    let input = session.alloc(ScalarKind::I64, 4).unwrap();
    let other = session.alloc(ScalarKind::I64, 4).unwrap();
    let output = session.alloc(ScalarKind::I64, 4).unwrap();
    let refusal = session
        .dispatch_map(
            &program,
            &artifact,
            &[input, other],
            output,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), STALE_HANDLE);
}
