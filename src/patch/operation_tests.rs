//! One authoritative operation sequence: parse, canonical render, derived
//! planner views and preflight attribution for every operation variant.

use super::*;

const MIXED_SOURCE: &str = r#"module patch.mixed;
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

fn mixed_path() -> PathBuf {
    PathBuf::from("mixed.spx")
}

/// The base revision and the one `marker<i64,bool>` call's expression and instance.
fn mixed_call() -> (String, String, String) {
    let program = parse(MIXED_SOURCE, &mixed_path()).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let tokens = lexer::lex(MIXED_SOURCE, "mixed.spx").unwrap();
    let index = SemanticSourceIndex::build(&program, &resolved, &tokens).unwrap();
    let call = index
        .calls
        .values()
        .find(|site| site.template == "generic.marker")
        .unwrap();
    (
        graph::revision(&program),
        call.expression.clone(),
        call.instance.clone().unwrap(),
    )
}

/// Every v2 family, deliberately interleaved against the planner's family order.
fn interleaved_lines(expression: &str, instance: &str) -> Vec<String> {
    vec![
        "require no-new-effects".to_owned(),
        format!("replace-call-type-argument expression {expression} template generic.marker old-instance {instance} index 1 from bool to i64"),
        "rename-case owner patch.outcome case patch.outcome.ok to Success".to_owned(),
        "rename-member owner patch.outcome.ok member patch.outcome.ok.value to payload".to_owned(),
        "rename generic.marker to flag".to_owned(),
        format!("replace-call-type-argument expression {expression} template generic.marker old-instance {instance} index 0 from i64 to bool"),
        "rename-member owner patch.box member patch.box.value to payload".to_owned(),
    ]
}

fn v2_patch(base: &str, lines: &[String]) -> String {
    let mut patch = format!("schema semaprax.semantic-patch.v2\nbase {base}\n");
    for line in lines {
        patch.push_str(line);
        patch.push('\n');
    }
    patch
}

fn operation_family(operation: &PreflightOperation) -> &'static str {
    match operation {
        PreflightOperation::AssignFunctionId { .. } => "assign",
        PreflightOperation::Rename { .. } => "rename",
        PreflightOperation::RenameMember { .. } => "member",
        PreflightOperation::RenameCase { .. } => "case",
        PreflightOperation::ReplaceCallTypeArgument { .. } => "call",
        PreflightOperation::RequireNoNewEffects { .. } => "require",
    }
}

fn change_indices(change: &PreflightChange) -> &BTreeSet<usize> {
    match change {
        PreflightChange::Rename {
            operation_indices, ..
        }
        | PreflightChange::CallInstance {
            operation_indices, ..
        } => operation_indices,
    }
}

#[test]
fn every_v2_variant_has_one_authored_payload_and_derived_views() {
    let (base, expression, instance) = mixed_call();
    let source = v2_patch(&base, &interleaved_lines(&expression, &instance));
    let patch = parse_patch(&source).unwrap();
    assert_eq!(patch.schema(), PatchSchema::V2);
    assert_eq!(patch.base(), base);
    assert_eq!(
        patch
            .operations()
            .iter()
            .map(operation_family)
            .collect::<Vec<_>>(),
        ["require", "call", "case", "member", "rename", "call", "member"]
    );
    assert!(patch
        .operations()
        .iter()
        .enumerate()
        .all(|(position, operation)| operation.index() == position));
    // Canonical emission follows authored order byte for byte.
    assert_eq!(canonical_patch(&patch), source);

    // Each view borrows the one payload and keeps authored order in its family.
    let renames = patch.renames().collect::<Vec<_>>();
    assert_eq!(renames.len(), 1);
    assert_eq!(
        (
            renames[0].stable_id,
            renames[0].new_name,
            renames[0].operation_index
        ),
        ("generic.marker", "flag", 4)
    );
    let PreflightOperation::Rename { target, .. } = &patch.operations()[4] else {
        panic!("operation 4 must be the function rename")
    };
    assert!(std::ptr::eq(renames[0].stable_id, target.as_str()));
    assert_eq!(
        patch
            .member_renames()
            .map(|rename| (
                rename.owner,
                rename.member,
                rename.new_name,
                rename.operation_index
            ))
            .collect::<Vec<_>>(),
        [
            (
                "patch.outcome.ok",
                "patch.outcome.ok.value",
                "payload",
                3usize
            ),
            ("patch.box", "patch.box.value", "payload", 6),
        ]
    );
    assert_eq!(
        patch
            .case_renames()
            .map(|rename| (
                rename.owner,
                rename.case,
                rename.new_name,
                rename.operation_index
            ))
            .collect::<Vec<_>>(),
        [("patch.outcome", "patch.outcome.ok", "Success", 2usize)]
    );
    assert_eq!(
        patch
            .call_type_argument_replacements()
            .map(|call| (
                call.expression,
                call.template,
                call.old_instance,
                call.index,
                call.from,
                call.to,
                call.operation_index
            ))
            .collect::<Vec<_>>(),
        [
            (
                expression.as_str(),
                "generic.marker",
                instance.as_str(),
                1,
                ScalarType::Bool,
                ScalarType::I64,
                1usize
            ),
            (
                expression.as_str(),
                "generic.marker",
                instance.as_str(),
                0,
                ScalarType::I64,
                ScalarType::Bool,
                5
            ),
        ]
    );
    assert!(patch.no_new_effects());
    assert!(patch.assign_function_id().is_none());
    assert_eq!(
        patch
            .operations()
            .iter()
            .map(|operation| operation_selector(operation).label())
            .collect::<Vec<_>>(),
        [
            "require:no-new-effects".to_owned(),
            format!("call:{expression}:1"),
            "case:patch.outcome:patch.outcome.ok".to_owned(),
            "member:patch.outcome.ok:patch.outcome.ok.value".to_owned(),
            "rename:generic.marker".to_owned(),
            format!("call:{expression}:0"),
            "member:patch.box:patch.box.value".to_owned(),
        ]
    );
}

