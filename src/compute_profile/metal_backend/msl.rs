//! Deterministic Metal Shading Language (MSL) generation from an admitted
//! [`KernelIr`], for the v1 Metal elementwise-map profile
//! (`i64`/`i32` buffers and results only; internal `bool` is admitted for
//! conditions and comparisons, since no non-trivial kernel body can avoid
//! one).
//!
//! # Checked-failure semantics
//!
//! MSL has no traps: an overflowing add or a division by zero is undefined
//! or silently wrapping, never a checked failure. This generator therefore
//! emits an explicit pre-check before every risky operation, each one
//! writing the exact `semaprax.status.v1` numeric code
//! ([`crate::cleanup_plan::StatusCase::code`]) into a per-invocation status
//! buffer and returning from the invocation immediately — matching the
//! host [`super::super::cpu_reference::kernel_ir::evaluate`] short-circuit
//! exactly: the first sub-expression that fails, in the language's
//! left-to-right evaluation order, is the only one whose status is ever
//! recorded, and no further computation for that invocation occurs.
//!
//! Generation is a pure function of the [`KernelIr`]: the same admitted IR
//! always lowers to byte-identical MSL source, which is what lets a stale
//! artifact be caught by re-deriving the source and comparing its SHA-256
//! digest (see [`super::device::MetalKernelArtifact`]).

use sha2::{Digest, Sha256};

use crate::ast::BinaryOp;
use crate::compute_profile::cpu_reference::kernel_ir::{KernelExpr, KernelIr, Scalar, ScalarKind};

/// The generated kernel function's fixed name inside its own MSL source.
pub(crate) const FUNCTION_NAME: &str = "semaprax_kernel";

const PREAMBLE: &str = r#"#include <metal_stdlib>
using namespace metal;

inline uint checked_add_i64(long a, long b, thread long* out) {
    if ((b > 0 && a > (9223372036854775807LL - b)) ||
        (b < 0 && a < ((-9223372036854775807LL - 1LL) - b))) { return 1u; }
    *out = a + b;
    return 0u;
}
inline uint checked_sub_i64(long a, long b, thread long* out) {
    if ((b < 0 && a > (9223372036854775807LL + b)) ||
        (b > 0 && a < ((-9223372036854775807LL - 1LL) + b))) { return 2u; }
    *out = a - b;
    return 0u;
}
inline uint checked_mul_i64(long a, long b, thread long* out) {
    // Computing `a * b` directly in `long` first and inspecting the result
    // is not safe here: on overflow that multiplication is signed integer
    // overflow, and a post-hoc division-based check reasoning about an
    // already-overflowed signed value is exactly the pattern an optimizer
    // is free to treat as unreachable. Every step below instead computes
    // in `ulong` (unsigned overflow is always wraparound, never undefined),
    // and only forms the final `long` result once it is known to fit.
    if (a == 0 || b == 0) { *out = 0; return 0u; }
    bool negative = (a < 0) != (b < 0);
    ulong ua = (a < 0) ? (0UL - (ulong)a) : (ulong)a;
    ulong ub = (b < 0) ? (0UL - (ulong)b) : (ulong)b;
    if (mulhi(ua, ub) != 0UL) { return 3u; }
    ulong product = ua * ub;
    ulong limit = negative ? 9223372036854775808UL : 9223372036854775807UL;
    if (product > limit) { return 3u; }
    *out = negative ? (long)(0UL - product) : (long)product;
    return 0u;
}
inline uint checked_div_i64(long a, long b, thread long* out) {
    if (b == 0) { return 4u; }
    if (a == (-9223372036854775807LL - 1LL) && b == -1LL) { return 5u; }
    *out = a / b;
    return 0u;
}
inline uint checked_rem_i64(long a, long b, thread long* out) {
    if (b == 0) { return 6u; }
    if (a == (-9223372036854775807LL - 1LL) && b == -1LL) { return 7u; }
    *out = a % b;
    return 0u;
}
inline uint checked_neg_i64(long a, thread long* out) {
    if (a == (-9223372036854775807LL - 1LL)) { return 8u; }
    *out = -a;
    return 0u;
}

