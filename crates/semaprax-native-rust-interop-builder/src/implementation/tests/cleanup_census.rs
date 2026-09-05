//! Retained cleanup census proofs over owned roots, branches, and
//! generic instances.

use super::*;

// Parsed source is capped below the private interop HIR ceiling. The synthetic
// depth-512 HIR walkers are exercised in `hir_traversal`; source-driven census
// tests use the deepest expression the public parser admits.
const MAX_PARSED_EXPRESSION_DEPTH: usize = 128;

#[test]
fn typed_cleanup_retained_census_covers_long_ids_and_many_owned_roots() {
    let long = "x".repeat(128);
    let mut source = String::from("module capacity.cleanup_typed;\n\n");
    writeln!(
        source,
        "@id(\"resource.{long}\") resource R0 {{ @id(\"lifecycle.{long}\") drop trivial; }}"
    )
    .unwrap();
    for index in 1..=64 {
        writeln!(
                source,
                "@id(\"record.{index:03}.{long}\") record R{index} {{ @id(\"field.{index:03}.{long}\") next: R{}, }}",
                index - 1
            )
            .unwrap();
    }
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}: own R64"))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(
        source,
        "@id(\"consume.typed\") fn consume({parameters}) -> i64 {{ 0 }}"
    )
    .unwrap();
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");

    let program = crate::parse(&source, Path::new("typed-cleanup-capacity.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(actual > MAX_PARAMETERS * 64 * 128);
    assert!(
        actual <= capacity.cleanup_retained_upper,
        "actual cleanup {actual} exceeds derived {}",
        capacity.cleanup_retained_upper
    );
}

#[test]
fn cleanup_retained_census_admits_depth_by_live_roots_with_long_identities() {
    let long = "x".repeat(128);
    let mut source = String::from("module capacity.cleanup_depth_live;\n\n");
    writeln!(
        source,
        "@id(\"resource.{long}\") resource R0 {{ @id(\"lifecycle.{long}\") drop trivial; }}"
    )
    .unwrap();
    source.push_str("@id(\"identity\") fn identity(value: own R0) -> R0 { value }\n");
    source.push_str("@id(\"consume\") fn consume(value: own R0) -> i64 { 1 }\n");
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}: own R0"))
        .chain(std::iter::once("value: i64".to_owned()))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(source, "@id(\"stress\") fn stress({parameters}) -> i64 {{").unwrap();
    for index in 0..MAX_PARAMETERS {
        writeln!(source, "let live{index} = identity(p{index});").unwrap();
    }
    source.push_str("let checked = value");
    for _ in 0..126 {
        source.push_str(" + 1");
    }
    source.push_str(";\nchecked + ");
    for index in 0..MAX_PARAMETERS {
        if index != 0 {
            source.push_str(" + ");
        }
        write!(source, "consume(live{index})").unwrap();
    }
    source.push_str("\n}\n@id(\"app.main\") fn main() -> i64 { 0 }\n");

    let program = crate::parse(&source, Path::new("cleanup-depth-live.spx")).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    let mut depth_scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let depth = scan_ast_capacity(
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures),
        &program,
        false,
        &mut depth_scan,
    )
    .unwrap()
    .max_depth;
    assert_eq!(depth, MAX_PARSED_EXPRESSION_DEPTH);
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let owner = ResolvedProgramOwner::new(
        resolved,
        Vec::with_capacity(capacity.disposal_frames),
        capacity.disposal_frames,
    );
    let actual = owner
        .program()
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "actual cleanup {actual} exceeds authority {}",
        capacity.cleanup_authority_upper
    );
    assert!(
        capacity.complete().unwrap() <= MAX_BUILDER_BYTES,
        "depth×live capacity terms: {:?}; actual cleanup: {actual}",
        hir_capacity_terms_for_test(&program, canonical.len()).unwrap()
    );
    drop(owner);
}