#[test]
fn mixed_family_preflight_keeps_grouped_planning_and_authored_attribution() {
    let (base, expression, instance) = mixed_call();
    let lines = interleaved_lines(&expression, &instance);
    let source = v2_patch(&base, &lines);
    let preflight =
        preflight_review_owned(MIXED_SOURCE.to_owned(), source.clone(), mixed_path(), 64).unwrap();
    // The retained patch is the only copy; the accessor borrows it.
    assert!(std::ptr::eq(
        preflight.operations(),
        preflight.patch.operations()
    ));
    assert_eq!(canonical_patch(&preflight.patch), source);
    // Families are planned in grouped order (rename, member, case, call), but
    // each change keeps its authored operation indices and the published
    // change list is ordered by first authored operation.
    let changes = preflight
        .changes()
        .iter()
        .map(|change| match change {
            PreflightChange::Rename { target, .. } => (target.clone(), change_indices(change)),
            PreflightChange::CallInstance { expression, .. } => {
                (format!("call:{expression}"), change_indices(change))
            }
        })
        .map(|(target, indices)| (target, indices.iter().copied().collect::<Vec<_>>()))
        .collect::<Vec<_>>();
    assert_eq!(
        changes,
        [
            (format!("call:{expression}"), vec![1, 5]),
            ("patch.outcome.ok".to_owned(), vec![2]),
            ("patch.outcome.ok.value".to_owned(), vec![3]),
            ("generic.marker".to_owned(), vec![4]),
            ("patch.box.value".to_owned(), vec![6]),
        ]
    );
    for edit in preflight.planned_edits() {
        assert!(!edit.operation_indices.is_empty());
        assert!(edit
            .operation_indices
            .is_subset(change_indices(&preflight.changes()[edit.change])));
        assert!(edit.start < edit.end && edit.end <= MIXED_SOURCE.len());
    }
    assert!(!preflight.planned_edits().is_empty());

    // Authored order changes attribution and canonical bytes, never the candidate.
    let mut grouped = lines.clone();
    grouped.sort_by_key(|line| {
        [
            "rename ",
            "rename-member",
            "rename-case",
            "replace-call",
            "require",
        ]
        .iter()
        .position(|prefix| line.starts_with(prefix))
    });
    let regrouped = preflight_review_owned(
        MIXED_SOURCE.to_owned(),
        v2_patch(&base, &grouped),
        mixed_path(),
        64,
    )
    .unwrap();
    assert_eq!(
        regrouped.canonical_candidate(),
        preflight.canonical_candidate()
    );
    assert_eq!(
        regrouped.candidate_revision(),
        preflight.candidate_revision()
    );
    assert_ne!(regrouped.patch_source(), preflight.patch_source());
    let spans = |preflight: &PatchPreflight| {
        preflight
            .planned_edits()
            .iter()
            .map(|edit| (edit.start, edit.end, edit.replacement.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(spans(&regrouped), spans(&preflight));
}

#[test]
fn grouped_planner_reports_the_first_failing_family_not_the_first_authored_line() {
    let (base, expression, instance) = mixed_call();
    let call_conflict = format!("replace-call-type-argument expression {expression} template generic.marker old-instance {instance} index 0 from i64 to i64");
    let invalid_case = "rename-case owner patch.outcome case patch.outcome.ok to 9case".to_owned();
    let invalid_member =
        "rename-member owner patch.box member patch.box.value to 9member".to_owned();
    let invalid_rename = "rename generic.marker to 9rename".to_owned();
    let preflight_error = |lines: &[String]| {
        preflight_review_owned(
            MIXED_SOURCE.to_owned(),
            v2_patch(&base, lines),
            mixed_path(),
            64,
        )
        .err()
        .unwrap()
    };
    // Authored first: call conflict, then case, then member, then rename.
    let diagnostics = preflight_error(&[
        call_conflict.clone(),
        invalid_case.clone(),
        invalid_member.clone(),
        invalid_rename,
    ]);
    assert_eq!(diagnostics[0].code, "SPX-G103");
    assert!(diagnostics[0].message.contains("9rename"));
    let diagnostics =
        preflight_error(&[call_conflict.clone(), invalid_case.clone(), invalid_member]);
    assert!(
        diagnostics[0].message.contains("9member"),
        "{diagnostics:?}"
    );
    let diagnostics = preflight_error(&[call_conflict.clone(), invalid_case]);
    assert!(diagnostics[0].message.contains("9case"), "{diagnostics:?}");
    let diagnostics = preflight_error(&[call_conflict]);
    assert_eq!(diagnostics[0].code, "SPX-G106");
    assert!(diagnostics[0].message.contains("already `i64`"));
}

#[test]
fn require_no_new_effects_is_derived_and_enforced_from_the_sequence() {
    let source = "module patch.effects;\n@id(\"helper.answer\") fn answer()->i64{42}\n@id(\"app.main\") fn main()->i64{answer()}\n";
    let base = graph::revision(&parse(source, &mixed_path()).unwrap());
    let v1 = format!("base {base}\nrequire no-new-effects\nrename helper.answer to computed\nrequire no-new-effects\n");
    let patch = parse_patch(&v1).unwrap();
    assert_eq!(patch.schema(), PatchSchema::V1);
    assert_eq!(patch.operations().len(), 3);
    assert!(patch.no_new_effects());
    assert_eq!(
        canonical_patch(&patch),
        format!("base {base}\nrequire no-new-effects\nrename helper.answer to computed\nrequire no-new-effects\n")
    );
    let preflight = preflight_review_owned(source.to_owned(), v1, mixed_path(), 64).unwrap();
    assert_eq!(preflight.operations().len(), 3);
    assert!(preflight.canonical_candidate().contains("fn computed()"));
    let plain = parse_patch(&format!("base {base}\nrename helper.answer to computed\n")).unwrap();
    assert!(!plain.no_new_effects());
}

#[test]
fn v1_v2_and_v3_parser_contracts_are_unchanged() {
    // V1 tolerates comments, blank lines, indentation and a repeated base, and
    // admits a repeated selector; v2 rejects both duplicates.
    let v1 = "# comment\n\n  base sha256:a\nbase sha256:b\nrename x to y\nrename x to z\n";
    let patch = parse_patch(v1).unwrap();
    assert_eq!(patch.base(), "sha256:b");
    assert_eq!(patch.renames().count(), 2);
    assert_eq!(
        canonical_patch(&patch),
        "base sha256:b\nrename x to y\nrename x to z\n"
    );
    for v2 in [
        "schema semaprax.semantic-patch.v2\nbase sha256:a\nbase sha256:a\n",
        "schema semaprax.semantic-patch.v2\nbase sha256:a\nrename x to y\nrename x to z\n",
        "schema semaprax.semantic-patch.v2\nbase sha256:a\nrequire no-new-effects\nrequire no-new-effects\n",
    ] {
        assert_eq!(parse_patch(v2).err().unwrap()[0].code, "SPX-G106");
    }
    assert_eq!(
        parse_patch("base sha256:a\nrename-member owner a member b to c\n")
            .err()
            .unwrap()[0]
            .code,
        "SPX-G101"
    );
    let assignment =
        "assign-function-id repair r1 diagnostic SPX-S103 target auto:x name helper to app.helper";
    let v3 = format!("schema semaprax.semantic-patch.v3\nbase sha256:a\n{assignment}\n");
    let patch = parse_patch(&v3).unwrap();
    assert_eq!(patch.schema(), PatchSchema::V3);
    assert_eq!(canonical_patch(&patch), v3);
    for hostile in [
        format!("schema semaprax.semantic-patch.v3\nbase sha256:a\n{assignment}"),
        format!("schema semaprax.semantic-patch.v3\r\nbase sha256:a\r\n{assignment}\r\n"),
        format!("schema semaprax.semantic-patch.v3\nbase sha256:a\n{assignment}\nrequire no-new-effects\n"),
        format!("schema semaprax.semantic-patch.v3\nbase  sha256:a\n{assignment}\n"),
    ] {
        assert_eq!(parse_patch(&hostile).err().unwrap()[0].code, "SPX-G101");
    }
}

#[test]
fn v3_assignment_view_parses_renders_and_preflights() {
    let source =
        "module patch.v3_view;\nfn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let directory =
        std::env::temp_dir().join(format!("semaprax-patch-v3-view-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    std::fs::write(&source_path, source).unwrap();
    let request =
        crate::repair::DiagnosticRepairQuery::assign_function_id("auto:patch.v3_view.helper")
            .unwrap();
    let report: serde_json::Value =
        serde_json::from_str(&crate::repair::query(&source_path, &request).unwrap()).unwrap();
    let repair_id = report["repair"]["id"].as_str().unwrap().to_owned();
    let preview: serde_json::Value = serde_json::from_str(
        &crate::repair::instantiate(
            &source_path,
            &repair_id,
            &crate::repair::PersistentDeclarationId::new("patch.v3_view.helper").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let patch_source = preview["patch"]["source"].as_str().unwrap().to_owned();

    let patch = parse_patch(&patch_source).unwrap();
    assert_eq!(patch.schema(), PatchSchema::V3);
    assert_eq!(patch.operations().len(), 1);
    assert_eq!(operation_family(&patch.operations()[0]), "assign");
    let assignment = patch.assign_function_id().unwrap();
    assert_eq!(assignment.repair_id, repair_id);
    assert_eq!(assignment.target, "auto:patch.v3_view.helper");
    assert_eq!(assignment.name, "helper");
    assert_eq!(assignment.to, "patch.v3_view.helper");
    assert_eq!(patch.renames().count(), 0);
    assert!(!patch.no_new_effects());
    assert_eq!(
        operation_selector(&patch.operations()[0]).label(),
        "assign:auto:patch.v3_view.helper"
    );
    assert_eq!(canonical_patch(&patch), patch_source);

    let preflight =
        preflight_review_owned(source.to_owned(), patch_source, source_path, 64).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(preflight.schema_label(), "semaprax.semantic-patch.v3");
    assert!(std::ptr::eq(
        preflight.operations(),
        preflight.patch.operations()
    ));
    assert!(preflight.identity_rebase().is_some());
    assert!(preflight.changes().is_empty() && preflight.planned_edits().is_empty());
    assert!(preflight
        .canonical_candidate()
        .contains("@id(\"patch.v3_view.helper\")"));
}

#[test]
fn stale_and_project_module_admission_keep_their_contracts() {
    let (base, _, _) = mixed_call();
    let stale = v2_patch(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        &["rename generic.marker to flag".to_owned()],
    );
    let diagnostics = preflight_review_owned(MIXED_SOURCE.to_owned(), stale, mixed_path(), 64)
        .err()
        .unwrap();
    assert_eq!(diagnostics[0].code, "SPX-G409");
    // The Project handoff admits exactly one Patch-v1 rename; the derived
    // count and no-new-effects views gate it.
    for patch in [
        v2_patch(&base, &["rename generic.marker to flag".to_owned()]),
        format!("base {base}\nrename generic.marker to flag\nrequire no-new-effects\n"),
        format!("base {base}\nrename generic.marker to flag\nrename patch.extract to take\n"),
    ] {
        let diagnostics =
            preflight_project_rename_parts(MIXED_SOURCE.to_owned(), patch, mixed_path())
                .err()
                .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-J109");
    }
    let preflight = preflight_project_rename_parts(
        MIXED_SOURCE.to_owned(),
        format!("base {base}\nrename generic.marker to flag\n"),
        mixed_path(),
    )
    .unwrap();
    assert_eq!(preflight.operations().len(), 1);
    assert!(preflight.canonical_candidate().contains("fn flag<T, U>()"));
}
