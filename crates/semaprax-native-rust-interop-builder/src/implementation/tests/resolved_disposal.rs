//! Resolved-program disposal proofs: preallocated frames, depth bounds,
//! and disposal on every late failure.

use super::*;

#[test]
fn resolved_owner_disposal_is_preallocated_and_depth_bounded() {
    let source = include_str!("../../../../../tests/fixtures/native_rust_hir_capacity.spx");
    let program = crate::parse(source, Path::new("native-rust-hir-capacity.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan)
        .unwrap()
        .disposal_frames;
    let resolved = hir::resolve(&program).unwrap();
    assert!(
        !resolved.function_instances.is_empty(),
        "generic instances are required"
    );
    assert!(resolved.interfaces.iter().any(|interface| {
        interface
            .imports
            .iter()
            .any(|import| !import.parameters.is_empty())
    }));
    assert!(resolved.functions.iter().any(|function| {
        function.cleanup.slots.iter().any(|slot| {
            matches!(
                slot.shape,
                semaprax::cleanup::FieldLivenessShape::Leaf { .. }
                    | semaprax::cleanup::FieldLivenessShape::Record { .. }
            )
        })
    }));
    let mut staged_sources = [false; 3];
    for transition in resolved.functions.iter().flat_map(|function| {
        function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
    }) {
        if let crate::cleanup_plan::CleanupTransition::StageCopyResult { source } = transition {
            match source {
                crate::cleanup_plan::StagedCopyResultSource::Body { .. } => {
                    staged_sources[0] = true
                }
                crate::cleanup_plan::StagedCopyResultSource::TryResidual { .. } => {
                    staged_sources[1] = true
                }
                crate::cleanup_plan::StagedCopyResultSource::TryOptionNone { .. } => {
                    staged_sources[2] = true
                }
            }
        }
    }
    assert_eq!(staged_sources, [true; 3]);
    RESOLVED_DISPOSE_HIGH_WATER.with(|water| water.set(0));
    RESOLVED_DISPOSE_COMPLETIONS.with(|count| count.set(0));
    RESOLVED_DISPOSE_CAPACITIES.with(|capacities| capacities.set([0; 2]));
    let frames = Vec::with_capacity(capacity);
    assert_eq!(frames.capacity(), capacity);
    let owner = ResolvedProgramOwner::new(resolved, frames, capacity);
    drop(owner);
    assert_eq!(RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get), 1);
    let high_water = RESOLVED_DISPOSE_HIGH_WATER.with(std::cell::Cell::get);
    assert!(high_water > 0);
    assert!(high_water <= capacity);
    assert_eq!(
        RESOLVED_DISPOSE_CAPACITIES.with(std::cell::Cell::get),
        [capacity; 2]
    );
    assert_eq!(std::mem::size_of::<ResolvedDisposeFrame>(), 56);
}

#[test]
fn resolved_owner_disposes_nested_patterns_and_514_level_resource_cleanup() {
    let pattern_source = "module disposal.patterns; @id(\"disposal.inner\") record Inner { @id(\"disposal.inner.value\") value: i64, } @id(\"disposal.outer\") record Outer { @id(\"disposal.outer.inner\") inner: Inner, } @id(\"disposal.choice\") variant Choice { @id(\"disposal.choice.value\") Value { @id(\"disposal.choice.value.payload\") payload: i64, }, @id(\"disposal.choice.empty\") Empty, } @id(\"disposal.record.match\") fn record_match(input: Outer) -> i64 { match input { Outer { inner: Inner { value } } => value, } } @id(\"disposal.variant.match\") fn variant_match(input: Choice) -> i64 { match input { Choice::Value { payload } => payload, Choice::Empty {} => 0, } } @id(\"app.main\") fn main() -> i64 { 0 }";
    let pattern_program = crate::parse(pattern_source, Path::new("disposal-patterns.spx")).unwrap();
    let pattern_canonical = crate::format::canonical(&pattern_program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let pattern_capacity =
        hir_pre_resolve_capacity(&pattern_program, pattern_canonical.len(), &mut scan)
            .unwrap()
            .disposal_frames;
    let pattern_resolved = hir::resolve(&pattern_program).unwrap();
    assert_resolved_owner_disposes_once_without_growth(pattern_resolved, pattern_capacity);

    let mut chain = String::from(
        "module disposal.cleanup_chain; @id(\"cleanup.r0\") resource R0 { @id(\"cleanup.r0.drop\") drop trivial; } ",
    );
    for index in 1..514 {
        use std::fmt::Write as _;
        write!(
                chain,
                "@id(\"cleanup.r{index}\") record R{index} {{ @id(\"cleanup.r{index}.value\") value: R{}, }} ",
                index - 1
            )
            .unwrap();
    }
    chain.push_str("@id(\"cleanup.consume\") fn consume(value: own R513) -> i64 { 1 } @id(\"app.main\") fn main() -> i64 { 0 }");
    let chain_program = crate::parse(&chain, Path::new("disposal-cleanup-chain.spx")).unwrap();
    let chain_canonical = crate::format::canonical(&chain_program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let chain_capacity = hir_pre_resolve_capacity(&chain_program, chain_canonical.len(), &mut scan)
        .unwrap()
        .disposal_frames;
    let chain_resolved = hir::resolve(&chain_program).unwrap();
    let consume = chain_resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "cleanup.consume")
        .unwrap();
    let mut maximum_shape_depth = 0usize;
    let mut pending = consume
        .cleanup_plan
        .slots
        .iter()
        .map(|slot| (&slot.field_liveness_shape, 1usize))
        .collect::<Vec<_>>();
    while let Some((shape, depth)) = pending.pop() {
        maximum_shape_depth = maximum_shape_depth.max(depth);
        if let semaprax::cleanup::FieldLivenessShape::Record { fields, .. } = shape {
            pending.extend(fields.iter().map(|field| (&field.shape, depth + 1)));
        }
    }
    assert_eq!(maximum_shape_depth, 514);
    assert_resolved_owner_disposes_once_without_growth(chain_resolved, chain_capacity);
}

#[test]
fn resolved_owner_disposes_every_statement_variant_without_growth() {
    let statement_source = r#"
module disposal.statements;
permit { unsafe }

@id("app.main")
fn main() -> i64 {
    let mut count = 0;
    @audit("disposal only")
    unsafe { 0 }
    while count < 1 {
        count = count + 1;
        false
    }
    count
}
"#;
    let statement_program =
        crate::parse(statement_source, Path::new("disposal-statements.spx")).unwrap();
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let statement_stats = scan_ast_capacity(
        statement_program.functions.iter().flat_map(|function| {
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
        }),
        &statement_program,
        false,
        &mut scan,
    )
    .unwrap();
    // This scalar-only fixture has no nominal declaration occurrence; the
    // shared disposal formula's minimum type-occurrence upper is exactly one.
    let maximum_type_occurrences = 1usize;
    let statement_capacity = statement_stats
        .max_depth
        .checked_mul(4)
        .and_then(|frames| frames.checked_add(maximum_type_occurrences.checked_mul(2)?))
        .and_then(|frames| frames.checked_add(16))
        .unwrap();
    let statement_resolved = hir::resolve(&statement_program).unwrap();
    assert_resolved_owner_disposes_once_without_growth(statement_resolved, statement_capacity);
}

#[test]
fn resolved_owner_disposes_exact_depth_guard_without_growth() {
    fn first_guard_arm(program: &mut Program) -> &mut crate::ast::MatchArm {
        let crate::ast::ExprKind::Block { tail, .. } = &mut program.functions[0].body.kind else {
            panic!("guard fixture function body must be a block");
        };
        let crate::ast::ExprKind::Match { arms, .. } = &mut tail.kind else {
            panic!("guard fixture tail must be a match");
        };
        &mut arms[0]
    }

    let guard_source = "module disposal.guard; @id(\"disposal.guard\") fn guarded(input: i64) -> i64 { match input { 0 if true => 0, _ => 1, } } @id(\"app.main\") fn main() -> i64 { 0 }";
    let mut guard_program =
        crate::parse(guard_source, Path::new("disposal-exact-guard.spx")).unwrap();
    let guard = first_guard_arm(&mut guard_program).guard.take().unwrap();
    let mut guard = *guard;
    // Function block -> match -> guard is depth three.
    for _ in 0..MAX_SEMANTIC_EXPRESSION_DEPTH - 3 {
        let span = guard.span;
        guard = crate::ast::Expr {
            kind: crate::ast::ExprKind::Unary {
                op: crate::ast::UnaryOp::Not,
                value: Box::new(guard),
            },
            span,
        };
    }
    first_guard_arm(&mut guard_program).guard = Some(Box::new(guard));
    validate_native_rust_source_expression_budget(&guard_program).unwrap();

    let exact_guard = first_guard_arm(&mut guard_program).guard.take().unwrap();
    let span = exact_guard.span;
    first_guard_arm(&mut guard_program).guard = Some(Box::new(crate::ast::Expr {
        kind: crate::ast::ExprKind::Unary {
            op: crate::ast::UnaryOp::Not,
            value: exact_guard,
        },
        span,
    }));
    assert_eq!(
        validate_native_rust_source_expression_budget(&guard_program)
            .unwrap_err()
            .code,
        "SPX-B109"
    );
    let over_guard = first_guard_arm(&mut guard_program).guard.take().unwrap();
    first_guard_arm(&mut guard_program).guard = match over_guard.kind {
        crate::ast::ExprKind::Unary { value, .. } => Some(value),
        _ => unreachable!(),
    };

    let guard_canonical = crate::format::canonical(&guard_program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let guard_capacity = hir_pre_resolve_capacity(&guard_program, guard_canonical.len(), &mut scan)
        .unwrap()
        .disposal_frames;
    let guard_resolved = hir::resolve(&guard_program).unwrap();
    validate_native_rust_expression_budget(&guard_resolved).unwrap();
    assert_resolved_owner_disposes_once_without_growth(guard_resolved, guard_capacity);
}

#[test]
fn resolved_owner_undersized_workspace_aborts_before_post_drop_marker() {
    const CHILD_ENV: &str = "SEMAPRAX_TEST_UNDERSIZED_RESOLVED_DISPOSE";
    const BEFORE_MARKER: &str = "before-drop";
    const FORBIDDEN_MARKER: &str = "after-drop";

    if let Some(marker_root) = std::env::var_os(CHILD_ENV) {
        let marker_root = std::path::PathBuf::from(marker_root);
        let source = include_str!("../../../../../tests/fixtures/native_rust_hir_capacity.spx");
        let program = crate::parse(source, Path::new("native-rust-hir-capacity.spx"))
            .expect("child fixture parses");
        let resolved = hir::resolve(&program).expect("child fixture resolves");
        let owner = ResolvedProgramOwner::new(resolved, Vec::with_capacity(1), 1);
        std::fs::write(marker_root.join(BEFORE_MARKER), b"entered drop")
            .expect("write pre-drop marker");
        drop(owner);
        std::fs::write(marker_root.join(FORBIDDEN_MARKER), b"drop returned")
            .expect("write forbidden post-drop marker");
        return;
    }

    let marker_root =
        std::env::temp_dir().join(format!("semaprax-resolved-dispose-{}", std::process::id()));
    std::fs::create_dir(&marker_root).expect("create child marker directory");
    let output = Command::new(std::env::current_exe().expect("test executable path"))
            .arg("implementation::tests::resolved_disposal::resolved_owner_undersized_workspace_aborts_before_post_drop_marker")
            .arg("--exact")
            .arg("--nocapture")
            .env(CHILD_ENV, &marker_root)
            .output()
            .expect("undersized disposal child starts");
    assert!(!output.status.success());
    assert!(marker_root.join(BEFORE_MARKER).is_file());
    assert!(!marker_root.join(FORBIDDEN_MARKER).exists());
    std::fs::remove_file(marker_root.join(BEFORE_MARKER)).expect("remove child marker");
    std::fs::remove_dir(&marker_root).expect("remove child marker directory");
}

#[test]
fn resolved_owner_disposes_on_every_late_prepare_failure() {
    let (program, spec) = fixture();
    for point in [
        PrepareFailurePoint::Closure,
        PrepareFailurePoint::Facts,
        PrepareFailurePoint::Render,
        PrepareFailurePoint::Replay,
    ] {
        RESOLVED_DISPOSE_COMPLETIONS.with(|count| count.set(0));
        RESOLVED_DISPOSE_CAPACITIES.with(|capacities| capacities.set([0; 2]));
        PREPARE_FAILURE_INJECTION.with(|selected| selected.set(Some(point)));
        let result = prepare_native_rust_interop(&program, spec.as_bytes());
        PREPARE_FAILURE_INJECTION.with(|selected| selected.set(None));
        let diagnostic = result.err().expect("injected stage must fail");
        assert_eq!(diagnostic.len(), 1, "{point:?}");
        assert_eq!(diagnostic[0].code, "SPX-B107", "{point:?}");
        assert_eq!(
            RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get),
            1,
            "{point:?}"
        );
        let capacities = RESOLVED_DISPOSE_CAPACITIES.with(std::cell::Cell::get);
        assert!(capacities[0] > 0, "{point:?}");
        assert_eq!(capacities[0], capacities[1], "{point:?}");
    }
}

#[test]
fn prebuilt_exact_depth_program_prepares_and_disposes_in_child() {
    const CHILD_ENV: &str = "SEMAPRAX_TEST_PREBUILT_DEPTH_DISPOSE";
    const CHILD_SHAPE_ENV: &str = "SEMAPRAX_TEST_PREBUILT_DEPTH_SHAPE";
    const CHILD_DEPTH_ENV: &str = "SEMAPRAX_TEST_PREBUILT_DEPTH_VALUE";
    const CHILD_MARKER_ENV: &str = "SEMAPRAX_TEST_PREBUILT_DEPTH_MARKERS";
    const READY: &str = "ready";
    const DONE: &str = "done";
    const REJECTED: &str = "rejected";

    if std::env::var_os(CHILD_ENV).is_some() {
        let shape_value = std::env::var(CHILD_SHAPE_ENV).expect("child shape");
        let shape = shape_value.as_str();
        let over = std::env::var(CHILD_DEPTH_ENV).as_deref() == Ok("513");
        let marker_root =
            std::path::PathBuf::from(std::env::var_os(CHILD_MARKER_ENV).expect("marker root"));
        let source = format!(
            "module prebuilt.{shape}; @id(\"prebuilt.{shape}.deep\") fn deep(value: bool) -> bool {{ value }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
        );
        let mut program = crate::parse(&source, Path::new("prebuilt-depth.spx")).unwrap();
        let mut serial = 1usize;
        loop {
            let function = program
                .functions
                .iter_mut()
                .find(|function| function.stable_id.ends_with(".deep"))
                .expect("selected function exists");
            let body = std::mem::replace(
                &mut function.body,
                crate::ast::Expr {
                    span: crate::ast::Span::default(),
                    kind: crate::ast::ExprKind::Bool(false),
                },
            );
            let span = crate::ast::Span {
                start: serial,
                end: serial + 1,
                line: serial + 1,
                column: 1,
            };
            serial += 2;
            function.body = crate::ast::Expr {
                span,
                kind: if shape == "if" {
                    crate::ast::ExprKind::If {
                        condition: Box::new(crate::ast::Expr {
                            span: crate::ast::Span {
                                start: serial,
                                end: serial + 1,
                                line: serial + 1,
                                column: 1,
                            },
                            kind: crate::ast::ExprKind::Bool(true),
                        }),
                        then_branch: Box::new(body),
                        else_branch: Box::new(crate::ast::Expr {
                            span: crate::ast::Span {
                                start: serial + 2,
                                end: serial + 3,
                                line: serial + 3,
                                column: 1,
                            },
                            kind: crate::ast::ExprKind::Bool(false),
                        }),
                    }
                } else {
                    crate::ast::ExprKind::Binary {
                        op: crate::ast::BinaryOp::And,
                        left: Box::new(crate::ast::Expr {
                            span: crate::ast::Span {
                                start: serial,
                                end: serial + 1,
                                line: serial + 1,
                                column: 1,
                            },
                            kind: crate::ast::ExprKind::Bool(true),
                        }),
                        right: Box::new(body),
                    }
                },
            };
            serial += 4;
            let _ = function;
            if validate_native_rust_source_expression_budget(&program).is_err() {
                if !over {
                    let function = program
                        .functions
                        .iter_mut()
                        .find(|function| function.stable_id.ends_with(".deep"))
                        .expect("selected function exists");
                    let wrapper = std::mem::replace(
                        &mut function.body,
                        crate::ast::Expr {
                            span,
                            kind: crate::ast::ExprKind::Bool(false),
                        },
                    );
                    function.body = match wrapper.kind {
                        crate::ast::ExprKind::If { then_branch, .. } => *then_branch,
                        crate::ast::ExprKind::Binary { right, .. } => *right,
                        _ => unreachable!(),
                    };
                }
                break;
            }
        }
        let canonical = crate::format::canonical(&program);
        let spec = render_spec(&Spec {
            module: program.module.clone(),
            source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical.as_bytes())),
            target: current_target().unwrap(),
            exports: vec![format!("prebuilt.{shape}.deep")],
            imports: Vec::new(),
            capabilities: Vec::new(),
        });
        std::fs::write(marker_root.join(READY), b"ready").unwrap();
        RESOLVED_DISPOSE_COMPLETIONS.with(|count| count.set(0));
        HIR_RESOLVE_PASS_COUNT.with(|count| count.set(0));
        POST_HIR_FACTS_ENTRY_COUNT.with(|count| count.set(0));
        let result = prepare_native_rust_interop(&program, spec.as_bytes());
        if over {
            let diagnostics = match result {
                Err(diagnostics) => diagnostics,
                Ok(_) => panic!("depth 513 unexpectedly prepared"),
            };
            assert_eq!(diagnostics[0].code, "SPX-B109");
            assert_eq!(RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get), 0);
            HIR_RESOLVE_PASS_COUNT.with(|count| assert_eq!(count.get(), 0));
            POST_HIR_FACTS_ENTRY_COUNT.with(|count| assert_eq!(count.get(), 0));
            std::fs::write(marker_root.join(REJECTED), b"rejected").unwrap();
        } else {
            let prepared = result.unwrap();
            assert_eq!(RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get), 1);
            drop(prepared);
            std::fs::write(marker_root.join(DONE), b"done").unwrap();
        }
        std::mem::forget(program);
        std::process::exit(0);
    }

    let marker_root = std::env::temp_dir().join(format!(
        "semaprax-prebuilt-depth-dispose-{}",
        std::process::id()
    ));
    std::fs::create_dir(&marker_root).expect("create hosted marker directory");
    for (shape, depth, marker) in [
        ("if", "512", DONE),
        ("if", "513", REJECTED),
        ("lazy", "512", DONE),
        ("lazy", "513", REJECTED),
    ] {
        let output = Command::new(std::env::current_exe().unwrap())
                .arg("implementation::tests::resolved_disposal::prebuilt_exact_depth_program_prepares_and_disposes_in_child")
                .arg("--exact")
                .arg("--nocapture")
                .env(CHILD_ENV, "1")
                .env(CHILD_SHAPE_ENV, shape)
                .env(CHILD_DEPTH_ENV, depth)
                .env(CHILD_MARKER_ENV, &marker_root)
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{shape}/{depth}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(marker_root.join(READY).is_file());
        std::fs::remove_file(marker_root.join(READY)).unwrap();
        assert!(marker_root.join(marker).is_file());
        std::fs::remove_file(marker_root.join(marker)).unwrap();
    }
    std::fs::remove_dir(&marker_root).expect("remove hosted marker directory");
}

