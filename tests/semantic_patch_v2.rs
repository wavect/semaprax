use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::hir::{self, ResolvedExpr, ResolvedExprKind};
use semaprax::{codegen, graph, parse, patch, wasm};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn fixture(label: &str, source: &str) -> (PathBuf, PathBuf, String) {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-semantic-patch-v2-{}-{label}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("change.spatch");
    std::fs::write(&source_path, source).unwrap();
    let revision = graph::revision(&parse(source, &source_path).unwrap());
    (source_path, patch_path, revision)
}

fn declaration_symbol(id: &str) -> String {
    id.bytes()
        .fold(String::from("spx_decl_"), |mut symbol, byte| {
            write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
            symbol
        })
}

fn first_call<'a>(expression: &'a ResolvedExpr, template: &str) -> Option<&'a ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::Call { callee, args, .. } => {
            if callee.as_str() == template {
                return Some(expression);
            }
            args.iter()
                .find_map(|argument| first_call(argument, template))
        }
        ResolvedExprKind::NativeRustImportCall(call) => call
            .args
            .iter()
            .find_map(|argument| first_call(argument, template)),
        ResolvedExprKind::HostCommandCall(call) => call
            .args
            .iter()
            .find_map(|argument| first_call(argument, template)),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. } => first_call(value, template),
        ResolvedExprKind::Binary { left, right, .. } => {
            first_call(left, template).or_else(|| first_call(right, template))
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => first_call(source, template)
            .or_else(|| first_call(start, template))
            .or_else(|| first_call(end, template)),
        ResolvedExprKind::Block { statements, tail } => statements
            .iter()
            .find_map(|statement| first_call(statement.value(), template))
            .or_else(|| first_call(tail, template)),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => first_call(condition, template)
            .or_else(|| first_call(then_branch, template))
            .or_else(|| first_call(else_branch, template)),
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .find_map(|field| first_call(&field.value, template)),
        ResolvedExprKind::Match { scrutinee, arms } => first_call(scrutinee, template)
            .or_else(|| arms.iter().find_map(|arm| first_call(&arm.value, template))),
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            first_call(base, template).or_else(|| {
                fields
                    .iter()
                    .find_map(|field| first_call(&field.value, template))
            })
        }
        ResolvedExprKind::Project { base, .. } => first_call(base, template),
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
        | ResolvedExprKind::BorrowPlace { .. } => None,
    }
}

#[test]
fn schema_less_v1_rename_remains_successful_and_v2_ops_require_schema() {
    let source = "module patch.v1;\n@id(\"helper.answer\") fn answer()->i64{42}\n@id(\"app.main\") fn main()->i64{answer()}\n";
    let (source_path, patch_path, revision) = fixture("v1", source);
    std::fs::write(
        &patch_path,
        format!("base {revision}\nrename helper.answer to computed\n"),
    )
    .unwrap();
    patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    assert!(changed.contains("fn computed()"));
    assert!(changed.contains("computed()"));

    let (source_path, patch_path, revision) = fixture("schema-confusion", source);
    std::fs::write(
        &patch_path,
        format!(
            "base {revision}\nrename-member owner record.x member record.x.field.y to renamed\n"
        ),
    )
    .unwrap();
    let diagnostics = patch::apply(&source_path, &patch_path).unwrap_err();
    assert_eq!(diagnostics[0].code, "SPX-G101");
    assert_eq!(std::fs::read_to_string(source_path).unwrap(), source);
}

