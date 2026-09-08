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
    FieldDeclaration, ModuleUseKind, Span, Type, TypeDeclaration, TypeDeclarationKind,
    TypeParameterDeclaration, VariantCaseDeclaration,
};

pub(crate) const SCHEMA_V1: &str = "semaprax.prelude.v1";
pub(crate) const SCHEMA_V2: &str = "semaprax.prelude.v2";
pub(crate) const SCHEMA_V3: &str = "semaprax.prelude.v3";
pub(crate) const SCHEMA_V4: &str = "semaprax.prelude.v4";
pub(crate) const SCHEMA_V5: &str = "semaprax.prelude.v5";

pub(crate) const OPTION_ID: &str = "core.option";
pub(crate) const OPTION_NONE_ID: &str = "core.option.none";
pub(crate) const OPTION_SOME_ID: &str = "core.option.some";
pub(crate) const OPTION_SOME_VALUE_ID: &str = "core.option.some.value";

pub(crate) const RESULT_ID: &str = "core.result";
pub(crate) const RESULT_OK_ID: &str = "core.result.ok";
pub(crate) const RESULT_OK_VALUE_ID: &str = "core.result.ok.value";
pub(crate) const RESULT_ERR_ID: &str = "core.result.err";
pub(crate) const RESULT_ERR_ERROR_ID: &str = "core.result.err.error";
pub(crate) const VEC_ID: &str = "core.vec";
pub(crate) const BOX_ID: &str = "core.box";

pub(crate) fn declarations() -> &'static [TypeDeclaration] {
    static DECLARATIONS: OnceLock<Vec<TypeDeclaration>> = OnceLock::new();
    DECLARATIONS.get_or_init(|| vec![option(), result(), owned_vec(), owned_box()])
}

pub(crate) fn declarations_for_program(
    program: &crate::ast::Program,
) -> &'static [TypeDeclaration] {
    if crate::box_ops::program_uses_owned_payload(program) || program_uses_box(program) {
        declarations()
    } else if program_uses_vec(program) {
        &declarations()[..3]
    } else {
        &declarations()[..2]
    }
}

pub(crate) fn is_reserved_type_name(name: &str) -> bool {
    matches!(name, "Option" | "Result" | "Vec")
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
            | VEC_ID
            | BOX_ID
    )
}

pub(crate) fn all_ids_v1() -> [&'static str; 9] {
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

pub(crate) fn all_type_ids_v2() -> [&'static str; 10] {
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
        VEC_ID,
    ]
}
pub(crate) fn all_type_ids_v4() -> [&'static str; 11] {
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
        VEC_ID,
        BOX_ID,
    ]
}

pub(crate) fn all_reserved_ids() -> [&'static str; 22] {
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
        VEC_ID,
        crate::vec_ops::WITH_CAPACITY_ID,
        crate::vec_ops::PUSH_ID,
        crate::vec_ops::LEN_ID,
        crate::vec_ops::CAPACITY_ID,
        crate::vec_ops::GET_ID,
        crate::vec_ops::RESERVE_EXACT_ID,
        crate::vec_ops::SET_ID,
        crate::vec_ops::CLEAR_ID,
        BOX_ID,
        crate::box_ops::NEW_ID,
        crate::box_ops::GET_ID,
        crate::box_ops::INTO_INNER_ID,
    ]
}

pub(crate) fn all_ids() -> [&'static str; 9] {
    all_ids_v1()
}

pub(crate) fn contract_bytes_v1() -> Vec<u8> {
    contract_bytes_for(SCHEMA_V1, &declarations()[..2])
}

pub(crate) fn contract_bytes_v2() -> Vec<u8> {
    let mut output = contract_bytes_for(SCHEMA_V2, &declarations()[..3]);
    write_vec_contract(&mut output);
    output
}

pub(crate) fn contract_bytes_v3() -> Vec<u8> {
    let mut output = contract_bytes_for(SCHEMA_V3, &declarations()[..3]);
    write_vec_contract(&mut output);
    write_vec_v3_contract(&mut output);
    output
}

