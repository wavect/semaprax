//! Reconstruct private decoded proof storage without changing proof values.
use crate::hir::{ResolvedFunction, ResolvedProgram};
use crate::loan_plan::LoanPlan;

use super::{capacity, invalid, Result, MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND};

mod bound;

pub(super) fn reconstruct(program: &mut ResolvedProgram) -> Result<()> {
    reconstruct_with_limit(program, MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND).map(|_| ())
}

fn functions(program: &ResolvedProgram) -> impl Iterator<Item = &ResolvedFunction> {
    program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    )
}

fn physical_bytes(plan: &LoanPlan) -> Result<usize> {
    crate::loan_plan::owned_capacity_bytes(plan)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<LoanPlan>()))
        .ok_or_else(|| capacity("decoded loan proof capacity cannot be accounted"))
}

fn reconstruct_with_limit(program: &mut ResolvedProgram, limit: usize) -> Result<usize> {
    if limit > MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND {
        return Err(capacity(
            "decoded loan proof replay limit exceeds its maximum",
        ));
    }
    // Empty proofs need no storage reconstruction. They remain unchanged for
    // the ordinary full validator to reject missing, extra, or malformed proof.
    if functions(program).all(|function| function.loan_plan.loans.is_empty()) {
        return Ok(0);
    }
    let (result, overflowed, used) = crate::bounded_output::with_limit_usage(limit, || {
        let construction = bound::construction_bytes(program)?;
        let mut old_bytes = 0usize;
        for function in functions(program).filter(|function| !function.loan_plan.loans.is_empty()) {
            old_bytes = old_bytes
                .checked_add(physical_bytes(&function.loan_plan)?)
                .ok_or_else(|| capacity("decoded loan proof peak accounting overflows"))?;
        }
        let peak = old_bytes
            .checked_add(construction)
            .ok_or_else(|| capacity("decoded loan proof peak accounting overflows"))?;
        if !crate::bounded_output::reserve_active_required(peak) {
            return Err(capacity(
                "decoded loan proof reconstruction exceeds its builder bound",
            ));
        }

        // The old proofs and complete construction allowance stay charged
        // together; replacement never refunds this private replay budget.
        let mut rebuilt_bytes = 0usize;
        for index in 0..program.functions.len() {
            if !program.functions[index].loan_plan.loans.is_empty() {
                let candidate = rebuild(
                    program,
                    &program.functions[index],
                    &mut rebuilt_bytes,
                    construction,
                )?;
                program.functions[index].loan_plan = candidate;
            }
        }
        for index in 0..program.function_instances.len() {
            if !program.function_instances[index]
                .function
                .loan_plan
                .loans
                .is_empty()
            {
                let candidate = rebuild(
                    program,
                    &program.function_instances[index].function,
                    &mut rebuilt_bytes,
                    construction,
                )?;
                program.function_instances[index].function.loan_plan = candidate;
            }
        }
        Ok(())
    });
    if overflowed {
        return Err(capacity(
            "decoded loan proof reconstruction exceeds its builder bound",
        ));
    }
    result?;
    Ok(used)
}