#[test]
fn noncanonical_two_index_generic_call_replacement_is_one_atomic_instance_delta() {
    let source = r#"module patch.generic;
@id("generic.marker") fn marker<T,U>()->bool{true}
@id("app.main") fn main()->i64{if marker<i64,bool>()&&marker<i64,bool>() {1}else{0}}
"#;
    let program = parse(source, Path::new("generic-patch.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let main = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let call = first_call(&main.body, "generic.marker").unwrap();
    let ResolvedExprKind::Call {
        instance: Some(instance),
        ..
    } = &call.kind
    else {
        panic!("marker call must have a concrete instance")
    };
    let (source_path, patch_path, revision) = fixture("two-index", source);
    std::fs::write(
        &patch_path,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 1 from bool to i64\nrequire no-new-effects\n",
            call.id, instance, call.id, instance
        ),
    )
    .unwrap();

    patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    assert!(changed.contains("marker<bool, i64>()"));
    assert!(changed.contains("marker<i64, bool>()"));
    let after = hir::resolve(&parse(&changed, &source_path).unwrap()).unwrap();
    let main = after
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let after_call = first_call(&main.body, "generic.marker").unwrap();
    let ResolvedExprKind::Call {
        type_arguments,
        instance: Some(after_instance),
        ..
    } = &after_call.kind
    else {
        panic!("patched marker call must have one concrete instance")
    };
    assert_eq!(
        type_arguments,
        &[hir::ResolvedType::Bool, hir::ResolvedType::I64]
    );
    assert_eq!(
        after_instance,
        &hir::FunctionInstanceId::derive(
            &hir::DeclarationId::new("generic.marker"),
            type_arguments
        )
    );
    assert_eq!(after.function_instances.len(), 2);
}

#[test]
fn member_rename_expands_only_pattern_shorthand_and_preserves_binding_identity() {
    let source = r#"module patch.member;
@id("patch.box") record Box { @id("patch.box.value") value: i64, }
@id("patch.other") record Other { @id("patch.other.value") value: i64, }
@id("patch.outer") record Outer { @id("patch.outer.inner") inner: Box, }
@id("patch.extract") fn extract(input: Box)->i64{match input {Box { value }=>value,}}
@id("patch.nested") fn nested(input: Outer)->i64{match input {Outer { inner: Box { value } }=>value,}}
@id("patch.bump") fn bump(input: Box)->i64{let next=input with { value: input.value+1 };next.value}
@id("app.main") fn main()->i64{extract(Box { value: 38 })+nested(Outer { inner: Box { value: 2 } })+bump(Box { value: 1 })+Other { value: 0 }.value}
"#;
    let before = hir::resolve(&parse(source, Path::new("member-before.spx")).unwrap()).unwrap();
    let extract = before
        .functions
        .iter()
        .find(|function| function.id.as_str() == "patch.extract")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &extract.body.kind else {
        panic!("extract body must be a block")
    };
    let ResolvedExprKind::Match { arms, .. } = &tail.kind else {
        panic!("extract tail must be a match")
    };
    let hir::ResolvedMatchPattern::Record { fields, .. } = &arms[0].pattern else {
        panic!("extract must use a record pattern")
    };
    let hir::ResolvedRecordMatchFieldPattern::Binding(binding) = &fields[0].pattern else {
        panic!("record field must bind")
    };
    let ResolvedExprKind::Place(place) = &arms[0].value.kind else {
        panic!("match value must reference the binding")
    };
    let before_binding_id = binding.id.clone();
    let before_place_root = place.root.clone();

    let (source_path, patch_path, revision) = fixture("member-shorthand", source);
    std::fs::write(
        &patch_path,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nrename-member owner patch.box member patch.box.value to payload\n"
        ),
    )
    .unwrap();
    patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    assert!(changed.contains("payload: i64"));
    assert!(changed.contains("Box { payload: value } => value"));
    assert!(changed.contains("Outer { inner: Box { payload: value } } => value"));
    assert!(changed.contains("input with { payload: input.payload + 1 }"));
    assert!(changed.contains("next.payload"));
    assert!(changed.contains("Box { payload: 38 }"));
    assert!(changed.contains("Other { value: 0 }.value"));

    let after = hir::resolve(&parse(&changed, &source_path).unwrap()).unwrap();
    let extract = after
        .functions
        .iter()
        .find(|function| function.id.as_str() == "patch.extract")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &extract.body.kind else {
        panic!("extract body must be a block")
    };
    let ResolvedExprKind::Match { arms, .. } = &tail.kind else {
        panic!("extract tail must be a match")
    };
    let hir::ResolvedMatchPattern::Record { fields, .. } = &arms[0].pattern else {
        panic!("extract must use a record pattern")
    };
    let hir::ResolvedRecordMatchFieldPattern::Binding(binding) = &fields[0].pattern else {
        panic!("record field must bind")
    };
    let ResolvedExprKind::Place(place) = &arms[0].value.kind else {
        panic!("match value must reference the binding")
    };
    assert_eq!(binding.name, "value");
    assert_eq!(binding.id, before_binding_id);
    assert_eq!(place.root, before_place_root);
}