fn write_vec_v3_contract(output: &mut Vec<u8>) {
    let mut contract = String::new();
    write!(
        contract,
        "operation {} {} <T>(own:Vec<T>,value:usize)->own:Vec<T>\noperation {} {} <T>(own:Vec<T>,value:usize,value:T)->own:Vec<T>\noperation {} {} <T>(own:Vec<T>)->own:Vec<T>\nrule reserve_exact target_capacity=max(old_capacity,length+additional) overflow_or_above_max_or_allocation_failure={}:{}\nrule set index_out_of_bounds={}:{}\nrule clear length=0 capacity=old_capacity\nrule successful_owner_mutation generation=next\n",
        crate::vec_ops::RESERVE_EXACT_ID,
        crate::vec_ops::RESERVE_EXACT_NAME,
        crate::vec_ops::SET_ID,
        crate::vec_ops::SET_NAME,
        crate::vec_ops::CLEAR_ID,
        crate::vec_ops::CLEAR_NAME,
        crate::vec_ops::STATUS_DOMAIN,
        crate::vec_ops::ALLOCATION_FAILURE_CODE,
        crate::vec_ops::STATUS_DOMAIN,
        crate::vec_ops::GET_OUT_OF_BOUNDS_CODE,
    )
    .expect("writing to String cannot fail");
    output.extend_from_slice(contract.as_bytes());
}

pub(crate) fn contract_bytes_v4() -> Vec<u8> {
    let mut output = contract_bytes_for(SCHEMA_V4, declarations());
    write_vec_contract(&mut output);
    write_vec_v3_contract(&mut output);
    let mut contract = String::new();
    write!(contract, "operation {} {} <T>(value:T)->own:Box<T>\noperation {} {} <T>(borrow:Box<T>)->value:T\noperation {} {} <T>(own:Box<T>)->value:T\nelements i64,i32,u8,usize,char,f32,f64,bool\nlimit max_live_allocations {}\nstatus {} allocation_failure:{}\n", crate::box_ops::NEW_ID, crate::box_ops::NEW_NAME, crate::box_ops::GET_ID, crate::box_ops::GET_NAME, crate::box_ops::INTO_INNER_ID, crate::box_ops::INTO_INNER_NAME, crate::box_ops::MAX_LIVE_ALLOCATIONS, crate::box_ops::STATUS_DOMAIN, crate::box_ops::ALLOCATION_FAILURE_CODE).expect("writing to String cannot fail");
    output.extend_from_slice(contract.as_bytes());
    output
}
pub(crate) fn contract_bytes_v5() -> Vec<u8> {
    let mut output = contract_bytes_v4();
    let legacy = String::from_utf8(output.clone()).expect("prelude contract is UTF-8");
    output = legacy.replacen(SCHEMA_V4, SCHEMA_V5, 1).into_bytes();
    output.extend_from_slice(b"operation core.box.new box_new <Bytes>(own:Bytes)->own:Box<Bytes>\noperation core.box.into-inner box_into_inner <Bytes>(own:Box<Bytes>)->own:Bytes\nrule core.box.get <Bytes>=closed\nwasm_imports spx_box_new_v2,spx_box_get_v2,spx_box_into_inner_v2,spx_box_drop_v2\n");
    output
}

fn write_vec_contract(output: &mut Vec<u8>) {
    let mut contract = String::new();
    write!(
        contract,
        "elements i64,i32,u8,usize,char,f32,f64,bool\noperation {} {} <T>(value:usize)->own:Vec<T>\noperation {} {} <T>(own:Vec<T>,value:T)->own:Vec<T>\noperation {} {} <T>(borrow:Vec<T>)->value:usize\noperation {} {} <T>(borrow:Vec<T>)->value:usize\noperation {} {} <T>(borrow:Vec<T>,value:usize)->value:T\nlimit max_capacity {}\nstatus {} push_full:{} get_out_of_bounds:{} allocation_failure:{}\n",
        crate::vec_ops::WITH_CAPACITY_ID,
        crate::vec_ops::WITH_CAPACITY_NAME,
        crate::vec_ops::PUSH_ID,
        crate::vec_ops::PUSH_NAME,
        crate::vec_ops::LEN_ID,
        crate::vec_ops::LEN_NAME,
        crate::vec_ops::CAPACITY_ID,
        crate::vec_ops::CAPACITY_NAME,
        crate::vec_ops::GET_ID,
        crate::vec_ops::GET_NAME,
        crate::vec_ops::MAX_CAPACITY,
        crate::vec_ops::STATUS_DOMAIN,
        crate::vec_ops::PUSH_FULL_CODE,
        crate::vec_ops::GET_OUT_OF_BOUNDS_CODE,
        crate::vec_ops::ALLOCATION_FAILURE_CODE,
    )
    .expect("writing to String cannot fail");
    output.extend_from_slice(contract.as_bytes());
}