inline uint checked_add_i32(int a, int b, thread int* out) {
    long r = (long)a + (long)b;
    if (r < (long)(-2147483647 - 1) || r > (long)2147483647) { return 1u; }
    *out = (int)r;
    return 0u;
}
inline uint checked_sub_i32(int a, int b, thread int* out) {
    long r = (long)a - (long)b;
    if (r < (long)(-2147483647 - 1) || r > (long)2147483647) { return 2u; }
    *out = (int)r;
    return 0u;
}
inline uint checked_mul_i32(int a, int b, thread int* out) {
    long r = (long)a * (long)b;
    if (r < (long)(-2147483647 - 1) || r > (long)2147483647) { return 3u; }
    *out = (int)r;
    return 0u;
}
inline uint checked_div_i32(int a, int b, thread int* out) {
    if (b == 0) { return 4u; }
    if (a == (int)(-2147483647 - 1) && b == -1) { return 5u; }
    *out = a / b;
    return 0u;
}
inline uint checked_rem_i32(int a, int b, thread int* out) {
    if (b == 0) { return 6u; }
    if (a == (int)(-2147483647 - 1) && b == -1) { return 7u; }
    *out = a % b;
    return 0u;
}
inline uint checked_neg_i32(int a, thread int* out) {
    if (a == (int)(-2147483647 - 1)) { return 8u; }
    *out = -a;
    return 0u;
}

inline uint checked_add_u8(uchar a, uchar b, thread uchar* out) {
    int r = (int)a + (int)b;
    if (r > 255) { return 1u; }
    *out = (uchar)r;
    return 0u;
}
inline uint checked_sub_u8(uchar a, uchar b, thread uchar* out) {
    int r = (int)a - (int)b;
    if (r < 0) { return 2u; }
    *out = (uchar)r;
    return 0u;
}
inline uint checked_mul_u8(uchar a, uchar b, thread uchar* out) {
    int r = (int)a * (int)b;
    if (r > 255) { return 3u; }
    *out = (uchar)r;
    return 0u;
}
inline uint checked_div_u8(uchar a, uchar b, thread uchar* out) {
    if (b == 0) { return 4u; }
    *out = a / b;
    return 0u;
}

// `ulong` (the Metal lowering for the language's target-independent 64-bit
// unsigned `usize`; MSL has no type spelled `usize`) has no checked
// remainder/overflow case beyond division-by-zero: unsigned add/sub/mul
// overflow are plain range checks, and there is no unsigned analogue of
// `MIN / -1`. `checked_div_u64`/`checked_rem_u64` deliberately never emit a
// native `/` or `%` on `ulong` operands: the system Metal compiler service
// crashed, deterministically, on 64-bit unsigned division reached through a
// division-based multiply-overflow check (see this module's `gen_expr` doc
// comment) on the one Apple M3 Pro this backend has been run on, and this
// backend keeps 64-bit unsigned division out of every generated kernel
// rather than assume that failure was scoped to the one call site it was
// first observed at. `checked_div_u64`/`checked_rem_u64` below instead
// compute the quotient and remainder with an ordinary bit-at-a-time binary
// long division, using only shifts, comparisons, and subtraction.
inline uint checked_mul_u64(ulong a, ulong b, thread ulong* out) {
    if (a == 0 || b == 0) { *out = 0; return 0u; }
    if (mulhi(a, b) != 0UL) { return 3u; }
    *out = a * b;
    return 0u;
}
inline void u64_long_division(ulong a, ulong b, thread ulong* quotient, thread ulong* remainder) {
    ulong q = 0;
    ulong r = 0;
    for (int i = 63; i >= 0; --i) {
        r = (r << 1) | ((a >> (uint)i) & 1UL);
        if (r >= b) {
            r -= b;
            q |= (1UL << (uint)i);
        }
    }
    *quotient = q;
    *remainder = r;
}
inline uint checked_add_u64(ulong a, ulong b, thread ulong* out) {
    if (a > (18446744073709551615UL - b)) { return 1u; }
    *out = a + b;
    return 0u;
}
inline uint checked_sub_u64(ulong a, ulong b, thread ulong* out) {
    if (a < b) { return 2u; }
    *out = a - b;
    return 0u;
}
inline uint checked_div_u64(ulong a, ulong b, thread ulong* out) {
    if (b == 0) { return 4u; }
    ulong quotient;
    ulong remainder;
    u64_long_division(a, b, &quotient, &remainder);
    *out = quotient;
    return 0u;
}
inline uint checked_rem_u64(ulong a, ulong b, thread ulong* out) {
    if (b == 0) { return 6u; }
    ulong quotient;
    ulong remainder;
    u64_long_division(a, b, &quotient, &remainder);
    *out = remainder;
    return 0u;
}
"#;

