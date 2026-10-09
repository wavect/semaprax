//! Allocation-free bounds over decoded HIR, before any proof reconstruction.
use std::mem::size_of;

use crate::hir::{
    OwnershipMode, Place, PlaceProjection, ResolvedBinding, ResolvedExpr, ResolvedExprKind,
    ResolvedMatchMode, ResolvedMatchPattern, ResolvedParam, ResolvedProgram,
    ResolvedRecordMatchFieldPattern, ResolvedType, ValueId,
};
use crate::loan_plan::{
    Loan, LoanEndpoint, LoanId, LoanPlan, LoanProgramPoint, MAX_LOANS_PER_FUNCTION_V1,
    MAX_LOAN_EDGES_V1, MAX_LOAN_ENDPOINTS_V1,
};

use super::{capacity, functions, Result, MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND};

// Rust's B-tree nodes hold eleven entries and twelve child pointers; non-root
// nodes hold at least five entries. Count every node as internal, add two full
// roots for the temporary split, and reserve sixteen pointers for child links, parent/header/alignment.
// Vec growth is bounded separately, with the four-slot minimum.
const TREE_ENTRIES: usize = 11;
const TREE_MIN_ENTRIES: usize = 5;
const TREE_HEADER_POINTERS: usize = 16;
// Retain old and new buffers during reallocations, not only final capacity.
const VECTOR_GROWTH: usize = 4;

#[derive(Default)]
struct Inventory {
    expressions: usize,
    edges: usize,
    diagnostic_bytes: usize,
    bindings: usize,
    drafts: usize,
    projections: usize,
    projection_text: usize,
    identity_bytes: usize,
    root_bytes: usize,
}

fn add(total: &mut usize, value: usize) -> Result<()> {
    *total = total
        .checked_add(value)
        .filter(|value| *value <= MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND)
        .ok_or_else(|| capacity("decoded HIR loan construction bound exceeds its maximum"))?;
    Ok(())
}

fn product(values: &[usize]) -> Result<usize> {
    values.iter().try_fold(1usize, |total, value| {
        total
            .checked_mul(*value)
            .filter(|value| *value <= MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND)
            .ok_or_else(|| capacity("decoded HIR loan construction bound exceeds its maximum"))
    })
}

fn sum(values: &[usize]) -> Result<usize> {
    let mut total = 0usize;
    for value in values {
        add(&mut total, *value)?;
    }
    Ok(total)
}

fn vector(count: usize, element: usize) -> Result<usize> {
    if count == 0 {
        return Ok(0);
    }
    product(&[VECTOR_GROWTH, count.max(4), element])
}

fn tree(count: usize, entry: usize) -> Result<usize> {
    if count == 0 {
        return Ok(0);
    }
    let node = product(&[TREE_ENTRIES, entry])?
        .checked_add(product(&[TREE_HEADER_POINTERS, size_of::<usize>()])?)
        .ok_or_else(|| capacity("decoded HIR loan tree bound overflows"))?;
    product(&[sum(&[count / TREE_MIN_ENTRIES, 2])?, node])
}

impl Inventory {
    fn binding(&mut self, binding: &ResolvedBinding) -> Result<()> {
        add(&mut self.bindings, 1)?;
        self.root_bytes = self.root_bytes.max(binding.id.as_str().len());
        Ok(())
    }

    fn parameter(&mut self, parameter: &ResolvedParam) -> Result<()> {
        // Ownership maps clone the parameter root; its name/type/ownership
        // are read directly and retain no additional payload allocation.
        add(&mut self.bindings, 1)?;
        self.root_bytes = self.root_bytes.max(parameter.id.as_str().len());
        Ok(())
    }

    fn place(&mut self, place: &Place) -> Result<()> {
        self.root_bytes = self.root_bytes.max(place.root.as_str().len());
        for projection in &place.projections {
            add(&mut self.projections, 1)?;
            match projection {
                PlaceProjection::Field(field) => {
                    add(&mut self.projection_text, field.as_str().len())?
                }
                PlaceProjection::VariantField { case, field } => {
                    add(&mut self.projection_text, case.as_str().len())?;
                    add(&mut self.projection_text, field.as_str().len())?;
                }
            }
        }
        Ok(())
    }

