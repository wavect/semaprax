//! Resolved-expression traversal coverage and HIR fingerprint proofs.

use super::*;

#[test]
fn resolved_positional_walkers_cover_every_statement_child_and_match_guard() {
    let source = r#"
module traversal.total;
permit { unsafe }

@id("traversal.predicate")
fn predicate(value: i64) -> bool { value > 0 }

@id("traversal.id")
fn id(value: i64) -> i64 { value }

@id("traversal.carrier")
fn carrier(value: i64) -> i64 { id(value) }

@id("traversal.counter")
class Counter {
    @id("traversal.counter.value")
    value: i64,
    @id("traversal.counter.add")
    fn add(self: Counter, delta: i64) -> i64 { self.value + delta }
}

@id("traversal.method_user")
fn method_user(payload: i64) -> i64 { Counter { value: 0 }.add(payload) }

@id("traversal.base")
class Base {
    @id("traversal.base.value")
    value: i64,
    @id("traversal.base.add")
    fn add(self: Base, delta: i64) -> i64 { self.value + delta }
}

@id("traversal.child")
class Child : Base {
    @id("traversal.child.extra")
    extra: i64,
    @id("traversal.super_user")
    fn add(self: Child, payload: i64) -> i64 { super.add(payload) + self.extra }
}

@id("traversal.deep")
fn deep(tag: i64, payload: i64) -> i64 {
    let mut output = 0;
    output = payload;
    @audit("total traversal") unsafe { payload }
    while payload > 0 { payload }
    output
}

@id("traversal.guarded")
fn guarded(tag: i64, payload: i64) -> i64 {
    match tag {
        0 if predicate(payload) => payload,
        _ => 0,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = crate::parse(source, Path::new("resolved-total-traversal.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let deep = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "traversal.deep")
        .unwrap();

    let guarded = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "traversal.guarded")
        .unwrap();
    let carrier = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "traversal.carrier")
        .unwrap();
    let method_user = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "traversal.method_user")
        .unwrap();
    let super_user = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "traversal.super_user")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &deep.body.kind else {
        panic!("deep fixture body must resolve to a block");
    };
    assert_eq!(statements.len(), 4);
    let mut block_cursor = 0;
    let mut block_paths = Vec::new();
    while let Some((path, _)) = resolved_expression_child(&deep.body, &mut block_cursor) {
        block_paths.push(path);
    }
    assert_eq!(block_paths, [0, 2, 4, 6, 7, 8]);

    let ResolvedExprKind::Block { tail, .. } = &guarded.body.kind else {
        panic!("guarded fixture body must resolve to a block");
    };
    let mut match_cursor = 0;
    let mut match_paths = Vec::new();
    while let Some((path, _)) = resolved_expression_child(tail, &mut match_cursor) {
        match_paths.push(path);
    }
    assert_eq!(match_paths, [0, 1, 2, 4]);

    let deep_census = expression_call_site_census(&deep.body).unwrap();
    let guarded_census = expression_call_site_census(&guarded.body).unwrap();
    assert_eq!(deep_census.function_sites, 0);
    assert_eq!(guarded_census.function_sites, 1);
    assert_eq!(deep_census.import_sites, 0);
    assert_eq!(guarded_census.import_sites, 0);
    let mut functions = BTreeSet::new();
    let mut imports = BTreeSet::new();
    visit_calls(&deep.body, &mut functions, &mut imports, 0, 0).unwrap();
    visit_calls(&guarded.body, &mut functions, &mut imports, 0, 0).unwrap();
    assert_eq!(
        functions.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        ["traversal.predicate"]
    );
    assert!(imports.is_empty());
    fingerprint_expression_types_scratch(&deep.body, 1).unwrap();
    fingerprint_expression_types_scratch(&guarded.body, 1).unwrap();
    fingerprint_expression_types_scratch(&method_user.body, 1).unwrap();
    fingerprint_expression_types_scratch(&super_user.body, 1).unwrap();

    for (function, expected) in [
        (method_user, "traversal.counter.add"),
        (super_user, "traversal.base.add"),
    ] {
        let census = expression_call_site_census(&function.body).unwrap();
        assert_eq!(census.function_sites, 1, "{}", function.id);
        let mut callees = BTreeSet::new();
        visit_calls(&function.body, &mut callees, &mut imports, 0, 0).unwrap();
        assert_eq!(
            callees.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
            [expected]
        );
    }

    let fingerprint = |expression: &ResolvedExpr| {
        let mut hasher = Sha256::new();
        hash_expr(&mut hasher, expression, 0).unwrap();
        hasher.finalize().to_vec()
    };
    let baseline = fingerprint(&guarded.body);
    let mut changed = guarded.body.clone();
    let ResolvedExprKind::Block { tail, .. } = &mut changed.kind else {
        unreachable!();
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        unreachable!();
    };
    let guard = arms[0].guard.as_mut().unwrap();
    guard.kind = ResolvedExprKind::Bool(false);
    assert_ne!(fingerprint(&changed), baseline);

    let ResolvedExprKind::Block {
        tail: carrier_tail, ..
    } = &carrier.body.kind
    else {
        panic!("carrier fixture body must resolve to a block");
    };
    let scalar_call = carrier_tail.as_ref().clone();
    let ResolvedExprKind::Block {
        tail: guarded_tail, ..
    } = &guarded.body.kind
    else {
        unreachable!();
    };
    let ResolvedExprKind::Match { arms, .. } = &guarded_tail.kind else {
        unreachable!();
    };
    let boolean_call = arms[0].guard.as_deref().unwrap().clone();
    for (label, child_index, replacement) in [
        ("assign", 2usize, scalar_call.clone()),
        ("unsafe", 4usize, scalar_call.clone()),
        ("while-condition", 6usize, boolean_call),
        ("while-body", 7usize, scalar_call),
    ] {
        let mut changed = deep.body.clone();
        let mut cursor = 0;
        let mut selected = None;
        while let Some((path, child)) = resolved_expression_child(&changed, &mut cursor) {
            if path == child_index {
                selected = Some(child.id.clone());
                break;
            }
        }
        let selected = selected.expect("statement child path must exist");
        let ResolvedExprKind::Block { statements, .. } = &mut changed.kind else {
            unreachable!();
        };
        let target = match label {
            "assign" => match &mut statements[1] {
                ResolvedStatement::Assign { value, .. } => value,
                _ => unreachable!(),
            },
            "unsafe" => match &mut statements[2] {
                ResolvedStatement::Unsafe { body, .. } => body,
                _ => unreachable!(),
            },
            "while-condition" => match &mut statements[3] {
                ResolvedStatement::While { condition, .. } => condition,
                _ => unreachable!(),
            },
            "while-body" => match &mut statements[3] {
                ResolvedStatement::While { body, .. } => body,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        assert_eq!(target.id, selected);
        *target = replacement;
        assert_eq!(
            expression_call_site_census(&changed)
                .unwrap()
                .function_sites,
            1,
            "{label}"
        );
        assert_ne!(fingerprint(&changed), fingerprint(&deep.body), "{label}");
        fingerprint_expression_types_scratch(&changed, 1).unwrap();
    }

    let spec = Spec {
        module: program.module.clone(),
        source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical.as_bytes())),
        target: current_target().unwrap(),
        exports: vec!["traversal.deep".to_owned()],
        imports: Vec::new(),
        capabilities: Vec::new(),
    };
    let diagnostics = match prepare_native_rust_interop(&program, render_spec(&spec).as_bytes()) {
        Ok(_) => panic!("unsupported scalar-profile statements must fail closed"),
        Err(diagnostics) => diagnostics,
    };
    assert_eq!(diagnostics[0].code, "SPX-B107");
    assert_eq!(
        diagnostics[0].message,
        "Native Rust Interop declaration set is unsupported: scalar value signature required"
    );
}

