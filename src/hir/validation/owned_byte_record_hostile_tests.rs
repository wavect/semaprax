use super::*;

const DECLARATION_FIXTURE: &str = r#"
module test.owned_byte_hir_shapes;
@id("box.type") class Box { @id("box.payload") payload: i64, }
@id("envelope.type") variant Envelope {
  @id("envelope.data") Data { @id("envelope.payload") payload: i64, },
}
@id("inner.type") record Inner { @id("inner.payload") payload: i64, }
@id("outer.type") record Outer { @id("outer.inner") inner: Inner, }
@id("marker.type") record Marker { @id("marker.value") value: i64, }
@id("packet.type") record Packet {
  @id("packet.payload") payload: i64,
  @id("packet.marker") marker: Marker,
}
@id("app.main") fn main() -> i64 { 0 }
"#;

fn declaration_fixture() -> ResolvedProgram {
    let parsed = crate::parse(
        DECLARATION_FIXTURE,
        std::path::Path::new("owned-byte-hostile-declarations.spx"),
    )
    .expect("fixture parses");
    crate::hir::resolve(&parsed).expect("Copy-only declaration fixture resolves")
}

fn set_record_field_type(
    program: &mut ResolvedProgram,
    owner: &str,
    field: &str,
    ty: ResolvedType,
) {
    let owner = DeclarationId::new(owner);
    let field = DeclarationId::new(field);
    let declaration = program
        .types
        .iter_mut()
        .find(|candidate| candidate.id == owner)
        .expect("record/class declaration");
    let fields = match &mut declaration.kind {
        ResolvedTypeDeclarationKind::Record { fields }
        | ResolvedTypeDeclarationKind::Class { fields, .. } => fields,
        _ => panic!("fixture declaration is record-like"),
    };
    fields
        .iter_mut()
        .find(|candidate| candidate.id == field)
        .expect("record/class field")
        .ty = ty.clone();
    program
        .declarations
        .record_fields
        .get_mut(&owner)
        .expect("indexed record/class fields")
        .iter_mut()
        .find(|candidate| candidate.id == field)
        .expect("indexed record/class field")
        .ty = ty;
    program.declarations.type_facts_by_id.clear();
}

fn set_variant_field_type(
    program: &mut ResolvedProgram,
    owner: &str,
    case: &str,
    field: &str,
    ty: ResolvedType,
) {
    let owner = DeclarationId::new(owner);
    let case = DeclarationId::new(case);
    let field = DeclarationId::new(field);
    let declaration = program
        .types
        .iter_mut()
        .find(|candidate| candidate.id == owner)
        .expect("variant declaration");
    let ResolvedTypeDeclarationKind::Variant { cases } = &mut declaration.kind else {
        panic!("fixture declaration is a variant")
    };
    cases
        .iter_mut()
        .find(|candidate| candidate.id == case)
        .expect("variant case")
        .fields
        .iter_mut()
        .find(|candidate| candidate.id == field)
        .expect("variant field")
        .ty = ty.clone();
    program
        .declarations
        .variant_cases
        .get_mut(&owner)
        .expect("indexed variant cases")
        .iter_mut()
        .find(|candidate| candidate.id == case)
        .expect("indexed variant case")
        .fields
        .iter_mut()
        .find(|candidate| candidate.id == field)
        .expect("indexed variant field")
        .ty = ty.clone();
    program
        .declarations
        .case_fields
        .get_mut(&case)
        .expect("indexed case fields")
        .iter_mut()
        .find(|candidate| candidate.id == field)
        .expect("indexed case field")
        .ty = ty;
    program.declarations.type_facts_by_id.clear();
}

fn assert_hir_rejects(program: &ResolvedProgram, expected: &str) {
    let diagnostic = crate::hir::validate(program).expect_err("hostile HIR must fail closed");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(diagnostic.message.contains(expected), "{diagnostic:?}");
}

#[test]
fn validation_admits_nested_records_but_rejects_non_record_and_forbidden_leaves() {
    let mut class = declaration_fixture();
    set_record_field_type(&mut class, "box.type", "box.payload", ResolvedType::Bytes);
    assert_hir_rejects(
        &class,
        "class contains compiler-owned Bytes outside flat v1",
    );

    let mut variant = declaration_fixture();
    set_record_field_type(
        &mut variant,
        "inner.type",
        "inner.payload",
        ResolvedType::Bytes,
    );
    set_variant_field_type(
        &mut variant,
        "envelope.type",
        "envelope.data",
        "envelope.payload",
        ResolvedType::Nominal {
            declaration: DeclarationId::new("inner.type"),
            arguments: Vec::new(),
        },
    );
    assert_hir_rejects(
        &variant,
        "variant contains compiler-owned Bytes outside flat v1",
    );

    let mut nested = declaration_fixture();
    set_record_field_type(
        &mut nested,
        "inner.type",
        "inner.payload",
        ResolvedType::Bytes,
    );
    crate::hir::validate(&nested).expect("nested monomorphic owned-Bytes record is admitted");

    let mut mixed = declaration_fixture();
    set_record_field_type(
        &mut mixed,
        "packet.type",
        "packet.payload",
        ResolvedType::Bytes,
    );
    crate::hir::validate(&mixed).expect("nested Copy-only record companion is admitted");

    let mut forbidden = mixed;
    set_record_field_type(
        &mut forbidden,
        "marker.type",
        "marker.value",
        ResolvedType::String,
    );
    assert_hir_rejects(
        &forbidden,
        "owned-Bytes record is outside the bounded acyclic nested profile",
    );
}

