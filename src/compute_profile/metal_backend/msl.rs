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
    ulong product = ua * ub;
    if (product / ua != ub) { return 3u; }
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
"#;

/// One deterministically generated kernel: its complete MSL source and the
/// SHA-256 digest [`super::device::MetalKernelArtifact`] binds to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedKernel {
    pub(crate) source: String,
    pub(crate) sha256: String,
}

fn msl_type(kind: ScalarKind) -> Result<&'static str, String> {
    match kind {
        ScalarKind::I64 => Ok("long"),
        ScalarKind::I32 => Ok("int"),
        ScalarKind::Bool => Ok("bool"),
        ScalarKind::U8 | ScalarKind::Usize => Err(format!("{kind:?} has no Metal v1 lowering")),
    }
}

fn literal_text(scalar: Scalar) -> Result<String, String> {
    match scalar {
        Scalar::I64(value) if value == i64::MIN => Ok("(-9223372036854775807LL - 1LL)".to_owned()),
        Scalar::I64(value) => Ok(format!("{value}LL")),
        Scalar::I32(value) if value == i32::MIN => Ok("((int)(-2147483647 - 1))".to_owned()),
        Scalar::I32(value) => Ok(format!("{value}")),
        Scalar::Bool(value) => Ok(if value { "true" } else { "false" }.to_owned()),
        other => Err(format!(
            "{:?} literal has no Metal v1 lowering",
            other.kind()
        )),
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
        _ => {
            return Err(format!(
                "checked arithmetic has no Metal v1 lowering for {kind:?}"
            ))
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

/// Per-generation state: fresh temporary names and the inferred kind of
/// every slot observed so far (parameters up front, `let`s as their
/// enclosing [`KernelExpr::Block`] is reached — always before any reference,
/// since lowering never admits a forward reference).
struct Codegen {
    next_tmp: u32,
    slot_kinds: Vec<Option<ScalarKind>>,
}

impl Codegen {
    fn fresh(&mut self) -> String {
        self.next_tmp += 1;
        format!("t{}", self.next_tmp)
    }
}

/// One piece of a straight-line sequence, generated left to right but
/// assembled right to left by [`fold`] (see its doc comment for why).
enum Step {
    /// Ordinary code with no failure mode: inserted verbatim, nothing to
    /// close.
    Flat(String),
    /// A checked operation's declaration and test, ending in an *open*
    /// `else {` that [`fold`] must close after nesting everything that
    /// follows inside it.
    Guarded(String),
}

/// Assemble a left-to-right sequence of [`Step`]s ending in `tail` into one
/// self-contained code fragment, folding from the last step to the first.
///
/// A checked operation's failure must skip every statement that would
/// otherwise follow it in its own scope, all the way to the end of the
/// kernel invocation (so a failed invocation never writes its output). An
/// earlier version of this generator did that with a bare `out_status[gid]
/// = st; return;` inside the guard, leaving the rest of the *current*
/// scope's statements — and, critically, any `if`/`else` reached later in
/// that same scope — textually **after** that `return`. That shape
/// compiled and ran correctly for a lone checked operation, or for checked
/// operations nested only inside `if`/`else` branches, but reproducibly
/// made the system Metal compiler service fail
/// (`XPC_ERROR_CONNECTION_INTERRUPTED`, confirmed identical across many
/// runs on real hardware) for the one fixture shape that combines both: a
/// `let`-bound checked value whose scope's *next* statement is an `if`.
/// Bisection isolated exactly that combination — a trivial-branched `if`
/// after such a `let` reproduced it just as reliably as the original
/// nested-arithmetic kernel, while the same nested arithmetic with no
/// preceding `let` did not.
///
/// This generator therefore never emits a bare `return` for a checked
/// failure (only the one, always-first, always-isolated
/// `if (gid >= element_count) { return; }` bounds guard remains — proven
/// fine on every kernel, including the one that fails here, so it was
/// never implicated). Instead every checked failure's `else` branch
/// physically *contains* everything that follows it, built by folding this
/// list from the end backward: each [`Step::Guarded`] wraps the
/// (already-assembled) rest of the sequence inside its `else { … }`, so a
/// failing invocation falls through to its own closing brace and the
/// kernel's implicit `void` return — the output is skipped structurally,
/// not by jumping out of it.
fn fold(steps: Vec<Step>, tail: &str) -> String {
    let mut rest = tail.to_owned();
    for step in steps.into_iter().rev() {
        rest = match step {
            Step::Flat(code) => code + &rest,
            Step::Guarded(prologue) => prologue + &rest + "    }\n",
        };
    }
    rest
}

fn gen_expr(
    cg: &mut Codegen,
    expr: &KernelExpr,
    steps: &mut Vec<Step>,
) -> Result<(String, ScalarKind), String> {
    match expr {
        KernelExpr::Slot(slot) => {
            let kind = cg
                .slot_kinds
                .get(*slot as usize)
                .copied()
                .flatten()
                .ok_or_else(|| format!("slot {slot} read before it is assigned"))?;
            Ok((format!("slot{slot}"), kind))
        }
        KernelExpr::Literal(scalar) => Ok((literal_text(*scalar)?, scalar.kind())),
        KernelExpr::Neg(inner) => {
            let (value, kind) = gen_expr(cg, inner, steps)?;
            let ty = msl_type(kind)?;
            let function = match kind {
                ScalarKind::I64 => "checked_neg_i64",
                ScalarKind::I32 => "checked_neg_i32",
                _ => return Err(format!("negation has no Metal v1 lowering for {kind:?}")),
            };
            let name = cg.fresh();
            steps.push(Step::Guarded(format!(
                "    {ty} {name};\n    uint st_{name} = {function}({value}, &{name});\n    if (st_{name}) {{ out_status[gid] = st_{name}; }} else {{\n"
            )));
            Ok((name, kind))
        }
        KernelExpr::Not(inner) => {
            let (value, kind) = gen_expr(cg, inner, steps)?;
            if kind != ScalarKind::Bool {
                return Err("`!` has no Metal v1 lowering for a non-bool operand".to_owned());
            }
            let name = cg.fresh();
            steps.push(Step::Flat(format!("    bool {name} = !{value};\n")));
            Ok((name, ScalarKind::Bool))
        }
        KernelExpr::Binary { op, left, right } => gen_binary(cg, *op, left, right, steps),
        KernelExpr::If {
            condition,
            then_branch,
            else_branch,
        } => gen_if(cg, condition, then_branch, else_branch, steps),
        KernelExpr::Block { lets, tail } => {
            for (slot, value) in lets {
                let (value_name, kind) = gen_expr(cg, value, steps)?;
                let ty = msl_type(kind)?;
                steps.push(Step::Flat(format!("    {ty} slot{slot} = {value_name};\n")));
                cg.slot_kinds[*slot as usize] = Some(kind);
            }
            gen_expr(cg, tail, steps)
        }
    }
}

fn gen_binary(
    cg: &mut Codegen,
    op: BinaryOp,
    left: &KernelExpr,
    right: &KernelExpr,
    steps: &mut Vec<Step>,
) -> Result<(String, ScalarKind), String> {
    if matches!(op, BinaryOp::And | BinaryOp::Or) {
        let (lvar, lkind) = gen_expr(cg, left, steps)?;
        if lkind != ScalarKind::Bool {
            return Err("`&&`/`||` need bool operands".to_owned());
        }
        let name = cg.fresh();
        let mut right_steps = Vec::new();
        let (rvar, rkind) = gen_expr(cg, right, &mut right_steps)?;
        if rkind != ScalarKind::Bool {
            return Err("`&&`/`||` need bool operands".to_owned());
        }
        let (short_circuit, taken) = match op {
            BinaryOp::And => ("false", "!"),
            BinaryOp::Or => ("true", ""),
            _ => unreachable!(),
        };
        let right_code = fold(right_steps, &format!("        {name} = {rvar};\n"));
        steps.push(Step::Flat(format!(
            "    bool {name};\n    if ({taken}{lvar}) {{ {name} = {short_circuit}; }} else {{\n{right_code}    }}\n"
        )));
        return Ok((name, ScalarKind::Bool));
    }

    let (lvar, lkind) = gen_expr(cg, left, steps)?;
    let (rvar, rkind) = gen_expr(cg, right, steps)?;
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
        steps.push(Step::Guarded(format!(
            "    {ty} {name};\n    uint st_{name} = {function}({lvar}, {rvar}, &{name});\n    if (st_{name}) {{ out_status[gid] = st_{name}; }} else {{\n"
        )));
        Ok((name, lkind))
    } else {
        let symbol = comparison_symbol(op);
        let name = cg.fresh();
        steps.push(Step::Flat(format!(
            "    bool {name} = ({lvar} {symbol} {rvar});\n"
        )));
        Ok((name, ScalarKind::Bool))
    }
}

fn gen_if(
    cg: &mut Codegen,
    condition: &KernelExpr,
    then_branch: &KernelExpr,
    else_branch: &KernelExpr,
    steps: &mut Vec<Step>,
) -> Result<(String, ScalarKind), String> {
    let (cvar, ckind) = gen_expr(cg, condition, steps)?;
    if ckind != ScalarKind::Bool {
        return Err("`if` condition must be bool".to_owned());
    }
    let mut then_steps = Vec::new();
    let (then_var, then_kind) = gen_expr(cg, then_branch, &mut then_steps)?;
    let mut else_steps = Vec::new();
    let (else_var, else_kind) = gen_expr(cg, else_branch, &mut else_steps)?;
    if then_kind != else_kind {
        return Err(format!(
            "`if` branches disagree in kind: {then_kind:?} vs {else_kind:?}"
        ));
    }
    let ty = msl_type(then_kind)?;
    let name = cg.fresh();
    let then_code = fold(then_steps, &format!("        {name} = {then_var};\n"));
    let else_code = fold(else_steps, &format!("        {name} = {else_var};\n"));
    // Both branches are already fully self-contained (closed) fragments, so
    // this whole `if`/`else` is itself a `Step::Flat`: nothing needs to
    // nest *around* it, `name` is simply available to whatever follows.
    steps.push(Step::Flat(format!(
        "    {ty} {name};\n    if ({cvar}) {{\n{then_code}    }} else {{\n{else_code}    }}\n"
    )));
    Ok((name, then_kind))
}

/// Generate the complete, deterministic MSL source for `ir`, bound to
/// `declaration` only as a source comment (never a semantic input).
///
/// Refuses (returns `Err`) any kind this v1 generator has no lowering for:
/// a `U8`/`Usize` parameter, result, or internal literal. The caller is
/// expected to have already refused those through the ordinary
/// `SPX-GC0xx` vocabulary before reaching generation; this is defense in
/// depth, not the primary admission path.
pub(crate) fn generate(declaration: &str, ir: &KernelIr) -> Result<GeneratedKernel, String> {
    for kind in &ir.params {
        if !matches!(kind, ScalarKind::I64 | ScalarKind::I32) {
            return Err(format!(
                "parameter kind {kind:?} has no Metal v1 buffer lowering"
            ));
        }
    }
    if !matches!(ir.result, ScalarKind::I64 | ScalarKind::I32) {
        return Err(format!(
            "result kind {:?} has no Metal v1 buffer lowering",
            ir.result
        ));
    }

    let mut cg = Codegen {
        next_tmp: 0,
        slot_kinds: vec![None; ir.slots as usize],
    };
    for (slot, kind) in ir.params.iter().enumerate() {
        cg.slot_kinds[slot] = Some(*kind);
    }

    let mut steps = Vec::new();
    let (result_var, result_kind) = gen_expr(&mut cg, &ir.body, &mut steps)?;
    if result_kind != ir.result {
        return Err("generated result kind disagrees with the checked signature".to_owned());
    }
    let body = fold(steps, &format!("    out[gid] = {result_var};\n"));

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
    source.push_str(&body);
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
        assert!(generated.source.contains("bool slot1"));
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
