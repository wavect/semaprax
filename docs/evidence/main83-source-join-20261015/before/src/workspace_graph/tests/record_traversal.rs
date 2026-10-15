use super::*;

pub(super) fn canonical_source(path: &str, source: &str) -> WorkspaceSource {
    let program = crate::parse(source, Path::new(path)).expect("test source must parse");
    WorkspaceSource {
        path: path.to_owned(),
        source: format::canonical(&program),
    }
}

fn sources() -> Vec<WorkspaceSource> {
    vec![
        canonical_source(
            "library.spx",
            r#"module library;
@id("library.row") record Row { @id("library.row.left") left:i64, @id("library.row.right") right:i64, }
@id("library.zero") fn zero()->i64 { 0 }
"#,
        ),
        canonical_source(
            "app.spx",
            r#"module app;
use type @id("library.row") from library as Row;
@id("app.walk") fn walk(rows:borrow Vec<Row>)->i64 {
 for borrow row at index in rows { let selected=row.left; let numbered=index; 0 }
 0
}
@id("app.main") fn main()->i64 {0}
"#,
        ),
    ]
}
#[test]
fn imported_record_row_length_and_read_edges_replay_source_exactly() {
    let build = build_owned(sources()).unwrap();
    let sites = build
        .edges
        .iter()
        .filter(|edge| edge.caller == "app.walk" && edge.kind == "type_reference")
        .map(|edge| edge.ast_path.as_str())
        .collect::<Vec<_>>();
    assert!(sites.contains(&"body.s0.value.s0.value.type_argument.0"));
    assert!(sites.contains(&"body.s0.value.s2.body.s1.value.s0.value.base.type_argument.0"));
    let source_programs = sources()
        .iter()
        .map(|source| crate::parse(&source.source, Path::new(&source.path)).unwrap())
        .collect::<Vec<_>>();
    super::super::record_traversal::validate(&source_programs, &build.hir.modules).unwrap();
}
#[test]
fn imported_row_read_under_return_keeps_one_exact_type_site_and_edge() {
    let mut input = sources();
    input[1] = canonical_source(
        "app.spx",
        r#"module app;
use type @id("library.row") from library as Row;
@id("app.walk") fn walk(rows:borrow Vec<Row>)->i64 {
 for borrow row at index in rows { let selected=row.left; if index==1usize {return row.right;}; 0 }
 0
}
@id("app.main") fn main()->i64 {0}
"#,
    );
    let programs = parsed_sources(&input);
    // The retained build itself compares the exact HIR/edge multiset.
    let build = build_owned(input).unwrap();
    super::super::record_traversal::validate(&programs, &build.hir.modules).unwrap();
    let reads = build
        .edges
        .iter()
        .filter(|edge| {
            edge.caller == "app.walk"
                && edge.kind == "type_reference"
                && edge.ast_path.ends_with(".base.type_argument.0")
        })
        .collect::<Vec<_>>();
    assert_eq!(reads.len(), 2, "the let read and the Return operand read");
    let returned = reads
        .iter()
        .find(|edge| edge.ast_path.contains(".then."))
        .expect("the authored Return operand read is a traversal row read")
        .ast_path
        .clone();
    validate_retained_facts(&programs, &build.hir.modules, &build.edges).unwrap();
    // The HIR site is never filtered: a missing or duplicated edge disagrees.
    for duplicate in [false, true] {
        let mut edges = build.edges.clone();
        let index = edges
            .iter()
            .position(|edge| edge.kind == "type_reference" && edge.ast_path == returned)
            .unwrap();
        if duplicate {
            let copy = edges[index].clone();
            edges.push(copy);
        } else {
            edges.remove(index);
        }
        let error = validate_retained_facts(&programs, &build.hir.modules, &edges).unwrap_err();
        assert_eq!(error[0].code, "SPX-G173");
        assert_eq!(
            error[0].message,
            "workspace explicit type-reference facts disagree with retained HIR"
        );
    }
}
#[test]
fn imported_row_byte_view_edge_names_the_inner_field_read_call() {
    let input = vec![
        canonical_source(
            "library.spx",
            r#"module library;
@id("library.entry") record Entry { @id("library.entry.n") n:i64, @id("library.entry.text") text:string, }
@id("library.zero") fn zero()->i64 { 0 }
"#,
        ),
        canonical_source(
            "app.spx",
            r#"module app;
use type @id("library.entry") from library as Entry;
@id("app.walk") fn walk(rows:borrow Vec<Entry>)->i64 {
 for borrow row at index in rows { let utf8=str_as_bytes(row.text); let numbered=index; 0 }
 0
}
@id("app.main") fn main()->i64 {0}
"#,
        ),
    ];
    let programs = parsed_sources(&input);
    // The retained build itself compares the exact HIR/edge multiset.
    let build = build_owned(input).unwrap();
    super::super::record_traversal::validate(&programs, &build.hir.modules).unwrap();
    let owner = hir::DeclarationId::new("app.walk");
    let outer = "body.s0.value.s2.body.s1.value.s0.value";
    let inner = format!("{outer}.arg.0");
    let type_path = format!("{inner}.type_argument.0");
    let inner_identity = hir::workspace_expression_identity(&owner, &inner);
    let outer_identity = hir::workspace_expression_identity(&owner, outer);
    let position = |edges: &[WorkspaceEdge]| {
        let matching = edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                edge.caller == "app.walk"
                    && edge.kind == "type_reference"
                    && edge.target == "library.entry"
                    && edge.ast_path == type_path
            })
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1, "one byte-view row type edge");
        matching[0].0
    };
    let index = position(&build.edges);
    assert_eq!(build.edges[index].expression, inner_identity);
    validate_retained_facts(&programs, &build.hir.modules, &build.edges).unwrap();
    // Rekeying to the outer str_as_bytes call, or dropping or duplicating the
    // edge, disagrees with the independently reconstructed HIR site.
    for attack in 0..3 {
        let mut edges = build.edges.clone();
        match attack {
            0 => edges[index].expression = outer_identity.clone(),
            1 => {
                edges.remove(index);
            }
            _ => {
                let copy = edges[index].clone();
                edges.push(copy);
            }
        }
        let error = validate_retained_facts(&programs, &build.hir.modules, &edges).unwrap_err();
        assert_eq!(error[0].code, "SPX-G173");
        assert_eq!(
            error[0].message,
            "workspace explicit type-reference facts disagree with retained HIR"
        );
    }
}
#[test]
fn same_typed_field_substitution_cannot_rebind_authored_row_projection() {
    let source_programs = sources()
        .iter()
        .map(|source| crate::parse(&source.source, Path::new(&source.path)).unwrap())
        .collect::<Vec<_>>();
    let mut build = build_owned(sources()).unwrap();
    let function = build
        .hir
        .modules
        .iter_mut()
        .find(|module| module.module == "app")
        .unwrap()
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.walk")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("body")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("wrapper")
    };
    let hir::ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
        panic!("region")
    };
    let hir::ResolvedStatement::While { body, .. } = &mut statements[2] else {
        panic!("while")
    };
    let hir::ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("iteration")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[1] else {
        panic!("authored")
    };
    let hir::ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
        panic!("authored block")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("selected")
    };
    let hir::ResolvedExprKind::Project { field, .. } = &mut value.kind else {
        panic!("row field")
    };
    *field = hir::DeclarationId::new("library.row.right");
    assert_eq!(
        super::super::record_traversal::validate(&source_programs, &build.hir.modules).unwrap_err()
            [0]
        .code,
        "SPX-G173"
    );
}