fn contract_bytes_for(schema: &str, declarations: &[TypeDeclaration]) -> Vec<u8> {
    let mut output = String::new();
    writeln!(output, "{schema}").expect("writing to String cannot fail");
    for declaration in declarations {
        write!(
            output,
            "{} {} {}<",
            match declaration.kind {
                TypeDeclarationKind::Variant { .. } => "variant",
                TypeDeclarationKind::Record { .. } => "record",
                TypeDeclarationKind::Class { .. } => "class",
                TypeDeclarationKind::Resource { .. } => "resource",
            },
            declaration.stable_id,
            declaration.name
        )
        .expect("writing to String cannot fail");
        for (index, parameter) in declaration.type_parameters.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            output.push_str(&parameter.name);
        }
        output.push_str(">\n");
        match &declaration.kind {
            TypeDeclarationKind::Variant { cases } => {
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
            TypeDeclarationKind::Record { fields } => {
                for field in fields {
                    writeln!(
                        output,
                        "field {}:{}:{}",
                        field.stable_id, field.name, field.ty
                    )
                    .expect("writing to String cannot fail");
                }
            }
            TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. } => {
                unreachable!("the ordinary prelude contains only variants and records")
            }
        }
    }
    output.into_bytes()
}

pub(crate) fn digest_v1() -> [u8; 32] {
    Sha256::digest(contract_bytes_v1()).into()
}

pub(crate) fn digest_v2() -> [u8; 32] {
    Sha256::digest(contract_bytes_v2()).into()
}