#[test]
fn v2_duplicate_and_stale_owner_selectors_fail_without_writes() {
    let source = r#"module patch.hostile;
@id("patch.box") record Box { @id("patch.box.value") value: i64, }
@id("app.main") fn main()->i64{Box { value: 42 }.value}
"#;
    let (source_path, patch_path, revision) = fixture("duplicate", source);
    std::fs::write(
        &patch_path,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nrename-member owner patch.box member patch.box.value to payload\nrename-member owner patch.box member patch.box.value to other\n"
        ),
    )
    .unwrap();
    let diagnostics = patch::apply(&source_path, &patch_path).unwrap_err();
    assert_eq!(diagnostics[0].code, "SPX-G106");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);

    std::fs::write(
        &patch_path,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nrename-member owner patch.other member patch.box.value to payload\n"
        ),
    )
    .unwrap();
    let diagnostics = patch::apply(&source_path, &patch_path).unwrap_err();
    assert_eq!(diagnostics[0].code, "SPX-G107");
    assert_eq!(std::fs::read_to_string(source_path).unwrap(), source);
}

#[test]
fn case_and_case_member_rename_cover_construction_and_variant_pattern() {
    let source = r#"module patch.variant;
@id("patch.outcome") variant Outcome {
 @id("patch.outcome.ok") Ok { @id("patch.outcome.ok.value") value: i64, },
 @id("patch.outcome.err") Err,
}
@id("patch.unwrap") fn unwrap(input: Outcome)->i64{match input {Outcome::Ok { value }=>value,Outcome::Err {}=>0,}}
@id("app.main") fn main()->i64{unwrap(Outcome::Ok { value: 42 })}
"#;
    let (source_path, patch_path, revision) = fixture("variant", source);
    std::fs::write(
        &patch_path,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nrename-case owner patch.outcome case patch.outcome.ok to Success\nrename-member owner patch.outcome.ok member patch.outcome.ok.value to payload\n"
        ),
    )
    .unwrap();
    patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    assert!(changed.contains("Success {"));
    assert!(changed.contains("Outcome::Success { payload: value } => value"));
    assert!(changed.contains("Outcome::Success { payload: 42 }"));
    assert!(changed.contains("Outcome::Err {} => 0"));
    hir::resolve(&parse(&changed, &source_path).unwrap()).unwrap();
}