#[test]
fn cleanup_retained_census_releases_sequential_early_move_epochs() {
    fn measure(delayed_moves: bool) -> (HirPreResolveCapacity, usize) {
        let long = "x".repeat(128);
        let mut source = String::from("module capacity.cleanup_sequential_moves;\n\n");
        writeln!(
            source,
            "@id(\"resource.{long}\") resource R0 {{ @id(\"lifecycle.{long}\") drop trivial; }}"
        )
        .unwrap();
        source.push_str("@id(\"identity\") fn identity(value: own R0) -> R0 { value }\n");
        source.push_str("@id(\"consume\") fn consume(value: own R0) -> i64 { 1 }\n");
        let parameters = (0..MAX_PARAMETERS)
            .map(|index| format!("p{index}: own R0"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(source, "@id(\"stress\") fn stress({parameters}) -> i64 {{").unwrap();
        for index in 0..MAX_PARAMETERS {
            writeln!(source, "let epoch{index} = identity(p{index});").unwrap();
            if !delayed_moves {
                writeln!(source, "let consumed{index} = consume(epoch{index});").unwrap();
            }
        }
        if delayed_moves {
            for index in 0..MAX_PARAMETERS {
                writeln!(source, "let consumed{index} = consume(epoch{index});").unwrap();
            }
        }
        source.push('0');
        for _ in 0..126 {
            source.push_str(" + 1");
        }
        source.push_str("\n}\n@id(\"app.main\") fn main() -> i64 { 0 }\n");

        let program = crate::parse(&source, Path::new("cleanup-sequential-moves.spx")).unwrap();
        let canonical = crate::format::canonical(&program);
        let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let owner = ResolvedProgramOwner::new(
            resolved,
            Vec::with_capacity(capacity.disposal_frames),
            capacity.disposal_frames,
        );
        let actual = owner
            .program()
            .functions
            .iter()
            .try_fold(0usize, |bytes, function| {
                bytes
                    .checked_add(
                        crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                            &function.cleanup,
                        )?,
                    )?
                    .checked_add(
                        crate::private_capacity_contract::cleanup_plan_owned_capacity(
                            &function.cleanup_plan,
                        )?,
                    )
            })
            .unwrap();
        assert!(actual <= capacity.cleanup_authority_upper);
        drop(owner);
        (capacity, actual)
    }

    let (early, early_actual) = measure(false);
    let (delayed, delayed_actual) = measure(true);
    assert!(early.cleanup_authority_upper < delayed.cleanup_authority_upper);
    assert!(early_actual < delayed_actual);
    for (arrangement, capacity) in [("early", early), ("delayed", delayed)] {
        assert!(
            capacity.complete().unwrap() <= MAX_BUILDER_BYTES,
            "sequential {arrangement}-move capacity {} exceeds {MAX_BUILDER_BYTES}",
            capacity.complete().unwrap()
        );
    }
}

#[test]
fn cleanup_binding_flow_releases_nested_moves_and_preserves_partial_projection() {
    fn measure(source: &str) -> (usize, HirPreResolveCapacity, usize, usize) {
        let program = crate::parse(source, Path::new("cleanup-nested-move.spx")).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.name == "stress")
            .unwrap();
        let mut traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let events =
            cleanup_parameter_finalizer_events(function, "value", &program, &mut traversal)
                .unwrap();
        let nodes = scan_ast_capacity(
            std::iter::once(&function.body),
            &program,
            false,
            &mut traversal,
        )
        .unwrap()
        .nodes;
        let canonical = crate::format::canonical(&program);
        let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut traversal).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let actual = resolved
            .functions
            .iter()
            .try_fold(0usize, |bytes, function| {
                bytes
                    .checked_add(
                        crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                            &function.cleanup,
                        )?,
                    )?
                    .checked_add(
                        crate::private_capacity_contract::cleanup_plan_owned_capacity(
                            &function.cleanup_plan,
                        )?,
                    )
            })
            .unwrap();
        assert!(actual <= capacity.cleanup_authority_upper);
        assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
        (events, capacity, actual, nodes)
    }

    let definitions = r#"
