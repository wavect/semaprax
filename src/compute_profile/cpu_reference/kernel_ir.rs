//! Lowering of one checked HIR function into the closed CPU-reference kernel
//! IR, its canonical fingerprint, and its checked scalar evaluation.
//!
//! Lowering reads only resolved HIR: parameter and binding identities are
//! [`ValueId`]s, never names, and every node's scalar kind comes from its
//! checked [`ResolvedType`]. A construct outside the closed vocabulary stops
//! lowering with the [`KernelOp`] marker the admission classifier refuses
//! (`FloatArithmetic` for `SPX-GC010`, `HostEffect` for `SPX-GC012`), so the
//! final refusal is always the classifier's own, in its own precedence.
//!
//! Evaluation implements the language's checked integer semantics exactly:
//! overflow, division or remainder by zero, and `MIN / -1` select the same
//! compiler-owned [`StatusCase`] the ordinary interpreter selects. There is
//! no wrapping arithmetic and no floating point.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::ast::{BinaryOp, UnaryOp};
use crate::cleanup_plan::StatusCase;
use crate::compute_profile::classifier::{KernelOp, ScalarType};
use crate::hir::{
    OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedStatement,
    ResolvedType, ValueId,
};

use super::{MAX_KERNEL_IR_DEPTH, MAX_KERNEL_IR_NODES};

/// The admitted kernel element kinds: every checked SEMAPRAX integer scalar
/// plus `bool`. `char` and floating point are outside the kernel vocabulary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ScalarKind {
    I64,
    I32,
    U8,
    Usize,
    Bool,
}

impl ScalarKind {
    pub(super) fn from_type(ty: &ResolvedType) -> Option<Self> {
        match ty {
            ResolvedType::I64 => Some(Self::I64),
            ResolvedType::I32 => Some(Self::I32),
            ResolvedType::U8 => Some(Self::U8),
            ResolvedType::Usize => Some(Self::Usize),
            ResolvedType::Bool => Some(Self::Bool),
            _ => None,
        }
    }

    /// The classifier scalar this kind denotes. `usize` is the
    /// target-independent checked unsigned 64-bit semantic integer.
    pub(super) fn classifier_scalar(self) -> ScalarType {
        match self {
            Self::I64 => ScalarType::I64,
            Self::I32 => ScalarType::I32,
            Self::U8 => ScalarType::U8,
            Self::Usize => ScalarType::U64,
            Self::Bool => ScalarType::Bool,
        }
    }

    pub fn zero(self) -> Scalar {
        match self {
            Self::I64 => Scalar::I64(0),
            Self::I32 => Scalar::I32(0),
            Self::U8 => Scalar::U8(0),
            Self::Usize => Scalar::Usize(0),
            Self::Bool => Scalar::Bool(false),
        }
    }

    fn tag(self) -> u8 {
        match self {
            Self::I64 => 1,
            Self::I32 => 2,
            Self::U8 => 3,
            Self::Usize => 4,
            Self::Bool => 5,
        }
    }
}

/// One kernel element value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scalar {
    I64(i64),
    I32(i32),
    U8(u8),
    Usize(u64),
    Bool(bool),
}

impl Scalar {
    pub fn kind(self) -> ScalarKind {
        match self {
            Self::I64(_) => ScalarKind::I64,
            Self::I32(_) => ScalarKind::I32,
            Self::U8(_) => ScalarKind::U8,
            Self::Usize(_) => ScalarKind::Usize,
            Self::Bool(_) => ScalarKind::Bool,
        }
    }

    fn encode(self, out: &mut Vec<u8>) {
        out.push(self.kind().tag());
        match self {
            Self::I64(value) => out.extend_from_slice(&value.to_le_bytes()),
            Self::I32(value) => out.extend_from_slice(&value.to_le_bytes()),
            Self::U8(value) => out.push(value),
            Self::Usize(value) => out.extend_from_slice(&value.to_le_bytes()),
            Self::Bool(value) => out.push(u8::from(value)),
        }
    }
}

/// One lowered kernel expression. Slots `0..params` hold the parameters in
/// signature order; each immutable `let` takes the next slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum KernelExpr {
    Slot(u32),
    Literal(Scalar),
    Neg(Box<KernelExpr>),
    Not(Box<KernelExpr>),
    Binary {
        op: BinaryOp,
        left: Box<KernelExpr>,
        right: Box<KernelExpr>,
    },
    If {
        condition: Box<KernelExpr>,
        then_branch: Box<KernelExpr>,
        else_branch: Box<KernelExpr>,
    },
    Block {
        lets: Vec<(u32, KernelExpr)>,
        tail: Box<KernelExpr>,
    },
}