#[test]
fn constructor_items_keep_the_ordinary_block_record_read_identity() {
    // A constructor inside a traversal body is a Vec allocation in a bounded
    // loop body and keeps its source refusal.
    let mut looped = sources();
    looped[1] = canonical_source(
        "app.spx",
        &looped[1].source.replace(
            "let selected = row.left;",
            "let selected = vec_of<i64>(row.left);",
        ),
    );
    assert!(looped[1].source.contains("vec_of<i64>(row.left)"));
    assert!(build_owned(looped)
        .err()
        .expect("loop-body constructor remains refused")
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T283"));
    // Admitted constructor items supply the traversed rows; reads through the
    // constructor-built binding keep the ordinary row read identity.
    let mut input = sources();
    input[1] = canonical_source(
        "app.spx",
        r#"module app;
use type @id("library.row") from library as Row;
@id("app.walk") fn walk(rows:borrow Vec<Row>)->i64 {
 for borrow row at index in rows { let selected=row.left; let numbered=index; 0 }
 0
}
@id("app.built") fn built()->i64 {
 let rows=vec_of<Row>(Row { left:19, right:23 }, Row { left:1, right:2 });
 for borrow row at index in rows { let selected=row.left; let numbered=index; 0 }
 0
}
@id("app.main") fn main()->i64 {0}
"#,
    );
    assert!(input[1].source.contains("vec_of<Row>("));
    let programs = input
        .iter()
        .map(|source| crate::parse(&source.source, Path::new(&source.path)).unwrap())
        .collect::<Vec<_>>();
    let build = build_owned(input).unwrap();
    super::super::record_traversal::validate(&programs, &build.hir.modules).unwrap();
}

#[test]
fn operation_projection_authenticates_constructor_provenance_and_children() {
    // A Workspace Semantic Graph holds 2..32 files (`SPX-G170`).
    let input = vec![
        sources().remove(0),
        canonical_source(
            "constructor.spx",
            r#"module constructor;
@id("constructor.helper") fn helper(value:i64)->i64 { value + 1 }
@id("constructor.main") fn main()->i64 {
 let rows=vec_of<i64>(helper(19),helper(23));
 if vec_len<i64>(rows)==2usize {42}else{0}
}
"#,
        ),
    ];
    let programs = input
        .iter()
        .map(|source| crate::parse(&source.source, Path::new(&source.path)).unwrap())
        .collect::<Vec<_>>();
    let (mut build, _) =
        build_owned_retaining_sources_for_operations(input.clone(), MAX_BUILDER_BYTES, 67_108_864)
            .unwrap();
    let authored = super::super::index_authored(&programs).unwrap();
    let sidecar = super::super::operation_sidecar::build_operation_sidecar(
        &programs,
        &input,
        &build.hir.modules,
        &authored,
    )
    .unwrap();
    assert!(sidecar
        .declarations
        .iter()
        .any(|declaration| declaration.id == "constructor.helper"
            && declaration.occurrences.len() >= 3));
    let function = build
        .hir
        .modules
        .iter_mut()
        .find(|module| module.module == "constructor")
        .unwrap()
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "constructor.main")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("body")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("constructor")
    };
    let hir::ResolvedExprKind::VecConstructor { count, .. } = &mut value.kind else {
        panic!("marker")
    };
    *count += 1;
    assert_eq!(
        super::super::operation_sidecar::build_operation_sidecar(
            &programs,
            &input,
            &build.hir.modules,
            &authored
        )
        .unwrap_err()[0]
            .code,
        "SPX-G173"
    );
}

