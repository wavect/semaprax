//! Real-hardware differential tests: the CPU reference's own interpreter
//! oracle vs a real Metal dispatch, byte-comparing outputs and selected
//! checked-failure status. Every test skips, with an explicit printed
//! reason, on any host with no real Metal device (`MTLCreateSystemDefaultDevice`
//! returns `None`) — that is a skip, never evidence about that host.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::ast::BinaryOp;
use crate::compute_profile::classifier::{ALIASING_BEYOND_CHECKED_RULE, GRID_SHAPE_OUT_OF_BOUNDS};
use crate::compute_profile::cpu_reference::kernel_ir::{
    self, KernelExpr, KernelIr, Scalar, ScalarKind,
};
use crate::compute_profile::cpu_reference::session;
use crate::compute_profile::cpu_reference::{
    ComputeCapability, CpuReferenceSession, DispatchControl, KernelShape, STALE_HANDLE,
    TRANSFER_OUT_OF_BOUNDS,
};
use crate::hir::ResolvedProgram;
use crate::interpreter::{self, ArgumentValue, InterpreterOptions};

use super::device::*;

/// The shared kernel module: every declaration Metal v1 admits (`i64`/`i32`
/// buffers and result), chosen to line up with the CPU reference's own
/// differential fixtures (`k.affine`, `k.halve32`, `k.ratio`, `k.rem`,
/// `k.neg`) so both backends are exercised against the identical checked
/// bodies.
const KERNELS: &str = r#"
module test.metal_backend;

@id("k.affine")
fn affine(x: i64, y: i64) -> i64
{
    let scaled = x * 3;
    if scaled > y { scaled - y } else { y - scaled + 1 }
}

@id("k.halve32")
fn halve32(x: i32) -> i32
{
    if x < 0i32 { 0i32 - x } else { x / 2i32 }
}

@id("k.ratio")
fn ratio(x: i64, y: i64) -> i64
{
    x / y
}

@id("k.rem")
fn rem(x: i64, y: i64) -> i64
{
    x % y
}

@id("k.neg")
fn neg(x: i64) -> i64
{
    -x
}

@id("k.affine_bisect_a")
fn affine_bisect_a(x: i64, y: i64) -> i64
{
    let scaled = x * 3;
    if scaled > y { scaled } else { y }
}

@id("k.affine_bisect_b")
fn affine_bisect_b(x: i64, y: i64) -> i64
{
    if x > y { x - y } else { y - x + 1 }
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

fn resolve(source: &str) -> ResolvedProgram {
    let ast = crate::parse(source, "metal-backend.spx").expect("fixture parses");
    crate::hir::resolve(&ast).expect("fixture resolves")
}

/// A fresh private source file for the file-based reference interpreter,
/// removed when the guard drops so a failing assertion cannot leak it into
/// the shared temp dir.
struct TempSource {
    path: PathBuf,
}

impl TempSource {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempSource {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn write_source(source: &str) -> TempSource {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "semaprax-metal-backend-{}-{}.spx",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).expect("temporary kernel source is writable");
    TempSource { path }
}

fn i64s(values: &[i64]) -> Vec<Scalar> {
    values.iter().copied().map(Scalar::I64).collect()
}

/// Print the real, local device this run observed, or explain why this
/// test is skipping, and hand back the provenance for the caller to use.
/// A macro (not a function) so its `return` exits the calling test.
macro_rules! device_or_skip {
    () => {
        match device_provenance() {
            Some(provenance) => {
                eprintln!(
                    "Metal device present: {} (registryID {}), {}",
                    provenance.device_name,
                    provenance.registry_id,
                    provenance.operating_system_version
                );
                provenance
            }
            None => {
                eprintln!(
                    "skipped: no Metal device is available on this host \
                     (MTLCreateSystemDefaultDevice returned None) — not evidence \
                     about any other machine"
                );
                return;
            }
        }
    };
}

/// One scalar result from the interpreter: a value, or the stable
/// `semaprax.status.v1` code of the checked failure it selected.
type Expected = Result<Scalar, u32>;

fn render(value: Scalar) -> String {
    match value {
        Scalar::I64(value) => value.to_string(),
        Scalar::I32(value) => format!("{value}i32"),
        other => panic!("Metal v1 differential fixtures are i64/i32 only, found {other:?}"),
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
                    other => panic!("non-kernel interpreter result {other:?}"),
                },
            )
        }
        Some("failed") => Err(outcome["status"]["code"].as_u64().expect("status code") as u32),
        other => panic!("unexpected interpreter outcome {other:?}"),
    }
}

