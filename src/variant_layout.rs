//! Deterministic target layouts for executable copy variants.
//!
//! Tags are declaration-order `u32` ordinals. Every variant reserves an
//! aligned maximum-payload region, including one inert byte for unit-only
//! payloads. Backends validate an independently reconstructed layout before
//! consuming it.

use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::aggregate_layout::{scalar_size_align, AggregateTarget};
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
    pub(crate) offset: u32,
    pub(crate) size: u32,
    pub(crate) align: u32,
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
        if arguments.len() != declaration.type_parameters.len()
            || arguments
                .iter()
                .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
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
        let (size, field_align) = scalar_size_align(target, &concrete_ty).map_err(|_| {
            layout_error(format!(
                "variant `{variant}` case `{}` field `{}` is not direct i64/bool",
                case.id, field.id
            ))
        })?;
        offset = align_up(offset, field_align)?;
        fields.push(VariantFieldLayout {
            field: field.id.clone(),
            template_ty: field.ty.clone(),
            ty: concrete_ty,
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
    collect_variant_type(program, &expression.ty, instances)?;
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_expr_variant_types(program, argument, instances)?;
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                collect_expr_variant_types(program, argument, instances)?;
            }
        }
        ResolvedExprKind::Unary { value, .. } | ResolvedExprKind::Project { base: value, .. } => {
            collect_expr_variant_types(program, value, instances)?;
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
            collect_expr_variant_types(program, operand, instances)?;
            collect_variant_type(program, residual_type, instances)?;
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_expr_variant_types(program, left, instances)?;
            collect_expr_variant_types(program, right, instances)?;
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let binding = statement.binding();
                collect_variant_type(program, &binding.ty, instances)?;
                collect_expr_variant_types(program, statement.value(), instances)?;
            }
            collect_expr_variant_types(program, tail, instances)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_variant_types(program, condition, instances)?;
            collect_expr_variant_types(program, then_branch, instances)?;
            collect_expr_variant_types(program, else_branch, instances)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_expr_variant_types(program, &field.value, instances)?;
            }
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            collect_expr_variant_types(program, scrutinee, instances)?;
            for arm in arms {
                if let crate::hir::ResolvedMatchPattern::Variant { fields, .. } = &arm.pattern {
                    for field in fields {
                        collect_variant_type(program, &field.binding.ty, instances)?;
                    }
                }
                collect_expr_variant_types(program, &arm.value, instances)?;
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_expr_variant_types(program, base, instances)?;
            for field in fields {
                collect_expr_variant_types(program, &field.value, instances)?;
            }
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::Place(_) => {}
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
mod tests {
    use std::path::Path;

    use super::{VariantLayout, VariantLayoutCache, VariantTarget};
    use crate::hir::{self, DeclarationId, ResolvedType, ResolvedTypeDeclarationKind};
    use crate::parse;

    const SOURCE: &str = r#"
module test.variant_layout;
@id("choice.type")
variant Choice {
    @id("choice.none") None,
    @id("choice.flag") Flag {
        @id("choice.flag.value") value: bool,
    },
    @id("choice.pair") Pair {
        @id("choice.pair.number") number: i64,
        @id("choice.pair.enabled") enabled: bool,
    },
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

    const GENERIC_SOURCE: &str = r#"
module test.generic_variant_layout;
@id("choice.generic")
variant Choice<T> {
    @id("choice.generic.none") None,
    @id("choice.generic.value") Value {
        @id("choice.generic.value.value") value: T,
    },
}
@id("choice.i64")
fn choice_i64() -> Choice<i64> { Choice<i64>::Value { value: 42 } }
@id("choice.bool")
fn choice_bool() -> Choice<bool> { Choice<bool>::Value { value: true } }
@id("option.i64")
fn option_i64() -> Option<i64> { Option<i64>::Some { value: 7 } }
@id("option.bool")
fn option_bool() -> Option<bool> { Option<bool>::None {} }
@id("result.value")
fn result_value() -> Result<i64, bool> { Result<i64, bool>::Err { error: true } }
@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn resolved() -> hir::ResolvedProgram {
        let parsed = parse(SOURCE, Path::new("variant-layout.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
    }

    fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
        ResolvedType::Nominal {
            declaration: DeclarationId::new(id),
            arguments,
        }
    }

    #[test]
    fn concrete_generic_and_prelude_instances_have_distinct_cached_layouts() {
        let parsed = parse(GENERIC_SOURCE, Path::new("generic-variant-layout.spx")).unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let native = VariantLayoutCache::build(&program, VariantTarget::Native64).unwrap();
        let wasm = VariantLayoutCache::build(&program, VariantTarget::Wasm32).unwrap();

        let choice_i64 = nominal("choice.generic", vec![ResolvedType::I64]);
        let choice_bool = nominal("choice.generic", vec![ResolvedType::Bool]);
        let option_i64 = nominal("core.option", vec![ResolvedType::I64]);
        let option_bool = nominal("core.option", vec![ResolvedType::Bool]);
        let result = nominal("core.result", vec![ResolvedType::I64, ResolvedType::Bool]);
        assert_eq!(native.layouts().len(), 5);
        assert_eq!(wasm.layouts().len(), 5);

        for cache in [&native, &wasm] {
            let integer = cache.layout(&choice_i64).unwrap();
            assert_eq!(
                (integer.payload_offset, integer.size, integer.align),
                (8, 16, 8)
            );
            assert_eq!(
                integer
                    .case(&DeclarationId::new("choice.generic.value"))
                    .unwrap()
                    .fields[0]
                    .ty,
                ResolvedType::I64
            );
            let result = cache.layout(&result).unwrap();
            assert_eq!(
                (result.payload_offset, result.size, result.align),
                (8, 16, 8)
            );
            assert_eq!(
                result.cases.iter().map(|case| case.tag).collect::<Vec<_>>(),
                vec![0, 1]
            );
        }

        let native_bool = native.layout(&choice_bool).unwrap();
        let wasm_bool = wasm.layout(&choice_bool).unwrap();
        assert_eq!((native_bool.payload_offset, native_bool.size), (4, 8));
        assert_eq!((wasm_bool.payload_offset, wasm_bool.size), (4, 8));
        assert_ne!(
            native.layout(&choice_i64).unwrap().digest_hex(),
            native_bool.digest_hex()
        );
        assert_ne!(
            native.layout(&option_i64).unwrap().digest_hex(),
            native.layout(&option_bool).unwrap().digest_hex()
        );
        assert_eq!(
            native.layout(&option_i64).unwrap().digest_hex(),
            "e728ce973bb0fa9d86027841615dcd25b9a1700cb15e4fd1704da163e658d60c"
        );
        assert_eq!(
            wasm.layout(&option_i64).unwrap().digest_hex(),
            "79194fc88011ac060877e60293d0a4272429dd9e2d720674d0d54e804562deda"
        );
        assert_eq!(
            native.layout(&result).unwrap().digest_hex(),
            "03ac11743e029e151b8cbc12420e899b2edfe42cbf7c68d5f6fb3ab0e043b3dc"
        );
        assert_eq!(
            wasm.layout(&result).unwrap().digest_hex(),
            "c01112f909a074343ae4eb3abde6ad70930280e4a8016c165e05f317bed9f199"
        );

        let mut confused = native.layout(&choice_i64).unwrap().clone();
        confused.instance = choice_bool;
        assert!(confused.validate(&program).is_err());
        let reversed_result = nominal("core.result", vec![ResolvedType::Bool, ResolvedType::I64]);
        assert_ne!(
            VariantLayout::for_type(&program, VariantTarget::Native64, &reversed_result)
                .unwrap()
                .digest_hex(),
            native.layout(&result).unwrap().digest_hex()
        );
        let wrong_arity = nominal("core.result", vec![ResolvedType::I64]);
        assert!(VariantLayout::for_type(&program, VariantTarget::Wasm32, &wrong_arity).is_err());
        let nested_argument = nominal("choice.generic", vec![option_i64.clone()]);
        assert!(
            VariantLayout::for_type(&program, VariantTarget::Wasm32, &nested_argument).is_err()
        );

        let mut missing = native.clone();
        missing.layouts.remove(&choice_i64);
        assert!(missing.layout(&choice_i64).is_err());

        for (owner, index) in [("foreign.owner", 0), ("choice.generic", 1)] {
            let mut hostile = program.clone();
            let declaration = hostile
                .types
                .iter_mut()
                .find(|item| item.id.as_str() == "choice.generic")
                .unwrap();
            let ResolvedTypeDeclarationKind::Variant { cases } = &mut declaration.kind else {
                panic!("generic choice is a variant")
            };
            cases[1].fields[0].ty = ResolvedType::TypeParameter {
                owner: DeclarationId::new(owner),
                index,
            };
            assert!(
                VariantLayout::for_type(&hostile, VariantTarget::Native64, &choice_i64).is_err()
            );
        }
    }

    #[test]
    fn native64_and_wasm32_layouts_freeze_tag_payload_and_bool_profiles() {
        let program = resolved();
        let variant = DeclarationId::new("choice.type");
        let native = VariantLayout::for_variant(&program, VariantTarget::Native64, &variant)
            .expect("native variant layout");
        let wasm = VariantLayout::for_variant(&program, VariantTarget::Wasm32, &variant)
            .expect("Wasm variant layout");

        for layout in [&native, &wasm] {
            assert_eq!(layout.variant, variant);
            assert_eq!(layout.tag_size, 4);
            assert_eq!(layout.payload_offset, 8);
            assert_eq!(layout.payload_size, 16);
            assert_eq!(layout.size, 24);
            assert_eq!(layout.align, 8);
            assert_eq!(
                layout
                    .cases
                    .iter()
                    .map(|case| (case.case.as_str(), case.tag))
                    .collect::<Vec<_>>(),
                vec![("choice.none", 0), ("choice.flag", 1), ("choice.pair", 2),]
            );
            let unit = layout.case(&DeclarationId::new("choice.none")).unwrap();
            assert_eq!((unit.size, unit.align), (1, 1));
            assert!(unit.fields.is_empty());
            layout.validate(&program).unwrap();
        }

        let native_flag = native
            .case(&DeclarationId::new("choice.flag"))
            .unwrap()
            .field(&DeclarationId::new("choice.flag.value"))
            .unwrap();
        assert_eq!(
            (native_flag.offset, native_flag.size, native_flag.align),
            (0, 1, 1)
        );
        let wasm_flag = wasm
            .case(&DeclarationId::new("choice.flag"))
            .unwrap()
            .field(&DeclarationId::new("choice.flag.value"))
            .unwrap();
        assert_eq!(
            (wasm_flag.offset, wasm_flag.size, wasm_flag.align),
            (0, 4, 4)
        );
        assert_eq!(
            native.digest_hex(),
            "60ff105b799ae1a6ec24b72587901bfa1a6be6a97dad8685c75f56db9423e1e1"
        );
        assert_eq!(
            wasm.digest_hex(),
            "4a1c07d4b2011b11c43acb27aa9951b0cb6a55af24e079c73833f7047c3700e6"
        );
    }

    #[test]
    fn hostile_layout_and_declaration_mutations_are_rejected_independently() {
        let program = resolved();
        let variant = DeclarationId::new("choice.type");
        let canonical = VariantLayout::for_variant(&program, VariantTarget::Wasm32, &variant)
            .expect("canonical layout");

        let mut reordered = canonical.clone();
        reordered.cases.swap(0, 1);
        assert!(reordered.validate(&program).is_err());

        let mut retagged = canonical.clone();
        retagged.cases[0].tag = 1;
        assert!(retagged.validate(&program).is_err());

        let mut overlapping = canonical.clone();
        overlapping.payload_offset = 4;
        assert!(overlapping.validate(&program).is_err());

        let mut undersized = canonical.clone();
        undersized.payload_size -= 1;
        assert!(undersized.validate(&program).is_err());

        let mut misaligned = canonical.clone();
        misaligned.cases[2].fields[1].offset = 9;
        assert!(misaligned.validate(&program).is_err());

        let mut digest_confused = canonical.clone();
        digest_confused.target = VariantTarget::Native64;
        assert!(digest_confused.validate(&program).is_err());

        let mut duplicate_case = program.clone();
        let declaration = duplicate_case
            .types
            .iter_mut()
            .find(|item| item.id == variant)
            .unwrap();
        let ResolvedTypeDeclarationKind::Variant { cases } = &mut declaration.kind else {
            panic!("choice is a variant")
        };
        cases[1].id = cases[0].id.clone();
        assert!(
            VariantLayout::for_variant(&duplicate_case, VariantTarget::Wasm32, &variant).is_err()
        );

        let mut noncanonical_tag = program;
        let declaration = noncanonical_tag
            .types
            .iter_mut()
            .find(|item| item.id == variant)
            .unwrap();
        let ResolvedTypeDeclarationKind::Variant { cases } = &mut declaration.kind else {
            panic!("choice is a variant")
        };
        cases[1].index = u32::MAX;
        assert!(
            VariantLayout::for_variant(&noncanonical_tag, VariantTarget::Wasm32, &variant).is_err()
        );
    }
}
