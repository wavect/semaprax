//! Source-side companion of `hir::owned_record_collection` (SPX-AI-019).
//!
//! Re-derives the same bounded owned-record collection element shape from
//! AST declared-type facts, independently of the HIR classifier: a
//! nonrecursive, non-generic authored `record` with exactly two direct
//! `Bytes` fields and exactly one direct admitted Copy-scalar field, and no
//! other fields. See `docs/OWNED-RECORD-COLLECTION-ELEMENT-V1.md`.
//!
//! No call site outside this module's own tests exists yet; see that spec
//! for what remains before source can actually declare a `Vec`/`Box` of this
//! element.
#![cfg_attr(not(test), allow(dead_code))]

use super::*;

pub(in crate::source_verify) fn is_admitted_owned_record_collection_element(
    types: &TypeTable<'_>,
    ty: &Type,
) -> bool {
    let Type::Named { name, arguments } = ty else {
        return false;
    };
    if !arguments.is_empty() {
        return false;
    }
    let Some(declaration) = types.declaration(name) else {
        return false;
    };
    if !declaration.type_parameters.is_empty() {
        return false;
    }
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return false;
    };
    admits_field_shape(fields)
}

/// The exact structural rule shared by the source and resolved classifiers:
/// exactly two `Bytes` fields and exactly one admitted Copy-scalar field, and
/// nothing else.
fn admits_field_shape(fields: &[FieldDeclaration]) -> bool {
    if fields.len() != 3 {
        return false;
    }
    let mut bytes_fields = 0usize;
    let mut copy_fields = 0usize;
    for field in fields {
        if field.ty == Type::Bytes {
            bytes_fields += 1;
        } else if crate::vec_ops::ast_element_is_admitted(&field.ty) {
            copy_fields += 1;
        } else {
            return false;
        }
    }
    bytes_fields == 2 && copy_fields == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(source: &str) -> Program {
        crate::parse(source, std::path::Path::new("owned-record-collection.spx")).unwrap()
    }

    #[test]
    fn exact_selected_payload_is_admitted() {
        let program = parsed(
            r#"module owned_record_collection.source.exact;
@id("owned_record_collection.source.exact.item") record Item {
  @id("owned_record_collection.source.exact.item.id") id: Bytes,
  @id("owned_record_collection.source.exact.item.label") label: Bytes,
  @id("owned_record_collection.source.exact.item.quantity") quantity: i64,
}
@id("owned_record_collection.source.exact.main") fn main() -> i64 { 0 }
"#,
        );
        let types = TypeTable::new(&program);
        let item = Type::Named {
            name: "Item".to_owned(),
            arguments: Vec::new(),
        };
        assert!(is_admitted_owned_record_collection_element(&types, &item));
    }

    #[test]
    fn extra_field_is_refused() {
        let program = parsed(
            r#"module owned_record_collection.source.extra;
@id("owned_record_collection.source.extra.item") record Item {
  @id("owned_record_collection.source.extra.item.id") id: Bytes,
  @id("owned_record_collection.source.extra.item.label") label: Bytes,
  @id("owned_record_collection.source.extra.item.quantity") quantity: i64,
  @id("owned_record_collection.source.extra.item.extra") extra: bool,
}
@id("owned_record_collection.source.extra.main") fn main() -> i64 { 0 }
"#,
        );
        let types = TypeTable::new(&program);
        let item = Type::Named {
            name: "Item".to_owned(),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(&types, &item));
    }

    #[test]
    fn string_field_is_refused() {
        // Mirrors the HIR-side `string_field_is_refused` regression: `string`
        // is neither `Bytes` nor an admitted Copy scalar, so it exercises the
        // immediate `return false` branch of `admits_field_shape` rather than
        // the bytes/copy-count branch the existing fixtures already cover.
        // Pins the doc's explicit claim ("a `String` field... is refused").
        // Unlike the HIR-side case, this classifier runs on raw parsed AST
        // facts before resolution, so it does not need the
        // `resolve_if_admitted` tolerance the HIR test needs for the
        // upstream `SPX-T268` owned-Bytes-record shape rule.
        let program = parsed(
            r#"module owned_record_collection.source.string_field;
@id("owned_record_collection.source.string_field.item") record Item {
  @id("owned_record_collection.source.string_field.item.id") id: Bytes,
  @id("owned_record_collection.source.string_field.item.label") label: string,
  @id("owned_record_collection.source.string_field.item.quantity") quantity: i64,
}
@id("owned_record_collection.source.string_field.main") fn main() -> i64 { 0 }
"#,
        );
        let types = TypeTable::new(&program);
        let item = Type::Named {
            name: "Item".to_owned(),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(&types, &item));
    }

    #[test]
    fn class_declaration_is_refused() {
        let program = parsed(
            r#"module owned_record_collection.source.class;
@id("owned_record_collection.source.class.item") class Item {
  @id("owned_record_collection.source.class.item.id") id: Bytes,
  @id("owned_record_collection.source.class.item.label") label: Bytes,
  @id("owned_record_collection.source.class.item.quantity") quantity: i64,
}
@id("owned_record_collection.source.class.main") fn main() -> i64 { 0 }
"#,
        );
        let types = TypeTable::new(&program);
        let item = Type::Named {
            name: "Item".to_owned(),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(&types, &item));
    }

    #[test]
    fn generic_record_is_refused() {
        let program = parsed(
            r#"module owned_record_collection.source.generic;
@id("owned_record_collection.source.generic.item") record Item<T> {
  @id("owned_record_collection.source.generic.item.id") id: Bytes,
  @id("owned_record_collection.source.generic.item.label") label: Bytes,
  @id("owned_record_collection.source.generic.item.quantity") quantity: T,
}
@id("owned_record_collection.source.generic.main") fn main() -> i64 { 0 }
"#,
        );
        let types = TypeTable::new(&program);
        let item = Type::Named {
            name: "Item".to_owned(),
            arguments: vec![Type::I64],
        };
        assert!(!is_admitted_owned_record_collection_element(&types, &item));
    }

    #[test]
    fn unknown_name_is_refused() {
        let program = parsed(
            r#"module owned_record_collection.source.unknown;
@id("owned_record_collection.source.unknown.main") fn main() -> i64 { 0 }
"#,
        );
        let types = TypeTable::new(&program);
        let missing = Type::Named {
            name: "DoesNotExist".to_owned(),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(
            &types, &missing
        ));
    }
}