#[test]
fn every_expression_shape_resolves_at_exact_depth_512_and_rejects_513() {
    fn wrap_source(mut expression: crate::ast::Expr, count: usize) -> crate::ast::Expr {
        for _ in 0..count {
            let span = expression.span;
            expression = crate::ast::Expr {
                kind: crate::ast::ExprKind::Unary {
                    op: crate::ast::UnaryOp::Neg,
                    value: Box::new(expression),
                },
                span,
            };
        }
        expression
    }

    fn replace_payload(
        expression: &mut crate::ast::Expr,
        replacement: &mut Option<crate::ast::Expr>,
    ) -> bool {
        use crate::ast::ExprKind;

        if matches!(&expression.kind, ExprKind::Var(name) if name == "payload") {
            *expression = replacement.take().expect("payload replacement is unique");
            return true;
        }
        match &mut expression.kind {
            ExprKind::Call { args, .. } => args
                .iter_mut()
                .any(|child| replace_payload(child, replacement)),
            ExprKind::Unary { value, .. }
            | ExprKind::Try { operand: value }
            | ExprKind::Project { base: value, .. } => replace_payload(value, replacement),
            ExprKind::SuperMethod { args, .. } => args
                .iter_mut()
                .any(|child| replace_payload(child, replacement)),
            ExprKind::Binary { left, right, .. } => {
                replace_payload(left, replacement) || replace_payload(right, replacement)
            }
            ExprKind::Block { statements, tail } => {
                statements.iter_mut().any(|statement| match statement {
                    crate::ast::Statement::Let { value, .. }
                    | crate::ast::Statement::Assign { value, .. } => {
                        replace_payload(value, replacement)
                    }
                    crate::ast::Statement::Unsafe { body, .. } => {
                        replace_payload(body, replacement)
                    }
                    crate::ast::Statement::While {
                        condition, body, ..
                    } => {
                        replace_payload(condition, replacement)
                            || replace_payload(body, replacement)
                    }
                }) || replace_payload(tail, replacement)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                replace_payload(condition, replacement)
                    || replace_payload(then_branch, replacement)
                    || replace_payload(else_branch, replacement)
            }
            ExprKind::ConstructRecord { fields, .. }
            | ExprKind::ConstructVariant { fields, .. } => fields
                .iter_mut()
                .any(|field| replace_payload(&mut field.value, replacement)),
            ExprKind::Match {
                scrutinee, arms, ..
            } => {
                replace_payload(scrutinee, replacement)
                    || arms.iter_mut().any(|arm| {
                        arm.guard
                            .as_mut()
                            .is_some_and(|guard| replace_payload(guard, replacement))
                            || replace_payload(&mut arm.value, replacement)
                    })
            }
            ExprKind::UpdateRecord { base, fields } => {
                replace_payload(base, replacement)
                    || fields
                        .iter_mut()
                        .any(|field| replace_payload(&mut field.value, replacement))
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                replace_payload(receiver, replacement)
                    || args
                        .iter_mut()
                        .any(|child| replace_payload(child, replacement))
            }
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Char(_)
            | ExprKind::Uint8(_)
            | ExprKind::Usize(_)
            | ExprKind::ArrayU8(_)
            | ExprKind::RepeatArrayU8 { .. }
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Var(_) => false,
        }
    }

    fn functions(program: &Program) -> impl Iterator<Item = &crate::ast::Function> {
        program
            .functions
            .iter()
            .chain(
                program
                    .types
                    .iter()
                    .flat_map(|declaration| match &declaration.kind {
                        crate::ast::TypeDeclarationKind::Class { methods, .. } => {
                            methods.as_slice()
                        }
                        _ => &[],
                    }),
            )
    }

    fn deep_function(program: &Program) -> &crate::ast::Function {
        functions(program)
            .find(|function| function.stable_id.ends_with(".deep"))
            .expect("fixture deep function must exist")
    }

    fn deep_function_mut(program: &mut Program) -> &mut crate::ast::Function {
        if let Some(index) = program
            .functions
            .iter()
            .position(|function| function.stable_id.ends_with(".deep"))
        {
            return &mut program.functions[index];
        }
        for declaration in &mut program.types {
            if let crate::ast::TypeDeclarationKind::Class { methods, .. } = &mut declaration.kind {
                if let Some(index) = methods
                    .iter()
                    .position(|function| function.stable_id.ends_with(".deep"))
                {
                    return &mut methods[index];
                }
            }
        }
        panic!("fixture deep function must exist")
    }

    fn source_depth(program: &Program) -> usize {
        let mut maximum = 0;
        let mut pending = functions(program)
            .flat_map(|function| {
                function
                    .requires
                    .iter()
                    .chain(std::iter::once(&function.body))
                    .chain(&function.ensures)
            })
            .map(|expression| (expression, 1_usize))
            .collect::<Vec<_>>();
        while let Some((expression, depth)) = pending.pop() {
            maximum = maximum.max(depth);
            let mut cursor = 0;
            while let Some((_, child)) = ast_child(expression, &mut cursor) {
                pending.push((child, depth + 1));
            }
        }
        maximum
    }

    fn payload_depth(program: &Program) -> usize {
        let deep = deep_function(program);
        let mut pending = vec![(&deep.body, 1_usize)];
        while let Some((expression, depth)) = pending.pop() {
            if matches!(&expression.kind, crate::ast::ExprKind::Var(name) if name == "payload") {
                return depth;
            }
            let mut cursor = 0;
            while let Some((_, child)) = ast_child(expression, &mut cursor) {
                pending.push((child, depth + 1));
            }
        }
        panic!("fixture payload must be present")
    }

    fn wrap_hir_body_once(program: &mut ResolvedProgram) {
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.id.as_str().ends_with(".deep"))
            .expect("fixture deep function must resolve");
        let placeholder = ResolvedExpr {
            id: function.body.id.clone(),
            ty: function.body.ty.clone(),
            ownership: function.body.ownership,
            span: function.body.span,
            kind: ResolvedExprKind::Int(0),
        };
        let body = std::mem::replace(&mut function.body, placeholder);
        function.body = ResolvedExpr {
            id: body.id.clone(),
            ty: body.ty.clone(),
            ownership: body.ownership,
            span: body.span,
            kind: ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Neg,
                value: Box::new(body),
            },
        };
    }

    let cases = [
        (
            "unary",
            "module depth.unary; @id(\"depth.unary.deep\") fn deep(payload: i64) -> i64 { -payload } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "binary",
            "module depth.binary; @id(\"depth.binary.deep\") fn deep(payload: i64) -> i64 { payload + 0 } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "binary-right",
            "module depth.binary_right; @id(\"depth.binary_right.deep\") fn deep(payload: i64) -> i64 { 0 + payload } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "if",
            "module depth.if_shape; @id(\"depth.if.deep\") fn deep(payload: i64) -> i64 { if true { payload } else { 0 } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "if-condition",
            "module depth.if_condition; @id(\"depth.if_condition.deep\") fn deep(payload: i64) -> i64 { if payload > 0 { 1 } else { 0 } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "if-else",
            "module depth.if_else; @id(\"depth.if_else.deep\") fn deep(payload: i64) -> i64 { if true { 0 } else { payload } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "block",
            "module depth.block; @id(\"depth.block.deep\") fn deep(payload: i64) -> i64 { let before = 0; payload } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "block-let-rhs",
            "module depth.block_let; @id(\"depth.block_let.deep\") fn deep(payload: i64) -> i64 { let value = payload; value } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "block-assign-rhs",
            "module depth.block_assign; @id(\"depth.block_assign.deep\") fn deep(payload: i64) -> i64 { let mut value = 0; value = payload; value } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "block-unsafe-body",
            "module depth.block_unsafe; permit { unsafe } @id(\"depth.block_unsafe.deep\") fn deep(payload: i64) -> i64 uses { unsafe } { @audit(\"depth fixture\") unsafe { payload } 0 } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "block-while-condition",
            "module depth.block_while_condition; @id(\"depth.block_while_condition.deep\") fn deep(payload: i64) -> i64 { while payload > 0 { 0 } 0 } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "block-while-body",
            "module depth.block_while_body; @id(\"depth.block_while_body.deep\") fn deep(payload: i64) -> i64 { while false { payload } 0 } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "call",
            "module depth.call; @id(\"depth.call.id\") fn id(value: i64) -> i64 { value } @id(\"depth.call.deep\") fn deep(payload: i64) -> i64 { id(payload) } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "call-first",
            "module depth.call_first; @id(\"depth.call_first.sum\") fn sum(a: i64, b: i64, c: i64) -> i64 { a + b + c } @id(\"depth.call_first.deep\") fn deep(payload: i64) -> i64 { sum(payload, 0, 0) } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "call-middle",
            "module depth.call_middle; @id(\"depth.call_middle.sum\") fn sum(a: i64, b: i64, c: i64) -> i64 { a + b + c } @id(\"depth.call_middle.deep\") fn deep(payload: i64) -> i64 { sum(0, payload, 0) } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "call-last",
            "module depth.call_last; @id(\"depth.call_last.sum\") fn sum(a: i64, b: i64, c: i64) -> i64 { a + b + c } @id(\"depth.call_last.deep\") fn deep(payload: i64) -> i64 { sum(0, 0, payload) } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "native-call",
            "module depth.native_call; permit { host.math } @id(\"host.math\") interface HostMath permits { host.math } { @id(\"host.add\") import rust fn host_add(left: i64, right: i64) -> i64 effects { host.math } failure status \"host.math.v1\"; } @id(\"depth.native_call.deep\") fn deep(payload: i64) -> i64 uses { host.math } { host_add(0, payload) } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "try",
            "module depth.try_shape; @id(\"depth.try.ok\") fn ok(value: i64) -> Result<i64, bool> { Result<i64, bool>::Ok { value: value } } @id(\"depth.try.deep\") fn deep(payload: i64) -> Result<i64, bool> { Result<i64, bool>::Ok { value: ok(payload)? } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "try-option",
            "module depth.try_option; @id(\"depth.try_option.some\") fn some(value: i64) -> Option<i64> { Option<i64>::Some { value: value } } @id(\"depth.try_option.deep\") fn deep(payload: i64) -> Option<i64> { Option<i64>::Some { value: some(payload)? } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "record-project",
            "module depth.record_project; @id(\"depth.pair\") record Pair { @id(\"depth.pair.x\") x: i64, } @id(\"depth.record_project.deep\") fn deep(payload: i64) -> i64 { Pair { x: payload }.x } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "record-field-first",
            "module depth.record_first; @id(\"depth.record_first.triple\") record Triple { @id(\"depth.record_first.triple.a\") a: i64, @id(\"depth.record_first.triple.b\") b: i64, @id(\"depth.record_first.triple.c\") c: i64, } @id(\"depth.record_first.deep\") fn deep(payload: i64) -> Triple { Triple { a: payload, b: 0, c: 0 } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "record-field-middle",
            "module depth.record_middle; @id(\"depth.record_middle.triple\") record Triple { @id(\"depth.record_middle.triple.a\") a: i64, @id(\"depth.record_middle.triple.b\") b: i64, @id(\"depth.record_middle.triple.c\") c: i64, } @id(\"depth.record_middle.deep\") fn deep(payload: i64) -> Triple { Triple { a: 0, b: payload, c: 0 } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "record-field-last",
            "module depth.record_last; @id(\"depth.record_last.triple\") record Triple { @id(\"depth.record_last.triple.a\") a: i64, @id(\"depth.record_last.triple.b\") b: i64, @id(\"depth.record_last.triple.c\") c: i64, } @id(\"depth.record_last.deep\") fn deep(payload: i64) -> Triple { Triple { a: 0, b: 0, c: payload } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "variant",
            "module depth.variant; @id(\"depth.choice\") variant Choice { @id(\"depth.choice.value\") Value { @id(\"depth.choice.value.value\") value: i64, }, } @id(\"depth.variant.deep\") fn deep(payload: i64) -> Choice { Choice::Value { value: payload } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "match",
            "module depth.match_shape; @id(\"depth.match.choice\") variant Choice { @id(\"depth.match.choice.none\") None, @id(\"depth.match.choice.value\") Value { @id(\"depth.match.choice.value.value\") value: i64, }, } @id(\"depth.match.deep\") fn deep(payload: i64) -> i64 { match Choice::Value { value: 0 } { Choice::Value { value } => payload, Choice::None {} => 0, } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "match-scrutinee",
            "module depth.match_scrutinee; @id(\"depth.match_scrutinee.choice\") variant Choice { @id(\"depth.match_scrutinee.choice.none\") None, @id(\"depth.match_scrutinee.choice.value\") Value { @id(\"depth.match_scrutinee.choice.value.value\") value: i64, }, } @id(\"depth.match_scrutinee.deep\") fn deep(payload: i64) -> i64 { match Choice::Value { value: payload } { Choice::Value { value } => value, Choice::None {} => 0, } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "match-later-arm",
            "module depth.match_later; @id(\"depth.match_later.choice\") variant Choice { @id(\"depth.match_later.choice.a\") A, @id(\"depth.match_later.choice.b\") B, @id(\"depth.match_later.choice.c\") C, } @id(\"depth.match_later.deep\") fn deep(choice: Choice, payload: i64) -> i64 { match choice { Choice::A {} => 0, Choice::B {} => payload, Choice::C {} => 0, } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "match-guard",
            "module depth.match_guard; @id(\"depth.match_guard.deep\") fn deep(tag: i64, payload: i64) -> i64 { match tag { 0 if payload > 0 => 1, _ => 0, } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "match-nested-record-pattern",
            "module depth.match_nested; @id(\"depth.match_nested.inner\") record Inner { @id(\"depth.match_nested.inner.value\") value: i64, } @id(\"depth.match_nested.outer\") record Outer { @id(\"depth.match_nested.outer.inner\") inner: Inner, @id(\"depth.match_nested.outer.other\") other: i64, } @id(\"depth.match_nested.deep\") fn deep(input: Outer, payload: i64) -> i64 { match input { Outer { inner: Inner { value }, other: _ } => payload, } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "update",
            "module depth.update; @id(\"depth.update.pair\") record Pair { @id(\"depth.update.pair.x\") x: i64, } @id(\"depth.update.deep\") fn deep(payload: i64) -> i64 { let pair = Pair { x: 0 }; (pair with { x: payload }).x } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "update-base",
            "module depth.update_base; @id(\"depth.update_base.pair\") record Pair { @id(\"depth.update_base.pair.x\") x: i64, } @id(\"depth.update_base.deep\") fn deep(payload: i64) -> i64 { (Pair { x: payload } with { x: 0 }).x } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "method-call-argument",
            "module depth.method_call; @id(\"depth.method_call.counter\") class Counter { @id(\"depth.method_call.counter.value\") value: i64, @id(\"depth.method_call.counter.add\") fn add(self: Counter, delta: i64) -> i64 { self.value + delta } } @id(\"depth.method_call.deep\") fn deep(payload: i64) -> i64 { Counter { value: 0 }.add(payload) } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
        (
            "super-method-argument",
            "module depth.super_method; @id(\"depth.super_method.base\") class Base { @id(\"depth.super_method.base.value\") value: i64, @id(\"depth.super_method.base.add\") fn add(self: Base, delta: i64) -> i64 { self.value + delta } } @id(\"depth.super_method.child\") class Child : Base { @id(\"depth.super_method.child.extra\") extra: i64, @id(\"depth.super_method.deep\") fn add(self: Child, payload: i64) -> i64 { super.add(payload) + self.extra } } @id(\"app.main\") fn main() -> i64 { 0 }",
        ),
    ];

    for (shape, source) in cases {
        let mut exact = crate::parse(source, Path::new("all-shape-depth.spx")).unwrap();
        let initial_depth = source_depth(&exact);
        assert!(initial_depth < MAX_SEMANTIC_EXPRESSION_DEPTH, "{shape}");
        let payload_depth = payload_depth(&exact);
        let replacement = wrap_source(
            crate::ast::Expr {
                kind: crate::ast::ExprKind::Var("payload".to_owned()),
                span: crate::ast::Span::default(),
            },
            MAX_SEMANTIC_EXPRESSION_DEPTH - payload_depth,
        );
        let function = deep_function_mut(&mut exact);
        assert!(replace_payload(&mut function.body, &mut Some(replacement)));
        assert_eq!(
            source_depth(&exact),
            MAX_SEMANTIC_EXPRESSION_DEPTH,
            "{shape}"
        );
        validate_native_rust_source_expression_budget(&exact).unwrap();
        let canonical = crate::format::canonical(&exact);
        let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let disposal_capacity = hir_pre_resolve_capacity(&exact, canonical.len(), &mut scan)
            .unwrap()
            .disposal_frames;
        let resolved = hir::resolve(&exact)
            .unwrap_or_else(|diagnostics| panic!("{shape} failed resolution: {diagnostics:?}"));
        validate_native_rust_expression_budget(&resolved).unwrap();
        assert_resolved_owner_disposes_once_without_growth(resolved, disposal_capacity);

        let mut resolved = hir::resolve(&exact)
            .unwrap_or_else(|diagnostics| panic!("{shape} failed resolution: {diagnostics:?}"));
        wrap_hir_body_once(&mut resolved);
        let over_disposal_capacity = disposal_capacity.checked_add(4).unwrap();

        let error = validate_native_rust_expression_budget(&resolved).unwrap_err();
        assert_eq!(error.code, "SPX-B109", "{shape}");
        assert_eq!(
            error.message, "Native Rust Interop max_semantic_expression_depth exceeds 512",
            "{shape}"
        );
        assert_resolved_owner_disposes_once_without_growth(resolved, over_disposal_capacity);

        let mut over_source = exact;
        let function = deep_function_mut(&mut over_source);
        let body = std::mem::replace(
            &mut function.body,
            crate::ast::Expr {
                kind: crate::ast::ExprKind::Int(0),
                span: crate::ast::Span::default(),
            },
        );
        function.body = wrap_source(body, 1);
        let error = validate_native_rust_source_expression_budget(&over_source).unwrap_err();
        assert_eq!(error.code, "SPX-B109", "{shape}");
        assert_eq!(
            error.message, "Native Rust Interop max_semantic_expression_depth exceeds 512",
            "{shape}"
        );
    }
}
