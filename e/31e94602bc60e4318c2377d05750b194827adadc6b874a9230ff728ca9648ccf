//! Retained authority is independently replayed, never inferred from v77.
use super::*;
mod paid_report;
mod retained_lexical_composition;
const WRITER: &str = include_str!("../../../../tests/owned_data/static_utf8_writer.spx");
fn build(source: &str) -> WorkspaceGraphBuild {
    let sources = [
        ("writer.spx", source),
        (
            "support.spx",
            "module support.ready; @id(\"ready\") fn ready()->i64 {0}",
        ),
    ]
    .into_iter()
    .map(|(path, source)| WorkspaceSource {
        path: path.to_owned(),
        source: format::canonical(&crate::parse(source, path).unwrap()),
    })
    .collect();
    build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
        .unwrap()
        .0
}
fn writer(build: &WorkspaceGraphBuild) -> &WorkspaceResolvedModule {
    build
        .hir
        .modules
        .iter()
        .find(|module| module.module == "static.writer")
        .unwrap()
}
fn writer_mut(build: &mut WorkspaceGraphBuild) -> &mut WorkspaceResolvedModule {
    build
        .hir
        .modules
        .iter_mut()
        .find(|module| module.module == "static.writer")
        .unwrap()
}
fn refused(build: &WorkspaceGraphBuild) {
    let errors = super::super::schema(writer(build), &build.hir.modules, &build.hir.declarations)
        .unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-G410"));
}
#[test]
fn retained_writer_replays_source_sites_and_composes_helper_renewal() {
    let build = build(WRITER);
    let module = writer(&build);
    assert_eq!(
        schema(module, &build.hir.declarations).unwrap(),
        Some("semaprax.graph.v77")
    );
    assert_eq!(
        build.source_graph_schemas().unwrap()["writer.spx"],
        "semaprax.graph.v77"
    );
    assert_eq!(module.static_text.as_ref().unwrap().sites.len(), 2);
    assert!(module.functions.iter().any(
        |function| function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
    ));
    assert_eq!(
        graph::graph_schema_from_parts_and_instances(
            &module.interfaces,
            &module.types,
            &module.functions,
            &module.function_templates,
            &module.function_instances
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );
    let changed = build_source_changed();
    assert_ne!(
        module.static_text.as_ref().unwrap().sites[0].text.as_str(),
        writer(&changed).static_text.as_ref().unwrap().sites[0]
            .text
            .as_str()
    );
}
fn build_source_changed() -> WorkspaceGraphBuild {
    build(&WRITER.replace("aé", "bé"))
}
#[test]
fn source_inventory_content_identity_namespace_and_presence_fail_closed() {
    for attack in 0..6 {
        let mut build = build(WRITER);
        let module = writer_mut(&mut build);
        let inventory = module.static_text.as_mut().unwrap();
        match attack {
            0 => inventory.sites[0].text = hir::StaticUtf8::from_text("forged"),
            1 => inventory.sites[0].path.push('x'),
            2 => inventory.sites[0].expression = module.functions[0].body.id.clone(),
            3 => inventory.path.push('x'),
            4 => inventory.sites[0].explicit = !inventory.sites[0].explicit,
            _ => module.static_text = None,
        }
        refused(&build);
    }
}
#[test]
fn retained_generic_literals_match_template_and_concrete_execution_sites() {
    let source = "module static.writer; @id(\"generic\") fn generic<T>(value:T)->T {let key=str\"é\";let second=str\"x\";value} @id(\"app.main\") fn main()->i64 {generic<i64>(42)}";
    let mut build = build(source);
    let module = writer(&build);
    assert_eq!(
        schema(module, &build.hir.declarations).unwrap(),
        Some("semaprax.graph.v77")
    );
    assert_eq!(module.function_templates.len(), 1);
    assert!(!module.function_instances.is_empty());
    let module = writer_mut(&mut build);
    let replacement = module.functions[0].body.id.clone();
    let instance = &mut module.function_instances[0].function;
    let hir::ResolvedExprKind::Block { statements, .. } = &mut instance.body.kind else {
        panic!("generic body")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("generic literal")
    };
    value.id = replacement;
    refused(&build);
    let mut duplicate = self::build(source);
    let module = writer_mut(&mut duplicate);
    let hir::ResolvedExprKind::Block { statements, .. } =
        &mut module.function_instances[0].function.body.kind
    else {
        panic!("generic body")
    };
    let hir::ResolvedStatement::Let { value: first, .. } = &statements[0] else {
        panic!("first literal")
    };
    let repeated = first.clone();
    let hir::ResolvedStatement::Let { value: second, .. } = &mut statements[1] else {
        panic!("second literal")
    };
    *second = repeated;
    // Even exact duplicate ID/span/content cannot substitute for the omitted
    // second source site; this selector proves its own per-instance bijection.
    refused(&duplicate);
}

const LEXICAL_SOURCE: &str = r#"module static.writer;
@id("static.lexical") fn run()->i64 {
 let key=str"x"; let mut output=bytes_zeroed(1usize); let mut at=0;
 while at<3 {
  output=bytes_set(output,0usize,65u8); at=at+1;
  if at==1 {continue;}; 0
 }
 return str_len_bytes(key);
}
@id("app.main") fn main()->i64 {run()}
"#;
#[test]
fn lexical_byte_renewal_has_direct_proof_and_retained_composition_stays_closed() {
    let source = LEXICAL_SOURCE;
    let ast = crate::check(source, "writer.spx").unwrap();
    let canonical = format::canonical(&ast);
    let ast = crate::check(&canonical, "writer.spx").unwrap();
    assert_eq!(format::canonical(&ast), canonical);
    let resolved = hir::resolve(&ast).unwrap();
    hir::validate(&resolved).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "static.lexical")
        .unwrap();
    assert!(crate::byte_ops::requires_same_owner_set(function));
    assert_eq!(
        function.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
    );
    assert!(hir::lexical_exit::requires(function).unwrap());
    let graph = graph::to_json(&ast).unwrap();
    graph::verify_json(&ast, &graph).unwrap();
    let changed = crate::check(&canonical.replace("str\"x\"", "str\"y\""), "writer.spx").unwrap();
    assert_eq!(
        graph::verify_json(&changed, &graph).unwrap_err()[0].code,
        "SPX-G411"
    );
    for attack in 0..5 {
        let mut candidate = resolved.clone();
        let function = candidate
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "static.lexical")
            .unwrap();
        let unrelated = function.body.id.clone();
        match attack {
            0 => function.cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17,
            1 => {
                function
                    .cleanup_plan
                    .lexical_control
                    .as_mut()
                    .unwrap()
                    .loops[0]
                    .condition = unrelated.clone()
            }
            _ => {
                let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind
                else {
                    panic!("block")
                };
                let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
                    panic!("literal")
                };
                match attack {
                    2 => value.ty = hir::ResolvedType::String,
                    3 => value.ownership = hir::OwnershipMode::Own,
                    _ => value.id = unrelated.clone(),
                }
            }
        }
        assert_eq!(hir::validate(&candidate).unwrap_err().code, "SPX-H006");
    }
    // The legacy route without an authenticated primitive source inventory
    // remains closed: v18 presence itself grants no authority.
    let mut retained = build(&canonical);
    // The legacy detached route has no source-derived primitive declaration context.
    writer_mut(&mut retained)
        .static_text
        .as_mut()
        .unwrap()
        .lexical = None;
    let errors = retained
        .source_graph_schemas()
        .expect_err("retained composite schema route remains explicitly closed");
    assert!(errors.iter().any(|error| error.code == "SPX-G410"));
}