#[test]
fn generic_call_tuple_mismatches_and_schema_confusion_are_atomic() {
    let source = r#"module patch.tuple;
@id("generic.marker") fn marker<T>()->bool{true}
@id("app.main") fn main()->i64{if marker<i64>() {1}else{0}}
"#;
    let program = parse(source, Path::new("tuple.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let main = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let call = first_call(&main.body, "generic.marker").unwrap();
    let ResolvedExprKind::Call {
        instance: Some(instance),
        ..
    } = &call.kind
    else {
        panic!("marker call must be concrete")
    };
    let valid = format!(
        "replace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool",
        call.id, instance
    );
    let hostile = [
        (
            valid.replace(call.id.as_str(), "declaration:0::expression:0:"),
            "SPX-G108",
        ),
        (
            valid.replace("template generic.marker", "template generic.other"),
            "SPX-G108",
        ),
        (
            valid.replace(instance.as_str(), "semaprax.function-instance.v1:stale"),
            "SPX-G108",
        ),
        (valid.replace("index 0", "index 1"), "SPX-G108"),
        (
            valid.replace("from i64 to bool", "from bool to i64"),
            "SPX-G108",
        ),
        (valid.replace("to bool", "to i64"), "SPX-G106"),
    ];
    for (sequence, (instruction, expected)) in hostile.into_iter().enumerate() {
        let (source_path, patch_path, revision) = fixture(&format!("tuple-{sequence}"), source);
        std::fs::write(
            &patch_path,
            format!("schema semaprax.semantic-patch.v2\nbase {revision}\n{instruction}\n"),
        )
        .unwrap();
        let diagnostics = patch::apply(&source_path, &patch_path).unwrap_err();
        assert_eq!(diagnostics[0].code, expected, "{instruction}");
        assert_eq!(std::fs::read_to_string(source_path).unwrap(), source);
    }

    let (source_path, patch_path, revision) = fixture("duplicate-call-index", source);
    std::fs::write(
        &patch_path,
        format!("schema semaprax.semantic-patch.v2\nbase {revision}\n{valid}\n{valid}\n"),
    )
    .unwrap();
    let diagnostics = patch::apply(&source_path, &patch_path).unwrap_err();
    assert_eq!(diagnostics[0].code, "SPX-G106");
    assert_eq!(std::fs::read_to_string(source_path).unwrap(), source);

    let (source_path, patch_path, _) = fixture("stale-v2", source);
    std::fs::write(
        &patch_path,
        format!(
            "schema semaprax.semantic-patch.v2\nbase sha256:0000000000000000000000000000000000000000000000000000000000000000\n{valid}\n"
        ),
    )
    .unwrap();
    let diagnostics = patch::apply(&source_path, &patch_path).unwrap_err();
    assert_eq!(diagnostics[0].code, "SPX-G409");
    assert_eq!(std::fs::read_to_string(source_path).unwrap(), source);

    for (sequence, patch_text) in [
        "schema semaprax.semantic-patch.v999\nbase ignored\n".to_owned(),
        format!("base ignored\nschema semaprax.semantic-patch.v2\n{valid}\n"),
        "schema semaprax.semantic-patch.v2\nbase ignored\nrequire no-new-effects\nrequire no-new-effects\n".to_owned(),
    ]
    .into_iter()
    .enumerate()
    {
        let (source_path, patch_path, _) = fixture(&format!("confusion-{sequence}"), source);
        std::fs::write(&patch_path, patch_text).unwrap();
        let diagnostics = patch::apply(&source_path, &patch_path).unwrap_err();
        assert!(matches!(diagnostics[0].code, "SPX-G101" | "SPX-G106"));
        assert_eq!(std::fs::read_to_string(source_path).unwrap(), source);
    }
}

#[test]
fn mixed_member_case_payload_and_two_index_call_commit_as_one_prestate_transaction() {
    let source = r#"module patch.mixed;
@id("patch.box") record Box { @id("patch.box.value") value: i64, }
@id("patch.outcome") variant Outcome {
 @id("patch.outcome.ok") Ok { @id("patch.outcome.ok.value") value: i64, },
 @id("patch.outcome.err") Err,
}
@id("generic.marker") fn marker<T,U>()->bool{true}
@id("patch.extract") fn extract(input: Box)->i64{match input {Box { value }=>value,}}
@id("patch.unwrap") fn unwrap(input: Outcome)->i64{match input {Outcome::Ok { value }=>value,Outcome::Err {}=>0,}}
@id("app.main") fn main()->i64{if marker<i64,bool>() {extract(Box { value: 20 })+unwrap(Outcome::Ok { value: 22 })}else{0}}
"#;
    let before_program = parse(source, Path::new("mixed.spx")).unwrap();
    let before = hir::resolve(&before_program).unwrap();
    let main = before
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let call = first_call(&main.body, "generic.marker").unwrap();
    let ResolvedExprKind::Call {
        instance: Some(old_instance),
        ..
    } = &call.kind
    else {
        panic!("marker call must be concrete")
    };
    assert_eq!(
        old_instance.as_str(),
        "semaprax.function-instance.v1:14:generic.marker:2:3:i644:bool"
    );
    let before_cleanup = before
        .functions
        .iter()
        .map(|function| (function.id.clone(), function.cleanup_plan.clone()))
        .collect::<Vec<_>>();
    let (source_path, patch_path, revision) = fixture("mixed", source);
    std::fs::write(
        &patch_path,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nrename generic.marker to flag\nrename-member owner patch.box member patch.box.value to payload\nrename-case owner patch.outcome case patch.outcome.ok to Success\nrename-member owner patch.outcome.ok member patch.outcome.ok.value to payload\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 1 from bool to i64\nrequire no-new-effects\n",
            call.id, old_instance, call.id, old_instance
        ),
    )
    .unwrap();
    patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    let after_program = parse(&changed, &source_path).unwrap();
    let after = hir::resolve(&after_program).unwrap();
    assert!(changed.contains("fn flag<T, U>()"));
    assert!(changed.contains("flag<bool, i64>()"));
    assert_eq!(
        graph::revision(&after_program),
        "sha256:f2f344c5a19591dfde2aa65ffd21918464be0848f526d6a59b977af9394805a7"
    );
    assert!(graph::to_json(&after_program)
        .unwrap()
        .contains("\"schema\":\"semaprax.graph.v14\""));

    let box_type = after
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == "patch.box")
        .unwrap();
    let hir::ResolvedTypeDeclarationKind::Record { fields } = &box_type.kind else {
        panic!("Box must remain a record")
    };
    assert_eq!(fields[0].id.as_str(), "patch.box.value");
    assert_eq!(fields[0].index, 0);
    assert_eq!(fields[0].name, "payload");
    let outcome = after
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == "patch.outcome")
        .unwrap();
    let hir::ResolvedTypeDeclarationKind::Variant { cases } = &outcome.kind else {
        panic!("Outcome must remain a variant")
    };
    assert_eq!(cases[0].id.as_str(), "patch.outcome.ok");
    assert_eq!(cases[0].index, 0);
    assert_eq!(cases[0].name, "Success");
    assert_eq!(cases[0].fields[0].id.as_str(), "patch.outcome.ok.value");
    assert_eq!(cases[0].fields[0].index, 0);
    assert_eq!(
        after
            .functions
            .iter()
            .map(|function| (function.id.clone(), function.cleanup_plan.clone()))
            .collect::<Vec<_>>(),
        before_cleanup
    );
    assert!(!after
        .function_instances
        .iter()
        .any(|instance| instance.id == *old_instance));
    assert!(after.function_instances.iter().any(|instance| {
        instance.template.as_str() == "generic.marker"
            && instance.id.as_str()
                == "semaprax.function-instance.v1:14:generic.marker:2:4:bool3:i64"
            && instance.type_arguments == [hir::ResolvedType::Bool, hir::ResolvedType::I64]
    }));
}