    fn pattern(&mut self, pattern: &ResolvedMatchPattern, depth: usize) -> Result<()> {
        check_depth(depth)?;
        match pattern {
            ResolvedMatchPattern::Variant { case, fields, .. } => {
                for field in fields {
                    self.binding(&field.binding)?;
                    // Native receiver-tied aliases can append this projection.
                    add(&mut self.projections, 1)?;
                    add(&mut self.projection_text, case.as_str().len())?;
                    add(&mut self.projection_text, field.field.as_str().len())?;
                }
            }
            ResolvedMatchPattern::Record { fields, .. } => {
                for field in fields {
                    self.record_pattern(&field.pattern, depth + 1)?;
                }
            }
            ResolvedMatchPattern::Binding(binding) => self.binding(binding)?,
            ResolvedMatchPattern::Or(alternatives) => {
                for pattern in alternatives {
                    self.pattern(pattern, depth + 1)?;
                }
            }
            ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => {}
        }
        Ok(())
    }

    fn record_pattern(
        &mut self,
        pattern: &ResolvedRecordMatchFieldPattern,
        depth: usize,
    ) -> Result<()> {
        check_depth(depth)?;
        match pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding) => self.binding(binding)?,
            ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
                for field in fields {
                    self.record_pattern(&field.pattern, depth + 1)?;
                }
            }
            ResolvedRecordMatchFieldPattern::Wildcard => {}
        }
        Ok(())
    }
}

fn check_depth(depth: usize) -> Result<()> {
    if depth >= crate::cache_codec::MAX_DEPTH {
        return Err(capacity(
            "decoded HIR loan traversal exceeds its depth bound",
        ));
    }
    Ok(())
}

fn walk(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    inventory: &mut Inventory,
    depth: usize,
) -> Result<()> {
    check_depth(depth)?;
    add(&mut inventory.expressions, 1)?;
    if inventory.expressions > MAX_LOAN_ENDPOINTS_V1 / 2 {
        return Err(capacity(
            "decoded HIR loan traversal exceeds its point bound",
        ));
    }
    add(&mut inventory.edges, edge_entries(expression)?)?;
    if inventory.edges > MAX_LOAN_EDGES_V1 {
        return Err(capacity(
            "decoded HIR loan traversal exceeds its edge bound",
        ));
    }
    add(
        &mut inventory.identity_bytes,
        expression
            .id
            .shared_allocation_bytes()
            .ok_or_else(|| capacity("decoded HIR loan identity backing cannot be accounted"))?,
    )?;
    match &expression.kind {
        ResolvedExprKind::Place(place) => {
            inventory.place(place)?;
            if matches!(expression.ty, ResolvedType::SliceU8 | ResolvedType::Str) {
                add(&mut inventory.drafts, 1)?;
            }
        }
        ResolvedExprKind::BorrowPlace { place, .. } => {
            inventory.place(place)?;
            add(&mut inventory.drafts, 1)?;
        }
        ResolvedExprKind::Closure {
            parameters,
            captures,
            ..
        } => {
            for binding in parameters {
                inventory.binding(binding)?;
            }
            // This is exactly the CFG owner: a closure body is a different
            // execution tree; only capture values execute in this function.
            for capture in captures {
                inventory.binding(&capture.binding)?;
                walk(program, &capture.value, inventory, depth + 1)?;
            }
        }
        ResolvedExprKind::Invoke { callable, args } => {
            walk(program, callable, inventory, depth + 1)?;
            for argument in args {
                walk(program, argument, inventory, depth + 1)?;
            }
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            add(&mut inventory.drafts, 1)?;
            for child in [source.as_ref(), start.as_ref(), end.as_ref()] {
                walk(program, child, inventory, depth + 1)?;
            }
        }
        ResolvedExprKind::Call {
            callee,
            instance,
            args,
            ..
        } => {
            let target = program.resolve_call_target(callee, instance.as_ref());
            for (index, argument) in args.iter().enumerate() {
                let borrowed = target.map_or_else(
                    || argument.ownership == OwnershipMode::Borrow,
                    |target| {
                        target
                            .params
                            .get(index)
                            .is_some_and(|parameter| parameter.ownership == OwnershipMode::Borrow)
                    },
                );
                if borrowed {
                    add(&mut inventory.drafts, 1)?;
                }
                walk(program, argument, inventory, depth + 1)?;
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            if matches!(
                call.result,
                crate::hir::ResolvedImportResultKind::BorrowedStr { .. }
            ) {
                add(&mut inventory.drafts, 1)?;
            }
            let target = program
                .interfaces
                .iter()
                .flat_map(|interface| &interface.imports)
                .find(|import| import.id == call.import);
            for (index, argument) in call.args.iter().enumerate() {
                if target.is_some_and(|import| {
                    import
                        .parameters
                        .get(index)
                        .is_some_and(|parameter| parameter.ownership == OwnershipMode::Borrow)
                }) {
                    add(&mut inventory.drafts, 1)?;
                }
                walk(program, argument, inventory, depth + 1)?;
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                walk(program, argument, inventory, depth + 1)?;
            }
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => {
            walk(program, value, inventory, depth + 1)?;
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            walk(program, left, inventory, depth + 1)?;
            walk(program, right, inventory, depth + 1)?;
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    crate::hir::ResolvedStatement::Let { binding, .. } => {
                        inventory.binding(binding)?;
                    }
                    crate::hir::ResolvedStatement::Assign { binding, field, .. } => {
                        inventory.binding(binding)?;
                        if let Some(field) = field {
                            add(&mut inventory.projections, 1)?;
                            add(&mut inventory.projection_text, field.as_str().len())?;
                        }
                    }
                    crate::hir::ResolvedStatement::Unsafe { .. }
                    | crate::hir::ResolvedStatement::While { .. } => {}
                }
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        walk(program, child, inventory, depth + 1)?;
                    }
                }
            }
            walk(program, tail, inventory, depth + 1)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            for child in [
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ] {
                walk(program, child, inventory, depth + 1)?;
            }
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                walk(program, &field.value, inventory, depth + 1)?;
            }
        }
        ResolvedExprKind::Match {
            mode,
            scrutinee,
            arms,
        } => {
            if *mode == ResolvedMatchMode::Borrow {
                add(&mut inventory.drafts, arms.len())?;
            }
            walk(program, scrutinee, inventory, depth + 1)?;
            for arm in arms {
                inventory.pattern(&arm.pattern, depth + 1)?;
                if let Some(guard) = &arm.guard {
                    walk(program, guard, inventory, depth + 1)?;
                }
                walk(program, &arm.value, inventory, depth + 1)?;
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            walk(program, base, inventory, depth + 1)?;
            for field in fields {
                walk(program, &field.value, inventory, depth + 1)?;
            }
        }
        ResolvedExprKind::FunctionReference { .. }
        | ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_) => {}
    }
    Ok(())
}

