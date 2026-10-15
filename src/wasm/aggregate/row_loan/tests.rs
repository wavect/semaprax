//! Frame-layout witness for the Row Loan v1 Wasm carrier. A row loan's nominal
//! type is the lent record, but the caller frame must hold only the 16-byte
//! `{owner, index}` block and one i32 local per loan, and each lent read one
//! 8-byte import scratch. The witness replays the planner on isolated frames,
//! independently of the complete function plan, so an unused record-sized
//! reservation for the loan expression cannot hide behind the descriptor.
use super::*;

const SOURCE: &str = r#"module wasm.row_loan;
@id("app.row") record Row { @id("app.row.n") n:i64, @id("app.row.k") k:i64, @id("app.row.text") text:string, @id("app.row.payload") payload:Bytes, }
@id("app.weight") fn weight(row:borrow Row)->i64
 requires row.k>0
{ row.n+row.k }
@id("app.main") fn main()->i64 {
 let resident=Row{n:5,k:1,text:"r",payload:bytes_zeroed(1usize)};
 let direct=weight(resident);
 let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{n:7,k:2,text:"x",payload:bytes_zeroed(2usize)});
 let mut total=0;
 for borrow row in rows { total=total+weight(row); 0 }
 total+direct
}
"#;

fn expressions(function: &ResolvedFunction) -> Vec<&ResolvedExpr> {
    let mut pending = std::iter::once(&function.body)
        .chain(&function.requires)
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    let mut all = Vec::new();
    while let Some(expression) = pending.pop() {
        all.push(expression);
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    all
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

#[test]
fn row_loan_frames_hold_descriptor_and_read_scratch_without_record_storage() {
    let program = crate::hir::resolve(&crate::check(SOURCE, "wasm-row-loan.spx").unwrap()).unwrap();
    crate::hir::validate(&program).unwrap();
    let layouts = VariantLayoutCache::build(&program, VariantTarget::Wasm32).unwrap();
    let element = ResolvedType::Nominal {
        declaration: DeclarationId::new("app.row"),
        arguments: Vec::new(),
    };
    let (record, _) = aggregate_size_align(&program, &layouts, &element).unwrap();
    // A record-sized reservation is distinguishable from the descriptor block.
    assert!(record > 16, "{record}");

    let main = function(&program, "app.main");
    let loans = expressions(main)
        .into_iter()
        .filter(|expression| matches!(expression.kind, ResolvedExprKind::RowLoan { .. }))
        .collect::<Vec<_>>();
    let [loan] = loans.as_slice() else {
        panic!("exactly one authored row loan: {}", loans.len())
    };
    assert_eq!(loan.ty, element);
    let ResolvedExprKind::RowLoan { args, .. } = &loan.kind else {
        unreachable!()
    };

    // Complete plan: the loan owns exactly one carrier and no other storage.
    let plan = FunctionPlan::build(&program, main, &layouts).unwrap();
    assert!(!plan.aggregate_expressions.contains_key(&loan.id));
    assert!(!plan.scalar_expressions.contains_key(&loan.id));
    assert!(!plan.call_out.contains_key(&loan.id));
    assert!(!plan.owned_leaf_scratch.contains_key(&loan.id));
    assert_eq!(plan.row_loans.carriers.len(), 1);
    assert!(
        plan.row_loans.reads.is_empty(),
        "main has no lent parameter"
    );
    let (block, local) = plan.row_loans.carriers[&loan.id];
    assert_eq!(block % 8, 0);
    assert!(block + 16 <= plan.frame_size);
    let parameters = u32::try_from(main.params.len()).unwrap();
    let relative = usize::try_from(local - parameters - 1).unwrap();
    assert_eq!(plan.local_types[relative], I32);
    // No other reservation keyed by an expression starts inside the block.
    for offset in plan
        .aggregate_expressions
        .values()
        .chain(plan.call_out.values())
        .chain(plan.owned_leaf_scratch.values())
    {
        assert!(
            *offset < block || *offset >= block + 16,
            "{offset} in {block}"
        );
    }

    // Independent replay on isolated frames: the loan node contributes no
    // frame bytes and no locals beyond its two children.
    let weight = function(&program, "app.weight");
    let mut node = FunctionPlan::build(&program, weight, &layouts).unwrap();
    let mut children = FunctionPlan::build(&program, weight, &layouts).unwrap();
    let (node_locals, children_locals) = (node.local_types.len(), children.local_types.len());
    let mut node_frame = FrameAllocator::default();
    let mut children_frame = FrameAllocator::default();
    node.collect_expr(&program, &layouts, loan, 1, &mut node_frame)
        .unwrap();
    children
        .collect_exprs(&program, &layouts, args, 1, &mut children_frame)
        .unwrap();
    assert_eq!(node_frame.size, children_frame.size);
    assert_eq!(
        node.local_types.len() - node_locals,
        children.local_types.len() - children_locals
    );
    assert!(!node.aggregate_expressions.contains_key(&loan.id));

    // The carrier pass alone reserves exactly one aligned block and local.
    let mut carrier = FunctionPlan::build(&program, weight, &layouts).unwrap();
    let before = carrier.local_types.len();
    carrier.row_loans = Plan::default();
    let mut carrier_frame = FrameAllocator::default();
    carrier
        .collect_row_loans(&program, main, &mut carrier_frame)
        .unwrap();
    assert_eq!(carrier_frame.size, 16);
    assert_eq!(carrier.local_types[before..], [I32]);

    // The helper, compiled once for both the resident and the loan caller,
    // reserves one 8-byte scratch per lent read in its body and contract.
    let helper = FunctionPlan::build(&program, weight, &layouts).unwrap();
    assert!(helper.row_loans.carriers.is_empty());
    let parameter = &weight.params[0].id;
    let lent_reads = expressions(weight)
        .into_iter()
        .filter(|expression| {
            matches!(&expression.kind, ResolvedExprKind::Place(place) if place.root == *parameter)
        })
        .collect::<Vec<_>>();
    assert_eq!(lent_reads.len(), 3, "requires row.k, body row.n and row.k");
    assert_eq!(helper.row_loans.reads.len(), lent_reads.len());
    for read in &lent_reads {
        assert_eq!(read.ty, ResolvedType::I64);
        assert_eq!(helper.row_loans.reads[&read.id] % 8, 0);
        assert!(!helper.aggregate_expressions.contains_key(&read.id));
    }
    let mut scratch = FunctionPlan::build(&program, main, &layouts).unwrap();
    scratch.row_loans = Plan::default();
    let mut scratch_frame = FrameAllocator::default();
    scratch
        .collect_row_loans(&program, weight, &mut scratch_frame)
        .unwrap();
    assert_eq!(scratch_frame.size, 8 * 3);
    assert!(scratch.row_loans.carriers.is_empty());
}
