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
    ComputeCapability, CpuReferenceSession, DispatchControl, KernelShape, KERNEL_SELECTION_REFUSED,
    STALE_HANDLE, TRANSFER_OUT_OF_BOUNDS,
};
use crate::hir::ResolvedProgram;
use crate::interpreter::{self, ArgumentValue, InterpreterOptions};

use super::device::*;

/// The shared kernel module: every declaration Metal v1 admits — both
/// [`KernelShape::ElementwiseMap`] and [`KernelShape::SequentialFold`], and
/// every [`ScalarKind`] (`i64`/`i32`/`u8`/`usize`/`bool`) — chosen to line up
/// with the CPU reference's own differential fixtures
/// (`src/compute_profile/cpu_reference/tests/mod.rs`'s `KERNELS`) so both
/// backends are exercised against the identical checked bodies.
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

@id("k.byte_mix")
fn byte_mix(a: u8, b: u8) -> u8
{
    a * 2u8 + b / 7u8
}

@id("k.div_u8")
fn div_u8(x: u8, y: u8) -> u8
{
    x / y
}

@id("k.index_scale")
fn index_scale(n: usize) -> usize
{
    n * 4usize - 1usize
}

@id("k.div_usize")
fn div_usize(x: usize, y: usize) -> usize
{
    x / y
}

@id("k.rem_usize")
fn rem_usize(x: usize, y: usize) -> usize
{
    x % y
}

@id("k.flag")
fn flag(x: i64, keep: bool) -> bool
{
    (keep && x != 0) || (!keep && x == 0)
}

@id("k.sum")
fn sum(acc: i64, element: i64) -> i64
{
    acc + element
}