#[test]
fn automatic_compiler_owned_wrong_kind_and_collision_targets_fail_closed() {
    let source = r#"module patch.identity_hostile;
@id("patch.auto_record") record AutoRecord { value: i64, other: i64, }
@id("patch.auto_variant") variant AutoVariant { Item { value: i64, }, Empty, }
@id("app.main") fn main()->i64{0}
"#;
    let program = parse(source, Path::new("identity-hostile.spx")).unwrap();
    let record = program
        .types
        .iter()
        .find(|declaration| declaration.stable_id == "patch.auto_record")
        .unwrap();
    let semaprax::ast::TypeDeclarationKind::Record { fields } = &record.kind else {
        panic!("AutoRecord must be a record")
    };
    let automatic_field = fields[0].stable_id.clone();
    let variant = program
        .types
        .iter()
        .find(|declaration| declaration.stable_id == "patch.auto_variant")
        .unwrap();
    let semaprax::ast::TypeDeclarationKind::Variant { cases } = &variant.kind else {
        panic!("AutoVariant must be a variant")
    };
    let automatic_case = cases[0].stable_id.clone();
    let automatic_payload = cases[0].fields[0].stable_id.clone();
    let hostile = [
        format!("rename-member owner patch.auto_record member {automatic_field} to renamed"),
        format!("rename-case owner patch.auto_variant case {automatic_case} to Renamed"),
        format!("rename-member owner {automatic_case} member {automatic_payload} to renamed"),
        format!("rename-case owner patch.auto_record case {automatic_field} to Renamed"),
        "rename-case owner core.option case core.option.some to Present".to_owned(),
        "rename-member owner core.result.ok member core.result.ok.value to payload".to_owned(),
    ];
    for (sequence, instruction) in hostile.into_iter().enumerate() {
        let (source_path, patch_path, revision) = fixture(&format!("identity-{sequence}"), source);
        std::fs::write(
            &patch_path,
            format!("schema semaprax.semantic-patch.v2\nbase {revision}\n{instruction}\n"),
        )
        .unwrap();
        let diagnostics = patch::apply(&source_path, &patch_path).unwrap_err();
        assert_eq!(diagnostics[0].code, "SPX-G107", "{instruction}");
        assert_eq!(std::fs::read_to_string(source_path).unwrap(), source);
    }

    let explicit_source = r#"module patch.collision_v2;
@id("patch.pair") record Pair {
 @id("patch.pair.left") left: i64,
 @id("patch.pair.right") right: i64,
}
@id("app.main") fn main()->i64{Pair { left: 20, right: 22 }.left}
"#;
    let (source_path, patch_path, revision) = fixture("collision-v2", explicit_source);
    std::fs::write(
        &patch_path,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nrename-member owner patch.pair member patch.pair.left to right\n"
        ),
    )
    .unwrap();
    assert!(patch::apply(&source_path, &patch_path).is_err());
    assert_eq!(
        std::fs::read_to_string(source_path).unwrap(),
        explicit_source
    );
}