/// One deterministically generated kernel: its complete MSL source and the
/// SHA-256 digest [`super::device::MetalKernelArtifact`] binds to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedKernel {
    pub(crate) source: String,
    pub(crate) sha256: String,
}

/// The Metal v1 buffer/scalar lowering for every admitted [`ScalarKind`].
/// `Usize` lowers to `ulong`: MSL has no type spelled `usize`, and `ulong`
/// (64-bit unsigned) is the closest native type to the language's
/// target-independent checked 64-bit unsigned `usize` — the same mapping
/// [`ScalarKind::classifier_scalar`] already uses (`ScalarType::U64`).
pub(crate) fn msl_type(kind: ScalarKind) -> Result<&'static str, String> {
    match kind {
        ScalarKind::I64 => Ok("long"),
        ScalarKind::I32 => Ok("int"),
        ScalarKind::U8 => Ok("uchar"),
        ScalarKind::Usize => Ok("ulong"),
        ScalarKind::Bool => Ok("bool"),
    }
}

fn literal_text(scalar: Scalar) -> Result<String, String> {
    match scalar {
        Scalar::I64(value) if value == i64::MIN => Ok("(-9223372036854775807LL - 1LL)".to_owned()),
        Scalar::I64(value) => Ok(format!("{value}LL")),
        Scalar::I32(value) if value == i32::MIN => Ok("((int)(-2147483647 - 1))".to_owned()),
        Scalar::I32(value) => Ok(format!("{value}")),
        Scalar::U8(value) => Ok(format!("((uchar){value})")),
        Scalar::Usize(value) => Ok(format!("{value}UL")),
        Scalar::Bool(value) => Ok(if value { "true" } else { "false" }.to_owned()),
    }
}

fn comparison_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        _ => unreachable!("comparison_symbol is only called for comparison operators"),
    }
}

fn checked_fn_name(op: BinaryOp, kind: ScalarKind) -> Result<String, String> {
    let width = match kind {
        ScalarKind::I64 => "i64",
        ScalarKind::I32 => "i32",
        ScalarKind::U8 => "u8",
        ScalarKind::Usize => "u64",
        ScalarKind::Bool => {
            return Err("checked arithmetic has no Metal v1 lowering for Bool".to_owned())
        }
    };
    let name = match op {
        BinaryOp::Add => "add",
        BinaryOp::Sub => "sub",
        BinaryOp::Mul => "mul",
        BinaryOp::Div => "div",
        BinaryOp::Rem => "rem",
        _ => unreachable!("checked_fn_name is only called for checked arithmetic operators"),
    };
    Ok(format!("checked_{name}_{width}"))
}