fn expected_map(path: &Path, declaration: &str, columns: &[Vec<Scalar>]) -> Vec<Expected> {
    (0..columns[0].len())
        .map(|index| {
            let arguments: Vec<Scalar> = columns.iter().map(|column| column[index]).collect();
            interpret(path, declaration, &arguments)
        })
        .collect()
}

/// Normalized outcome of one real Metal map dispatch, the same shape the
/// CPU reference's own differential tests compare against the interpreter.
type MetalOutcome = Result<Vec<Scalar>, (usize, u32)>;

fn normalize(
    outcome: MetalDispatchOutcome,
    session: &mut MetalSession,
    output: MetalBufferHandle,
    len: usize,
) -> MetalOutcome {
    match outcome {
        MetalDispatchOutcome::Completed { invocations } => {
            assert_eq!(invocations, len);
            Ok(session.download(output, 0, len).unwrap())
        }
        MetalDispatchOutcome::Failed(MetalSessionFailure::KernelStatus {
            invocation,
            status,
            ..
        }) => Err((invocation, status.code())),
        MetalDispatchOutcome::Failed(other) => panic!("uninjected Metal failure {other:?}"),
    }
}

fn compare(metal: &MetalOutcome, expected: &[Expected]) {
    let first_failure = expected.iter().position(Result::is_err);
    match (metal, first_failure) {
        (Ok(values), None) => {
            let expected: Vec<Scalar> = expected.iter().map(|value| value.unwrap()).collect();
            assert_eq!(
                *values, expected,
                "Metal output differs from the interpreter reference"
            );
        }
        (Err((invocation, code)), Some(index)) => {
            let expected_code = expected[index].unwrap_err();
            assert_eq!(
                *invocation, index,
                "Metal selected a different failing invocation"
            );
            assert_eq!(
                *code, expected_code,
                "Metal selected a different status code"
            );
        }
        (metal, first_failure) => panic!(
            "outcome differs: metal {metal:?}, interpreter first failure at {first_failure:?}"
        ),
    }
}