#[test]
fn patched_generic_call_executes_equivalently_at_native_o0_o2_and_node_wasm() {
    let source = r#"module patch.runtime;
@id("generic.marker") fn marker<T,U>()->bool{true}
@id("app.main") fn main()->i64{if marker<i64,bool>() {42}else{0}}
"#;
    let program = parse(source, Path::new("runtime-before.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let main = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let call = first_call(&main.body, "generic.marker").unwrap();
    let ResolvedExprKind::Call {
        instance: Some(instance),
        ..
    } = &call.kind
    else {
        panic!("runtime call must be concrete")
    };
    let (source_path, patch_path, revision) = fixture("runtime", source);
    std::fs::write(
        &patch_path,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 1 from bool to i64\n",
            call.id, instance, call.id, instance
        ),
    )
    .unwrap();
    patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    let program = parse(&changed, &source_path).unwrap();

    let generated = codegen::emit_c(&program).unwrap();
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(8)];
    struct spx_context context = {{0}};
    int64_t result = INT64_C(-1);
    if (!spx_context_init(&context, UINT64_C(20260810), entries, UINT32_C(8), NULL, NULL, NULL)) return 10;
    if ({}(&context, &result) != SPX_STATUS_SUCCESS || result != INT64_C(42)) return 11;
    return 0;
}}
"#,
        declaration_symbol("app.main")
    );
    if Command::new("clang").arg("--version").output().is_ok() {
        for optimization in ["-O0", "-O2"] {
            let stem = format!(
                "semaprax-patch-v2-runtime-{}-{}",
                std::process::id(),
                &optimization[2..]
            );
            let c_path = std::env::temp_dir().join(format!("{stem}.c"));
            let executable =
                std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
            std::fs::write(&c_path, format!("{generated}\n{probe}")).unwrap();
            let compiled = Command::new("clang")
                .args([
                    "-std=c11",
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_NO_ENTRY_WRAPPER",
                ])
                .arg(&c_path)
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "patched C failed at {optimization}: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let executed = Command::new(&executable).output().unwrap();
            let _ = std::fs::remove_file(&c_path);
            let _ = std::fs::remove_file(&executable);
            assert!(executed.status.success(), "patched C runtime failed");
        }
    }

    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());
    if Command::new("node").arg("--version").output().is_ok() {
        let stem = format!("semaprax-patch-v2-wasm-{}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();
        std::fs::write(
            &script_path,
            r#"import { readFile } from "node:fs/promises";
const fail = (name) => () => { throw new Error(`unexpected import ${name}`); };
const bytes = await readFile(process.argv[2]);
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
} });
for (let i = 0; i < 256; i += 1) {
  if (instance.exports.semaprax_main() !== 42n) throw new Error("patched result mismatch");
}
console.log("semantic-patch-v2-runtime-ok");
"#,
        )
        .unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "patched Node/Wasm runtime failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "semantic-patch-v2-runtime-ok"
        );
    }
}