#[test]
fn retained_primitive_static_lexical_replays_graph_and_runtime() {
    let retained = build(LEXICAL_SOURCE);
    assert_eq!(
        schema(writer(&retained), &retained.hir.declarations).unwrap(),
        Some("semaprax.graph.v77"),
        "exact literal-site authentication precedes primitive lexical replay"
    );
    assert_eq!(
        retained.source_graph_schemas().unwrap()["writer.spx"],
        "semaprax.graph.v77"
    );
    let ast = crate::check(LEXICAL_SOURCE, "writer.spx").unwrap();
    let canonical = format::canonical(&ast);
    let current = crate::check(&canonical, "writer.spx").unwrap();
    assert_eq!(format::canonical(&current), canonical);
    let resolved = hir::resolve(&current).unwrap();
    let evaluated =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&resolved, "app.main", 10000).unwrap();
    assert!(matches!(
        evaluated.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));
}

#[test]
fn retained_primitive_lexical_rejects_source_signature_schema_and_certificate_drift() {
    for attack in 0..10 {
        let mut retained = build(LEXICAL_SOURCE);
        let module = writer_mut(&mut retained);
        let function = module
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "static.lexical")
            .unwrap();
        match attack {
            0 => function.name.push('x'),
            1 => function.return_type = hir::ResolvedType::Bool,
            2 => function.cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19,
            3 => {
                function
                    .cleanup_plan
                    .lexical_control
                    .as_mut()
                    .unwrap()
                    .exits[0]
                    .site = function.body.id.clone()
            }
            4 => module
                .static_text
                .as_mut()
                .unwrap()
                .lexical
                .as_mut()
                .unwrap()
                .functions[0]
                .name
                .push('x'),
            _ => {
                // These preserve the captured source and function signatures.
                // Every expected body digest remains private and unchanged.
                let condition = match &function.body.kind {
                    hir::ResolvedExprKind::Block { statements, .. } => statements
                        .iter()
                        .find_map(|statement| match statement {
                            hir::ResolvedStatement::LexicalWhile { condition, .. } => {
                                Some(condition.as_ref().clone())
                            }
                            _ => None,
                        })
                        .unwrap(),
                    _ => panic!("source block"),
                };
                let main = module
                    .functions
                    .iter_mut()
                    .find(|f| f.id.as_str() == "app.main")
                    .unwrap();
                match attack {
                    5 => main.body.kind = hir::ResolvedExprKind::Int(7),
                    6 => {
                        let hir::ResolvedExprKind::Block { tail, .. } = &mut main.body.kind else {
                            panic!("source main block")
                        };
                        let hir::ResolvedExprKind::Call { callee, .. } = &mut tail.kind else {
                            panic!("source call")
                        };
                        *callee = main.id.clone();
                    }
                    7 => main.requires.push(condition),
                    8 => main.body.ty = hir::ResolvedType::Bool,
                    _ => main.span.end = main.span.end.checked_add(1).unwrap(),
                }
            }
        }
        refused(&retained);
    }
}