@id("flow.r") resource R { @id("flow.r.drop") drop trivial; }
@id("flow.consume") fn consume(value: own R) -> i64 { 1 }
"#;
    let cases = [
        (
            "block",
            "{ let moved = { consume(value) }; let observed = checked + 1; moved + observed }",
            "{ let observed = checked + 1; let moved = { consume(value) }; moved + observed }",
            false,
            true,
            "",
            "value: own R, checked: i64",
        ),
        (
            "if",
            "{ let moved = if condition { consume(value) } else { consume(value) }; let observed = checked + 1; moved + observed }",
            "{ let observed = checked + 1; let moved = if condition { consume(value) } else { consume(value) }; moved + observed }",
            false,
            true,
            "",
            "value: own R, checked: i64, condition: bool",
        ),
        (
            "match",
            "{ let moved = match choice { Choice::A {} => consume(value), Choice::B {} => consume(value), }; let observed = checked + 1; moved + observed }",
            "{ let observed = checked + 1; let moved = match choice { Choice::A {} => consume(value), Choice::B {} => consume(value), }; moved + observed }",
            false,
            true,
            "@id(\"flow.choice\") variant Choice { @id(\"flow.choice.a\") A {}, @id(\"flow.choice.b\") B {}, }",
            "value: own R, checked: i64, choice: Choice",
        ),
        (
            "construct",
            "{ let moved = consume_box(Box { value: value }); let observed = checked + 1; moved + observed }",
            "{ let observed = checked + 1; let moved = consume_box(Box { value: value }); moved + observed }",
            false,
            false,
            "@id(\"flow.box\") record Box { @id(\"flow.box.value\") value: R, } @id(\"flow.consume_box\") fn consume_box(value: own Box) -> i64 { 1 }",
            "value: own R, checked: i64",
        ),
        (
            "update",
            "{ let moved = consume_box(value with { item: replacement }); let observed = checked + 1; moved + observed }",
            "{ let observed = checked + 1; let moved = consume_box(value with { item: replacement }); moved + observed }",
            false,
            false,
            "@id(\"flow.box\") record Box { @id(\"flow.box.item\") item: R, } @id(\"flow.consume_box\") fn consume_box(value: own Box) -> i64 { 1 }",
            "value: own Box, replacement: own R, checked: i64",
        ),
        (
            "projection",
            "{ let moved = consume(value.left); let observed = checked + 1; moved + observed }",
            "{ let observed = checked + 1; let moved = consume(value.left); moved + observed }",
            true,
            false,
            "@id(\"flow.pair\") record Pair { @id(\"flow.pair.left\") left: R, @id(\"flow.pair.right\") right: R, }",
            "value: own Pair, checked: i64",
        ),
    ];
    for (shape, early_body, delayed_body, conservative, authority_drop, extra, parameters) in cases
    {
        let source = |body: &str| {
            format!(
                "module capacity.flow_{shape};\n{definitions}\n{extra}\n@id(\"flow.stress\") fn stress({parameters}) -> i64 {body}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
            )
        };
        let (early_events, early, _, early_nodes) = measure(&source(early_body));
        let (delayed_events, delayed, _, delayed_nodes) = measure(&source(delayed_body));
        assert_eq!(early_nodes, delayed_nodes, "{shape}");
        if conservative {
            assert_eq!(early_events, delayed_events, "{shape}");
        } else {
            assert!(early_events < delayed_events, "{shape}");
        }
        if authority_drop {
            assert!(
                early.cleanup_authority_upper < delayed.cleanup_authority_upper,
                "{shape}"
            );
        }
    }
}