#[test]
fn type_facts_hostile_envelopes_are_bound_to_canonical_fixtures() {
    fn layered(resource: bool, levels: usize) -> String {
        let mut source = String::from("module capacity.typefacts.layers;\n\n");
        if resource {
            source.push_str(
                    "@id(\"layer.r0\")\nresource R0 {\n    @id(\"layer.r0.drop\")\n    drop trivial;\n}\n\n",
                );
        } else {
            source.push_str(
                    "@id(\"layer.r0\")\nrecord R0 {\n    @id(\"layer.r0.value\")\n    value: i64,\n}\n\n",
                );
        }
        for level in 1..=levels {
            writeln!(
                    source,
                    "@id(\"layer.r{level}\")\nrecord R{level} {{\n    @id(\"layer.r{level}.a\")\n    a: R{},\n    @id(\"layer.r{level}.b\")\n    b: R{},\n}}\n",
                    level - 1,
                    level - 1
                )
                .unwrap();
        }
        source.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
        source
    }

    fn envelope(source: &str, name: &str) -> (String, usize, usize, usize) {
        let program = crate::parse(source, Path::new(name)).unwrap();
        let canonical = crate::format::canonical(&program);
        let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut stack).unwrap();
        let type_facts_phase = capacity.phase_peaks()[7];
        (
            raw_digest(canonical.as_bytes()),
            capacity.retained_upper,
            type_facts_phase,
            capacity
                .retained_upper
                .checked_add(type_facts_phase)
                .unwrap(),
        )
    }

    let scalar = layered(false, 12);
    let resource = layered(true, 12);
    let mut wide = String::from("module capacity.typefacts.wide;\n\n");
    for index in 0..514 {
        writeln!(
                wide,
                "@id(\"wide.r{index}\")\nrecord R{index} {{\n    @id(\"wide.r{index}.value\")\n    value: i64,\n}}\n"
            )
            .unwrap();
    }
    wide.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
    let mut chain = String::from(
        "module capacity.typefacts.chain;\n\n@id(\"chain.r0\")\nrecord R0 {\n    @id(\"chain.r0.value\")\n    value: i64,\n}\n\n",
    );
    for index in 1..514 {
        writeln!(
                chain,
                "@id(\"chain.r{index}\")\nrecord R{index} {{\n    @id(\"chain.r{index}.next\")\n    next: R{},\n}}\n",
                index - 1
            )
            .unwrap();
    }
    chain.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");

    assert_eq!(
        [
            envelope(&scalar, "typefacts-layered-scalar.spx"),
            envelope(&resource, "typefacts-layered-resource.spx"),
            envelope(&wide, "typefacts-wide.spx"),
            envelope(&chain, "typefacts-chain.spx"),
        ],
        [
            (
                "sha256:cfa16985be87d169c3fb81d5958126347ec82b4c1afed878e2d98d1fbfe72c80"
                    .to_owned(),
                220_111_630,
                438_720_350,
                658_831_980,
            ),
            (
                "sha256:461611e4315e312330af0285273568e5d09cd8e5770a35dcf66a82783aa15ae6"
                    .to_owned(),
                147_076_236,
                293_107_472,
                440_183_708,
            ),
            (
                "sha256:dc19474b86def3eaf6e3c60cc2224694e6aa7cf2811cca6115943c11102f95fc"
                    .to_owned(),
                42_049_179,
                80_965_504,
                123_014_683,
            ),
            (
                "sha256:d2692d4883957575ee95df8f9ee7057343599e1da945c386cedea714c716f66d"
                    .to_owned(),
                10_529_689_379,
                21_056_178_704,
                31_585_868_083,
            ),
        ],
        "canonical fixture or independently computed envelope terms drifted"
    );
}