/// Per-generation state: fresh temporary names, and the MSL variable name
/// plus inferred kind already holding every slot observed so far
/// (parameters up front — `slot{index}`, matching the kernel prologue's own
/// declarations — `let`s as their enclosing [`KernelExpr::Block`] is
/// reached, always before any reference since lowering never admits a
/// forward reference). A `let` is never given its own fresh `slotN`
/// declaration distinct from the synthetic temporary its value already
/// computed into: it simply records that temporary's name, so
/// [`KernelExpr::Slot`] resolves straight to it.
struct Codegen {
    next_tmp: u32,
    slots: Vec<Option<(String, ScalarKind)>>,
    /// `true` while generating a [`generate_fold`] body: every checked-failure
    /// guard then writes a fixed one-element `out_status`/`out_invocation`
    /// pair instead of indexing `out_status` by the per-thread `gid` a map
    /// kernel dispatches with (see [`push_guarded`]).
    fold: bool,
}

impl Codegen {
    fn fresh(&mut self) -> String {
        self.next_tmp += 1;
        format!("t{}", self.next_tmp)
    }
}

/// How a checked failure is recorded and how it stops the rest of the
/// invocation, and why this generator's shape is what it is.
///
/// # Metal compiler constraint (confirmed on hardware)
///
/// The system Metal compiler service (`newLibraryWithSource`) failed with
/// `XPC_ERROR_CONNECTION_INTERRUPTED`, deterministically, on every kernel
/// containing `*` and on no other kernel. The cause was the overflow check
/// in `checked_mul_i64`, which detected overflow with a 64-bit unsigned
/// division (`product / ua != ub`). Replacing it with `mulhi(ua, ub) != 0`
/// (the high half of the 128-bit product) made the whole differential suite
/// pass on an Apple M3 Pro. Three earlier control-flow theories (an early
/// `return` before a later `if`, a redundant `let` copy, and an `if` nested
/// inside a guard) were each ruled out on hardware first; do not re-derive
/// them. Keep 64-bit unsigned division out of the generated helpers.
///
/// Checked failures are recorded with a single mutable `ok` flag rather
/// than nested guards, which keeps the generated control flow flat:
///
/// ```text
/// long tN = 0;
/// if (ok) {
///     uint stN = checked_op(args, &tN);
///     if (stN) { out_status[gid] = stN; ok = false; }
/// }
/// ```
///
/// `ok` starts `true`; the first failure sets it `false` and every later
/// guard's own `if (ok)` then skips its body, so only the first (lowest
/// ordinal) failure's status is ever recorded and no further checked
/// operation after it runs. Nothing else needs `ok`: a comparison, `if`, or
/// `!`/`&&`/`||` cannot itself fail, so — once a prior guard has already
/// set `ok = false` — evaluating one against a zero-initialized (never
/// UB, never garbage) temporary is harmless; the *only* place that
/// matters is gated explicitly: the kernel's very last statement is
/// `if (ok) { out[gid] = result; }`, so a failed invocation never
/// publishes an output regardless of what unnecessary-but-safe branching
/// happened after the failure. Every `if`/`else` this generator emits is
/// therefore always plain, ordinary, user-level nesting — never wrapped
/// around or inside a guard — matching exactly the one shape every run
/// this session has proven fine.
fn gen_expr(
    cg: &mut Codegen,
    expr: &KernelExpr,
    out: &mut String,
) -> Result<(String, ScalarKind), String> {
    match expr {
        KernelExpr::Slot(slot) => cg
            .slots
            .get(*slot as usize)
            .cloned()
            .flatten()
            .ok_or_else(|| format!("slot {slot} read before it is assigned")),
        KernelExpr::Literal(scalar) => Ok((literal_text(*scalar)?, scalar.kind())),
        KernelExpr::Neg(inner) => {
            let (value, kind) = gen_expr(cg, inner, out)?;
            let ty = msl_type(kind)?;
            let function = match kind {
                ScalarKind::I64 => "checked_neg_i64",
                ScalarKind::I32 => "checked_neg_i32",
                _ => return Err(format!("negation has no Metal v1 lowering for {kind:?}")),
            };
            let name = cg.fresh();
            push_guarded(
                cg.fold,
                out,
                ty,
                &name,
                &format!("{function}({value}, &{name})"),
            );
            Ok((name, kind))
        }
        KernelExpr::Not(inner) => {
            let (value, kind) = gen_expr(cg, inner, out)?;
            if kind != ScalarKind::Bool {
                return Err("`!` has no Metal v1 lowering for a non-bool operand".to_owned());
            }
            let name = cg.fresh();
            out.push_str(&format!("    bool {name} = !{value};\n"));
            Ok((name, ScalarKind::Bool))
        }
        KernelExpr::Binary { op, left, right } => gen_binary(cg, *op, left, right, out),
        KernelExpr::If {
            condition,
            then_branch,
            else_branch,
        } => gen_if(cg, condition, then_branch, else_branch, out),
        KernelExpr::Block { lets, tail } => {
            for (slot, value) in lets {
                let (value_name, kind) = gen_expr(cg, value, out)?;
                cg.slots[*slot as usize] = Some((value_name, kind));
            }
            gen_expr(cg, tail, out)
        }
    }
}