// The complete old selector is independently compiled, with exact named glue.
mod before_static_lexical_selector;
use before_static_lexical_selector::before_select;
// The complete old selector is independently compiled in that child.
// It documents the genuine prior widening; it is never used for admission.
mod missing_lexical_authority {
    use super::*;

    #[test]
    fn absent_private_lexical_inventory_cannot_inherit_static_or_helper_schema_authority() {
        let mut build = build(LEXICAL_SOURCE);
        assert_eq!(
            build.source_graph_schemas().unwrap()["writer.spx"],
            "semaprax.graph.v77"
        );
        writer_mut(&mut build).static_text.as_mut().unwrap().lexical = None;
        let module = writer(&build);
        let modules = &build.hir.modules;
        let facts = &build.hir.declarations;
        let static_schema = schema(module, facts).unwrap();
        assert_eq!(static_schema, Some("semaprax.graph.v77"));
        assert!(module
            .functions
            .iter()
            .any(|f| hir::lexical_exit::requires(f).unwrap()));
        assert_eq!(
            before_select(
                module,
                modules,
                facts,
                &mut byte_helper::Proofs::new(modules, facts),
                static_schema,
                false
            )
            .unwrap(),
            "semaprax.graph.v77"
        );
        let expected = REPORT.legacy();
        assert_eq!(
            super::super::super::select(
                module,
                modules,
                facts,
                &mut byte_helper::Proofs::new(modules, facts),
                static_schema,
                false
            )
            .unwrap_err(),
            expected,
            "complete six fixed public fields, never version-derived authority"
        );
        assert_eq!(build.source_graph_schemas().unwrap_err(), expected);
        // Replay remains mandatory even when a forged member advertises a
        // helper cleanup generation. Its genuine authored lexical body wins.
        for function in writer_mut(&mut build).functions.iter_mut() {
            if hir::lexical_exit::requires(function).unwrap() {
                function.cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19;
                function.cleanup_plan.lexical_control = None;
            }
        }
        assert!(build.source_graph_schemas().is_err());
    }