#[test]
fn hir_fingerprint_admits_exact_depth_result_and_option_try_chains() {
    let (program, _) = fixture();
    let resolved = hir::resolve(&program).unwrap();
    let seed_id = resolved.functions[0].body.id.clone();
    for option in [false, true] {
        let leaf = ResolvedExpr {
            id: seed_id.clone(),
            ty: ResolvedType::I64,
            ownership: OwnershipMode::Value,
            span: crate::ast::Span::default(),
            kind: ResolvedExprKind::Int(1),
        };
        let wrap = |operand: ResolvedExpr, _index: usize| ResolvedExpr {
            // Fingerprinting does not validate expression identity uniqueness. Reusing a
            // resolver-issued ID keeps this forged, parser-independent depth fixture within
            // the public HIR construction surface.
            id: seed_id.clone(),
            ty: ResolvedType::I64,
            ownership: OwnershipMode::Value,
            span: crate::ast::Span::default(),
            kind: if option {
                ResolvedExprKind::TryOption {
                    operand: Box::new(operand),
                    option: DeclarationId::new("prelude.option".to_owned()),
                    some_case: DeclarationId::new("prelude.option.some".to_owned()),
                    some_field: DeclarationId::new("prelude.option.some.value".to_owned()),
                    none_case: DeclarationId::new("prelude.option.none".to_owned()),
                    residual_type: ResolvedType::I64,
                }
            } else {
                ResolvedExprKind::Try {
                    operand: Box::new(operand),
                    result: DeclarationId::new("prelude.result".to_owned()),
                    ok_case: DeclarationId::new("prelude.result.ok".to_owned()),
                    ok_field: DeclarationId::new("prelude.result.ok.value".to_owned()),
                    err_case: DeclarationId::new("prelude.result.err".to_owned()),
                    err_field: DeclarationId::new("prelude.result.err.error".to_owned()),
                    residual_type: ResolvedType::I64,
                }
            },
        };
        let mut exact = leaf;
        for index in 1..MAX_SEMANTIC_EXPRESSION_DEPTH {
            exact = wrap(exact, index);
        }
        let mut hasher = Sha256::new();
        hash_expr(&mut hasher, &exact, 0).unwrap();
        assert_eq!(
            format!(
                "sha256:{:x}",
                semaprax::digest_hex::LowerHex(hasher.finalize())
            )
            .len(),
            71
        );

        let over = wrap(exact, MAX_SEMANTIC_EXPRESSION_DEPTH);
        let mut hasher = Sha256::new();
        assert_eq!(
            hash_expr(&mut hasher, &over, 0).unwrap_err().code,
            "SPX-B109"
        );

        // Iteratively dismantle this deliberately forged test tree; the
        // production builder receives validated HIR through `resolve`.
        let mut current = over;
        loop {
            current = match current.kind {
                ResolvedExprKind::Try { operand, .. }
                | ResolvedExprKind::TryOption { operand, .. } => *operand,
                _ => break,
            };
        }
    }
}