/// Emit a checked operation gated on `ok` (see [`gen_expr`]'s doc comment):
/// zero-initialize `name` (never an uninitialized read, even if skipped),
/// then only call `call` and only ever set `ok = false` while still `ok`.
///
/// A map kernel dispatches one GPU thread per invocation (`gid` is the real
/// [[thread_position_in_grid]]), so a failure is recorded into that
/// invocation's own slot of a `len`-sized `out_status` array; the host
/// selects the lowest failing ordinal by scanning it afterwards. A fold
/// kernel ([`fold` == `true`]) is one single-threaded invocation that loops
/// sequentially over `gid` itself (see [`generate_fold`]), so there is only
/// ever one failure to record: it writes a fixed one-element `out_status`
/// and separately records which loop ordinal failed into `out_invocation`,
/// since a fold's `gid` is not a buffer index the host can otherwise recover
/// after the loop has moved on.
fn push_guarded(fold: bool, out: &mut String, ty: &str, name: &str, call: &str) {
    let record = if fold {
        format!("out_status[0] = st_{name}; out_invocation[0] = gid; ok = false;")
    } else {
        format!("out_status[gid] = st_{name}; ok = false;")
    };
    out.push_str(&format!(
        "    {ty} {name} = 0;\n    if (ok) {{\n        uint st_{name} = {call};\n        if (st_{name}) {{ {record} }}\n    }}\n"
    ));
}

fn gen_binary(
    cg: &mut Codegen,
    op: BinaryOp,
    left: &KernelExpr,
    right: &KernelExpr,
    out: &mut String,
) -> Result<(String, ScalarKind), String> {
    if matches!(op, BinaryOp::And | BinaryOp::Or) {
        let (lvar, lkind) = gen_expr(cg, left, out)?;
        if lkind != ScalarKind::Bool {
            return Err("`&&`/`||` need bool operands".to_owned());
        }
        let name = cg.fresh();
        out.push_str(&format!("    bool {name};\n"));
        let mut branch = String::new();
        let (rvar, rkind) = gen_expr(cg, right, &mut branch)?;
        if rkind != ScalarKind::Bool {
            return Err("`&&`/`||` need bool operands".to_owned());
        }
        let (short_circuit, taken) = match op {
            BinaryOp::And => ("false", "!"),
            BinaryOp::Or => ("true", ""),
            _ => unreachable!(),
        };
        out.push_str(&format!(
            "    if ({taken}{lvar}) {{ {name} = {short_circuit}; }} else {{\n{branch}        {name} = {rvar};\n    }}\n"
        ));
        return Ok((name, ScalarKind::Bool));
    }

    let (lvar, lkind) = gen_expr(cg, left, out)?;
    let (rvar, rkind) = gen_expr(cg, right, out)?;
    if lkind != rkind {
        return Err(format!(
            "binary operand kinds disagree: {lkind:?} vs {rkind:?}"
        ));
    }
    if matches!(
        op,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
    ) {
        let ty = msl_type(lkind)?;
        let function = checked_fn_name(op, lkind)?;
        let name = cg.fresh();
        push_guarded(
            cg.fold,
            out,
            ty,
            &name,
            &format!("{function}({lvar}, {rvar}, &{name})"),
        );
        Ok((name, lkind))
    } else {
        let symbol = comparison_symbol(op);
        let name = cg.fresh();
        out.push_str(&format!("    bool {name} = ({lvar} {symbol} {rvar});\n"));
        Ok((name, ScalarKind::Bool))
    }
}