/// The lowered body plus its checked signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KernelIr {
    pub(crate) params: Vec<ScalarKind>,
    pub(crate) result: ScalarKind,
    pub(crate) slots: u32,
    pub(crate) body: KernelExpr,
    /// The classifier body ops this lowering observed, in authored order.
    pub(crate) ops: Vec<KernelOp>,
}

/// Why lowering stopped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LowerStop {
    /// A construct the classifier refuses through this body op marker.
    Refused { op: KernelOp, detail: String },
    /// The node or depth bound was exceeded.
    Bound { detail: String },
}

struct Lowerer {
    slots: BTreeMap<ValueId, (u32, ScalarKind)>,
    next_slot: u32,
    nodes: usize,
    ops: Vec<KernelOp>,
}

fn host_effect(detail: impl Into<String>) -> LowerStop {
    LowerStop::Refused {
        op: KernelOp::HostEffect,
        detail: detail.into(),
    }
}

fn float_op(detail: impl Into<String>) -> LowerStop {
    LowerStop::Refused {
        op: KernelOp::FloatArithmetic,
        detail: detail.into(),
    }
}

/// Lower one checked function whose parameters and result have already been
/// admitted as scalar kinds by the caller.
pub(crate) fn lower(
    function: &ResolvedFunction,
    params: &[ScalarKind],
    result: ScalarKind,
) -> Result<KernelIr, LowerStop> {
    if !function.requires.is_empty() || !function.ensures.is_empty() {
        return Err(host_effect("kernel declarations carry no contracts in v1"));
    }
    if function.yields.is_some() {
        return Err(host_effect("a kernel cannot yield"));
    }
    let mut lowerer = Lowerer {
        slots: BTreeMap::new(),
        next_slot: 0,
        nodes: 0,
        ops: Vec::new(),
    };
    for (param, kind) in function.params.iter().zip(params) {
        let slot = lowerer.next_slot;
        lowerer.next_slot += 1;
        lowerer.slots.insert(param.id.clone(), (slot, *kind));
    }
    let (body, kind) = lowerer.lower_expr(&function.body, 0)?;
    if kind != result {
        return Err(host_effect(
            "kernel body kind disagrees with its result type",
        ));
    }
    Ok(KernelIr {
        params: params.to_vec(),
        result,
        slots: lowerer.next_slot,
        body,
        ops: lowerer.ops,
    })
}