// Count edge-set insertions before materialization, including inserts that
// subsequently deduplicate. Every branch mirrors the CFG builder's expansion;
// oversized decoded trees are refused before the planner's pre-cap collections.
fn edge_entries(expression: &ResolvedExpr) -> Result<usize> {
    use crate::ast::BinaryOp;
    let children = match &expression.kind {
        ResolvedExprKind::Block { statements, .. } => {
            let mut edges = 2usize;
            for statement in statements {
                add(
                    &mut edges,
                    if matches!(statement, crate::hir::ResolvedStatement::While { .. }) {
                        3
                    } else {
                        1
                    },
                )?;
            }
            return Ok(edges);
        }
        ResolvedExprKind::If { .. } => return Ok(5),
        ResolvedExprKind::Binary {
            op: BinaryOp::And | BinaryOp::Or,
            ..
        } => return Ok(4),
        ResolvedExprKind::Match { arms, .. } => {
            let mut edges = 1usize;
            for (index, arm) in arms.iter().enumerate() {
                add(&mut edges, 2)?;
                if arm.guard.is_some() {
                    add(&mut edges, 1)?;
                    if index + 1 < arms.len() {
                        add(&mut edges, 1)?;
                    }
                }
            }
            return Ok(edges);
        }
        ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => return Ok(3),
        ResolvedExprKind::Closure { captures, .. } => captures.len(),
        ResolvedExprKind::Invoke { args, .. } => sum(&[args.len(), 1])?,
        ResolvedExprKind::Call { args, .. } => args.len(),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.len(),
        ResolvedExprKind::HostCommandCall(call) => call.args.len(),
        ResolvedExprKind::ByteRange { .. } => 3,
        ResolvedExprKind::Unary { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Upcast { .. }
        | ResolvedExprKind::Yield { .. } => 1,
        ResolvedExprKind::Binary { .. } => 2,
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields.len(),
        ResolvedExprKind::UpdateRecord { fields, .. } => sum(&[fields.len(), 1])?,
        ResolvedExprKind::FunctionReference { .. }
        | ResolvedExprKind::Int(_)
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
        | ResolvedExprKind::BorrowPlace { .. } => 0,
    };
    children
        .checked_add(1)
        .ok_or_else(|| capacity("decoded HIR loan edge bound overflows"))
}

pub(super) fn construction_bytes(program: &ResolvedProgram) -> Result<usize> {
    let mut total = 0usize;
    for function in functions(program).filter(|function| !function.loan_plan.loans.is_empty()) {
        let mut inventory = Inventory::default();
        add(&mut inventory.diagnostic_bytes, function.id.as_str().len())?;
        add(&mut inventory.diagnostic_bytes, function.name.len())?;
        for parameter in &function.params {
            inventory.parameter(parameter)?;
        }
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            walk(program, expression, &mut inventory, 0)?;
        }
        add(&mut total, function_bytes(&inventory)?)?;
    }
    Ok(total)
}

