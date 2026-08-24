//! Compiler-owned ordinary algebraic declarations.
//!
//! These declarations participate in name resolution, HIR, type facts, and
//! Graph meaning exactly like source variants, but are never projected into a
//! user's canonical `.spx` file. The revision digest binds this schema so an
//! implicit-prelude change cannot silently retain an old semantic base.

use std::fmt::Write as _;
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

use crate::ast::{
    FieldDeclaration, Span, Type, TypeDeclaration, TypeDeclarationKind, TypeParameterDeclaration,
    VariantCaseDeclaration,
};

pub(crate) const SCHEMA_V1: &str = "semaprax.prelude.v1";

pub(crate) const OPTION_ID: &str = "core.option";
pub(crate) const OPTION_NONE_ID: &str = "core.option.none";
pub(crate) const OPTION_SOME_ID: &str = "core.option.some";
pub(crate) const OPTION_SOME_VALUE_ID: &str = "core.option.some.value";

pub(crate) const RESULT_ID: &str = "core.result";
pub(crate) const RESULT_OK_ID: &str = "core.result.ok";
pub(crate) const RESULT_OK_VALUE_ID: &str = "core.result.ok.value";
pub(crate) const RESULT_ERR_ID: &str = "core.result.err";
pub(crate) const RESULT_ERR_ERROR_ID: &str = "core.result.err.error";

pub(crate) fn declarations() -> &'static [TypeDeclaration] {
    static DECLARATIONS: OnceLock<Vec<TypeDeclaration>> = OnceLock::new();
    DECLARATIONS.get_or_init(|| vec![option(), result()])
}

pub(crate) fn is_reserved_type_name(name: &str) -> bool {
    matches!(name, "Option" | "Result")
}

pub(crate) fn is_compiler_owned_id(id: &str) -> bool {
    matches!(
        id,
        OPTION_ID
            | OPTION_NONE_ID
            | OPTION_SOME_ID
            | OPTION_SOME_VALUE_ID
            | RESULT_ID
            | RESULT_OK_ID
            | RESULT_OK_VALUE_ID
            | RESULT_ERR_ID
            | RESULT_ERR_ERROR_ID
    )
}

pub(crate) fn all_ids() -> [&'static str; 9] {
    [
        OPTION_ID,
        OPTION_NONE_ID,
        OPTION_SOME_ID,
        OPTION_SOME_VALUE_ID,
        RESULT_ID,
        RESULT_OK_ID,
        RESULT_OK_VALUE_ID,
        RESULT_ERR_ID,
        RESULT_ERR_ERROR_ID,
    ]
}

pub(crate) fn contract_bytes_v1() -> Vec<u8> {
    let mut output = String::new();
    writeln!(output, "{SCHEMA_V1}").expect("writing to String cannot fail");
    for declaration in declarations() {
        write!(
            output,
            "variant {} {}<",
            declaration.stable_id, declaration.name
        )
        .expect("writing to String cannot fail");
        for (index, parameter) in declaration.type_parameters.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            output.push_str(&parameter.name);
        }
        output.push_str(">\n");
        let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
            unreachable!("the ordinary prelude contains only variants")
        };
        for case in cases {
            write!(
                output,
                "{} {} {}",
                case_index(cases, case),
                case.stable_id,
                case.name
            )
            .expect("writing to String cannot fail");
            for field in &case.fields {
                write!(output, " {}:{}:{}", field.stable_id, field.name, field.ty)
                    .expect("writing to String cannot fail");
            }
            output.push('\n');
        }
    }
    output.into_bytes()
}

pub(crate) fn digest_v1() -> [u8; 32] {
    Sha256::digest(contract_bytes_v1()).into()
}

pub(crate) fn digest_text_v1() -> String {
    let digest = digest_v1();
    let mut output = String::with_capacity("sha256:".len() + digest.len() * 2);
    output.push_str("sha256:");
    for byte in digest {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn case_index(cases: &[VariantCaseDeclaration], case: &VariantCaseDeclaration) -> usize {
    cases
        .iter()
        .position(|candidate| std::ptr::eq(candidate, case))
        .expect("prelude case belongs to its declaration")
}

fn parameter(name: &str) -> TypeParameterDeclaration {
    TypeParameterDeclaration {
        name: name.to_owned(),
        span: Span::default(),
    }
}

fn parameter_type(name: &str) -> Type {
    Type::Named {
        name: name.to_owned(),
        arguments: Vec::new(),
    }
}

fn field(id: &str, name: &str, ty: Type) -> FieldDeclaration {
    FieldDeclaration {
        stable_id: id.to_owned(),
        explicit_id: true,
        name: name.to_owned(),
        name_span: Span::default(),
        ty,
        span: Span::default(),
    }
}

fn case(id: &str, name: &str, fields: Vec<FieldDeclaration>) -> VariantCaseDeclaration {
    VariantCaseDeclaration {
        stable_id: id.to_owned(),
        explicit_id: true,
        name: name.to_owned(),
        name_span: Span::default(),
        fields,
        span: Span::default(),
    }
}

fn option() -> TypeDeclaration {
    TypeDeclaration {
        stable_id: OPTION_ID.to_owned(),
        explicit_id: true,
        name: "Option".to_owned(),
        name_span: Span::default(),
        type_parameters: vec![parameter("T")],
        kind: TypeDeclarationKind::Variant {
            cases: vec![
                case(OPTION_NONE_ID, "None", Vec::new()),
                case(
                    OPTION_SOME_ID,
                    "Some",
                    vec![field(OPTION_SOME_VALUE_ID, "value", parameter_type("T"))],
                ),
            ],
        },
        extends: None,
        span: Span::default(),
    }
}

fn result() -> TypeDeclaration {
    TypeDeclaration {
        stable_id: RESULT_ID.to_owned(),
        explicit_id: true,
        name: "Result".to_owned(),
        name_span: Span::default(),
        type_parameters: vec![parameter("T"), parameter("E")],
        kind: TypeDeclarationKind::Variant {
            cases: vec![
                case(
                    RESULT_OK_ID,
                    "Ok",
                    vec![field(RESULT_OK_VALUE_ID, "value", parameter_type("T"))],
                ),
                case(
                    RESULT_ERR_ID,
                    "Err",
                    vec![field(RESULT_ERR_ERROR_ID, "error", parameter_type("E"))],
                ),
            ],
        },
        extends: None,
        span: Span::default(),
    }
}