fn gen_if(
    cg: &mut Codegen,
    condition: &KernelExpr,
    then_branch: &KernelExpr,
    else_branch: &KernelExpr,
    out: &mut String,
) -> Result<(String, ScalarKind), String> {
    let (cvar, ckind) = gen_expr(cg, condition, out)?;
    if ckind != ScalarKind::Bool {
        return Err("`if` condition must be bool".to_owned());
    }
    let mut then_code = String::new();
    let (then_var, then_kind) = gen_expr(cg, then_branch, &mut then_code)?;
    let mut else_code = String::new();
    let (else_var, else_kind) = gen_expr(cg, else_branch, &mut else_code)?;
    if then_kind != else_kind {
        return Err(format!(
            "`if` branches disagree in kind: {then_kind:?} vs {else_kind:?}"
        ));
    }
    let ty = msl_type(then_kind)?;
    let name = cg.fresh();
    out.push_str(&format!(
        "    {ty} {name};\n    if ({cvar}) {{\n{then_code}        {name} = {then_var};\n    }} else {{\n{else_code}        {name} = {else_var};\n    }}\n"
    ));
    Ok((name, then_kind))
}

/// Generate the complete, deterministic MSL source for an
/// [`KernelShape::ElementwiseMap`](super::super::cpu_reference::KernelShape::ElementwiseMap)
/// `ir`, bound to `declaration` only as a source comment (never a semantic
/// input).
///
/// Refuses (returns `Err`) any kind this v1 generator has no [`msl_type`]
/// lowering for. Every [`ScalarKind`] the CPU reference admits has one, so
/// this can only fail on a future kind added there before this generator is
/// taught it; the caller is expected to have already refused any such kind
/// through the ordinary `SPX-GC0xx` vocabulary before reaching generation,
/// so this is defense in depth, not the primary admission path.
pub(crate) fn generate(declaration: &str, ir: &KernelIr) -> Result<GeneratedKernel, String> {
    for kind in &ir.params {
        msl_type(*kind)?;
    }
    msl_type(ir.result)?;

    let mut cg = Codegen {
        next_tmp: 0,
        slots: vec![None; ir.slots as usize],
        fold: false,
    };
    for (slot, kind) in ir.params.iter().enumerate() {
        cg.slots[slot] = Some((format!("slot{slot}"), *kind));
    }

    let mut body = String::new();
    let (result_var, result_kind) = gen_expr(&mut cg, &ir.body, &mut body)?;
    if result_kind != ir.result {
        return Err("generated result kind disagrees with the checked signature".to_owned());
    }
    body.push_str(&format!("    if (ok) {{ out[gid] = {result_var}; }}\n"));

    let out_index = ir.params.len();
    let status_index = out_index + 1;
    let count_index = status_index + 1;
    let out_ty = msl_type(ir.result)?;

    let mut source = String::new();
    source.push_str(PREAMBLE);
    source.push_str(&format!(
        "\n// SEMAPRAX kernel `{declaration}`\nkernel void {FUNCTION_NAME}(\n"
    ));
    for (index, kind) in ir.params.iter().enumerate() {
        let ty = msl_type(*kind)?;
        source.push_str(&format!(
            "    device const {ty}* in{index} [[buffer({index})]],\n"
        ));
    }
    source.push_str(&format!(
        "    device {out_ty}* out [[buffer({out_index})]],\n"
    ));
    source.push_str(&format!(
        "    device uint* out_status [[buffer({status_index})]],\n"
    ));
    source.push_str(&format!(
        "    constant uint& element_count [[buffer({count_index})]],\n"
    ));
    source.push_str("    uint gid [[thread_position_in_grid]])\n{\n");
    source.push_str("    if (gid >= element_count) { return; }\n");
    for (index, kind) in ir.params.iter().enumerate() {
        let ty = msl_type(*kind)?;
        source.push_str(&format!("    {ty} slot{index} = in{index}[gid];\n"));
    }
    source.push_str("    bool ok = true;\n");
    source.push_str(&body);
    source.push_str("}\n");

    let sha256 = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    );
    Ok(GeneratedKernel { source, sha256 })
}