#[test]
fn cleanup_retained_census_joins_mutually_exclusive_owned_branches() {
    let long = "x".repeat(128);
    let mut source = format!(
        "module capacity.cleanup_branch_live;\n@id(\"resource.{long}\") resource R {{ @id(\"lifecycle.{long}\") drop trivial; }}\n@id(\"identity\") fn identity(value: own R) -> R {{ value }}\n@id(\"consume\") fn consume(value: own R) -> i64 {{ 1 }}\n"
    );
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}: own R"))
        .chain(["condition: bool".to_owned(), "value: i64".to_owned()])
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(
        source,
        "@id(\"branch.stress\") fn stress({parameters}) -> i64 {{ if condition {{"
    )
    .unwrap();
    for index in 0..4 {
        writeln!(source, "let live{index} = identity(p{index});").unwrap();
    }
    source.push_str("let checked = value");
    for _ in 0..124 {
        source.push_str(" + 1");
    }
    source.push_str("; checked");
    for index in 0..4 {
        write!(source, " + consume(live{index})").unwrap();
    }
    source.push_str(" } else { ");
    for index in 4..8 {
        writeln!(source, "let live{index} = identity(p{index});").unwrap();
    }
    source.push_str("let checked = value");
    for _ in 0..124 {
        source.push_str(" + 1");
    }
    source.push_str("; checked");
    for index in 4..8 {
        write!(source, " + consume(live{index})").unwrap();
    }
    source.push_str(" } }\n@id(\"app.main\") fn main() -> i64 { 0 }\n");

    let program = crate::parse(&source, Path::new("cleanup-branch-live.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let stress = program
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    assert_eq!(
        scan_ast_capacity(std::iter::once(&stress.body), &program, false, &mut scan)
            .unwrap()
            .max_depth,
        MAX_PARSED_EXPRESSION_DEPTH
    );
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    assert!(
        capacity.complete().unwrap() <= MAX_BUILDER_BYTES,
        "branch capacity terms {:?}, plan structural {}, complete {}",
        hir_capacity_terms_for_test(&program, canonical.len()).unwrap(),
        capacity.cleanup_plan_structural_upper,
        capacity.complete().unwrap()
    );
    let resolved = hir::resolve(&program).unwrap();
    let owner = ResolvedProgramOwner::new(
        resolved,
        Vec::with_capacity(capacity.disposal_frames),
        capacity.disposal_frames,
    );
    let actual = owner
        .program()
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "branch actual cleanup {actual} exceeds authority {} (retained {}, structural {})",
        capacity.cleanup_authority_upper,
        capacity.cleanup_retained_upper,
        capacity.cleanup_authority_upper - capacity.cleanup_retained_upper
    );
    drop(owner);
}

#[test]
fn guarded_owned_match_result_census_is_position_independent_and_fail_closed() {
    fn fixture(guard: &str) -> Program {
        let source = format!(
            r#"
module capacity.cleanup_guarded_owned;

@id("cleanup.guard.resource")
resource R {{
    @id("cleanup.guard.resource.drop")
    drop trivial;
}}

@id("cleanup.guard.identity")
fn identity(value: own R) -> R {{ value }}

@id("cleanup.guard.choose")
fn choose(tag: i64, condition: bool, value: own R) -> R {{
    let selected = match tag {{
        0{guard} => identity(value),
        _ => identity(value),
    }};
    selected
}}

@id("app.main")
fn main() -> i64 {{ 0 }}
"#
        );
        crate::parse(&source, Path::new("cleanup-guarded-owned.spx")).unwrap()
    }

    let unguarded = fixture("");
    let guarded = fixture(" if condition");
    let unguarded_canonical = crate::format::canonical(&unguarded);
    let guarded_canonical = crate::format::canonical(&guarded);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let unguarded_capacity =
        hir_pre_resolve_capacity(&unguarded, unguarded_canonical.len(), &mut scan).unwrap();
    let guarded_capacity =
        hir_pre_resolve_capacity(&guarded, guarded_canonical.len(), &mut scan).unwrap();

    // A scalar guard adds no owned carrier. The guarded and unguarded first
    // arm values must therefore contribute the same resource roots; indexing
    // the guard as the arm value would make this exact equality fail.
    assert_eq!(
        guarded_capacity.cleanup_proof.stats.roots,
        unguarded_capacity.cleanup_proof.stats.roots
    );
    assert_eq!(
        guarded_capacity.cleanup_proof.stats.leaves,
        unguarded_capacity.cleanup_proof.stats.leaves
    );

    let diagnostic = hir::resolve(&guarded).unwrap_err();
    assert!(diagnostic.iter().any(|diagnostic| {
        diagnostic.code == "SPX-T258"
            && diagnostic
                .message
                .contains("aggregate-valued match arms are outside the executable match profile")
    }));
}

