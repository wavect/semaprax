//! Deterministic, target-specific layouts for executable aggregate records.
//!
//! Backends consume this module instead of independently reconstructing record
//! offsets.  The constructor and validator are deliberately separate: a
//! backend may only use a layout after it has survived exact reconstruction
//! from resolved HIR.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ResolvedFieldDeclaration, ResolvedProgram, ResolvedResourceDropKind,
    ResolvedType, ResolvedTypeDeclaration, ResolvedTypeDeclarationKind,
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
        let mut visiting = BTreeSet::new();
        let value = layout_nominal(program, target, record, &mut visiting)?;
        let ValueLayoutKind::Record { fields } = value.kind else {
            return Err(layout_error(format!("`{record}` is not a record")));
        };
        Ok(Self {
            target,
            record: record.clone(),
            size: value.size,
            align: value.align,
            fields,
            digest: value.digest,
        })
    }

    /// Reconstructs this layout from HIR and rejects any changed byte.
    pub(crate) fn validate(&self, program: &ResolvedProgram) -> Result<(), Diagnostic> {
        let expected = Self::for_record(program, self.target, &self.record)?;
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
    visiting: &mut BTreeSet<DeclarationId>,
) -> Result<ValueLayout, Diagnostic> {
    match ty {
        ResolvedType::I64 => scalar_layout(target, ty, 8, 8),
        ResolvedType::Bool => match target {
            AggregateTarget::Native64 => scalar_layout(target, ty, 1, 1),
            AggregateTarget::Wasm32 => scalar_layout(target, ty, 4, 4),
        },
        ResolvedType::TypeParameter { .. } => Err(layout_error(
            "generic aggregate layouts are outside executable records v1",
        )),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            if !arguments.is_empty() {
                return Err(layout_error(
                    "generic aggregate layouts are outside executable records v1",
                ));
            }
            layout_nominal(program, target, declaration, visiting)
        }
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
    visiting: &mut BTreeSet<DeclarationId>,
) -> Result<ValueLayout, Diagnostic> {
    let item = unique_type(program, declaration)?;
    match &item.kind {
        ResolvedTypeDeclarationKind::Resource { drop } => {
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
            if !visiting.insert(declaration.clone()) {
                return Err(layout_error(format!(
                    "record `{declaration}` has a recursive by-value layout"
                )));
            }
            let result = layout_record(program, target, declaration, fields, visiting);
            visiting.remove(declaration);
            result
        }
    }
}

fn layout_record(
    program: &ResolvedProgram,
    target: AggregateTarget,
    record: &DeclarationId,
    declarations: &[ResolvedFieldDeclaration],
    visiting: &mut BTreeSet<DeclarationId>,
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

        let value = layout_type(program, target, &declaration.ty, visiting)?;
        offset = align_up(offset, value.align)?;
        let end = offset
            .checked_add(value.size)
            .ok_or_else(|| layout_error(format!("record `{record}` layout overflows u32")))?;
        fields.push(AggregateFieldLayout {
            field: declaration.id.clone(),
            ty: declaration.ty.clone(),
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
        arguments: Vec::new(),
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

    use super::{align_up, AggregateLayout, AggregateTarget};
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
}
