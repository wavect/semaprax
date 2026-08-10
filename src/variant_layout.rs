//! Deterministic target layouts for executable copy variants.
//!
//! Tags are declaration-order `u32` ordinals. Every variant reserves an
//! aligned maximum-payload region, including one inert byte for unit-only
//! payloads. Backends validate an independently reconstructed layout before
//! consuming it.

use std::collections::BTreeSet;
#[cfg(test)]
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::aggregate_layout::{scalar_size_align, AggregateTarget};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ResolvedProgram, ResolvedType, ResolvedTypeDeclarationKind,
    ResolvedVariantCaseDeclaration,
};

const VARIANT_LAYOUT_DOMAIN: &[u8] = b"semaprax.variant-layout.v1\0";

pub(crate) type VariantTarget = AggregateTarget;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VariantFieldLayout {
    pub(crate) field: DeclarationId,
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
    pub(crate) size: u32,
    pub(crate) align: u32,
    pub(crate) tag_size: u32,
    pub(crate) payload_offset: u32,
    pub(crate) payload_size: u32,
    pub(crate) cases: Vec<VariantCaseLayout>,
    digest: [u8; 32],
}

impl VariantLayout {
    pub(crate) fn for_variant(
        program: &ResolvedProgram,
        target: VariantTarget,
        variant: &DeclarationId,
    ) -> Result<Self, Diagnostic> {
        let declaration = unique_variant(program, variant)?;
        let ResolvedTypeDeclarationKind::Variant { cases } = &declaration.kind else {
            return Err(layout_error(format!("`{variant}` is not a variant")));
        };
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
            let layout = layout_case(target, variant, case, tag)?;
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
        let expected = Self::for_variant(program, self.target, &self.variant)?;
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

    #[cfg(test)]
    fn digest_hex(&self) -> String {
        let mut output = String::with_capacity(64);
        for byte in self.digest {
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }
}

fn layout_case(
    target: VariantTarget,
    variant: &DeclarationId,
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
        let (size, field_align) = scalar_size_align(target, &field.ty).map_err(|_| {
            layout_error(format!(
                "variant `{variant}` case `{}` field `{}` is not direct i64/bool",
                case.id, field.id
            ))
        })?;
        offset = align_up(offset, field_align)?;
        fields.push(VariantFieldLayout {
            field: field.id.clone(),
            ty: field.ty.clone(),
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
            digest_string(&mut digest, &field.ty.identity_key());
            for value in [field.offset, field.size, field.align] {
                digest.update(value.to_le_bytes());
            }
        }
    }
    digest.finalize().into()
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

    use super::{VariantLayout, VariantTarget};
    use crate::hir::{self, DeclarationId, ResolvedTypeDeclarationKind};
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

    fn resolved() -> hir::ResolvedProgram {
        let parsed = parse(SOURCE, Path::new("variant-layout.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
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
            "81abd9e45c4fcacef2ef8485192d0ecb57fcd99dbc2269d17617183e2e0c82e2"
        );
        assert_eq!(
            wasm.digest_hex(),
            "0c7b1db30be96256aee8e4cc74216438bfb4e5cc713fbe22bc1c32b3c53d5917"
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