fn differential_map(declaration: &str, result: ScalarKind, columns: Vec<Vec<Scalar>>) {
    device_or_skip!();
    let program = resolve(KERNELS);
    let source = write_source(KERNELS);
    let expected = expected_map(source.path(), declaration, &columns);

    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    let artifact = session
        .load_kernel(&program, declaration, 4)
        .expect("kernel admitted for Metal v1");
    let len = columns[0].len();
    let inputs: Vec<MetalBufferHandle> = columns
        .iter()
        .map(|column| {
            let handle = session.alloc(column[0].kind(), len).unwrap();
            session.upload(handle, 0, column).unwrap();
            handle
        })
        .collect();
    let output = session.alloc(result, len).unwrap();
    let outcome = session
        .dispatch_map(&program, &artifact, &inputs, output)
        .expect("dispatch admitted");
    let metal = normalize(outcome, &mut session, output, len);
    compare(&metal, &expected);

    let settlement = session.settle();
    assert_eq!(settlement.releases.len(), columns.len() + 1);
    assert!(settlement.selected.is_none() || matches!(metal, Err(_)));
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
fn i64_overflow_selects_the_same_lowest_failing_invocation() {
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
    differential_map(
        "k.rem",
        ScalarKind::I64,
        vec![i64s(&[4, i64::MIN, 5]), i64s(&[2, -1, 0])],
    );
}

#[test]
fn unary_negation_normal_and_overflow_match_the_interpreter() {
    differential_map("k.neg", ScalarKind::I64, vec![i64s(&[0, 1, -7, 42])]);
    differential_map("k.neg", ScalarKind::I64, vec![i64s(&[3, i64::MIN, 5])]);
}

#[test]
fn i32_map_matches_the_interpreter() {
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
}

#[test]
fn mismatched_buffer_lengths_refuse_as_out_of_bounds() {
    device_or_skip!();
    let program = resolve(KERNELS);
    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    let artifact = session
        .load_kernel(&program, "k.affine", 4)
        .expect("kernel admitted");
    let x = session.alloc(ScalarKind::I64, 4).unwrap();
    session.upload(x, 0, &i64s(&[1, 2, 3, 4])).unwrap();
    let y = session.alloc(ScalarKind::I64, 3).unwrap();
    session.upload(y, 0, &i64s(&[1, 2, 3])).unwrap();
    let output = session.alloc(ScalarKind::I64, 4).unwrap();

    let refusal = session
        .dispatch_map(&program, &artifact, &[x, y], output)
        .unwrap_err();
    assert_eq!(refusal.code(), TRANSFER_OUT_OF_BOUNDS);
}

#[test]
fn a_kernel_bound_to_a_different_checked_body_is_refused_as_stale() {
    device_or_skip!();
    let program = resolve(KERNELS);
    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    let artifact = session
        .load_kernel(&program, "k.affine", 4)
        .expect("kernel admitted");

    // The identical declaration, recompiled from a body that no longer
    // matches: `x * 3` became `x * 4`.
    let mutated_program = resolve(&KERNELS.replace("x * 3", "x * 4"));

    let x = session.alloc(ScalarKind::I64, 2).unwrap();
    session.upload(x, 0, &i64s(&[1, 2])).unwrap();
    let y = session.alloc(ScalarKind::I64, 2).unwrap();
    session.upload(y, 0, &i64s(&[1, 2])).unwrap();
    let output = session.alloc(ScalarKind::I64, 2).unwrap();

    let refusal = session
        .dispatch_map(&mutated_program, &artifact, &[x, y], output)
        .unwrap_err();
    assert_eq!(refusal.code(), STALE_HANDLE);
}

#[test]
fn settlement_releases_every_buffer_exactly_once_in_reverse_order() {
    device_or_skip!();
    let program = resolve(KERNELS);
    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    let artifact = session
        .load_kernel(&program, "k.affine", 4)
        .expect("kernel admitted");
    let x = session.alloc(ScalarKind::I64, 2).unwrap(); // index 0
    session.upload(x, 0, &i64s(&[1, 2])).unwrap();
    let y = session.alloc(ScalarKind::I64, 2).unwrap(); // index 1
    session.upload(y, 0, &i64s(&[3, 4])).unwrap();
    let output = session.alloc(ScalarKind::I64, 2).unwrap(); // index 2

    let outcome = session
        .dispatch_map(&program, &artifact, &[x, y], output)
        .unwrap();
    assert!(matches!(
        outcome,
        MetalDispatchOutcome::Completed { invocations: 2 }
    ));

    session.release(x).unwrap();
    let settlement = session.settle();
    assert_eq!(
        settlement
            .releases
            .iter()
            .map(|event| event.buffer)
            .collect::<Vec<_>>(),
        vec![0, 2, 1],
        "buffer 0 releases explicitly first, then settlement releases the rest in reverse \
         allocation order"
    );
    assert_eq!(settlement.releases[0].cause, MetalReleaseCause::Explicit);
    assert!(settlement.releases[1..]
        .iter()
        .all(|event| event.cause == MetalReleaseCause::Settlement));
    assert!(settlement.selected.is_none());
}

/// Compile-only regression coverage for the Metal compiler service failure
/// once triggered by 64-bit unsigned division in `checked_mul_i64` (see the
/// `msl` module docs): `k.affine_bisect_a` multiplies before an `if`,
/// `k.affine_bisect_b` exercises nested checked arithmetic without `*`.
#[test]
fn bisect_let_before_if_with_trivial_branches_compiles() {
    device_or_skip!();
    let program = resolve(KERNELS);
    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    session
        .load_kernel(&program, "k.affine_bisect_a", 4)
        .expect("k.affine_bisect_a (let + trivial if/else) must compile");
}

#[test]
fn bisect_nested_arithmetic_branches_without_let_compiles() {
    device_or_skip!();
    let program = resolve(KERNELS);
    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    session
        .load_kernel(&program, "k.affine_bisect_b", 4)
        .expect("k.affine_bisect_b (nested-arithmetic if/else, no let) must compile");
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

/// Negative control: a kernel whose generated MSL has one operator mutated
/// (the lowered IR's first `+` becomes a `-`, so the generated MSL calls
/// `checked_sub_i64` where the faithful kernel called `checked_add_i64`)
/// must disagree with the interpreter reference — proving the differential
/// oracle actually exercises what runs on the GPU rather than vacuously
/// agreeing. The tampered artifact is also independently refused by the
/// session's own stale-artifact check when dispatched the ordinary way.
#[test]
fn negative_control_a_mutated_generated_kernel_disagrees_with_the_reference() {
    device_or_skip!();
    let program = resolve(KERNELS);
    let source = write_source(KERNELS);
    let columns = vec![i64s(&[0, 1, -7, 40]), i64s(&[0, 5, -30, 100])];
    let expected = expected_map(source.path(), "k.affine", &columns);

    let shape = KernelShape::ElementwiseMap { workgroup_size: 4 };
    let (mut mutant_ir, faithful_fingerprint) =
        session::bind(&program, "k.affine", shape).expect("kernel admitted");
    assert!(
        swap_first_add_for_sub(&mut mutant_ir.body),
        "k.affine's lowered body must contain at least one `+`"
    );
    let mutant_fingerprint = kernel_ir::fingerprint("k.affine", &shape.encode(), &mutant_ir);
    assert_ne!(
        mutant_fingerprint, faithful_fingerprint,
        "a mutated body must not collide with the faithful one's fingerprint"
    );

    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    let mutant_artifact = session
        .load_kernel_from_ir_for_test("k.affine", 4, mutant_ir, mutant_fingerprint)
        .expect("the mutant kernel still compiles: it is a valid, just different, kernel body");

    let len = columns[0].len();
    let x = session.alloc(ScalarKind::I64, len).unwrap();
    session.upload(x, 0, &columns[0]).unwrap();
    let y = session.alloc(ScalarKind::I64, len).unwrap();
    session.upload(y, 0, &columns[1]).unwrap();
    let output = session.alloc(ScalarKind::I64, len).unwrap();

    let outcome = session
        .dispatch_map_unchecked_for_test(&mutant_artifact, &[x, y], output)
        .expect("the mutant dispatch itself is admitted");
    let mutant = normalize(outcome, &mut session, output, len);

    // `compare` panics on agreement failure elsewhere; here disagreement is
    // the required outcome, so check it explicitly instead of via `compare`.
    let disagrees = match (&mutant, expected.iter().position(Result::is_err)) {
        (Ok(values), None) => {
            let expected_values: Vec<Scalar> =
                expected.iter().map(|value| value.unwrap()).collect();
            *values != expected_values
        }
        (Err((invocation, code)), Some(index)) => {
            *invocation != index || *code != expected[index].unwrap_err()
        }
        _ => true,
    };
    assert!(
        disagrees,
        "the mutant kernel must disagree with the faithful reference"
    );

    // Defense in depth: the ordinary dispatch path independently refuses
    // this same tampered artifact, because it no longer matches what
    // `session::bind` derives from the real checked program.
    let output2 = session.alloc(ScalarKind::I64, len).unwrap();
    let refusal = session
        .dispatch_map(&program, &mutant_artifact, &[x, y], output2)
        .unwrap_err();
    assert_eq!(refusal.code(), STALE_HANDLE);
}

/// An input handle that is also the output handle is a `MayOverlap` claim
/// the classifier never admits (`SPX-GC011`) — checked at dispatch time,
/// against the real bound buffers, not merely at load time against a
/// nominal `Disjoint` claim. Both backends reach this refusal through the
/// identical `cpu_reference::session::classify_map_dispatch` call, so this
/// asserts Metal's refusal and the CPU reference's refusal for the
/// identical shape side by side, rather than trusting Metal alone.
#[test]
fn aliased_input_output_buffer_refuses_identically_on_metal_and_the_cpu_reference() {
    device_or_skip!();
    let program = resolve(KERNELS);

    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    let artifact = session
        .load_kernel(&program, "k.affine", 4)
        .expect("kernel admitted");
    let x = session.alloc(ScalarKind::I64, 4).unwrap();
    session.upload(x, 0, &i64s(&[1, 2, 3, 4])).unwrap();
    let out = session.alloc(ScalarKind::I64, 4).unwrap();

    let refusal = session
        .dispatch_map(&program, &artifact, &[x, out], out)
        .unwrap_err();
    assert_eq!(refusal.code(), ALIASING_BEYOND_CHECKED_RULE);

    let mut cpu = CpuReferenceSession::open(ComputeCapability::cpu_reference_all());
    let cpu_artifact = cpu
        .load_kernel(
            &program,
            "k.affine",
            KernelShape::ElementwiseMap { workgroup_size: 4 },
        )
        .expect("kernel admitted");
    let cx = cpu.alloc(ScalarKind::I64, 4).unwrap();
    cpu.upload(cx, 0, &i64s(&[1, 2, 3, 4])).unwrap();
    let cout = cpu.alloc(ScalarKind::I64, 4).unwrap();
    let cpu_refusal = cpu
        .dispatch_map(
            &program,
            &cpu_artifact,
            &[cx, cout],
            cout,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(cpu_refusal.code(), ALIASING_BEYOND_CHECKED_RULE);
}

/// A single-workgroup dispatch whose *real* grid (from the bound buffers'
/// actual length, `65_536` at `workgroup_size: 1`) exceeds `MAX_GRID_DIM`
/// (`65_535`) must refuse (`SPX-GC006`), even though the *nominal* grid
/// `load_kernel` classified was `[1, 1, 1]` and admitted. Both backends
/// reach this refusal through the identical
/// `cpu_reference::session::classify_map_dispatch` call, so this asserts
/// Metal's refusal and the CPU reference's refusal for the identical shape
/// side by side, rather than trusting Metal alone. On Metal this must be
/// refused before any GPU allocation or dispatch inside `execute_map`.
#[test]
fn oversized_dispatch_time_grid_refuses_identically_on_metal_and_the_cpu_reference() {
    device_or_skip!();
    let program = resolve(KERNELS);
    const LEN: usize = 65_536;

    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    let artifact = session
        .load_kernel(&program, "k.neg", 1)
        .expect("kernel admitted");
    let x = session.alloc(ScalarKind::I64, LEN).unwrap();
    let out = session.alloc(ScalarKind::I64, LEN).unwrap();

    let refusal = session
        .dispatch_map(&program, &artifact, &[x], out)
        .unwrap_err();
    assert_eq!(refusal.code(), GRID_SHAPE_OUT_OF_BOUNDS);

    let mut cpu = CpuReferenceSession::open(ComputeCapability::cpu_reference_all());
    let cpu_artifact = cpu
        .load_kernel(
            &program,
            "k.neg",
            KernelShape::ElementwiseMap { workgroup_size: 1 },
        )
        .expect("kernel admitted");
    let cx = cpu.alloc(ScalarKind::I64, LEN).unwrap();
    let cout = cpu.alloc(ScalarKind::I64, LEN).unwrap();
    let cpu_refusal = cpu
        .dispatch_map(
            &program,
            &cpu_artifact,
            &[cx],
            cout,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(cpu_refusal.code(), GRID_SHAPE_OUT_OF_BOUNDS);
}