#[test]
fn cleanup_retained_census_covers_shared_transition_and_staging_families() {
    let source = include_str!("../../../../../tests/fixtures/native_rust_hir_capacity.spx");
    let program = crate::parse(source, Path::new("native-rust-hir-capacity.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let actual = resolved
        .functions
        .iter()
        .chain(
            resolved
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    let actual_exits = resolved
        .functions
        .iter()
        .chain(
            resolved
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .try_fold(0usize, |count, function| {
            count.checked_add(function.cleanup_plan.exits.len())
        })
        .unwrap();
    assert!(resolved.functions.iter().any(|function| {
        function.cleanup_plan.blocks.iter().any(|block| {
            block.transitions.iter().any(|transition| {
                matches!(
                    transition,
                    semaprax::cleanup_plan::CleanupTransition::CallCommit { .. }
                )
            })
        })
    }));
    assert!(resolved.functions.iter().any(|function| {
        function.cleanup_plan.blocks.iter().any(|block| {
            block.transitions.iter().any(|transition| {
                matches!(
                    transition,
                    semaprax::cleanup_plan::CleanupTransition::StageCopyResult { .. }
                )
            })
        })
    }));
    assert!(resolved.functions.iter().any(|function| {
        function.cleanup_plan.edges.iter().any(|edge| {
            matches!(
                edge.condition,
                semaprax::cleanup_plan::EdgeCondition::VariantCase { .. }
            )
        })
    }));
    assert!(actual_exits <= capacity.cleanup_exit_events_upper);
    assert!(actual <= capacity.cleanup_retained_upper);
}

#[test]
fn hir_retained_capacity_counts_complete_loan_plans_on_generic_instances() {
    let source = r#"
module capacity.loan_plan_instance;

@id("loan.generic")
fn generic_marker<T>(marker: T) -> i64 { 1 }

@id("loan.concrete")
fn concrete_loan() -> i64 {
    let source = [7u8, 8u8, 9u8];
    let owned = bytes_copy(array_as_slice(source));
    let parent = bytes_as_slice(owned);
    let child = byte_range(parent, 1usize, byte_len(parent));
    if byte_len(child) == 2usize { 1 } else { 0 }
}

@id("app.main")
fn main() -> i64 { generic_marker<i64>(0) }
"#;
    let program = crate::parse(source, Path::new("loan-plan-instance-capacity.spx")).unwrap();
    let mut resolved = hir::resolve(&program).unwrap();
    let instance_index = resolved
        .function_instances
        .iter()
        .position(|instance| instance.template.as_str() == "loan.generic")
        .expect("generic call materializes a concrete function instance");
    let canonical_plan = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "loan.concrete")
        .expect("concrete loan function is retained")
        .loan_plan
        .clone();
    assert!(!canonical_plan.loans.is_empty());
    // Generic templates are intentionally scalar-only today. Attach a plan
    // produced by the real canonical builder to the concrete instance solely
    // to prove that this retained-capacity census does not omit instances if
    // their semantic admission grows in a later schema.
    resolved.function_instances[instance_index]
        .function
        .loan_plan = canonical_plan;
    let plan = &resolved.function_instances[instance_index]
        .function
        .loan_plan;
    assert!(!plan.loans.is_empty());
    assert!(!plan.endpoints.is_empty());
    assert!(!plan.edges.is_empty());
    assert!(plan.loans.iter().any(|loan| !loan.ends.is_empty()));
    assert!(plan.loans.iter().any(|loan| !loan.end_edges.is_empty()));
    assert!(plan.edges.iter().any(|edge| !edge.live.is_empty()));

    let plan_capacity = capacity::hir_loan_plan_owned_capacity(plan).unwrap();
    assert!(
        plan_capacity > plan.loans.capacity() * std::mem::size_of::<semaprax::loan_plan::Loan>(),
        "nested identities and CFG vectors must be charged in addition to loan headers"
    );
    let with_plan = hir_owned_capacity(&resolved).unwrap();
    resolved.function_instances[instance_index]
        .function
        .loan_plan = semaprax::loan_plan::LoanPlan {
        schema: semaprax::loan_plan::LOAN_PLAN_SCHEMA_V1,
        loans: Vec::new(),
        endpoints: Vec::new(),
        edges: Vec::new(),
    };
    let without_plan = hir_owned_capacity(&resolved).unwrap();
    assert_eq!(with_plan.checked_sub(without_plan), Some(plan_capacity));
}

#[test]
fn cleanup_fieldwise_payload_and_vec_floors_are_covered() {
    let source = include_str!("../../../../../tests/fixtures/native_rust_hir_capacity.spx");
    let program = crate::parse(source, Path::new("native-rust-hir-capacity.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let proof = capacity.cleanup_proof;
    let resolved = hir::resolve(&program).unwrap();
    let mut observed = ObservedCleanupProof::default();
    for function in resolved.functions.iter().chain(
        resolved
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        assert!(
            observe_cleanup_function(function, &mut observed).is_some(),
            "cleanup proof observer encountered an unadmitted non-exhaustive family"
        );
    }

    let stats = proof.stats;
    assert!(observed.slot_payload_bytes <= stats.ordinary_slot_payload_bytes);
    assert!(observed.call_argument_slot_payload_bytes <= stats.call_argument_owned_bytes);
    assert!(observed.shape_identity_bytes <= stats.shape_ids * 2);
    assert!(observed.flag_lifecycle_bytes <= stats.lifecycle_ids);
    assert!(observed.flag_projection_bytes <= stats.projection_ids);
    assert!(
        observed.place_storage_bytes
            <= stats.ordinary_place_storage_bytes + stats.call_argument_owned_bytes
    );
    assert!(observed.place_projection_bytes <= stats.place_projection_ids);
    assert!(observed.finalizer_storage_bytes <= stats.ordinary_finalizer_storage_bytes);
    assert!(observed.finalizer_projection_bytes <= stats.finalizer_projection_ids);
    assert!(observed.finalizer_lifecycle_bytes <= stats.finalizer_lifecycle_ids);

    for (observed, derived, family) in [
        (
            observed.inventory_slot_capacity_entries,
            proof.inventory_slot_capacity_entries,
            "inventory slots",
        ),
        (
            observed.inventory_flag_capacity_entries,
            proof.inventory_flag_capacity_entries,
            "inventory flags",
        ),
        (
            observed.inventory_entry_capacity_entries,
            proof.inventory_entry_capacity_entries,
            "inventory entry state",
        ),
        (
            observed.plan_slot_capacity_entries,
            proof.plan_slot_capacity_entries,
            "plan slots",
        ),
        (
            observed.plan_entry_capacity_entries,
            proof.plan_entry_capacity_entries,
            "plan entry state",
        ),
        (
            observed.shape_field_capacity_entries,
            proof.shape_field_capacity_entries,
            "shape fields",
        ),
        (
            observed.flag_projection_capacity_entries,
            proof.flag_projection_capacity_entries,
            "flag projections",
        ),
        (
            observed.place_projection_capacity_entries,
            proof.place_projection_capacity_entries,
            "plan-place projections",
        ),
        (
            observed.finalizer_projection_capacity_entries,
            proof.finalizer_projection_capacity_entries,
            "finalizer projections",
        ),
        (
            observed.finalizer_capacity_entries,
            proof.finalizer_capacity_entries,
            "finalizers",
        ),
        (
            observed.block_capacity_entries,
            proof.block_capacity_entries,
            "blocks",
        ),
        (
            observed.edge_capacity_entries,
            proof.edge_capacity_entries,
            "edges",
        ),
        (
            observed.region_capacity_entries,
            proof.region_capacity_entries,
            "regions",
        ),
        (
            observed.exit_capacity_entries,
            proof.exit_capacity_entries,
            "exits",
        ),
        (
            observed.status_capacity_entries,
            proof.status_capacity_entries,
            "status sources",
        ),
        (
            observed.transition_capacity_entries,
            proof.transition_capacity_entries,
            "transitions",
        ),
        (
            observed.branch_edge_capacity_entries,
            proof.branch_edge_capacity_entries,
            "branch edge vectors",
        ),
        (
            observed.region_slot_capacity_entries,
            proof.region_slot_capacity_entries,
            "region slots",
        ),
        (
            observed.exit_region_capacity_entries,
            proof.exit_region_capacity_entries,
            "exit region vectors",
        ),
        (
            observed.status_case_capacity_entries,
            proof.status_case_capacity_entries,
            "status case vectors",
        ),
    ] {
        assert!(
            observed <= derived,
            "observed {family} capacity {observed} exceeds derived {derived}"
        );
    }
}

#[test]
fn cleanup_generic_arity_two_checked_call_includes_exact_instance_identities() {
    let long = "x".repeat(128);
    let source = format!(
        r#"
module capacity.cleanup_generic_checked;
@id("checked.{long}")
fn checked<T, U>(left: T, right: U, value: i64) -> i64 {{ value + 1 }}
@id("app.main")
fn main() -> i64 {{
    checked<i64, bool>(1, true, 1)
}}
"#
    );
    let program = crate::parse(&source, Path::new("cleanup-generic-checked.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let template = program
        .functions
        .iter()
        .find(|function| function.name == "checked")
        .unwrap();
    let expected_instance_len = generic_function_instance_identity_upper(&program, template)
        .expect("valid concrete arity-two arguments have an identity upper");
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let instance = resolved
        .function_instances
        .iter()
        .find(|instance| instance.template.as_str() == template.stable_id)
        .expect("checked call materializes its generic instance");
    assert_eq!(instance.type_arguments.len(), 2);
    assert_eq!(expected_instance_len, instance.id.as_str().len());
    let checked_expression = &instance
        .function
        .cleanup_plan
        .status_sources
        .iter()
        .find(|status| {
            matches!(
                status.producer,
                semaprax::cleanup_plan::StatusProducer::CheckedArithmetic { .. }
            )
        })
        .expect("generic checked body has an arithmetic status source")
        .id
        .expression;
    let mut checked_clones = 0usize;
    let mut checked_clone_bytes = 0usize;
    let mut note = |expression: &crate::hir::ExpressionId| {
        if expression == checked_expression {
            checked_clones += 1;
            checked_clone_bytes += expression.as_str().len();
        }
    };
    for status in &instance.function.cleanup_plan.status_sources {
        note(&status.id.expression);
    }
    for block in &instance.function.cleanup_plan.blocks {
        for transition in &block.transitions {
            match transition {
                semaprax::cleanup_plan::CleanupTransition::Initialize { at, .. }
                | semaprax::cleanup_plan::CleanupTransition::InitializeVariant { at, .. }
                | semaprax::cleanup_plan::CleanupTransition::Transfer { at, .. }
                | semaprax::cleanup_plan::CleanupTransition::TransferVariant { at, .. }
                | semaprax::cleanup_plan::CleanupTransition::AuthenticateVariantCase {
                    at, ..
                } => note(at),
                semaprax::cleanup_plan::CleanupTransition::CallCommit { call, .. } => note(call),
                semaprax::cleanup_plan::CleanupTransition::SelectFailure { source } => {
                    note(&source.expression)
                }
                semaprax::cleanup_plan::CleanupTransition::StageCopyResult { source } => {
                    match source {
                        semaprax::cleanup_plan::StagedCopyResultSource::Body {
                            expression, ..
                        } => note(expression),
                        semaprax::cleanup_plan::StagedCopyResultSource::TryResidual {
                            expression,
                            operand,
                            ..
                        }
                        | semaprax::cleanup_plan::StagedCopyResultSource::TryOptionNone {
                            expression,
                            operand,
                            ..
                        } => {
                            note(expression);
                            note(operand);
                        }
                    }
                }
            }
        }
    }
    for edge in &instance.function.cleanup_plan.edges {
        match &edge.condition {
            semaprax::cleanup_plan::EdgeCondition::BooleanResult(expression, _) => note(expression),
            semaprax::cleanup_plan::EdgeCondition::VariantCase { scrutinee, .. } => note(scrutinee),
            semaprax::cleanup_plan::EdgeCondition::ArmSelected { scrutinee, .. } => note(scrutinee),
            semaprax::cleanup_plan::EdgeCondition::StatusZero(source)
            | semaprax::cleanup_plan::EdgeCondition::StatusNonzero(source) => {
                note(&source.expression)
            }
            semaprax::cleanup_plan::EdgeCondition::Always => {}
        }
    }
    for exit in &instance.function.cleanup_plan.exits {
        match &exit.continuation {
            semaprax::cleanup_plan::ExitContinuation::CommitResult {
                source: semaprax::cleanup_plan::CleanupResultSource::Scalar { expression },
            } => note(expression),
            semaprax::cleanup_plan::ExitContinuation::ReturnFailure { source } => {
                note(&source.expression)
            }
            _ => {}
        }
    }
    // StatusSource, SelectFailure, two status edges, ReturnFailure.
    assert_eq!(checked_clones, 5);
    assert_eq!(
        checked_clone_bytes,
        checked_clones * checked_expression.as_str().len()
    );
    let actual = resolved
        .functions
        .iter()
        .chain(
            resolved
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "generic arity-two cleanup {actual} exceeds authority {}",
        capacity.cleanup_authority_upper
    );
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}