@id("k.count_small")
fn count_small(acc: usize, element: u8) -> usize
{
    if element < 10u8 { acc + 1usize } else { acc }
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

fn u8s(values: &[u8]) -> Vec<Scalar> {
    values.iter().copied().map(Scalar::U8).collect()
}

fn usizes(values: &[u64]) -> Vec<Scalar> {
    values.iter().copied().map(Scalar::Usize).collect()
}

fn bools(values: &[bool]) -> Vec<Scalar> {
    values.iter().copied().map(Scalar::Bool).collect()
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
    assert!(settlement.selected.is_none() || metal.is_err());
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
fn u8_map_normal_and_overflow_match_the_interpreter() {
    differential_map(
        "k.byte_mix",
        ScalarKind::U8,
        vec![u8s(&[0, 3, 100, 127]), u8s(&[6, 13, 255, 0])],
    );
    // `a * 2u8` overflows at invocation 1 (`128 * 2`); `b / 7u8` never
    // fails since `7u8` is never zero here.
    differential_map(
        "k.byte_mix",
        ScalarKind::U8,
        vec![u8s(&[1, 128, 2]), u8s(&[0, 0, 0])],
    );
}

/// `x / y` on `u8` where `y` is a variable, non-constant-folded zero in one
/// lane (invocation 1) among otherwise-valid divisors: the generated
/// `checked_div_u8` guard must select `DivisionByZero` for exactly that
/// lane, on real hardware, the same as it would for a constant `/ 0u8` the
/// checked verifier would refuse at compile time were it written literally.
#[test]
fn u8_division_by_a_variable_zero_divisor_selects_the_failing_lane() {
    differential_map(
        "k.div_u8",
        ScalarKind::U8,
        vec![u8s(&[10, 20, 30]), u8s(&[2, 0, 5])],
    );
}

#[test]
fn usize_map_normal_and_underflow_match_the_interpreter() {
    differential_map(
        "k.index_scale",
        ScalarKind::Usize,
        vec![usizes(&[1, 2, 1 << 40, 0, 5])],
    );
    differential_map(
        "k.rem_usize",
        ScalarKind::Usize,
        vec![usizes(&[10, 7, 0, u64::MAX]), usizes(&[3, 7, 5, 6])],
    );
    // Remainder by zero at invocation 1.
    differential_map(
        "k.rem_usize",
        ScalarKind::Usize,
        vec![usizes(&[9, 1]), usizes(&[4, 0])],
    );
}

/// `usize` (MSL `ulong`) `/` and `%` on real hardware, exercising every
/// value class the checked bit-at-a-time software division
/// (`checked_div_u64`/`checked_rem_u64`, see the `msl` module docs) must
/// handle without ever emitting a native 64-bit unsigned `/`/`%`: divisor
/// `1`; divisor spanning the low/high 32-bit halves (`2^32`, `2^40`);
/// divisor at the sign-bit boundary (`2^63`) and just past it, in
/// `(2^63, 2^64)` (`2^63 + 2^62`), both against `u64::MAX`; divisor
/// `u64::MAX` itself against both `u64::MAX` and a small dividend; a zero
/// dividend; and a divisor exceeding its dividend (quotient `0`). None of
/// these selects a checked failure — `usize` has no signed-overflow case,
/// only division/remainder by zero, covered separately below.
#[test]
fn usize_division_and_remainder_cover_every_value_class_on_hardware() {
    let dividends = usizes(&[
        12_345,
        (1u64 << 40) + 5,
        (1u64 << 41) + 7,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        5,
        0,
        3,
    ]);
    let divisors = usizes(&[
        1,
        1u64 << 32,
        1u64 << 40,
        1u64 << 63,
        (1u64 << 63) + (1u64 << 62),
        u64::MAX,
        u64::MAX,
        7,
        10,
    ]);
    differential_map(
        "k.div_usize",
        ScalarKind::Usize,
        vec![dividends.clone(), divisors.clone()],
    );
    differential_map("k.rem_usize", ScalarKind::Usize, vec![dividends, divisors]);
}

/// `usize` division/remainder by zero, on real hardware, selects the
/// lowest-ordinal failing lane exactly as the CPU reference does, for both
/// `/` and `%`.
#[test]
fn usize_division_and_remainder_by_zero_select_the_lowest_failing_lane() {
    let dividends = usizes(&[20, 9, 3]);
    let divisors = usizes(&[4, 0, 0]);
    differential_map(
        "k.div_usize",
        ScalarKind::Usize,
        vec![dividends.clone(), divisors.clone()],
    );
    differential_map("k.rem_usize", ScalarKind::Usize, vec![dividends, divisors]);
}

#[test]
fn bool_map_with_lazy_operators_matches_the_interpreter() {
    differential_map(
        "k.flag",
        ScalarKind::Bool,
        vec![
            i64s(&[0, 1, 0, -4, 9]),
            bools(&[true, true, false, false, true]),
        ],
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

/// Run `declaration` as a [`KernelShape::SequentialFold`] left to right from
/// `initial`, through both the ordinary reference interpreter (one
/// invocation per element, exactly [`session::run_fold`]'s own left-to-right
/// order) and a real Metal fold dispatch, and assert they select the
/// identical outcome: the same final accumulator, or the same lowest-ordinal
/// failing element and checked status.
fn differential_fold(declaration: &str, initial: Scalar, elements: Vec<Scalar>) {
    device_or_skip!();
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

    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    let artifact = session
        .load_fold_kernel(&program, declaration)
        .expect("fold kernel admitted for Metal v1");
    let input = session.alloc(elements[0].kind(), elements.len()).unwrap();
    session.upload(input, 0, &elements).unwrap();
    let output = session.alloc(initial.kind(), 1).unwrap();
    let outcome = session
        .dispatch_fold(&program, &artifact, initial, input, output)
        .expect("dispatch admitted");
    match (outcome, expected) {
        (MetalDispatchOutcome::Completed { invocations }, Ok(())) => {
            assert_eq!(invocations, elements.len());
            assert_eq!(session.download(output, 0, 1).unwrap(), vec![accumulator]);
        }
        (
            MetalDispatchOutcome::Failed(MetalSessionFailure::KernelStatus {
                invocation,
                status,
                ..
            }),
            Err((index, code)),
        ) => {
            assert_eq!(
                invocation, index,
                "Metal selected a different failing invocation"
            );
            assert_eq!(
                status.code(),
                code,
                "Metal selected a different status code"
            );
        }
        (outcome, expected) => {
            panic!("fold differs: metal {outcome:?}, interpreter {expected:?}")
        }
    }

    let settlement = session.settle();
    assert_eq!(settlement.releases.len(), 2);
}

#[test]
fn sequential_folds_match_left_to_right_interpreter_folds() {
    differential_fold("k.sum", Scalar::I64(5), i64s(&[1, -2, 30, 400, -5000]));
    // A checked overflow partway through the left-to-right fold.
    differential_fold("k.sum", Scalar::I64(i64::MAX - 10), i64s(&[4, 6, 1, -100]));
    // An accumulator kind (`usize`) that differs from its element kind
    // (`u8`), exercising the same fold-signature relaxation the CPU
    // reference itself admits (only `params[0] == result` is required).
    differential_fold(
        "k.count_small",
        Scalar::Usize(0),
        u8s(&[1, 200, 9, 10, 0, 255]),
    );
}

/// A fold artifact can never be dispatched as a map, and a map artifact can
/// never be dispatched as a fold: both guards live at the very top of
/// `dispatch_map`/`dispatch_fold`, before any buffer is even inspected, so
/// this only needs artifacts and buffers of the right *kind*, not a
/// matching parameter count.
#[test]
fn dispatch_map_refuses_a_fold_artifact_and_dispatch_fold_refuses_a_map_artifact() {
    device_or_skip!();
    let program = resolve(KERNELS);
    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");

    let fold_artifact = session
        .load_fold_kernel(&program, "k.sum")
        .expect("fold kernel admitted");
    let output = session.alloc(ScalarKind::I64, 1).unwrap();
    let refusal = session
        .dispatch_map(&program, &fold_artifact, &[], output)
        .unwrap_err();
    assert_eq!(refusal.code(), KERNEL_SELECTION_REFUSED);

    let map_artifact = session
        .load_kernel(&program, "k.neg", 4)
        .expect("map kernel admitted");
    let input = session.alloc(ScalarKind::I64, 1).unwrap();
    let accumulator = session.alloc(ScalarKind::I64, 1).unwrap();
    let refusal = session
        .dispatch_fold(&program, &map_artifact, Scalar::I64(0), input, accumulator)
        .unwrap_err();
    assert_eq!(refusal.code(), KERNEL_SELECTION_REFUSED);
}

/// An input handle that is also the fold's one-element accumulator/output
/// handle is a `MayOverlap` claim the classifier never admits (`SPX-GC011`),
/// exactly as for a map (see
/// `aliased_input_output_buffer_refuses_identically_on_metal_and_the_cpu_reference`
/// below): checked at dispatch time, against the real bound buffers, through
/// the identical `cpu_reference::session::classify_fold_dispatch` call on
/// both backends.
#[test]
fn aliased_fold_input_output_buffer_refuses_identically_on_metal_and_the_cpu_reference() {
    device_or_skip!();
    let program = resolve(KERNELS);

    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    let artifact = session
        .load_fold_kernel(&program, "k.sum")
        .expect("fold kernel admitted");
    let buffer = session.alloc(ScalarKind::I64, 1).unwrap();
    let refusal = session
        .dispatch_fold(&program, &artifact, Scalar::I64(0), buffer, buffer)
        .unwrap_err();
    assert_eq!(refusal.code(), ALIASING_BEYOND_CHECKED_RULE);

    let mut cpu = CpuReferenceSession::open(ComputeCapability::cpu_reference_all());
    let cpu_artifact = cpu
        .load_kernel(&program, "k.sum", KernelShape::SequentialFold)
        .expect("fold kernel admitted");
    let cpu_buffer = cpu.alloc(ScalarKind::I64, 1).unwrap();
    let cpu_refusal = cpu
        .dispatch_fold(
            &program,
            &cpu_artifact,
            Scalar::I64(0),
            cpu_buffer,
            cpu_buffer,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(cpu_refusal.code(), ALIASING_BEYOND_CHECKED_RULE);
}

/// Negative control for the fold shape: a `k.sum` artifact whose lowered
/// body has its first `+` mutated to a `-` must disagree with the
/// interpreter reference, proving the fold differential oracle actually
/// exercises what runs on the GPU. The tampered artifact is also
/// independently refused by the session's own stale-artifact check when
/// dispatched the ordinary way. Mirrors
/// `negative_control_a_mutated_generated_kernel_disagrees_with_the_reference`
/// for the map shape.
#[test]
fn negative_control_a_mutated_generated_fold_kernel_disagrees_with_the_reference() {
    device_or_skip!();
    let program = resolve(KERNELS);
    let source = write_source(KERNELS);
    let elements = i64s(&[1, 2, 3, 4]);
    let initial = Scalar::I64(10);

    let mut accumulator = initial;
    for element in &elements {
        accumulator = interpret(source.path(), "k.sum", &[accumulator, *element])
            .expect("k.sum never fails on these small inputs");
    }

    let shape = KernelShape::SequentialFold;
    let (mut mutant_ir, faithful_fingerprint) =
        session::bind(&program, "k.sum", shape).expect("kernel admitted");
    assert!(
        swap_first_add_for_sub(&mut mutant_ir.body),
        "k.sum's lowered body must contain at least one `+`"
    );
    let mutant_fingerprint = kernel_ir::fingerprint("k.sum", &shape.encode(), &mutant_ir);
    assert_ne!(
        mutant_fingerprint, faithful_fingerprint,
        "a mutated body must not collide with the faithful one's fingerprint"
    );

    let mut session =
        MetalSession::open(MetalCapability::all()).expect("device present (just checked)");
    let mutant_artifact = session
        .load_kernel_from_ir_for_test("k.sum", shape, mutant_ir, mutant_fingerprint)
        .expect("the mutant kernel still compiles: it is a valid, just different, kernel body");

    let input = session.alloc(ScalarKind::I64, elements.len()).unwrap();
    session.upload(input, 0, &elements).unwrap();
    let output = session.alloc(ScalarKind::I64, 1).unwrap();
    let outcome = session
        .dispatch_fold_unchecked_for_test(&mutant_artifact, initial, input, output)
        .expect("the mutant dispatch itself is admitted");
    let MetalDispatchOutcome::Completed { .. } = outcome else {
        panic!("k.sum with a mutated `+` still never fails on these small inputs: {outcome:?}");
    };
    let mutant_result = session.download(output, 0, 1).unwrap()[0];
    assert_ne!(
        mutant_result, accumulator,
        "the mutant fold must disagree with the faithful reference"
    );

    // Defense in depth: the ordinary dispatch path independently refuses
    // this same tampered artifact, because it no longer matches what
    // `session::bind` derives from the real checked program.
    let output2 = session.alloc(ScalarKind::I64, 1).unwrap();
    let refusal = session
        .dispatch_fold(&program, &mutant_artifact, initial, input, output2)
        .unwrap_err();
    assert_eq!(refusal.code(), STALE_HANDLE);
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
        .load_kernel_from_ir_for_test("k.affine", shape, mutant_ir, mutant_fingerprint)
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