#[test]
fn validation_rejects_a_projected_borrow_match_scrutinee() {
    let source = r#"
module test.projected_owned_byte_borrow;
@id("packet.type") record Packet {
  @id("packet.payload") payload: Bytes,
  @id("packet.marker") marker: i64,
}
@id("holder.type") record Holder { @id("holder.packet") packet: i64, }
@id("packet.inspect") fn inspect(packet: own Packet) -> i64 {
  match borrow packet { Packet { payload, marker: _ } => 0, }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(
        source,
        std::path::Path::new("projected-owned-byte-borrow.spx"),
    )
    .expect("fixture parses");
    let mut program = crate::hir::resolve(&parsed).expect("flat fixture resolves");
    let packet_ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("packet.type"),
        arguments: Vec::new(),
    };
    set_record_field_type(
        &mut program,
        "holder.type",
        "holder.packet",
        packet_ty.clone(),
    );
    let inspect = {
        let inspect = program
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "packet.inspect")
            .expect("inspect function");
        inspect.params[0].ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("holder.type"),
            arguments: Vec::new(),
        };
        let ResolvedExprKind::Block { tail, .. } = &mut inspect.body.kind else {
            panic!("inspect body remains a block")
        };
        let ResolvedExprKind::Match { scrutinee, .. } = &mut tail.kind else {
            panic!("inspect tail remains a match")
        };
        scrutinee.ty = packet_ty;
        let ResolvedExprKind::Place(place) = &mut scrutinee.kind else {
            panic!("borrow scrutinee remains a place")
        };
        place.projections = vec![PlaceProjection::Field(DeclarationId::new("holder.packet"))];
        inspect.clone()
    };

    // Exercise expression authentication directly so the deliberately
    // nested hostile carrier cannot be rejected first by declaration
    // admission; this test pins the independent unprojected-place check.
    let execution = FunctionExecutionId::Monomorphic(inspect.id.clone());
    let diagnostic = HirValidator::new(&program)
        .expect("hostile identities remain indexed")
        .validate_function(&inspect, &execution)
        .expect_err("projected borrow scrutinee must fail closed");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(
        diagnostic
            .message
            .contains("resolved record match mode disagrees with its scrutinee"),
        "{diagnostic:?}"
    );
}

#[test]
fn validation_rejects_a_non_place_owned_byte_record_borrow_argument() {
    let source = r#"
module test.owned_byte_borrow_argument;
@id("packet.type") record Packet {
  @id("packet.payload") payload: Bytes,
  @id("packet.marker") marker: i64,
}

@id("packet.inspect") fn inspect(packet: borrow Packet) -> i64 { 0 }
@id("packet.caller") fn caller(packet: own Packet) -> i64 { inspect(packet) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(
        source,
        std::path::Path::new("owned-byte-borrow-argument.spx"),
    )
    .expect("fixture parses");
    let program = crate::hir::resolve(&parsed).expect("named-place borrow resolves");
    let caller = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "packet.caller")
        .expect("caller function");
    let ResolvedExprKind::Block { tail, .. } = &caller.body.kind else {
        panic!("caller body remains a block")
    };
    let ResolvedExprKind::Call { args, .. } = &tail.kind else {
        panic!("caller tail remains a call")
    };
    let mut hostile_argument = args[0].clone();
    hostile_argument.kind = ResolvedExprKind::Int(0);
    let param = &program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "packet.inspect")
        .expect("inspect function")
        .params[0];
    let diagnostic = HirValidator::new(&program)
        .expect("fixture identities remain indexed")
        .validate_argument_ownership(&hostile_argument, param)
        .expect_err("non-place record borrow must fail closed");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(
        diagnostic
            .message
            .contains("argument ownership is incompatible"),
        "{diagnostic:?}"
    );
}

#[test]
fn validation_rejects_a_forged_nested_owned_byte_record_update() {
    let source = r#"
module test.nested_update_hostile;
@id("update.inner") record Inner { @id("update.inner.marker") marker: i64, }
@id("update.packet") record Packet {
  @id("update.packet.payload") payload: Bytes,
  @id("update.packet.marker") marker: i64,
}
@id("update.apply") fn update(value: own Packet) -> Packet {
  value with { marker: 1 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(source, std::path::Path::new("nested-update-hostile.spx"))
        .expect("flat update fixture parses");
    let mut program = crate::hir::resolve(&parsed).expect("flat update fixture resolves");
    set_record_field_type(
        &mut program,
        "update.packet",
        "update.packet.marker",
        ResolvedType::Nominal {
            declaration: DeclarationId::new("update.inner"),
            arguments: Vec::new(),
        },
    );
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "update.apply")
        .expect("update function")
        .clone();
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let diagnostic = HirValidator::new(&program)
        .expect("hostile identities remain indexed")
        .validate_function(&function, &execution)
        .expect_err("forged nested update must fail closed");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(
        diagnostic
            .message
            .contains("SPX-O117: record updates over nested owned-Bytes records remain closed"),
        "{diagnostic:?}"
    );
}