fn row_loan_mut(expression: &mut hir::ResolvedExpr) -> Option<&mut hir::ResolvedExpr> {
    if matches!(expression.kind, hir::ResolvedExprKind::RowLoan { .. }) {
        return Some(expression);
    }
    match &mut expression.kind {
        hir::ResolvedExprKind::Block { statements, tail } => {
            for statement in statements.iter_mut() {
                let found = match statement {
                    hir::ResolvedStatement::Let { value, .. }
                    | hir::ResolvedStatement::CopyDiscard { value, .. }
                    | hir::ResolvedStatement::Assign { value, .. } => row_loan_mut(value),
                    hir::ResolvedStatement::Unsafe { body, .. } => row_loan_mut(body),
                    hir::ResolvedStatement::While {
                        condition, body, ..
                    }
                    | hir::ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => match row_loan_mut(condition) {
                        Some(found) => Some(found),
                        None => row_loan_mut(body),
                    },
                };
                if let Some(found) = found {
                    return Some(found);
                }
            }
            row_loan_mut(tail)
        }
        hir::ResolvedExprKind::Binary { left, right, .. } => match row_loan_mut(left) {
            Some(found) => Some(found),
            None => row_loan_mut(right),
        },
        hir::ResolvedExprKind::Call { args, .. } => args.iter_mut().find_map(row_loan_mut),
        _ => None,
    }
}