impl Lowerer {
    fn lower_expr(
        &mut self,
        expr: &ResolvedExpr,
        depth: usize,
    ) -> Result<(KernelExpr, ScalarKind), LowerStop> {
        if depth >= MAX_KERNEL_IR_DEPTH {
            return Err(LowerStop::Bound {
                detail: format!("expression `{}` is nested too deeply", expr.id.as_str()),
            });
        }
        self.nodes += 1;
        if self.nodes > MAX_KERNEL_IR_NODES {
            return Err(LowerStop::Bound {
                detail: "the body has too many expression nodes".to_owned(),
            });
        }
        if matches!(expr.ty, ResolvedType::F32 | ResolvedType::F64)
            || matches!(
                expr.kind,
                ResolvedExprKind::Float32(_) | ResolvedExprKind::Float64(_)
            )
        {
            return Err(float_op(format!(
                "expression `{}` is floating point",
                expr.id.as_str()
            )));
        }
        let Some(kind) = ScalarKind::from_type(&expr.ty) else {
            return Err(host_effect(format!(
                "expression `{}` has a type outside the kernel scalar vocabulary",
                expr.id.as_str()
            )));
        };
        let lowered = match &expr.kind {
            ResolvedExprKind::Int(value) => KernelExpr::Literal(Scalar::I64(*value)),
            ResolvedExprKind::Int32(value) => KernelExpr::Literal(Scalar::I32(*value)),
            ResolvedExprKind::Uint8(value) => KernelExpr::Literal(Scalar::U8(*value)),
            ResolvedExprKind::Usize(value) => KernelExpr::Literal(Scalar::Usize(*value)),
            ResolvedExprKind::Bool(value) => KernelExpr::Literal(Scalar::Bool(*value)),
            ResolvedExprKind::Place(place) => {
                if !place.projections.is_empty() {
                    return Err(host_effect("a kernel reads no projected place"));
                }
                let Some(&(slot, slot_kind)) = self.slots.get(&place.root) else {
                    return Err(host_effect("a kernel reads only its parameters and lets"));
                };
                if slot_kind != kind {
                    return Err(host_effect("place kind disagrees with its binding"));
                }
                KernelExpr::Slot(slot)
            }
            ResolvedExprKind::Unary { op, value } => {
                let (inner, inner_kind) = self.lower_expr(value, depth + 1)?;
                self.ops.push(KernelOp::IntegerArithmetic);
                match op {
                    UnaryOp::Neg
                        if inner_kind == kind
                            && matches!(kind, ScalarKind::I64 | ScalarKind::I32) =>
                    {
                        KernelExpr::Neg(Box::new(inner))
                    }
                    UnaryOp::Not if inner_kind == ScalarKind::Bool && kind == ScalarKind::Bool => {
                        KernelExpr::Not(Box::new(inner))
                    }
                    _ => return Err(host_effect("unary operator outside the kernel vocabulary")),
                }
            }
            ResolvedExprKind::Binary { op, left, right } => {
                let (left, left_kind) = self.lower_expr(left, depth + 1)?;
                let (right, right_kind) = self.lower_expr(right, depth + 1)?;
                self.ops.push(KernelOp::IntegerArithmetic);
                if left_kind != right_kind || !binary_result_matches(*op, left_kind, kind) {
                    return Err(host_effect("binary operator outside the kernel vocabulary"));
                }
                KernelExpr::Binary {
                    op: *op,
                    left: Box::new(left),
                    right: Box::new(right),
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let (condition, condition_kind) = self.lower_expr(condition, depth + 1)?;
                let (then_branch, then_kind) = self.lower_expr(then_branch, depth + 1)?;
                let (else_branch, else_kind) = self.lower_expr(else_branch, depth + 1)?;
                if condition_kind != ScalarKind::Bool || then_kind != kind || else_kind != kind {
                    return Err(host_effect("if branch kinds disagree"));
                }
                KernelExpr::If {
                    condition: Box::new(condition),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                }
            }
            ResolvedExprKind::Block { statements, tail } => {
                let mut lets = Vec::with_capacity(statements.len());
                for statement in statements {
                    let ResolvedStatement::Let {
                        binding,
                        mutable: false,
                        value,
                        ..
                    } = statement
                    else {
                        return Err(host_effect(
                            "a kernel block admits only immutable `let` statements",
                        ));
                    };
                    if binding.ownership != OwnershipMode::Value {
                        return Err(host_effect("a kernel `let` binds only Copy scalars"));
                    }
                    let (value, value_kind) = self.lower_expr(value, depth + 1)?;
                    if ScalarKind::from_type(&binding.ty) != Some(value_kind) {
                        return Err(host_effect("`let` kind disagrees with its value"));
                    }
                    let slot = self.next_slot;
                    self.next_slot += 1;
                    self.slots.insert(binding.id.clone(), (slot, value_kind));
                    lets.push((slot, value));
                }
                let (tail, tail_kind) = self.lower_expr(tail, depth + 1)?;
                if tail_kind != kind {
                    return Err(host_effect("block tail kind disagrees with the block"));
                }
                KernelExpr::Block {
                    lets,
                    tail: Box::new(tail),
                }
            }
            ResolvedExprKind::Call { .. }
            | ResolvedExprKind::Invoke { .. }
            | ResolvedExprKind::NativeRustImportCall(_)
            | ResolvedExprKind::HostCommandCall(_) => {
                return Err(host_effect(format!(
                    "expression `{}` calls out of the kernel body",
                    expr.id.as_str()
                )))
            }
            _ => {
                return Err(host_effect(format!(
                    "expression `{}` is outside the kernel body vocabulary",
                    expr.id.as_str()
                )))
            }
        };
        Ok((lowered, kind))
    }
}

fn binary_result_matches(op: BinaryOp, operand: ScalarKind, result: ScalarKind) -> bool {
    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
            operand != ScalarKind::Bool && result == operand
        }
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            operand != ScalarKind::Bool && result == ScalarKind::Bool
        }
        BinaryOp::Eq | BinaryOp::Ne => result == ScalarKind::Bool,
        BinaryOp::And | BinaryOp::Or => operand == ScalarKind::Bool && result == ScalarKind::Bool,
    }
}

