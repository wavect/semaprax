//! Deterministic target layouts for executable variants.
//!
//! Tags are declaration-order `u32` ordinals. Every variant reserves an
//! aligned maximum-payload region, including one inert byte for unit-only
//! payloads. Backends validate an independently reconstructed layout before
//! consuming it. Layout support for an owned leaf records ownership explicitly;
//! it does not widen any executable Copy-variant profile.

use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::aggregate_layout::{owned_bytes_size_align, scalar_size_align, AggregateTarget};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    substitute_type, DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedType,
    ResolvedTypeDeclarationKind, ResolvedVariantCaseDeclaration,
};

const VARIANT_LAYOUT_DOMAIN: &[u8] = b"semaprax.variant-layout.v2\0";

pub(crate) type VariantTarget = AggregateTarget;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VariantFieldLayout {
    pub(crate) field: DeclarationId,
    pub(crate) template_ty: ResolvedType,
    pub(crate) ty: ResolvedType,
    /// Fixed carrier layout is independent of semantic Copy admission.
    pub(crate) value_kind: VariantFieldValueKind,
    pub(crate) offset: u32,
    pub(crate) size: u32,
    pub(crate) align: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VariantFieldValueKind {
    Copy,
    OwnedBytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VariantCaseLayout {
    pub(crate) case: DeclarationId,
    pub(crate) tag: u32,
    pub(crate) size: u32,
    pub(crate) align: u32,
    pub(crate) fields: Vec<VariantFieldLayout>,
}

impl VariantCaseLayout {
    pub(crate) fn field(&self, field: &DeclarationId) -> Option<&VariantFieldLayout> {
        self.fields.iter().find(|item| item.field == *field)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VariantLayout {
    pub(crate) target: VariantTarget,
    pub(crate) variant: DeclarationId,
    pub(crate) instance: ResolvedType,
    pub(crate) size: u32,
    pub(crate) align: u32,
    pub(crate) tag_size: u32,
    pub(crate) payload_offset: u32,
    pub(crate) payload_size: u32,
    pub(crate) cases: Vec<VariantCaseLayout>,
    digest: [u8; 32],
}

impl VariantLayout {
    #[cfg(test)]
    pub(crate) fn for_variant(
        program: &ResolvedProgram,
        target: VariantTarget,
        variant: &DeclarationId,
    ) -> Result<Self, Diagnostic> {
        Self::for_type(
            program,
            target,
            &ResolvedType::Nominal {
                declaration: variant.clone(),
                arguments: Vec::new(),
            },
        )
    }

    pub(crate) fn for_type(
        program: &ResolvedProgram,
        target: VariantTarget,
        instance: &ResolvedType,
    ) -> Result<Self, Diagnostic> {
        let ResolvedType::Nominal {
            declaration: variant,
            arguments,
        } = instance
        else {
            return Err(layout_error(format!(
                "variant layout requires a nominal instance, found `{}`",
                instance.identity_key()
            )));
        };
        let declaration = unique_variant(program, variant)?;
        let ResolvedTypeDeclarationKind::Variant { cases } = &declaration.kind else {
            return Err(layout_error(format!("`{variant}` is not a variant")));
        };
        let compiler_byte_option = variant.as_str() == crate::prelude::OPTION_ID
            && arguments.as_slice() == [ResolvedType::U8];
        let compiler_owned_byte_algebra = matches!(
            (variant.as_str(), arguments.as_slice()),
            (crate::prelude::OPTION_ID, [ResolvedType::Bytes])
                | (
                    crate::prelude::RESULT_ID,
                    [ResolvedType::Bytes, ResolvedType::I64 | ResolvedType::Bool],
                )
                | (
                    crate::prelude::RESULT_ID,
                    [ResolvedType::I64 | ResolvedType::Bool, ResolvedType::Bytes],
                )
        );
        if arguments.len() != declaration.type_parameters.len()
            || (!compiler_byte_option
                && !compiler_owned_byte_algebra
                && arguments
                    .iter()
                    .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)))
        {
            return Err(layout_error(format!(
                "variant `{variant}` has invalid concrete arguments"
            )));
        }
        if cases.is_empty() {
            return Err(layout_error(format!(
                "variant `{variant}` has no executable cases"
            )));
        }
        let mut seen = BTreeSet::new();
        let mut layouts = Vec::with_capacity(cases.len());
        let mut payload_size = 1_u32;
        let mut payload_align = 1_u32;
        for (ordinal, case) in cases.iter().enumerate() {
            let tag = u32::try_from(ordinal)
                .map_err(|_| layout_error(format!("variant `{variant}` has too many cases")))?;
            if case.index != tag {
                return Err(layout_error(format!(
                    "variant `{variant}` case tags are not declaration-order canonical"
                )));
            }
            if !seen.insert(case.id.clone()) {
                return Err(layout_error(format!(
                    "variant `{variant}` repeats case `{}`",
                    case.id
                )));
            }
            let layout = layout_case(target, variant, arguments, case, tag)?;
            payload_size = payload_size.max(layout.size);
            payload_align = payload_align.max(layout.align);
            layouts.push(layout);
        }
        let tag_size = 4_u32;
        let payload_offset = align_up(tag_size, payload_align)?;
        let align = 4_u32.max(payload_align);
        let size = align_up(
            payload_offset
                .checked_add(payload_size)
                .ok_or_else(|| layout_error("variant payload end overflows u32"))?,
            align,
        )?;
        let digest = digest_layout(
            target,
            variant,
            instance,
            size,
            align,
            tag_size,
            payload_offset,
            payload_size,
            &layouts,
        );
        Ok(Self {
            target,
            variant: variant.clone(),
            instance: instance.clone(),
            size,
            align,
            tag_size,
            payload_offset,
            payload_size,
            cases: layouts,
            digest,
        })
    }

    pub(crate) fn validate(&self, program: &ResolvedProgram) -> Result<(), Diagnostic> {
        let expected = Self::for_type(program, self.target, &self.instance)?;
        if *self != expected {
            return Err(layout_error(format!(
                "variant `{}` layout is not the canonical {:?} layout",
                self.variant, self.target
            )));
        }
        Ok(())
    }

    pub(crate) fn case(&self, case: &DeclarationId) -> Option<&VariantCaseLayout> {
        self.cases.iter().find(|item| item.case == *case)
    }

    #[cfg(any(test, feature = "unstable-wit-component-harness"))]
    pub(crate) const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    #[cfg(test)]
    fn digest_hex(&self) -> String {
        let mut output = String::with_capacity(64);
        for byte in self.digest {
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VariantLayoutCache {
    target: VariantTarget,
    layouts: BTreeMap<ResolvedType, VariantLayout>,
}

impl VariantLayoutCache {
    /// A selected backend profile that has already rejected every nominal
    /// type needs no whole-program layout discovery. Any accidental nominal
    /// lookup still fails through the ordinary missing-layout check.
    pub(crate) fn for_scalar_only(target: VariantTarget) -> Self {
        Self {
            target,
            layouts: BTreeMap::new(),
        }
    }

    pub(crate) fn build(
        program: &ResolvedProgram,
        target: VariantTarget,
    ) -> Result<Self, Diagnostic> {
        let mut instances = BTreeSet::new();
        for function in &program.functions {
            for parameter in &function.params {
                collect_variant_type(program, &parameter.ty, &mut instances)?;
            }
            collect_variant_type(program, &function.return_type, &mut instances)?;
            for contract in &function.requires {
                collect_expr_variant_types(program, contract, &mut instances)?;
            }
            collect_expr_variant_types(program, &function.body, &mut instances)?;
            for contract in &function.ensures {
                collect_expr_variant_types(program, contract, &mut instances)?;
            }
        }
        let mut layouts = BTreeMap::new();
        for instance in instances {
            let layout = VariantLayout::for_type(program, target, &instance)?;
            layout.validate(program)?;
            if layouts.insert(instance, layout).is_some() {
                return Err(layout_error("duplicate concrete variant instance"));
            }
        }
        Ok(Self { target, layouts })
    }

    pub(crate) fn layout(&self, instance: &ResolvedType) -> Result<&VariantLayout, Diagnostic> {
        self.layouts.get(instance).ok_or_else(|| {
            layout_error(format!(
                "missing {:?} layout for concrete variant `{}`",
                self.target,
                instance.identity_key()
            ))
        })
    }

    pub(crate) fn layouts(&self) -> impl ExactSizeIterator<Item = &VariantLayout> {
        self.layouts.values()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.layouts.is_empty()
    }
}

fn layout_case(
    target: VariantTarget,
    variant: &DeclarationId,
    arguments: &[ResolvedType],
    case: &ResolvedVariantCaseDeclaration,
    tag: u32,
) -> Result<VariantCaseLayout, Diagnostic> {
    let mut seen = BTreeSet::new();
    let mut offset = u32::from(case.fields.is_empty());
    let mut align = 1_u32;
    let mut fields = Vec::with_capacity(case.fields.len());
    for (ordinal, field) in case.fields.iter().enumerate() {
        let expected = u32::try_from(ordinal)
            .map_err(|_| layout_error(format!("variant case `{}` has too many fields", case.id)))?;
        if field.index != expected {
            return Err(layout_error(format!(
                "variant case `{}` field indices are not declaration-order canonical",
                case.id
            )));
        }
        if !seen.insert(field.id.clone()) {
            return Err(layout_error(format!(
                "variant case `{}` repeats field `{}`",
                case.id, field.id
            )));
        }
        let concrete_ty = substitute_type(&field.ty, variant, arguments)?;
        let (size, field_align, value_kind) = if concrete_ty == ResolvedType::Bytes {
            let (size, align) = owned_bytes_size_align(target);
            (size, align, VariantFieldValueKind::OwnedBytes)
        } else {
            let (size, align) = scalar_size_align(target, &concrete_ty).map_err(|_| {
                layout_error(format!(
                    "variant `{variant}` case `{}` field `{}` is not a direct Copy scalar or owned Bytes leaf",
                    case.id, field.id
                ))
            })?;
            (size, align, VariantFieldValueKind::Copy)
        };
        offset = align_up(offset, field_align)?;
        fields.push(VariantFieldLayout {
            field: field.id.clone(),
            template_ty: field.ty.clone(),
            ty: concrete_ty,
            value_kind,
            offset,
            size,
            align: field_align,
        });
        offset = offset
            .checked_add(size)
            .ok_or_else(|| layout_error("variant case field end overflows u32"))?;
        align = align.max(field_align);
    }
    let size = align_up(offset, align)?;
    Ok(VariantCaseLayout {
        case: case.id.clone(),
        tag,
        size,
        align,
        fields,
    })
}

fn unique_variant<'a>(
    program: &'a ResolvedProgram,
    variant: &DeclarationId,
) -> Result<&'a crate::hir::ResolvedTypeDeclaration, Diagnostic> {
    let mut matches = program.types.iter().filter(|item| item.id == *variant);
    let item = matches
        .next()
        .ok_or_else(|| layout_error(format!("unknown variant `{variant}`")))?;
    if matches.next().is_some() {
        return Err(layout_error(format!(
            "variant `{variant}` has duplicate declarations"
        )));
    }
    Ok(item)
}

fn align_up(value: u32, align: u32) -> Result<u32, Diagnostic> {
    if !align.is_power_of_two() {
        return Err(layout_error("variant alignment is not a power of two"));
    }
    let mask = align - 1;
    value
        .checked_add(mask)
        .map(|aligned| aligned & !mask)
        .ok_or_else(|| layout_error("variant alignment overflows u32"))
}

#[allow(clippy::too_many_arguments)]
fn digest_layout(
    target: VariantTarget,
    variant: &DeclarationId,
    instance: &ResolvedType,
    size: u32,
    align: u32,
    tag_size: u32,
    payload_offset: u32,
    payload_size: u32,
    cases: &[VariantCaseLayout],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(VARIANT_LAYOUT_DOMAIN);
    digest.update([match target {
        VariantTarget::Native64 => 1,
        VariantTarget::Wasm32 => 2,
    }]);
    digest_string(&mut digest, variant.as_str());
    digest_string(&mut digest, &instance.identity_key());
    for value in [size, align, tag_size, payload_offset, payload_size] {
        digest.update(value.to_le_bytes());
    }
    digest.update(u32::try_from(cases.len()).unwrap_or(u32::MAX).to_le_bytes());
    for case in cases {
        digest_string(&mut digest, case.case.as_str());
        for value in [case.tag, case.size, case.align] {
            digest.update(value.to_le_bytes());
        }
        digest.update(
            u32::try_from(case.fields.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for field in &case.fields {
            digest_string(&mut digest, field.field.as_str());
            digest_string(&mut digest, &field.template_ty.identity_key());
            digest_string(&mut digest, &field.ty.identity_key());
            for value in [field.offset, field.size, field.align] {
                digest.update(value.to_le_bytes());
            }
        }
    }
    digest.finalize().into()
}

fn collect_variant_type(
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
        collect_variant_type(program, argument, instances)?;
    }
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| layout_error(format!("unknown concrete type `{declaration}`")))?;
    if matches!(item.kind, ResolvedTypeDeclarationKind::Variant { .. }) {
        instances.insert(ty.clone());
    }
    Ok(())
}

fn collect_expr_variant_types(
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
            collect_variant_type(program, ty, instances)?;
            continue;
        };
        collect_variant_type(program, &expression.ty, instances)?;
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
                    if let crate::hir::ResolvedStatement::Let { binding, .. } = statement {
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
                    if let crate::hir::ResolvedMatchPattern::Variant { fields, .. } = &arm.pattern {
                        pending.extend(
                            fields
                                .iter()
                                .rev()
                                .map(|field| Work::Type(&field.binding.ty)),
                        );
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
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::Place(_) => {}
        }
    }
    Ok(())
}

fn digest_string(digest: &mut Sha256, value: &str) {
    digest.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(value.as_bytes());
}

fn layout_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

#[cfg(test)]
#[path = "variant_layout/tests.rs"]
mod tests;