// Row Loan v1: an authored `score(row)` slot binds to exactly one retained
// descriptor whose callee and parameter come from current provider source.
#[test]
fn imported_row_loan_slots_replay_current_provider_source_and_refuse_forgery() {
    let input = vec![
        canonical_source(
            "library.spx",
            r#"module library;
@id("library.row") record Row { @id("library.row.left") left:i64, @id("library.row.label") label:string, }
@id("library.score") fn score(row:borrow Row)->i64 { row.left }
@id("library.zero") fn zero()->i64 { 0 }
"#,
        ),
        canonical_source(
            "app.spx",
            r#"module app;
use type @id("library.row") from library as Row;
use function @id("library.score") from library as score;
@id("app.walk") fn walk(rows:borrow Vec<Row>)->i64 {
 let mut total=0;
 for borrow row in rows { total=total+score(row); 0 }
 total
}
@id("app.main") fn main()->i64 {0}
"#,
        ),
    ];
    let programs = parsed_sources(&input);
    let build = build_owned(input.clone()).unwrap();
    super::super::record_traversal::validate(&programs, &build.hir.modules).unwrap();
    // Current authored provider ownership is a source refusal. Replaying old
    // consumer HIR must not misclassify this source edit as a forged descriptor.
    let mut owning_input = input.clone();
    owning_input[0].source = owning_input[0]
        .source
        .replace("row: borrow Row", "row: own Row");
    assert_ne!(owning_input[0].source, input[0].source);
    let owning_programs = parsed_sources(&owning_input);
    let errors =
        super::super::record_traversal::validate(&owning_programs, &build.hir.modules).unwrap_err();
    assert_eq!(errors[0].code, "SPX-T284");
    assert_eq!(
        errors[0].message,
        "record traversal row cannot escape or materialize aggregate storage; select a field"
    );
    assert_eq!(errors[0].path.as_deref(), Some("app.spx"));
    assert!(errors[0].span.is_some());
    let errors = build_owned(owning_input)
        .err()
        .expect("owning provider must refuse");
    assert!(errors.iter().any(|error| error.code == "SPX-T284"));
    for attack in 0..4 {
        let mut build = build_owned(input.clone()).unwrap();
        let modules = &mut build.hir.modules;
        let walk = modules
            .iter_mut()
            .find(|module| module.module == "app")
            .unwrap()
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.walk")
            .unwrap();
        let loan = row_loan_mut(&mut walk.body).expect("retained row loan");
        match attack {
            // Same signature shape, different stable callee identity.
            0 => {
                let hir::ResolvedExprKind::RowLoan { loan, .. } = &mut loan.kind else {
                    unreachable!()
                };
                loan.callee = hir::DeclarationId::new("library.zero");
            }
            // The descriptor's parameter must be the callee's exact one.
            1 => {
                let hir::ResolvedExprKind::RowLoan { loan, .. } = &mut loan.kind else {
                    unreachable!()
                };
                loan.parameter = loan.bound.clone();
            }
            // Its Vec and index children must be the authored loop's places.
            2 => {
                let hir::ResolvedExprKind::RowLoan { args, .. } = &mut loan.kind else {
                    unreachable!()
                };
                args.swap(0, 1);
            }
            // A retained slot without its descriptor is not an authored loan.
            _ => {
                let hir::ResolvedExprKind::RowLoan { args, .. } = &loan.kind else {
                    unreachable!()
                };
                let vector = args[0].clone();
                *loan = vector;
            }
        }
        let errors =
            super::super::record_traversal::validate(&programs, &build.hir.modules).unwrap_err();
        assert_eq!(errors[0].code, "SPX-G173", "attack {attack}");
    }
}

#[test]
fn explicit_and_row_field_read_forms_are_source_bound_in_both_directions() {
    let source = r#"module app.origin;
@id("row") record Row { @id("row.text") text:string,@id("row.number") number:i64 }
@id("reader") fn reader(rows:borrow Vec<Row>)->i64 {
 for borrow row at index in rows { let derived=row.text;let explicit=vec_field<Row>(rows,index,"text");str_len_bytes(derived)+str_len_bytes(explicit) }
 0
}
@id("app.main") fn main()->i64 {0}
"#;
    let input = || {
        vec![
            canonical_source("app.spx", source),
            canonical_source(
                "aux.spx",
                "module aux; @id(\"aux.marker\") fn marker()->i64 {0}",
            ),
        ]
    };
    let programs = parsed_sources(&input());
    let build = build_owned(input()).unwrap();
    super::super::record_traversal::validate(&programs, &build.hir.modules).unwrap();
    fn change(expression: &mut hir::ResolvedExpr, target: hir::VecFieldReadOrigin) -> bool {
        if let hir::ResolvedExprKind::VecFieldRead { origin, .. } = &mut expression.kind {
            if *origin == target {
                *origin = match target {
                    hir::VecFieldReadOrigin::Explicit => hir::VecFieldReadOrigin::Traversal,
                    hir::VecFieldReadOrigin::Traversal => hir::VecFieldReadOrigin::Explicit,
                };
                return true;
            }
        }
        if let hir::ResolvedExprKind::Block { statements, tail } = &mut expression.kind {
            for statement in statements {
                match statement {
                    hir::ResolvedStatement::While {
                        condition, body, ..
                    }
                    | hir::ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => {
                        if change(condition, target) {
                            return true;
                        }
                        if change(body, target) {
                            return true;
                        }
                    }
                    _ => {
                        if change(statement.value_mut(), target) {
                            return true;
                        }
                    }
                }
            }
            return change(tail, target);
        }
        if let hir::ResolvedExprKind::If {
            then_branch,
            else_branch,
            ..
        } = &mut expression.kind
        {
            return change(then_branch, target) || change(else_branch, target);
        }
        false
    }
    for target in [
        hir::VecFieldReadOrigin::Explicit,
        hir::VecFieldReadOrigin::Traversal,
    ] {
        let mut modules = build_owned(input()).unwrap().hir.modules;
        let reader = modules[0]
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "reader")
            .unwrap();
        assert!(change(&mut reader.body, target));
        let error = super::super::record_traversal::validate(&programs, &modules).unwrap_err();
        assert_eq!(error[0].code, "SPX-G173");
    }
}