pub(crate) fn digest_v3() -> [u8; 32] {
    Sha256::digest(contract_bytes_v3()).into()
}
pub(crate) fn digest_v4() -> [u8; 32] {
    Sha256::digest(contract_bytes_v4()).into()
}
pub(crate) fn digest_v5() -> [u8; 32] {
    Sha256::digest(contract_bytes_v5()).into()
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

pub(crate) fn digest_text_v2() -> String {
    let digest = digest_v2();
    let mut output = String::with_capacity("sha256:".len() + digest.len() * 2);
    output.push_str("sha256:");
    for byte in digest {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

pub(crate) fn digest_text_v3() -> String {
    let digest = digest_v3();
    let mut output = String::with_capacity("sha256:".len() + digest.len() * 2);
    output.push_str("sha256:");
    for byte in digest {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
pub(crate) fn digest_text_v4() -> String {
    let digest = digest_v4();
    let mut output = String::with_capacity("sha256:".len() + digest.len() * 2);
    output.push_str("sha256:");
    for byte in digest {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
pub(crate) fn digest_text_v5() -> String {
    let digest = digest_v5();
    let mut output = String::with_capacity("sha256:".len() + digest.len() * 2);
    output.push_str("sha256:");
    for byte in digest {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

pub(crate) fn program_uses_box(program: &crate::ast::Program) -> bool {
    fn function_uses_box(function: &crate::ast::Function) -> bool {
        crate::box_ops::wrapper_by_id(&function.stable_id).is_some()
            || function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(|expression| {
                    let mut found = false;
                    expression.visit_calls(&mut |name, _| {
                        found |= crate::box_ops::by_name(name).is_some()
                    });
                    found
                })
    }
    program.module_uses.iter().any(|module_use| module_use.kind == ModuleUseKind::Function && module_use.target_module == crate::box_ops::MODULE && crate::box_ops::wrapper_by_id(&module_use.persistent_id).is_some())
        || program.functions.iter().any(function_uses_box)
        || program.types.iter().any(|declaration| matches!(&declaration.kind, TypeDeclarationKind::Class { methods, .. } if methods.iter().any(function_uses_box)))
}

fn vec_v3_op(op: crate::vec_ops::VecOp) -> bool {
    matches!(
        op,
        crate::vec_ops::VecOp::ReserveExact
            | crate::vec_ops::VecOp::Set
            | crate::vec_ops::VecOp::Clear
    )
}

pub(crate) fn program_uses_vec_v3(program: &crate::ast::Program) -> bool {
    fn function_uses_vec_v3(function: &crate::ast::Function) -> bool {
        crate::vec_ops::wrapper_by_id(&function.stable_id).is_some_and(vec_v3_op)
            || function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(|expression| {
                    let mut found = false;
                    expression.visit_calls(&mut |name, _| {
                        found |= crate::vec_ops::by_name(name).is_some_and(vec_v3_op);
                    });
                    found
                })
    }

    program.module_uses.iter().any(|module_use| {
        module_use.kind == ModuleUseKind::Function
            && module_use.target_module == crate::vec_ops::MODULE
            && crate::vec_ops::wrapper_by_id(&module_use.persistent_id).is_some_and(vec_v3_op)
    }) || program.functions.iter().any(function_uses_vec_v3)
        || program.types.iter().any(|declaration| {
            matches!(
                &declaration.kind,
                TypeDeclarationKind::Class { methods, .. }
                    if methods.iter().any(function_uses_vec_v3)
            )
        })
}

pub(crate) fn program_uses_vec(program: &crate::ast::Program) -> bool {
    fn type_uses_vec(ty: &Type) -> bool {
        let mut pending = vec![ty];
        while let Some(ty) = pending.pop() {
            if let Type::Named { name, arguments } = ty {
                if name == "Vec" {
                    return true;
                }
                pending.extend(arguments);
            }
        }
        false
    }

    fn function_uses_vec(function: &crate::ast::Function) -> bool {
        if type_uses_vec(&function.return_type)
            || function.params.iter().any(|param| type_uses_vec(&param.ty))
        {
            return true;
        }
        let mut found = false;
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            expression.visit_calls(&mut |name, _| {
                found |= crate::vec_ops::by_name(name).is_some();
            });
        }
        found
    }

    program.types.iter().any(|declaration| {
        declaration.extends.as_ref().is_some_and(type_uses_vec)
            || match &declaration.kind {
                TypeDeclarationKind::Resource { .. } => false,
                TypeDeclarationKind::Record { fields } => {
                    fields.iter().any(|field| type_uses_vec(&field.ty))
                }
                TypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .any(|field| type_uses_vec(&field.ty)),
                TypeDeclarationKind::Class { fields, methods } => {
                    fields.iter().any(|field| type_uses_vec(&field.ty))
                        || methods.iter().any(function_uses_vec)
                }
            }
    }) || program.functions.iter().any(function_uses_vec)
}

#[cfg(test)]
pub(crate) fn source_uses_vec(source: &str) -> bool {
    let Ok(program) = crate::parse(source, "<prelude-selection>") else {
        return false;
    };
    program_uses_vec(&program)
}

pub(crate) fn selected_for_source(source: &str) -> (&'static str, Vec<u8>, String) {
    let Ok(program) = crate::parse(source, "<prelude-selection>") else {
        return (SCHEMA_V1, contract_bytes_v1(), digest_text_v1());
    };
    selected_for_program(&program)
}

pub(crate) fn selected_for_program(
    program: &crate::ast::Program,
) -> (&'static str, Vec<u8>, String) {
    if crate::box_ops::program_uses_owned_payload(program) {
        (SCHEMA_V5, contract_bytes_v5(), digest_text_v5())
    } else if program_uses_box(program) {
        (SCHEMA_V4, contract_bytes_v4(), digest_text_v4())
    } else if program_uses_vec_v3(program) {
        (SCHEMA_V3, contract_bytes_v3(), digest_text_v3())
    } else if program_uses_vec(program) {
        (SCHEMA_V2, contract_bytes_v2(), digest_text_v2())
    } else {
        (SCHEMA_V1, contract_bytes_v1(), digest_text_v1())
    }
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

fn owned_vec() -> TypeDeclaration {
    TypeDeclaration {
        stable_id: VEC_ID.to_owned(),
        explicit_id: true,
        name: "Vec".to_owned(),
        name_span: Span::default(),
        type_parameters: vec![parameter("T")],
        kind: TypeDeclarationKind::Record { fields: Vec::new() },
        extends: None,
        span: Span::default(),
    }
}

fn owned_box() -> TypeDeclaration {
    TypeDeclaration {
        stable_id: BOX_ID.to_owned(),
        explicit_id: true,
        name: "Box".to_owned(),
        name_span: Span::default(),
        type_parameters: vec![parameter("T")],
        kind: TypeDeclarationKind::Record { fields: Vec::new() },
        extends: None,
        span: Span::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_vec_changes_the_bound_digest_without_reinterpreting_the_legacy_contract() {
        assert_eq!(
            String::from_utf8(contract_bytes_v1()).unwrap(),
            "semaprax.prelude.v1\nvariant core.option Option<T>\n0 core.option.none None\n1 core.option.some Some core.option.some.value:value:T\nvariant core.result Result<T,E>\n0 core.result.ok Ok core.result.ok.value:value:T\n1 core.result.err Err core.result.err.error:error:E\n"
        );
        assert_eq!(
            digest_text_v1(),
            "sha256:d37bad7e3911669bbf2c66b25c8b31d5c2e36eb181cc54fdc86c3a49a8fb9c5e"
        );
        assert_ne!(digest_v1(), digest_v2());
        assert_eq!(
            digest_text_v2(),
            "sha256:ee05c5fc884e558eee9dc89f351af530494bebecaff940081cfae3b9ce805291"
        );
        assert_eq!(
            String::from_utf8(contract_bytes_v2()).unwrap(),
            "semaprax.prelude.v2\nvariant core.option Option<T>\n0 core.option.none None\n1 core.option.some Some core.option.some.value:value:T\nvariant core.result Result<T,E>\n0 core.result.ok Ok core.result.ok.value:value:T\n1 core.result.err Err core.result.err.error:error:E\nrecord core.vec Vec<T>\nelements i64,i32,u8,usize,char,f32,f64,bool\noperation core.vec.with-capacity vec_with_capacity <T>(value:usize)->own:Vec<T>\noperation core.vec.push vec_push <T>(own:Vec<T>,value:T)->own:Vec<T>\noperation core.vec.len vec_len <T>(borrow:Vec<T>)->value:usize\noperation core.vec.capacity vec_capacity <T>(borrow:Vec<T>)->value:usize\noperation core.vec.get vec_get <T>(borrow:Vec<T>,value:usize)->value:T\nlimit max_capacity 8192\nstatus semaprax.vec.v1 push_full:1 get_out_of_bounds:2 allocation_failure:3\n"
        );
        assert_eq!(
            digest_text_v3(),
            "sha256:7df663ea708bfbb4c8b98a07607d992ac01b2b1a1942505c9fb305a43fc93938"
        );
        assert_eq!(
            String::from_utf8(contract_bytes_v3()).unwrap(),
            "semaprax.prelude.v3\nvariant core.option Option<T>\n0 core.option.none None\n1 core.option.some Some core.option.some.value:value:T\nvariant core.result Result<T,E>\n0 core.result.ok Ok core.result.ok.value:value:T\n1 core.result.err Err core.result.err.error:error:E\nrecord core.vec Vec<T>\nelements i64,i32,u8,usize,char,f32,f64,bool\noperation core.vec.with-capacity vec_with_capacity <T>(value:usize)->own:Vec<T>\noperation core.vec.push vec_push <T>(own:Vec<T>,value:T)->own:Vec<T>\noperation core.vec.len vec_len <T>(borrow:Vec<T>)->value:usize\noperation core.vec.capacity vec_capacity <T>(borrow:Vec<T>)->value:usize\noperation core.vec.get vec_get <T>(borrow:Vec<T>,value:usize)->value:T\nlimit max_capacity 8192\nstatus semaprax.vec.v1 push_full:1 get_out_of_bounds:2 allocation_failure:3\noperation core.vec.reserve-exact vec_reserve_exact <T>(own:Vec<T>,value:usize)->own:Vec<T>\noperation core.vec.set vec_set <T>(own:Vec<T>,value:usize,value:T)->own:Vec<T>\noperation core.vec.clear vec_clear <T>(own:Vec<T>)->own:Vec<T>\nrule reserve_exact target_capacity=max(old_capacity,length+additional) overflow_or_above_max_or_allocation_failure=semaprax.vec.v1:3\nrule set index_out_of_bounds=semaprax.vec.v1:2\nrule clear length=0 capacity=old_capacity\nrule successful_owner_mutation generation=next\n"
        );
        assert!(!source_uses_vec("module test.scalar; fn main()->i64{0}"));
        assert!(!source_uses_vec(
            "module test.text; fn main()->string{\"Vec<i64> vec_push\"}"
        ));
        assert!(!source_uses_vec(
            "module test.names; fn main()->i64{let vec_len=1; let vec_push=2; if vec_len < vec_push { vec_len } else { vec_push }}"
        ));
        assert!(source_uses_vec(
            "module test.vec; fn main()->Vec<i64>{vec_with_capacity<i64>(1usize)}"
        ));
        assert_eq!(
            selected_for_source(
                "module test.old_vec; fn main()->Vec<i64>{vec_with_capacity<i64>(1usize)}"
            )
            .0,
            SCHEMA_V2
        );
        assert_eq!(
            selected_for_source(
                "module test.new_vec; fn main()->Vec<i64>{let mut v=vec_with_capacity<i64>(1usize);v=vec_clear<i64>(v);v}"
            )
            .0,
            SCHEMA_V3
        );
        assert_eq!(
            selected_for_source(
                "module test.imported; use function @id(\"std.collections.vec.clear\") from std.collections as wipe; fn wipe_once(values: own Vec<i64>)->Vec<i64>{wipe<i64>(values)}"
            )
            .0,
            SCHEMA_V3
        );
        assert_eq!(
            selected_for_source(
                "module test.imported_old; use function @id(\"std.collections.vec.push\") from std.collections as append; fn append_once(values: own Vec<i64>)->Vec<i64>{append<i64>(values,1)}"
            )
            .0,
            SCHEMA_V2
        );
        assert_eq!(
            selected_for_source(
                "module test.imported_spoof; use function @id(\"std.collections.vec.clear\") from user.collections as wipe; fn wipe_once(values: own Vec<i64>)->Vec<i64>{wipe<i64>(values)}"
            )
            .0,
            SCHEMA_V2
        );
    }

    #[test]
    fn box_selects_exact_additive_v4_without_reinterpreting_authored_box() {
        let contract = String::from_utf8(contract_bytes_v4()).unwrap();
        assert!(contract.starts_with("semaprax.prelude.v4\n"));
        for fact in [
            "record core.box Box<T>",
            "operation core.box.new box_new <T>(value:T)->own:Box<T>",
            "operation core.box.get box_get <T>(borrow:Box<T>)->value:T",
            "operation core.box.into-inner box_into_inner <T>(own:Box<T>)->value:T",
            "limit max_live_allocations 4096",
            "status semaprax.box.v1 allocation_failure:1",
            "operation core.vec.clear vec_clear",
            "rule successful_owner_mutation generation=next",
        ] {
            assert!(contract.contains(fact), "missing {fact}");
        }
        assert_eq!(
            digest_text_v4(),
            "sha256:04d6c7bcff11395cfe72c429f6cf1b076f766fdbabba423edbb02e1692d1fd89"
        );
        assert_eq!(selected_for_source("module test.box;@id(\"app.main\") fn main()->i64{box_into_inner<i64>(box_new<i64>(1))}").0,SCHEMA_V4);
        assert_eq!(selected_for_source("module test.mixed;@id(\"app.main\") fn main()->i64{let mut v=vec_with_capacity<i64>(0usize);v=vec_clear<i64>(v);box_into_inner<i64>(box_new<i64>(1))}").0,SCHEMA_V4);
        assert_eq!(selected_for_source("module test.imported;use function @id(\"std.mem.box.new\") from std.mem as new;@id(\"app.main\") fn main()->i64{let value=new<i64>(1);0}").0,SCHEMA_V4);
        assert_eq!(selected_for_source("module test.spoof;use function @id(\"std.mem.box.new\") from user.mem as new;@id(\"app.main\") fn main()->i64{let value=new<i64>(1);0}").0,SCHEMA_V1);
        assert_eq!(selected_for_source("module test.legacy;@id(\"legacy.box\") record Box<T>{@id(\"legacy.box.value\") value:T,}@id(\"app.main\") fn main()->i64{0}").0,SCHEMA_V1);
    }
}