    #[test]
    fn authenticated_lexical_and_nonlexical_static_positive_routes_keep_their_own_authority() {
        for source in [LEXICAL_SOURCE, WRITER] {
            let build = build(source);
            assert_eq!(
                build.source_graph_schemas().unwrap()["writer.spx"],
                "semaprax.graph.v77"
            );
        }
        for attack in 0..2 {
            let mut build = build(LEXICAL_SOURCE);
            writer_mut(&mut build).static_text.as_mut().unwrap().lexical = None;
            if attack == 0 {
                writer_mut(&mut build).module.push_str(".foreign");
            } else {
                let f = writer_mut(&mut build)
                    .functions
                    .iter_mut()
                    .find(|f| f.id.as_str() == "static.lexical")
                    .unwrap();
                f.return_type = hir::ResolvedType::Bool;
            }
            assert!(build
                .source_graph_schemas()
                .unwrap_err()
                .iter()
                .any(|e| e.code == "SPX-G410"));
        }
    }

    #[test]
    fn uninstantiated_static_lexical_template_cannot_skip_the_actual_census_guard() {
        let malformed_source = "module static.writer; @id(\"generic\") fn generic<T>(value:T)->T {let key=str\"x\";let mut at=0;while at<2 {at=at+1;if at==1 {continue;};0};value} @id(\"app.main\") fn main()->i64 {0}";
        let error = crate::parse(malformed_source, "writer.spx").unwrap_err();
        assert_eq!(error.code, "SPX-P201");
        assert_eq!(error.message, "expected an expression");
        assert_eq!(error.path.as_deref(), Some("writer.spx"));
        assert!(error.help.is_none());
        let span = error.span.unwrap();
        assert_eq!((span.start, span.end), (135, 136));
        assert_eq!(malformed_source.matches(";0};value}").count(), 1);
        let source = malformed_source.replacen(";0};value}", ";0}value}", 1);
        assert_ne!(source, malformed_source);
        // Fixing the parse error does not admit lexical exits in a generic
        // template. Keep that independent frontend boundary explicit.
        let ast = crate::parse(&source, "writer.spx").unwrap();
        let expected = Diagnostic::error(
            "SPX-T226",
            "generic function `generic` uses an expression outside the direct-scalar slice",
            ast.functions[0].span,
        )
        .at_path("writer.spx");
        assert_eq!(
            crate::check(&source, "writer.spx").unwrap_err(),
            vec![expected]
        );

        // This admitted template stays scalar. The separately authored concrete
        // function supplies the lexical exit; no generic restriction is relaxed.
        let source = LEXICAL_SOURCE.replace(
            "@id(\"app.main\")",
            "@id(\"generic\") fn generic<T>(value:T)->T {let key=str\"x\";value} @id(\"app.main\")",
        );
        let checked = crate::check(&source, "writer.spx").unwrap();
        hir::validate(&hir::resolve(&checked).unwrap()).unwrap();
        let mut build = build(&source);
        assert!(!writer(&build).function_templates.is_empty());
        assert!(writer(&build).function_instances.is_empty());
        writer_mut(&mut build).static_text.as_mut().unwrap().lexical = None;
        let module = writer(&build);
        let facts = &build.hir.declarations;
        let modules = &build.hir.modules;
        let static_schema = schema(module, facts).unwrap();
        assert_eq!(
            before_select(
                module,
                modules,
                facts,
                &mut byte_helper::Proofs::new(modules, facts),
                static_schema,
                false
            )
            .unwrap(),
            "semaprax.graph.v77"
        );
        assert_eq!(build.source_graph_schemas().unwrap_err(), REPORT.legacy());
    }