/// Why evaluation of one invocation stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EvalStop {
    /// The checked operation selected this compiler-owned status.
    Status(StatusCase),
    /// An impossible post-lowering state; never reached on lowered IR.
    Guard,
}

/// Evaluate one invocation with its arguments.
pub(crate) fn evaluate(ir: &KernelIr, arguments: &[Scalar]) -> Result<Scalar, EvalStop> {
    if arguments.len() != ir.params.len()
        || arguments
            .iter()
            .zip(&ir.params)
            .any(|(argument, kind)| argument.kind() != *kind)
    {
        return Err(EvalStop::Guard);
    }
    let mut slots = vec![None; ir.slots as usize];
    for (slot, argument) in slots.iter_mut().zip(arguments) {
        *slot = Some(*argument);
    }
    eval(&ir.body, &mut slots)
}

fn eval(expr: &KernelExpr, slots: &mut [Option<Scalar>]) -> Result<Scalar, EvalStop> {
    match expr {
        KernelExpr::Slot(slot) => slots
            .get(*slot as usize)
            .copied()
            .flatten()
            .ok_or(EvalStop::Guard),
        KernelExpr::Literal(value) => Ok(*value),
        KernelExpr::Neg(inner) => match eval(inner, slots)? {
            Scalar::I64(value) => value
                .checked_neg()
                .map(Scalar::I64)
                .ok_or(EvalStop::Status(StatusCase::NegationOverflow)),
            Scalar::I32(value) => value
                .checked_neg()
                .map(Scalar::I32)
                .ok_or(EvalStop::Status(StatusCase::NegationOverflow)),
            _ => Err(EvalStop::Guard),
        },
        KernelExpr::Not(inner) => match eval(inner, slots)? {
            Scalar::Bool(value) => Ok(Scalar::Bool(!value)),
            _ => Err(EvalStop::Guard),
        },
        KernelExpr::Binary { op, left, right } => {
            let lhs = eval(left, slots)?;
            match (op, lhs) {
                (BinaryOp::And, Scalar::Bool(false)) => return Ok(Scalar::Bool(false)),
                (BinaryOp::Or, Scalar::Bool(true)) => return Ok(Scalar::Bool(true)),
                (BinaryOp::And | BinaryOp::Or, Scalar::Bool(_)) => {
                    return match eval(right, slots)? {
                        value @ Scalar::Bool(_) => Ok(value),
                        _ => Err(EvalStop::Guard),
                    };
                }
                (BinaryOp::And | BinaryOp::Or, _) => return Err(EvalStop::Guard),
                _ => {}
            }
            let rhs = eval(right, slots)?;
            combine(*op, lhs, rhs)
        }
        KernelExpr::If {
            condition,
            then_branch,
            else_branch,
        } => match eval(condition, slots)? {
            Scalar::Bool(true) => eval(then_branch, slots),
            Scalar::Bool(false) => eval(else_branch, slots),
            _ => Err(EvalStop::Guard),
        },
        KernelExpr::Block { lets, tail } => {
            for (slot, value) in lets {
                let value = eval(value, slots)?;
                *slots.get_mut(*slot as usize).ok_or(EvalStop::Guard)? = Some(value);
            }
            eval(tail, slots)
        }
    }
}