/// Generate the complete, deterministic MSL source for an
/// [`KernelShape::SequentialFold`](super::super::cpu_reference::KernelShape::SequentialFold)
/// `ir`: `acc = f(acc, in[i])` for every `i` in ascending order, from an
/// explicit initial value already written into the one-element accumulator
/// buffer by the caller (see [`super::device::MetalSession::dispatch_fold`]).
///
/// A fold is inherently sequential — the CPU reference's own
/// [`session::run_fold`](super::super::cpu_reference::session::run_fold)
/// folds left to right one element at a time, and a checked failure must
/// select the exact same lowest-ordinal element a parallel-per-thread
/// dispatch could not guarantee (a parallel tree reduction can reorder which
/// partial sums overflow first, and reordering unsigned wraparound or
/// division-by-zero across a checked accumulator would silently disagree
/// with the interpreter's own left-to-right order). So this generates one
/// single-thread kernel that loops over every element in a plain `for`,
/// rather than dispatching one GPU thread per element the way
/// [`generate`]'s map kernel does. Reusing [`gen_expr`] unmodified (with
/// [`Codegen::fold`] set) keeps the checked-arithmetic lowering identical
/// between both shapes; only the surrounding prologue/loop/epilogue differ.
pub(crate) fn generate_fold(declaration: &str, ir: &KernelIr) -> Result<GeneratedKernel, String> {
    if ir.params.len() != 2 {
        return Err(format!(
            "a fold kernel takes exactly 2 parameters (accumulator, element), found {}",
            ir.params.len()
        ));
    }
    if ir.params[0] != ir.result {
        return Err("a fold kernel's accumulator parameter must match its result kind".to_owned());
    }
    let accumulator_kind = ir.params[0];
    let element_kind = ir.params[1];
    msl_type(accumulator_kind)?;
    msl_type(element_kind)?;

    let mut cg = Codegen {
        next_tmp: 0,
        slots: vec![None; ir.slots as usize],
        fold: true,
    };
    cg.slots[0] = Some(("slot0".to_owned(), accumulator_kind));
    cg.slots[1] = Some(("slot1".to_owned(), element_kind));

    let mut loop_body = String::new();
    loop_body.push_str("        if (!ok) { break; }\n");
    let element_ty = msl_type(element_kind)?;
    loop_body.push_str(&format!("        {element_ty} slot1 = in0[gid];\n"));
    let (result_var, result_kind) = gen_expr(&mut cg, &ir.body, &mut loop_body)?;
    if result_kind != ir.result {
        return Err("generated result kind disagrees with the checked signature".to_owned());
    }
    loop_body.push_str(&format!("        if (ok) {{ slot0 = {result_var}; }}\n"));

    let accumulator_ty = msl_type(accumulator_kind)?;

    let mut source = String::new();
    source.push_str(PREAMBLE);
    source.push_str(&format!(
        "\n// SEMAPRAX fold kernel `{declaration}`\nkernel void {FUNCTION_NAME}(\n"
    ));
    source.push_str(&format!(
        "    device const {element_ty}* in0 [[buffer(0)]],\n"
    ));
    source.push_str(&format!(
        "    device {accumulator_ty}* acc [[buffer(1)]],\n"
    ));
    source.push_str("    device uint* out_status [[buffer(2)]],\n");
    source.push_str("    device uint* out_invocation [[buffer(3)]],\n");
    source.push_str("    constant uint& element_count [[buffer(4)]])\n{\n");
    source.push_str(&format!("    {accumulator_ty} slot0 = acc[0];\n"));
    source.push_str("    bool ok = true;\n");
    source.push_str("    for (uint gid = 0; gid < element_count; gid++) {\n");
    source.push_str(&loop_body);
    source.push_str("    }\n");
    source.push_str("    if (ok) { acc[0] = slot0; }\n");
    source.push_str("}\n");

    let sha256 = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    );
    Ok(GeneratedKernel { source, sha256 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compute_profile::cpu_reference::session;
    use crate::compute_profile::cpu_reference::KernelShape;

    const SOURCE: &str = r#"
module test.metal_codegen;

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

@id("k.byte_flag")
fn byte_flag(x: i64) -> i64
{
    let positive = x > 0;
    if positive { x } else { -x }
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

    fn lower(declaration: &str) -> KernelIr {
        let ast = crate::parse(SOURCE, "metal-codegen.spx").expect("fixture parses");
        let program = crate::hir::resolve(&ast).expect("fixture resolves");
        let (ir, _fingerprint) = session::bind(
            &program,
            declaration,
            KernelShape::ElementwiseMap { workgroup_size: 4 },
        )
        .expect("fixture kernel is admitted");
        ir
    }

    #[test]
    fn generation_is_deterministic_for_the_same_ir() {
        let ir = lower("k.affine");
        let first = generate("k.affine", &ir).expect("i64 kernel generates");
        let second = generate("k.affine", &ir).expect("i64 kernel generates");
        assert_eq!(first.source, second.source);
        assert_eq!(first.sha256, second.sha256);
    }

    #[test]
    fn generated_source_contains_the_checked_helpers_it_calls() {
        let ir = lower("k.affine");
        let generated = generate("k.affine", &ir).expect("i64 kernel generates");
        assert!(generated.source.contains("checked_mul_i64"));
        assert!(generated.source.contains("checked_sub_i64"));
        assert!(generated.source.contains("out_status[gid]"));
        assert!(generated
            .source
            .contains("if (gid >= element_count) { return; }"));
    }

    #[test]
    fn i32_kernel_uses_the_i32_checked_helpers() {
        let ir = lower("k.halve32");
        let generated = generate("k.halve32", &ir).expect("i32 kernel generates");
        assert!(generated.source.contains("device const int* in0"));
        assert!(generated.source.contains("device int* out"));
        assert!(
            generated.source.contains("checked_sub_i32")
                || generated.source.contains("checked_div_i32")
        );
    }

    #[test]
    fn internal_bool_condition_is_admitted_on_an_i64_signature() {
        let ir = lower("k.byte_flag");
        let generated = generate("k.byte_flag", &ir).expect("bool-flagged i64 kernel generates");
        // The `let positive = x > 0;` binding never gets its own `slot1`
        // declaration (see `Codegen`'s doc comment): it aliases straight to
        // the comparison's own temporary, so a `bool` value is what a
        // `let` of a comparison looks like here.
        assert!(generated.source.contains("bool t"));
        assert!(generated.source.contains("checked_neg_i64"));
    }

    #[test]
    fn mutating_the_ir_changes_the_digest() {
        // The negative-control property this generator must have: two
        // different (but each individually valid) IRs never share a
        // digest, so a stale-artifact check that compares digests actually
        // detects drift instead of vacuously agreeing.
        let affine = generate("k.affine", &lower("k.affine")).expect("generates");
        let halve = generate("k.halve32", &lower("k.halve32")).expect("generates");
        assert_ne!(affine.sha256, halve.sha256);
        assert_ne!(affine.source, halve.source);
    }
}