fn rebuild(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    rebuilt_bytes: &mut usize,
    construction: usize,
) -> Result<LoanPlan> {
    // The ordinary planner retains its endpoint/edge/loan and checked-work
    // limits. Body identities supply backing; pointers grant no proof authority.
    let candidate = crate::loan_plan::build_plan(program, function).map_err(|error| vec![error])?;
    if candidate != function.loan_plan {
        return Err(invalid(
            "decoded loan proof disagrees with independent reconstruction",
        ));
    }
    *rebuilt_bytes = rebuilt_bytes
        .checked_add(physical_bytes(&candidate)?)
        .filter(|bytes| *bytes <= construction)
        .ok_or_else(|| capacity("reconstructed loan proof exceeds its construction allowance"))?;
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> ResolvedProgram {
        let source = r#"
module loan_snapshot;
@id("loan.take") fn take(value: own Bytes) -> i64 { 1 }
@id("loan.run") fn run(input: borrow Slice<u8>) -> i64 {
    let owned = bytes_copy(input);
    let view = bytes_as_slice(owned);
    let observed = byte_len(view);
    take(owned)
}
@id("loan.main") fn main() -> i64 { 0 }
"#;
        let ast = crate::parse(source, "loan-snapshot.spx").unwrap();
        assert!(crate::verify::verify(&ast).is_empty());
        let resolved = crate::hir::resolve(&ast).unwrap();
        crate::hir::validate(&resolved).unwrap();
        let wire = crate::cache_codec::encode(&resolved).unwrap();
        crate::cache_codec::decode(&wire).unwrap()
    }

    #[test]
    fn decoded_proof_reconstruction_preserves_bytes_and_restores_shared_capacity() {
        let mut decoded = fixture();
        let original_wire = crate::cache_codec::encode(&decoded).unwrap();
        let index = decoded
            .functions
            .iter()
            .position(|function| !function.loan_plan.loans.is_empty())
            .unwrap();
        let old_capacity = physical_bytes(&decoded.functions[index].loan_plan).unwrap();
        reconstruct(&mut decoded).unwrap();
        assert!(physical_bytes(&decoded.functions[index].loan_plan).unwrap() < old_capacity);
        assert_eq!(crate::cache_codec::encode(&decoded).unwrap(), original_wire);
        crate::hir::validate(&decoded).unwrap();
    }

    #[test]
    fn decoded_proof_reconstruction_has_an_exact_budget_and_preserves_hostile_proofs() {
        let mut decoded = fixture();
        let used = reconstruct_with_limit(&mut decoded, MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND)
            .unwrap();
        assert!(used > 0);
        let mut exact = fixture();
        assert_eq!(reconstruct_with_limit(&mut exact, used).unwrap(), used);
        let mut refused = fixture();
        let wire = crate::cache_codec::encode(&refused).unwrap();
        assert_eq!(
            reconstruct_with_limit(&mut refused, used - 1).unwrap_err()[0].code,
            "SPX-G256"
        );
        assert_eq!(crate::cache_codec::encode(&refused).unwrap(), wire);

        let mut forged = fixture();
        let index = forged
            .functions
            .iter()
            .position(|function| !function.loan_plan.loans.is_empty())
            .unwrap();
        assert!(!forged.functions[index].loan_plan.loans[0].ends.is_empty());
        forged.functions[index].loan_plan.loans[0].ends.clear();
        let wire = crate::cache_codec::encode(&forged).unwrap();
        assert_eq!(reconstruct(&mut forged).unwrap_err()[0].code, "SPX-G255");
        assert_eq!(crate::cache_codec::encode(&forged).unwrap(), wire);
    }

    #[test]
    fn missing_decoded_proof_remains_invalid_without_being_repaired() {
        let mut forged = fixture();
        let index = forged
            .functions
            .iter()
            .position(|function| !function.loan_plan.loans.is_empty())
            .unwrap();
        forged.functions[index].loan_plan.loans.clear();
        let wire = crate::cache_codec::encode(&forged).unwrap();
        reconstruct(&mut forged).unwrap();
        assert_eq!(crate::cache_codec::encode(&forged).unwrap(), wire);
        assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
    }

    #[test]
    fn tiny_stored_ast_cannot_authorize_large_decoded_hir_or_hide_occurrences() {
        let tiny_source = "module tiny; @id(\"tiny.main\") fn main() -> i64 { 0 }";
        let mut entry = super::super::Entry {
            path: String::from("tiny.spx"),
            source: String::from(tiny_source),
            synthetic: crate::parse(tiny_source, "tiny.spx").unwrap(),
            resolved: fixture(),
            resolver_bytes: 1,
            retained_loan_bytes: 1,
        };
        assert_eq!(entry.synthetic.functions.len(), 1);
        let function = entry
            .resolved
            .functions
            .iter_mut()
            .find(|function| !function.loan_plan.loans.is_empty())
            .unwrap();
        let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("fixture must retain its block");
        };
        let repeated = statements[0].clone();
        for _ in 0..crate::loan_plan::MAX_LOAN_ENDPOINTS_V1 / 2 {
            statements.push(repeated.clone());
        }
        // Equal cloned identities are still distinct decoded tree occurrences;
        // the CFG's duplicate-ID check must never materialize this wide tree.
        let wire = crate::cache_codec::encode(&entry.resolved).unwrap();
        let error = reconstruct(&mut entry.resolved).unwrap_err();
        assert_eq!(error[0].code, "SPX-G256");
        assert_eq!(
            error[0].message,
            "decoded HIR loan traversal exceeds its edge bound"
        );
        assert_eq!(crate::cache_codec::encode(&entry.resolved).unwrap(), wire);
    }

    #[test]
    fn large_decoded_assignment_field_is_refused_before_planner_cloning() {
        let mut forged = fixture();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| !function.loan_plan.loans.is_empty())
            .unwrap();
        let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("fixture must retain its block");
        };
        let crate::hir::ResolvedStatement::Let {
            binding,
            value,
            span,
            ..
        } = statements[0].clone()
        else {
            panic!("fixture must begin with a let");
        };
        statements[0] = crate::hir::ResolvedStatement::Assign {
            binding,
            // The current bound shares a distinct projection prefix instead of
            // multiplying it by every binding. Make the field itself exceed
            // one eighth of the unchanged construction limit so the retained
            // place/map terms still reach the pre-cloning capacity frontier.
            field: Some(crate::hir::DeclarationId::new("field".repeat(
                MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND / (8 * "field".len()) + 1,
            ))),
            value,
            span,
        };
        let wire = crate::cache_codec::encode(&forged).unwrap();
        let error = reconstruct(&mut forged).unwrap_err();
        assert_eq!(error[0].code, "SPX-G256");
        assert_eq!(
            error[0].message,
            "decoded HIR loan construction bound exceeds its maximum"
        );
        assert_eq!(crate::cache_codec::encode(&forged).unwrap(), wire);
    }

    #[test]
    fn repeated_projected_borrows_keep_exact_reconstruction_budget_and_full_wire() {
        let source = r#"
module projection_cache;
@id("projection.packet") record Packet {
    @id("projection.payload") payload: Bytes,
    @id("projection.marker") marker: i64,
}
@id("projection.inspect") fn inspect(value: borrow Bytes) -> usize {
    byte_len(bytes_as_slice(value))
}
@id("projection.run") fn run() -> i64 {
    let packet = Packet { payload: bytes_zeroed(7usize), marker: 42 };
    let first = inspect(packet.payload);
    let second = inspect(packet.payload);
    let view = bytes_as_slice(packet.payload);
    let alias = view;
    if first == second && byte_len(alias) == 7usize { packet.marker } else { 0 }
}
@id("projection.main") fn main() -> i64 { run() }
"#;
        for links in [1usize, 32] {
            let mut aliases = String::from("let alias0 = view;\n");
            for index in 1..links {
                aliases.push_str(&format!("let alias{index} = alias{};\n", index - 1));
            }
            aliases.push_str(&format!("let alias = alias{};", links - 1));
            let source = source.replace("let alias = view;", &aliases);
            let ast = crate::check(&source, "projection-cache.spx").unwrap();
            let original = crate::hir::resolve(&ast).unwrap();
            crate::hir::validate(&original).unwrap();
            let wire = crate::cache_codec::encode(&original).unwrap();
            let mut decoded = crate::cache_codec::decode(&wire).unwrap();
            let (used, observed) = crate::test_allocation_observer::measure(|| {
                reconstruct_with_limit(&mut decoded, MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND)
                    .unwrap()
            });
            assert!(!observed.invalid);
            assert!(observed.allocated <= used);
            assert!(observed.highwater <= used);
            assert_eq!(crate::cache_codec::encode(&decoded).unwrap(), wire);
            crate::hir::validate(&decoded).unwrap();
            assert_eq!(
                decoded
                    .functions
                    .iter()
                    .map(|f| &f.loan_plan)
                    .collect::<Vec<_>>(),
                original
                    .functions
                    .iter()
                    .map(|f| &f.loan_plan)
                    .collect::<Vec<_>>()
            );

            let mut exact = crate::cache_codec::decode(&wire).unwrap();
            assert_eq!(reconstruct_with_limit(&mut exact, used).unwrap(), used);
            let mut short = crate::cache_codec::decode(&wire).unwrap();
            assert_eq!(
                reconstruct_with_limit(&mut short, used - 1).unwrap_err()[0].code,
                "SPX-G256"
            );
            assert_eq!(crate::cache_codec::encode(&short).unwrap(), wire);
            crate::hir::validate(&short).unwrap();

            let mut forged: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
            let loan = forged
                .functions
                .iter_mut()
                .flat_map(|f| &mut f.loan_plan.loans)
                .find(|loan| !loan.origin.projections.is_empty())
                .unwrap();
            loan.origin.projections[0] = crate::hir::PlaceProjection::VariantField {
                case: crate::hir::DeclarationId::new("foreign.native.case"),
                field: crate::hir::DeclarationId::new("foreign.native.field"),
            };
            let forged_wire = crate::cache_codec::encode(&forged).unwrap();
            assert_eq!(reconstruct(&mut forged).unwrap_err()[0].code, "SPX-G255");
            assert_eq!(crate::cache_codec::encode(&forged).unwrap(), forged_wire);
            assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
        }
    }
}