fn function_bytes(inventory: &Inventory) -> Result<usize> {
    let expressions = inventory.expressions;
    let points = product(&[2, expressions])?;
    let edges = inventory.edges;
    let drafts = inventory.drafts;
    let loans = drafts.min(MAX_LOANS_PER_FUNCTION_V1);
    let bindings = inventory.bindings;
    // Alias resolution follows at most one link per distinct binding; a link
    // may carry any source/native-pattern projection prefix. Growth and two
    // simultaneously live old/new prefixes are retained, never refunded.
    let projection_payload = inventory
        .projection_text
        .checked_add(vector(inventory.projections, size_of::<PlaceProjection>())?)
        .ok_or_else(|| capacity("decoded HIR loan place bound overflows"))?;
    let place = product(&[2, sum(&[bindings, 1])?, projection_payload])?
        .checked_add(inventory.root_bytes)
        .ok_or_else(|| capacity("decoded HIR loan place bound overflows"))?;
    let mut bytes = size_of::<LoanPlan>();
    add(&mut bytes, inventory.identity_bytes)?;
    add(
        &mut bytes,
        product(&[4, sum(&[product(&[8, inventory.diagnostic_bytes])?, 1024])?])?,
    )?;
    // CFG vectors, both adjacency directions, maps/sets, traversal queues and
    // classification temporaries. Eight map-shaped inventories cover seen,
    // root, point, alias, bound, ownership, binding-loan and match-parent keys.
    add(
        &mut bytes,
        product(&[
            8,
            tree(
                sum(&[points, bindings, drafts])?,
                size_of::<(LoanProgramPoint, Place, ValueId)>(),
            )?,
        ])?,
    )?;
    add(&mut bytes, vector(points, size_of::<LoanProgramPoint>())?)?;
    add(
        &mut bytes,
        vector(expressions, size_of::<(&ResolvedExpr, &ResolvedExpr)>())?,
    )?;
    add(&mut bytes, vector(edges, size_of::<(u16, u16)>())?)?;
    add(
        &mut bytes,
        product(&[4, vector(points, size_of::<Vec<u16>>())?])?,
    )?;
    add(&mut bytes, product(&[5, vector(edges, size_of::<u16>())?])?)?;
    add(&mut bytes, vector(points, size_of::<u16>())?)?;
    add(
        &mut bytes,
        product(&[
            expressions,
            crate::loan_plan::REACHABILITY_BYTES_PER_EXPRESSION,
        ])?,
    )?;
    add(&mut bytes, crate::loan_plan::REACHABILITY_FIXED_BYTES)?;
    // Unfiltered drafts/aliases coexist with returned loan origins. Planner
    // drafts contain a place, optional roots/binding, point, cause, seed set
    // and span; two Loan-sized carriers cover that fixed representation.
    add(
        &mut bytes,
        product(&[2, vector(drafts, size_of::<Loan>())?])?,
    )?;
    add(
        &mut bytes,
        product(&[4, sum(&[bindings, drafts, loans, 1])?, place])?,
    )?;
    // Seeds, live sets and a replaced parent seed/live set can all coexist.
    add(
        &mut bytes,
        product(&[3, loans, tree(points, size_of::<u16>())?])?,
    )?;
    // Termination/liveness rows and returned endpoint/edge proof rows. Charge
    // growth on each nested vector, not only its outer carrier.
    add(&mut bytes, vector(loans, size_of::<Loan>())?)?;
    add(&mut bytes, vector(points, size_of::<LoanEndpoint>())?)?;
    add(
        &mut bytes,
        vector(edges, size_of::<crate::loan_plan::LoanEdge>())?,
    )?;
    add(
        &mut bytes,
        product(&[8, points, vector(loans, size_of::<LoanId>())?])?,
    )?;
    add(
        &mut bytes,
        product(&[3, edges, vector(loans, size_of::<LoanId>())?])?,
    )?;
    add(
        &mut bytes,
        product(&[2, loans, vector(edges, size_of::<u16>())?])?,
    )?;
    add(
        &mut bytes,
        product(&[loans, vector(edges, size_of::<LoanProgramPoint>())?])?,
    )?;
    add(
        &mut bytes,
        product(&[sum(&[loans, edges, points])?, size_of::<Vec<u16>>()])?,
    )?;
    // Physical-capacity verification inventories every point reference plus
    // each loan site/start/end; it charges this allocation after the prebound.
    add(
        &mut bytes,
        vector(
            sum(&[points, product(&[loans, sum(&[edges, 2])?])?])?,
            size_of::<&crate::hir::ExpressionId>(),
        )?,
    )?;
    Ok(bytes)
}
