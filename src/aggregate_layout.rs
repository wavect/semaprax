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
    pub(crate) offset: u32,
    pub(crate) size: u32,
    pub(crate) align: u32,
    nested_digest: [u8; 32],
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
    Resource,
    Record { fields: Vec<AggregateFieldLayout> },
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
        | ResolvedType::Char
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool => {
            let (size, align) = scalar_size_align(target, ty)?;
            scalar_layout(target, ty, size, align)
        }
        ResolvedType::TypeParameter { .. } => Err(layout_error(
            "generic aggregate layouts are outside executable records v1",
        )),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => layout_nominal(program, target, declaration, arguments, visiting),
    }
}

pub(crate) fn scalar_size_align(
    target: AggregateTarget,
    ty: &ResolvedType,
) -> Result<(u32, u32), Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok((8, 8)),
        ResolvedType::Char => Ok((4, 4)),
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
        ResolvedTypeDeclarationKind::Record { fields } => {
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
        offset = align_up(offset, value.align)?;
        let end = offset
            .checked_add(value.size)
            .ok_or_else(|| layout_error(format!("record `{record}` layout overflows u32")))?;
        fields.push(AggregateFieldLayout {
            field: declaration.id.clone(),
            ty: field_ty,
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
    if let ResolvedTypeDeclarationKind::Record { fields } = &item.kind {
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
    collect_record_type(program, &expression.ty, instances)?;
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_expr_record_types(program, argument, instances)?;
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                collect_expr_record_types(program, argument, instances)?;
            }
        }
        ResolvedExprKind::Unary { value, .. } | ResolvedExprKind::Project { base: value, .. } => {
            collect_expr_record_types(program, value, instances)?;
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
            collect_expr_record_types(program, operand, instances)?;
            collect_record_type(program, residual_type, instances)?;
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_expr_record_types(program, left, instances)?;
            collect_expr_record_types(program, right, instances)?;
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let { binding, value, .. } = statement;
                collect_record_type(program, &binding.ty, instances)?;
                collect_expr_record_types(program, value, instances)?;
            }
            collect_expr_record_types(program, tail, instances)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_record_types(program, condition, instances)?;
            collect_expr_record_types(program, then_branch, instances)?;
            collect_expr_record_types(program, else_branch, instances)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_expr_record_types(program, &field.value, instances)?;
            }
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            collect_expr_record_types(program, scrutinee, instances)?;
            for arm in arms {
                collect_expr_record_types(program, &arm.value, instances)?;
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_expr_record_types(program, base, instances)?;
            for field in fields {
                collect_expr_record_types(program, &field.value, instances)?;
            }
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::Place(_) => {}
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
mod tests {
    use std::path::Path;

    use super::{align_up, AggregateLayout, AggregateLayoutCache, AggregateTarget};
    use crate::hir::{
        self, DeclarationId, ResolvedResourceDropKind, ResolvedType, ResolvedTypeDeclarationKind,
    };
    use crate::parse;

    const SOURCE: &str = r#"
module test.aggregate_layout;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("inner.type")
record Inner {
    @id("inner.flag")
    flag: bool,
    @id("inner.token")
    token: Token,
}

@id("outer.type")
record Outer {
    @id("outer.flag")
    flag: bool,
    @id("outer.count")
    count: i64,
    @id("outer.inner")
    inner: Inner,
}

@id("empty.type")
record Empty {}

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program() -> hir::ResolvedProgram {
        hir::resolve(&parse(SOURCE, Path::new("aggregate-layout.spx")).unwrap()).unwrap()
    }

    #[test]
    fn native_and_wasm_layouts_have_frozen_offsets_and_digests() {
        let program = program();
        let record = DeclarationId::new("outer.type");
        let native =
            AggregateLayout::for_record(&program, AggregateTarget::Native64, &record).unwrap();
        assert_eq!((native.size, native.align), (32, 8));
        assert_eq!(
            native
                .fields
                .iter()
                .map(|field| (field.field.as_str(), field.offset, field.size, field.align))
                .collect::<Vec<_>>(),
            vec![
                ("outer.flag", 0, 1, 1),
                ("outer.count", 8, 8, 8),
                ("outer.inner", 16, 16, 8),
            ]
        );
        assert_eq!(
            native.digest_hex(),
            "695de29a8ad639fd9272ebaa04faf75d64205881d37f06f93ad4a339e3c36c1f"
        );
        native.validate(&program).unwrap();

        let wasm = AggregateLayout::for_record(&program, AggregateTarget::Wasm32, &record).unwrap();
        assert_eq!((wasm.size, wasm.align), (24, 8));
        assert_eq!(
            wasm.fields
                .iter()
                .map(|field| (field.field.as_str(), field.offset, field.size, field.align))
                .collect::<Vec<_>>(),
            vec![
                ("outer.flag", 0, 4, 4),
                ("outer.count", 8, 8, 8),
                ("outer.inner", 16, 8, 4),
            ]
        );
        assert_eq!(
            wasm.digest_hex(),
            "4e2d57512c9a80b6c4bfecc371373e52f1c1045206f7c63d94ebdb579cc5b11d"
        );
        wasm.validate(&program).unwrap();

        let empty = DeclarationId::new("empty.type");
        let native_empty =
            AggregateLayout::for_record(&program, AggregateTarget::Native64, &empty).unwrap();
        let wasm_empty =
            AggregateLayout::for_record(&program, AggregateTarget::Wasm32, &empty).unwrap();
        assert_eq!((native_empty.size, native_empty.align), (1, 1));
        assert_eq!((wasm_empty.size, wasm_empty.align), (1, 1));
        assert!(native_empty.fields.is_empty());
        assert!(wasm_empty.fields.is_empty());
        assert_eq!(
            native_empty.digest_hex(),
            "13d181500c46e00b711fd2374705971496e00a04f0cbacc546bff1e2339e3140"
        );
        assert_eq!(
            wasm_empty.digest_hex(),
            "2cef34f3db54e52a15b8d8e123867b6ba592de0d1f6fc49597a98714f03b3f1d"
        );
        native_empty.validate(&program).unwrap();
        wasm_empty.validate(&program).unwrap();
        let mut zero_sized = native_empty;
        zero_sized.size = 0;
        assert!(zero_sized.validate(&program).is_err());
    }

    #[test]
    fn exact_reconstruction_rejects_reorder_overlap_undersize_and_alignment_mutations() {
        let program = program();
        let canonical = AggregateLayout::for_record(
            &program,
            AggregateTarget::Native64,
            &DeclarationId::new("outer.type"),
        )
        .unwrap();

        let mut reordered = canonical.clone();
        reordered.fields.swap(0, 1);
        assert!(reordered.validate(&program).is_err());

        let mut overlapping = canonical.clone();
        overlapping.fields[1].offset = 0;
        assert!(overlapping.validate(&program).is_err());

        let mut undersized = canonical.clone();
        undersized.size -= 1;
        assert!(undersized.validate(&program).is_err());

        let mut misaligned = canonical;
        misaligned.fields[1].align = 4;
        assert!(misaligned.validate(&program).is_err());
    }

    #[test]
    fn unknown_duplicate_recursive_and_imported_resource_inputs_fail_closed() {
        let mut hostile_program = program();
        assert!(AggregateLayout::for_record(
            &hostile_program,
            AggregateTarget::Native64,
            &DeclarationId::new("missing.type")
        )
        .is_err());

        let duplicate = hostile_program.types[1].clone();
        hostile_program.types.push(duplicate);
        assert!(AggregateLayout::for_record(
            &hostile_program,
            AggregateTarget::Native64,
            &DeclarationId::new("inner.type")
        )
        .is_err());

        hostile_program.types.pop();
        let inner = hostile_program
            .types
            .iter_mut()
            .find(|item| item.id.as_str() == "inner.type")
            .unwrap();
        let ResolvedTypeDeclarationKind::Record { fields } = &mut inner.kind else {
            unreachable!()
        };
        fields[0].ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("inner.type"),
            arguments: Vec::new(),
        };
        assert!(AggregateLayout::for_record(
            &hostile_program,
            AggregateTarget::Native64,
            &DeclarationId::new("inner.type")
        )
        .is_err());

        let mut imported_program = program();
        let token = imported_program
            .types
            .iter_mut()
            .find(|item| item.id.as_str() == "token.type")
            .unwrap();
        let ResolvedTypeDeclarationKind::Resource { drop } = &mut token.kind else {
            unreachable!()
        };
        drop.kind = ResolvedResourceDropKind::Imported {
            import: DeclarationId::new("host.drop"),
            import_key: "host.drop".to_owned(),
        };
        assert!(AggregateLayout::for_record(
            &imported_program,
            AggregateTarget::Wasm32,
            &DeclarationId::new("outer.type")
        )
        .is_err());

        assert!(align_up(u32::MAX, 8).is_err());
    }

    #[test]
    fn field_lookup_uses_stable_identity() {
        let program = program();
        let layout = AggregateLayout::for_record(
            &program,
            AggregateTarget::Native64,
            &DeclarationId::new("outer.type"),
        )
        .unwrap();
        assert_eq!(
            layout
                .field(&DeclarationId::new("outer.inner"))
                .map(|field| field.offset),
            Some(16)
        );
        assert!(layout.field(&DeclarationId::new("inner.token")).is_none());
    }

    #[test]
    fn generic_instances_bind_cache_digest_and_field_substitution_even_when_layouts_match() {
        let source = r#"
module test.generic_aggregate_layout;
@id("test.box") record Box<T> { @id("test.box.value") value: T, }
@id("test.phantom") record Phantom<T> { @id("test.phantom.marker") marker: bool, }
@id("test.use_i64") fn use_i64(value: Phantom<i64>) -> bool { value.marker }
@id("test.use_bool") fn use_bool(value: Phantom<bool>) -> bool { value.marker }
@id("test.make_i64") fn make_i64(value: i64) -> Box<i64> { Box<i64> { value: value } }
@id("test.make_bool") fn make_bool(value: bool) -> Box<bool> { Box<bool> { value: value } }
@id("app.main") fn main() -> i64 { 0 }
"#;
        let program =
            hir::resolve(&parse(source, Path::new("generic-aggregate-layout.spx")).unwrap())
                .unwrap();
        let phantom_i64 = ResolvedType::Nominal {
            declaration: DeclarationId::new("test.phantom"),
            arguments: vec![ResolvedType::I64],
        };
        let phantom_bool = ResolvedType::Nominal {
            declaration: DeclarationId::new("test.phantom"),
            arguments: vec![ResolvedType::Bool],
        };
        let i64_layout =
            AggregateLayout::for_type(&program, AggregateTarget::Native64, &phantom_i64).unwrap();
        let bool_layout =
            AggregateLayout::for_type(&program, AggregateTarget::Native64, &phantom_bool).unwrap();
        assert_eq!(
            (i64_layout.size, i64_layout.align),
            (bool_layout.size, bool_layout.align)
        );
        assert_eq!(
            i64_layout
                .fields
                .iter()
                .map(|field| (field.offset, field.size, field.align))
                .collect::<Vec<_>>(),
            bool_layout
                .fields
                .iter()
                .map(|field| (field.offset, field.size, field.align))
                .collect::<Vec<_>>()
        );
        assert_ne!(phantom_i64.identity_key(), phantom_bool.identity_key());
        assert_ne!(i64_layout.digest, bool_layout.digest);

        let cache = AggregateLayoutCache::build(&program, AggregateTarget::Native64).unwrap();
        assert_eq!(cache.layout(&phantom_i64).unwrap(), &i64_layout);
        assert_eq!(cache.layout(&phantom_bool).unwrap(), &bool_layout);
        assert_eq!(
            cache
                .layouts()
                .filter(|layout| layout.record.as_str() == "test.phantom")
                .count(),
            2
        );

        let mut relabeled = i64_layout;
        relabeled.instance = phantom_bool;
        assert!(relabeled.validate(&program).is_err());

        let box_i64 = ResolvedType::Nominal {
            declaration: DeclarationId::new("test.box"),
            arguments: vec![ResolvedType::I64],
        };
        let box_bool = ResolvedType::Nominal {
            declaration: DeclarationId::new("test.box"),
            arguments: vec![ResolvedType::Bool],
        };
        assert_eq!(
            cache.layout(&box_i64).unwrap().fields[0].ty,
            ResolvedType::I64
        );
        assert_eq!(
            cache.layout(&box_bool).unwrap().fields[0].ty,
            ResolvedType::Bool
        );
    }
}