    #[test]
    fn hostile_uninstantiated_template_lexical_body_reaches_its_own_census_guard() {
        // Both donors are complete source-derived builds. Only the subsequent
        // typed-HIR mutation is hostile; generic lexical source stays T226.
        let source = "module static.writer; @id(\"generic\") fn generic<T>(value:T)->T {let key=str\"x\";value} @id(\"app.main\") fn main()->i64 {0}";
        let mut retained = build(source);
        let donor = build(LEXICAL_SOURCE);
        let body = writer(&donor)
            .functions
            .iter()
            .find(|f| f.id.as_str() == "static.lexical")
            .unwrap()
            .body
            .clone();
        assert!(writer(&retained).function_instances.is_empty());
        let module = writer_mut(&mut retained);
        assert_eq!(module.function_templates.len(), 1);
        module.function_templates.iter_mut().next().unwrap().body = body;
        module.static_text.as_mut().unwrap().lexical = None;
        let module = writer(&retained);
        // The ordinary-member guard cannot explain this refusal: its entire
        // concrete body/contract census has no lexical exits.
        assert!(module
            .functions
            .iter()
            .all(|function| !hir::lexical_exit::requires(function).unwrap()));
        assert!(module.function_instances.is_empty());
        let mut found = false;
        let mut pending = vec![&module.function_templates[0].body];
        while let Some(expression) = pending.pop() {
            found |= matches!(expression.kind, hir::ResolvedExprKind::LexicalExit { .. });
            let children = hir::lexical_exit::ChildCursor::new(expression);
            for child in children {
                pending.push(child);
            }
        }
        assert!(
            found,
            "the actual template, not an ordinary member, holds the exit"
        );
        let functions = module.functions.iter().cloned().collect::<Vec<_>>();
        let templates = module
            .function_templates
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let inventory = module.static_text.as_deref().unwrap();
        let sites = inventory
            .sites
            .iter()
            .map(|site| {
                (
                    site.owner.clone(),
                    site.explicit,
                    site.expression.clone(),
                    site.path.clone(),
                    site.span,
                    site.text.clone(),
                )
            })
            .collect::<Vec<_>>();
        let identity = (
            module.path.clone(),
            module.module.clone(),
            inventory.path.clone(),
            inventory.module.clone(),
        );
        let modules = &retained.hir.modules;
        let facts = &retained.hir.declarations;
        assert_eq!(
            byte_helper::before_static_guard_schema_with_proofs(
                module,
                modules,
                facts,
                &mut byte_helper::Proofs::new(modules, facts)
            )
            .unwrap(),
            None,
            "complete old helper skips the uninstantiated-template lexical census"
        );
        assert_eq!(
            byte_helper::schema_with_proofs(
                module,
                modules,
                facts,
                &mut byte_helper::Proofs::new(modules, facts)
            )
            .unwrap_err(),
            REPORT.legacy(),
            "all six public fields at the actual template census frontier"
        );
        assert_eq!(
            module.functions.iter().cloned().collect::<Vec<_>>(),
            functions
        );
        assert_eq!(
            module
                .function_templates
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            templates
        );
        let after = module.static_text.as_deref().unwrap();
        assert!(after.lexical.is_none());
        assert_eq!(
            (
                module.path.clone(),
                module.module.clone(),
                after.path.clone(),
                after.module.clone()
            ),
            identity
        );
        assert_eq!(
            after
                .sites
                .iter()
                .map(|site| (
                    site.owner.clone(),
                    site.explicit,
                    site.expression.clone(),
                    site.path.clone(),
                    site.span,
                    site.text.clone()
                ))
                .collect::<Vec<_>>(),
            sites
        );
        assert!(
            retained.source_graph_schemas().is_err(),
            "outer authority also refuses this hostile typed-HIR input"
        );
    }

    #[test]
    fn whole_old_selector_and_original_regression_are_retained_with_exact_glue() {
        let original = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/static-lexical-authority-guard-20261018/before/src/workspace_graph/source_schema.rs"));
        let start = original.find("fn select<'a>(").unwrap();
        let end = original[start..].find("\n// Hostile controls").unwrap() + start;
        let expected = original[start..end].trim_end().replacen("fn select<", "pub(super) fn before_select<", 1)
            .replacen("byte_helper::schema_with_proofs(module, modules, facts, proofs)", "byte_helper::before_static_guard_schema_with_proofs(module, modules, facts, proofs)", 1);
        let selector = include_str!("tests/before_static_lexical_selector.rs");
        let start = selector.find("pub(super) fn before_select<'a>(").unwrap();
        assert_eq!(selector[start..].trim_end(), expected);
        let current = include_str!("tests.rs");
        let original_tests = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/static-lexical-authority-guard-20261018/before/src/workspace_graph/source_schema/static_text/tests.rs"));
        assert!(
            current.starts_with(original_tests),
            "every original assertion and entire old test file remains byte-exact prefix"
        );
        assert_eq!(
            include_str!("../../source_schema.rs"),
            original,
            "select and its entire owning source are byte-exact original"
        );
    }
}