#[test]
fn fingerprint_type_identity_exact_writer_matches_hir_and_named_topology() {
    for depth in [0usize, 1, 32, MAX_SEMANTIC_EXPRESSION_DEPTH - 1] {
        let mut ty = ResolvedType::TypeParameter {
            owner: DeclarationId::new("type.owner".to_owned()),
            index: u32::MAX,
        };
        for index in 0..depth {
            ty = ResolvedType::Nominal {
                declaration: DeclarationId::new(format!("type.layer.{index}")),
                arguments: vec![ty, ResolvedType::Bool],
            };
        }
        let expected = ty.identity_key();
        let upper = type_identity_scratch_upper(&ty).unwrap();
        POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
        let actual = fingerprint_type_identity(&ty, 0, 0).unwrap();
        let observed = POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(std::cell::Cell::get);
        assert_eq!(actual, expected);
        assert!(
            observed <= upper,
            "depth {depth} identity scratch actual/formula: {observed}/{upper}"
        );
    }
    let over_work = ResolvedType::Nominal {
        declaration: DeclarationId::new("type.too-wide".to_owned()),
        arguments: vec![ResolvedType::Bool; FINGERPRINT_ACTION_SLOTS],
    };
    assert_eq!(
        type_identity_metrics(&over_work, 1).unwrap_err().code,
        "SPX-B109"
    );
}