macro_rules! checked_integer {
    ($op:expr, $a:expr, $b:expr, $wrap:path, $signed_min:expr) => {{
        let (a, b) = ($a, $b);
        let status = |case| Err(EvalStop::Status(case));
        match $op {
            BinaryOp::Add => a
                .checked_add(b)
                .map_or_else(|| status(StatusCase::AddOverflow), |v| Ok($wrap(v))),
            BinaryOp::Sub => a
                .checked_sub(b)
                .map_or_else(|| status(StatusCase::SubOverflow), |v| Ok($wrap(v))),
            BinaryOp::Mul => a
                .checked_mul(b)
                .map_or_else(|| status(StatusCase::MulOverflow), |v| Ok($wrap(v))),
            BinaryOp::Div => {
                if b == 0 {
                    status(StatusCase::DivisionByZero)
                } else if $signed_min == Some(a) && b.checked_neg() == Some(1) {
                    status(StatusCase::DivisionOverflow)
                } else {
                    Ok($wrap(a / b))
                }
            }
            BinaryOp::Rem => {
                if b == 0 {
                    status(StatusCase::RemainderByZero)
                } else if $signed_min == Some(a) && b.checked_neg() == Some(1) {
                    status(StatusCase::RemainderOverflow)
                } else {
                    Ok($wrap(a % b))
                }
            }
            BinaryOp::Eq => Ok(Scalar::Bool(a == b)),
            BinaryOp::Ne => Ok(Scalar::Bool(a != b)),
            BinaryOp::Lt => Ok(Scalar::Bool(a < b)),
            BinaryOp::Le => Ok(Scalar::Bool(a <= b)),
            BinaryOp::Gt => Ok(Scalar::Bool(a > b)),
            BinaryOp::Ge => Ok(Scalar::Bool(a >= b)),
            BinaryOp::And | BinaryOp::Or => Err(EvalStop::Guard),
        }
    }};
}

fn combine(op: BinaryOp, lhs: Scalar, rhs: Scalar) -> Result<Scalar, EvalStop> {
    match (lhs, rhs) {
        (Scalar::I64(a), Scalar::I64(b)) => checked_integer!(op, a, b, Scalar::I64, Some(i64::MIN)),
        (Scalar::I32(a), Scalar::I32(b)) => checked_integer!(op, a, b, Scalar::I32, Some(i32::MIN)),
        (Scalar::U8(a), Scalar::U8(b)) => checked_integer!(op, a, b, Scalar::U8, None::<u8>),
        (Scalar::Usize(a), Scalar::Usize(b)) => {
            checked_integer!(op, a, b, Scalar::Usize, None::<u64>)
        }
        (Scalar::Bool(a), Scalar::Bool(b)) => match op {
            BinaryOp::Eq => Ok(Scalar::Bool(a == b)),
            BinaryOp::Ne => Ok(Scalar::Bool(a != b)),
            _ => Err(EvalStop::Guard),
        },
        _ => Err(EvalStop::Guard),
    }
}

fn encode_expr(expr: &KernelExpr, out: &mut Vec<u8>) {
    match expr {
        KernelExpr::Slot(slot) => {
            out.push(1);
            out.extend_from_slice(&slot.to_le_bytes());
        }
        KernelExpr::Literal(value) => {
            out.push(2);
            value.encode(out);
        }
        KernelExpr::Neg(inner) => {
            out.push(3);
            encode_expr(inner, out);
        }
        KernelExpr::Not(inner) => {
            out.push(4);
            encode_expr(inner, out);
        }
        KernelExpr::Binary { op, left, right } => {
            out.push(5);
            out.extend_from_slice(op.text().as_bytes());
            out.push(0);
            encode_expr(left, out);
            encode_expr(right, out);
        }
        KernelExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            out.push(6);
            encode_expr(condition, out);
            encode_expr(then_branch, out);
            encode_expr(else_branch, out);
        }
        KernelExpr::Block { lets, tail } => {
            out.push(7);
            out.extend_from_slice(&(lets.len() as u64).to_le_bytes());
            for (slot, value) in lets {
                out.extend_from_slice(&slot.to_le_bytes());
                encode_expr(value, out);
            }
            encode_expr(tail, out);
        }
    }
}

/// The canonical SHA-256 binding of one lowered kernel to its schema,
/// declaration identity, shape, signature, and body, as lowercase hex.
pub(crate) fn fingerprint(declaration: &str, shape_bytes: &[u8], ir: &KernelIr) -> String {
    let mut bytes = Vec::new();
    for part in [
        super::CPU_REFERENCE_SCHEMA.as_bytes(),
        declaration.as_bytes(),
    ] {
        bytes.extend_from_slice(&(part.len() as u64).to_le_bytes());
        bytes.extend_from_slice(part);
    }
    bytes.extend_from_slice(&(shape_bytes.len() as u64).to_le_bytes());
    bytes.extend_from_slice(shape_bytes);
    bytes.extend_from_slice(&(ir.params.len() as u64).to_le_bytes());
    for kind in &ir.params {
        bytes.push(kind.tag());
    }
    bytes.push(ir.result.tag());
    bytes.extend_from_slice(&ir.slots.to_le_bytes());
    encode_expr(&ir.body, &mut bytes);
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(&bytes)))
}
