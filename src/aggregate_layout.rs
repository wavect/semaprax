//! Deterministic, target-specific layouts for executable aggregate records.
//!
//! Backends consume this module instead of independently reconstructing record
//! offsets.  The constructor and validator are deliberately separate: a
//! backend may only use a layout after it has survived exact reconstruction
//! from resolved HIR.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedFieldDeclaration, ResolvedProgram,
    ResolvedResourceDropKind, ResolvedStatement, ResolvedType, ResolvedTypeDeclaration,
    ResolvedTypeDeclarationKind,
};

const LAYOUT_DIGEST_DOMAIN: &[u8] = b"semaprax.aggregate-layout.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AggregateTarget {
    Native64,
    Wasm32,
}

impl AggregateTarget {
    fn tag(self) -> u8 {
        match self {
            Self::Native64 => 1,
            Self::Wasm32 => 2,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AggregateFieldLayout {
    pub(crate) field: DeclarationId,
    pub(crate) ty: ResolvedType,
    /// Physical layout never implies semantic Copy. In particular, `Bytes`
    /// is one owned carrier whose bits may only move under cleanup-plan
    /// authority even though its target representation has a fixed size.
    pub(crate) value_kind: AggregateFieldValueKind,
    pub(crate) offset: u32,
    pub(crate) size: u32,
    pub(crate) align: u32,
    nested_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AggregateFieldValueKind {
    Copy,
    OwnedBytes,
    Resource,
    Aggregate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AggregateLayout {
    pub(crate) target: AggregateTarget,
    pub(crate) record: DeclarationId,
    pub(crate) instance: ResolvedType,
    pub(crate) size: u32,
    pub(crate) align: u32,
    pub(crate) fields: Vec<AggregateFieldLayout>,
    digest: [u8; 32],
}

impl AggregateLayout {
    /// Computes the one canonical layout for `record` and `target`.
    pub(crate) fn for_record(
        program: &ResolvedProgram,
        target: AggregateTarget,
        record: &DeclarationId,
    ) -> Result<Self, Diagnostic> {
        Self::for_type(
            program,
            target,
            &ResolvedType::Nominal {
                declaration: record.clone(),
                arguments: Vec::new(),
            },
        )
    }

    /// Computes the canonical layout for one exact concrete record instance.
    pub(crate) fn for_type(
        program: &ResolvedProgram,
        target: AggregateTarget,
        instance: &ResolvedType,
    ) -> Result<Self, Diagnostic> {
        let ResolvedType::Nominal {
            declaration: record,
            arguments,
        } = instance
        else {
            return Err(layout_error("record instance is not nominal"));
        };
        let mut visiting = BTreeSet::new();
        let value = layout_nominal(program, target, record, arguments, &mut visiting)?;
        let ValueLayoutKind::Record { fields } = value.kind else {
            return Err(layout_error(format!("`{record}` is not a record")));
        };
        Ok(Self {
            target,
            record: record.clone(),
            instance: instance.clone(),
            size: value.size,
            align: value.align,
            fields,
            digest: value.digest,
        })
    }

    /// Reconstructs this layout from HIR and rejects any changed byte.
    pub(crate) fn validate(&self, program: &ResolvedProgram) -> Result<(), Diagnostic> {
        let expected = Self::for_type(program, self.target, &self.instance)?;
        if *self != expected {
            return Err(layout_error(format!(
                "record `{}` layout is not the canonical {:?} layout",
                self.record, self.target
            )));
        }
        Ok(())
    }

    pub(crate) fn field(&self, field: &DeclarationId) -> Option<&AggregateFieldLayout> {
        self.fields.iter().find(|item| item.field == *field)
    }

    #[cfg(any(test, feature = "unstable-wit-component-harness"))]
    pub(crate) const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    #[cfg(test)]
    fn digest_hex(&self) -> String {
        let mut output = String::with_capacity(64);
        for byte in self.digest {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AggregateLayoutCache {
    target: AggregateTarget,
    layouts: BTreeMap<ResolvedType, AggregateLayout>,
}

impl AggregateLayoutCache {
    pub(crate) fn build(
        program: &ResolvedProgram,
        target: AggregateTarget,
    ) -> Result<Self, Diagnostic> {
        let mut instances = BTreeSet::new();
        for function in &program.functions {
            for parameter in &function.params {
                collect_record_type(program, &parameter.ty, &mut instances)?;
            }
            collect_record_type(program, &function.return_type, &mut instances)?;
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                collect_expr_record_types(program, expression, &mut instances)?;
            }
        }
        let mut layouts = BTreeMap::new();
        for instance in instances {
            let layout = match &instance {
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } if arguments.is_empty() => {
                    AggregateLayout::for_record(program, target, declaration)?
                }
                _ => AggregateLayout::for_type(program, target, &instance)?,
            };
            layout.validate(program)?;
            if layouts.insert(instance, layout).is_some() {
                return Err(layout_error("duplicate concrete record instance"));
            }
        }
        Ok(Self { target, layouts })
    }

    pub(crate) fn layout(&self, instance: &ResolvedType) -> Result<&AggregateLayout, Diagnostic> {
        self.layouts.get(instance).ok_or_else(|| {
            layout_error(format!(
                "missing {:?} layout for concrete record `{}`",
                self.target,
                instance.identity_key()
            ))
        })
    }

    pub(crate) fn layouts(&self) -> impl ExactSizeIterator<Item = &AggregateLayout> {
        self.layouts.values()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValueLayout {
    size: u32,
    align: u32,
    digest: [u8; 32],
    kind: ValueLayoutKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ValueLayoutKind {
    Scalar,
    OwnedBytes,
    Resource,
    Record { fields: Vec<AggregateFieldLayout> },
}

impl ValueLayoutKind {
    const fn field_kind(&self) -> AggregateFieldValueKind {
        match self {
            Self::Scalar => AggregateFieldValueKind::Copy,
            Self::OwnedBytes => AggregateFieldValueKind::OwnedBytes,
            Self::Resource => AggregateFieldValueKind::Resource,
            Self::Record { .. } => AggregateFieldValueKind::Aggregate,
        }
    }
}

fn layout_type(
    program: &ResolvedProgram,
    target: AggregateTarget,
    ty: &ResolvedType,
    visiting: &mut BTreeSet<String>,
) -> Result<ValueLayout, Diagnostic> {
    match ty {
        ResolvedType::Unit => Err(layout_error("unit has no aggregate value layout")),
        ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool => {
            let (size, align) = scalar_size_align(target, ty)?;
            scalar_layout(target, ty, size, align)
        }
        ResolvedType::ArrayU8(length) => Ok(ValueLayout {
            size: *length,
            align: 1,
            digest: digest_value(target, ty, *length, 1, &[]),
            kind: ValueLayoutKind::Scalar,
        }),
        ResolvedType::String => Err(layout_error(
            "owned string values have no aggregate value layout in v1",
        )),
        ResolvedType::Bytes => {
            let (size, align) = owned_bytes_size_align(target);
            Ok(ValueLayout {
                size,
                align,
                digest: digest_value(target, ty, size, align, &[]),
                kind: ValueLayoutKind::OwnedBytes,
            })
        }
        ResolvedType::Str => Err(layout_error(
            "borrowed string views have no aggregate value layout",
        )),
        ResolvedType::SliceU8 => Err(layout_error(
            "borrowed byte-slice views have no aggregate value layout",
        )),
        ResolvedType::TypeParameter { .. } => Err(layout_error(
            "generic aggregate layouts are outside executable records v1",
        )),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => layout_nominal(program, target, declaration, arguments, visiting),
    }
}

/// Exact physical carrier used by the existing owned-byte runtimes. This is
/// deliberately separate from `scalar_size_align`: fixed representation does
/// not make an owned value Copy.
pub(crate) const fn owned_bytes_size_align(target: AggregateTarget) -> (u32, u32) {
    match target {
        // `spx_bytes_v1` is `{ uint8_t *ptr; uint64_t len; }` on Native64.
        AggregateTarget::Native64 => (16, 8),
        // Wasm represents one authenticated owned-byte token in an `i64`.
        AggregateTarget::Wasm32 => (8, 8),
    }
}

pub(crate) fn scalar_size_align(
    target: AggregateTarget,
    ty: &ResolvedType,
) -> Result<(u32, u32), Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok((8, 8)),
        ResolvedType::I32 => Ok((4, 4)),
        ResolvedType::Char => Ok((4, 4)),
        ResolvedType::U8 => match target {
            AggregateTarget::Native64 => Ok((1, 1)),
            AggregateTarget::Wasm32 => Ok((4, 4)),
        },
        ResolvedType::Usize => Ok((8, 8)),
        ResolvedType::F32 => Ok((4, 4)),
        ResolvedType::F64 => Ok((8, 8)),
        ResolvedType::Bool => match target {
            AggregateTarget::Native64 => Ok((1, 1)),
            AggregateTarget::Wasm32 => Ok((4, 4)),
        },
        _ => Err(layout_error(format!(
            "type `{}` is not a scalar aggregate-layout value",
            ty.identity_key()
        ))),
    }
}

fn scalar_layout(
    target: AggregateTarget,
    ty: &ResolvedType,
    size: u32,
    align: u32,
) -> Result<ValueLayout, Diagnostic> {
    Ok(ValueLayout {
        size,
        align,
        digest: digest_value(target, ty, size, align, &[]),
        kind: ValueLayoutKind::Scalar,
    })
}

fn layout_nominal(
    program: &ResolvedProgram,
    target: AggregateTarget,
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
    visiting: &mut BTreeSet<String>,
) -> Result<ValueLayout, Diagnostic> {
    let item = unique_type(program, declaration)?;
    match &item.kind {
        ResolvedTypeDeclarationKind::Resource { drop } => {
            if !arguments.is_empty() {
                return Err(layout_error(format!(
                    "resource `{declaration}` cannot take generic arguments"
                )));
            }
            if !matches!(drop.kind, ResolvedResourceDropKind::Trivial) {
                return Err(layout_error(format!(
                    "resource `{declaration}` is not direct-trivial"
                )));
            }
            let (size, align) = match target {
                AggregateTarget::Native64 => (8, 8),
                AggregateTarget::Wasm32 => (4, 4),
            };
            let ty = ResolvedType::Nominal {
                declaration: declaration.clone(),
                arguments: Vec::new(),
            };
            Ok(ValueLayout {
                size,
                align,
                digest: digest_value(target, &ty, size, align, &[]),
                kind: ValueLayoutKind::Resource,
            })
        }
        ResolvedTypeDeclarationKind::Record { fields }
        | ResolvedTypeDeclarationKind::Class { fields, .. } => {
            if arguments.len() != item.type_parameters.len()
                || arguments
                    .iter()
                    .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
            {
                return Err(layout_error(format!(
                    "record `{declaration}` has invalid concrete arguments"
                )));
            }
            let instance = ResolvedType::Nominal {
                declaration: declaration.clone(),
                arguments: arguments.to_vec(),
            };
            let instance_key = instance.identity_key();
            if !visiting.insert(instance_key.clone()) {
                return Err(layout_error(format!(
                    "record instance `{instance_key}` has a recursive by-value layout"
                )));
            }
            let result = layout_record(program, target, declaration, arguments, fields, visiting);
            visiting.remove(&instance_key);
            result
        }
        ResolvedTypeDeclarationKind::Variant { .. } => Err(layout_error(format!(
            "variant `{declaration}` requires the variant-layout profile"
        ))),
    }
}

fn layout_record(
    program: &ResolvedProgram,
    target: AggregateTarget,
    record: &DeclarationId,
    arguments: &[ResolvedType],
    declarations: &[ResolvedFieldDeclaration],
    visiting: &mut BTreeSet<String>,
) -> Result<ValueLayout, Diagnostic> {
    let mut seen = BTreeSet::new();
    // C11 has no zero-sized object representation and Wasm frame slots must
    // not alias merely because a record has no fields. Freeze one inert byte
    // for the empty product on every target; non-empty records retain their
    // ordinary declaration-ordered layout below.
    let mut offset = u32::from(declarations.is_empty());
    let mut record_align = 1_u32;
    let mut fields = Vec::with_capacity(declarations.len());

    for (ordinal, declaration) in declarations.iter().enumerate() {
        let expected_index = u32::try_from(ordinal)
            .map_err(|_| layout_error(format!("record `{record}` has too many fields")))?;
        if declaration.index != expected_index {
            return Err(layout_error(format!(
                "record `{record}` field indices are not declaration-order canonical"
            )));
        }
        if !seen.insert(declaration.id.clone()) {
            return Err(layout_error(format!(
                "record `{record}` repeats field `{}`",
                declaration.id
            )));
        }

        let field_ty = crate::hir::substitute_type(&declaration.ty, record, arguments)?;
        let value = layout_type(program, target, &field_ty, visiting)?;
        let value_kind = value.kind.field_kind();
        offset = align_up(offset, value.align)?;
        let end = offset
            .checked_add(value.size)
            .ok_or_else(|| layout_error(format!("record `{record}` layout overflows u32")))?;
        fields.push(AggregateFieldLayout {
            field: declaration.id.clone(),
            ty: field_ty,
            value_kind,
            offset,
            size: value.size,
            align: value.align,
            nested_digest: value.digest,
        });
        offset = end;
        record_align = record_align.max(value.align);
    }

    let size = align_up(offset, record_align)?;
    let ty = ResolvedType::Nominal {
        declaration: record.clone(),
        arguments: arguments.to_vec(),
    };
    let field_digests = fields
        .iter()
        .map(|field| field.nested_digest)
        .collect::<Vec<_>>();
    Ok(ValueLayout {
        size,
        align: record_align,
        digest: digest_value_with_fields(target, &ty, size, record_align, &fields, &field_digests),
        kind: ValueLayoutKind::Record { fields },
    })
}

fn unique_type<'a>(
    program: &'a ResolvedProgram,
    declaration: &DeclarationId,
) -> Result<&'a ResolvedTypeDeclaration, Diagnostic> {
    let mut matches = program.types.iter().filter(|item| item.id == *declaration);
    let item = matches
        .next()
        .ok_or_else(|| layout_error(format!("unknown aggregate type `{declaration}`")))?;
    if matches.next().is_some() {
        return Err(layout_error(format!(
            "aggregate type `{declaration}` is duplicated"
        )));
    }
    Ok(item)
}

fn collect_record_type(
    program: &ResolvedProgram,
    ty: &ResolvedType,
    instances: &mut BTreeSet<ResolvedType>,
) -> Result<(), Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(());
    };
    for argument in arguments {
        collect_record_type(program, argument, instances)?;
    }
    let item = unique_type(program, declaration)?;
    if let ResolvedTypeDeclarationKind::Record { fields }
    | ResolvedTypeDeclarationKind::Class { fields, .. } = &item.kind
    {
        if instances.insert(ty.clone()) {
            for field in fields {
                let field_ty = crate::hir::substitute_type(&field.ty, declaration, arguments)?;
                collect_record_type(program, &field_ty, instances)?;
            }
        }
    }
    Ok(())
}

fn collect_expr_record_types(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    instances: &mut BTreeSet<ResolvedType>,
) -> Result<(), Diagnostic> {
    enum Work<'a> {
        Expression(&'a ResolvedExpr),
        Type(&'a ResolvedType),
    }

    let mut pending = vec![Work::Expression(expression)];
    while let Some(work) = pending.pop() {
        let Work::Expression(expression) = work else {
            let Work::Type(ty) = work else { unreachable!() };
            collect_record_type(program, ty, instances)?;
            continue;
        };
        collect_record_type(program, &expression.ty, instances)?;
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => {
                pending.extend(args.iter().rev().map(Work::Expression));
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                pending.extend(call.args.iter().rev().map(Work::Expression));
            }
            ResolvedExprKind::HostCommandCall(call) => {
                pending.extend(call.args.iter().rev().map(Work::Expression));
            }
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => {
                pending.push(Work::Expression(end));
                pending.push(Work::Expression(start));
                pending.push(Work::Expression(source));
            }
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => {
                pending.push(Work::Expression(value));
            }
            ResolvedExprKind::Try {
                operand,
                residual_type,
                ..
            }
            | ResolvedExprKind::TryOption {
                operand,
                residual_type,
                ..
            } => {
                pending.push(Work::Type(residual_type));
                pending.push(Work::Expression(operand));
            }
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(Work::Expression(right));
                pending.push(Work::Expression(left));
            }
            ResolvedExprKind::Block { statements, tail } => {
                pending.push(Work::Expression(tail));
                for statement in statements.iter().rev() {
                    for index in (0..statement.child_count()).rev() {
                        if let Some(child) = statement.child(index) {
                            pending.push(Work::Expression(child));
                        }
                    }
                    if let ResolvedStatement::Let { binding, .. } = statement {
                        pending.push(Work::Type(&binding.ty));
                    }
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(Work::Expression(else_branch));
                pending.push(Work::Expression(then_branch));
                pending.push(Work::Expression(condition));
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(
                    fields
                        .iter()
                        .rev()
                        .map(|field| Work::Expression(&field.value)),
                );
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                for arm in arms.iter().rev() {
                    pending.push(Work::Expression(&arm.value));
                    if let Some(guard) = &arm.guard {
                        pending.push(Work::Expression(guard));
                    }
                }
                pending.push(Work::Expression(scrutinee));
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.extend(
                    fields
                        .iter()
                        .rev()
                        .map(|field| Work::Expression(&field.value)),
                );
                pending.push(Work::Expression(base));
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. } => {}
        }
    }
    Ok(())
}

fn align_up(value: u32, align: u32) -> Result<u32, Diagnostic> {
    if !align.is_power_of_two() {
        return Err(layout_error("aggregate alignment is not a power of two"));
    }
    let mask = align - 1;
    value
        .checked_add(mask)
        .map(|rounded| rounded & !mask)
        .ok_or_else(|| layout_error("aggregate alignment overflows u32"))
}

fn digest_value(
    target: AggregateTarget,
    ty: &ResolvedType,
    size: u32,
    align: u32,
    fields: &[AggregateFieldLayout],
) -> [u8; 32] {
    digest_value_with_fields(target, ty, size, align, fields, &[])
}

fn digest_value_with_fields(
    target: AggregateTarget,
    ty: &ResolvedType,
    size: u32,
    align: u32,
    fields: &[AggregateFieldLayout],
    field_digests: &[[u8; 32]],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(LAYOUT_DIGEST_DOMAIN);
    hasher.update([target.tag()]);
    frame(&mut hasher, ty.identity_key().as_bytes());
    hasher.update(size.to_le_bytes());
    hasher.update(align.to_le_bytes());
    hasher.update(
        u32::try_from(fields.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for (field, nested) in fields.iter().zip(field_digests) {
        frame(&mut hasher, field.field.as_str().as_bytes());
        frame(&mut hasher, field.ty.identity_key().as_bytes());
        hasher.update(field.offset.to_le_bytes());
        hasher.update(field.size.to_le_bytes());
        hasher.update(field.align.to_le_bytes());
        hasher.update(nested);
    }
    hasher.finalize().into()
}

fn frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

fn layout_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H007", format!("aggregate layout: {}", message.into()))
}

#[cfg(test)]
#[path = "aggregate_layout/tests.rs"]
mod tests;
