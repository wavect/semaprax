//! Two bounded consumers of one invocation's immutable literal census.
//! The finished certificate remains DATA; every handoff has its own lender fee.
use super::super::{call_carriers, held_projection, private_layout, SyntheticBuilderCosts};
use super::{size_of, Context, Costs, Failure, Index, Invoice, Program};
use crate::diagnostic::Diagnostic;

type ResultPair = Result<(call_carriers::Proof, Costs), Vec<Diagnostic>>;
// Internal lender rejection must not format/debit a diagnostic in a foreign
// current budget. Only the owning production boundary publishes diagnostics.
#[derive(Debug)]
enum ReadError {
    Payment(Failure),
    Census(Vec<Diagnostic>),
}
type ReadResult = Result<(call_carriers::Proof, Costs), ReadError>;

pub(in crate::workspace_graph::expected_projection) struct Census<'p, 'a> {
    context: Context<'p, 'a>,
    invoice: Invoice,
    cached: Option<(call_carriers::Proof, Costs)>,
    used: u8,
    reusable: bool,
    failed: bool,
}

type Entry<'p, 'a> = (
    &'p Program,
    &'p Program,
    &'a Index<'a>,
    &'a Index<'a>,
    &'a [Program],
    &'a [Program],
    Option<usize>,
    Option<usize>,
    u8,
    u8,
    bool,
    bool,
    Option<&'p private_layout::registry::LayoutRegistry<'p>>,
    Option<&'p private_layout::registry::LayoutRegistry<'p>>,
    SyntheticBuilderCosts,
    SyntheticBuilderCosts,
    SyntheticBuilderCosts,
    SyntheticBuilderCosts,
    Result<(SyntheticBuilderCosts, SyntheticBuilderCosts), Vec<Diagnostic>>,
    Result<(SyntheticBuilderCosts, SyntheticBuilderCosts), Vec<Diagnostic>>,
);
type Acquire<'p, 'a> = (
    ReadResult,
    ReadResult,
    ReadError,
    ReadError,
    Entry<'p, 'a>,
    Census<'p, 'a>,
    Census<'p, 'a>,
    Context<'p, 'a>,
    Context<'p, 'a>,
    &'p Program,
    &'p Program,
    &'p Vec<crate::ast::ModuleUse>,
    &'p Vec<crate::ast::ModuleUse>,
    &'p Vec<crate::ast::InterfaceDeclaration>,
    &'p Vec<crate::ast::InterfaceDeclaration>,
    bool,
    bool,
    bool,
    bool,
    u8,
    u8,
    Option<(call_carriers::Proof, Costs)>,
    Option<(call_carriers::Proof, Costs)>,
    ResultPair,
    ResultPair,
    Result<Invoice, Failure>,
    Result<Invoice, Failure>,
);

type Read<'p, 'a> = (
    ReadResult,
    ReadResult,
    ReadError,
    ReadError,
    Option<&'p mut Census<'p, 'a>>,
    Option<&'p mut Census<'p, 'a>>,
    &'p mut Census<'p, 'a>,
    &'p mut Census<'p, 'a>,
    Context<'p, 'a>,
    Context<'p, 'a>,
    &'p Program,
    &'p Program,
    &'p Program,
    &'p Program,
    &'p Index<'a>,
    &'p Index<'a>,
    &'p Index<'a>,
    &'p Index<'a>,
    &'p [Program],
    &'p [Program],
    &'p [Program],
    &'p [Program],
    &'p Invoice,
    &'p Invoice,
    &'p Option<(call_carriers::Proof, Costs)>,
    &'p mut Option<(call_carriers::Proof, Costs)>,
    &'p mut Option<(call_carriers::Proof, Costs)>,
    Option<(call_carriers::Proof, Costs)>,
    Option<(call_carriers::Proof, Costs)>,
    bool,
    bool,
    bool,
    bool,
    &'p (call_carriers::Proof, Costs),
    &'p (call_carriers::Proof, Costs),
    (call_carriers::Proof, Costs),
    (call_carriers::Proof, Costs),
    &'p call_carriers::Proof,
    &'p Costs,
    call_carriers::Proof,
    call_carriers::Proof,
    Costs,
    Costs,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    u8,
    u8,
    u8,
    usize,
    usize,
    usize,
    Option<usize>,
    Result<(), Failure>,
    ResultPair,
    ResultPair,
);

fn diagnostic(_: Failure) -> Vec<Diagnostic> {
    vec![super::super::super::limit_error(
        "builder_bytes",
        super::super::super::active_builder_limit(),
    )]
}

impl<'p, 'a> Census<'p, 'a> {
    fn acquire(
        context: Context<'p, 'a>,
        mode: u8,
        first_imports: bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        let invoice = Invoice::acquire_with::<Acquire<'p, 'a>>().map_err(diagnostic)?;
        if mode != 6 || !first_imports {
            return Err(diagnostic(Failure::Foreign));
        }
        // These are the ONLY source surfaces inspected by the lookup selector
        // under include_imports. With both empty, true/false select the exact
        // same directory, names walk, body and constructor loans. The bool's
        // typed control width is equal; no arbitrary model equivalence is used.
        let reusable =
            context.program.module_uses.is_empty() && context.program.interfaces.is_empty();
        Ok(Self {
            context,
            invoice,
            cached: None,
            used: 0,
            reusable,
            failed: false,
        })
    }

    #[allow(
        clippy::redundant_pattern_matching,
        reason = "direct discriminants preserve separately prepaid receiver boundaries"
    )]
    fn read(&mut self, context: Context<'p, 'a>, imports: bool) -> ReadResult {
        // The live Invoice checks the real current lender BEFORE either use.
        // Its cumulative paid bytes remain in both conservative forecasts;
        // retaining a scalar fee does not debit a second census in the ledger.
        self.invoice
            .pay::<Read<'_, '_>>()
            .map_err(ReadError::Payment)?;
        if !std::ptr::eq(self.context.program, context.program)
            || !std::ptr::eq(self.context.authored, context.authored)
            || !std::ptr::eq(self.context.programs, context.programs)
            || self.failed
            || !matches!((self.used, imports), (0, true) | (1, false))
        {
            return Err(ReadError::Payment(Failure::Foreign));
        }
        // Consume this phase before any fallible fallback. A failed read is
        // never a token that may be replayed after restoring the old budget.
        self.used += 1;
        let fresh = if !self.reusable || matches!(self.cached, None) {
            match call_carriers::count_modular(context, imports) {
                Ok(fresh) => Some(fresh),
                Err(error) => {
                    self.failed = true;
                    return Err(ReadError::Census(error));
                }
            }
        } else {
            None
        };
        let (proof, mut cost) = if self.reusable {
            if let Some(fresh) = fresh {
                self.cached = Some(fresh);
            }
            let Some((proof, cost)) = &self.cached else {
                return Err(ReadError::Payment(Failure::Foreign));
            };
            (
                call_carriers::Proof {
                    roots: proof.roots,
                    bytes: proof.bytes,
                    retained: proof.retained,
                    ..Default::default()
                },
                Costs {
                    slots: cost.slots,
                    bytes: cost.bytes,
                },
            )
        } else {
            match fresh {
                Some(fresh) => fresh,
                None => return Err(ReadError::Payment(Failure::Foreign)),
            }
        };
        cost.bytes = cost
            .bytes
            .checked_add(self.invoice.bytes())
            .ok_or(ReadError::Payment(Failure::Arithmetic))?;
        Ok((proof, cost))
    }
}

pub(in crate::workspace_graph::expected_projection) fn read_or_count<'p, 'a>(
    census: Option<&mut Census<'p, 'a>>,
    context: Context<'p, 'a>,
    imports: bool,
) -> ResultPair {
    match census {
        Some(census) => census.read(context, imports).map_err(|error| match error {
            ReadError::Payment(failure) => diagnostic(failure),
            ReadError::Census(error) => error,
        }),
        None => call_carriers::count_modular(context, imports),
    }
}

pub(in crate::workspace_graph::expected_projection) fn estimates<'a>(
    program: &Program,
    authored: &'a Index<'a>,
    programs: &'a [Program],
    maximum: Option<usize>,
    mode: u8,
    first_imports: bool,
    registry: Option<&private_layout::registry::LayoutRegistry<'_>>,
) -> Result<(SyntheticBuilderCosts, SyntheticBuilderCosts), Vec<Diagnostic>> {
    let context = Context {
        program,
        authored,
        programs,
    };
    let mut census = Census::acquire(context, mode, first_imports)?;
    let full = held_projection::modular_projection::estimate_with_census(
        program,
        authored,
        programs,
        maximum,
        6,
        true,
        registry,
        &mut census,
    )?;
    let retained = held_projection::modular_projection::estimate_with_census(
        program,
        authored,
        programs,
        maximum,
        6,
        false,
        registry,
        &mut census,
    )?;
    Ok((full, retained))
}

// This invocation wrapper is not HIR backing or a callee-slot discount. Its
// actual lender pays before selecting either complete producer; no Drop refund
// or cache entry can return that consumed reservation.
const SELECT: usize = 2 * size_of::<(
    &Program,
    &Index<'_>,
    &[Program],
    Option<usize>,
    u8,
    bool,
    Option<&private_layout::registry::LayoutRegistry<'_>>,
    Result<(SyntheticBuilderCosts, SyntheticBuilderCosts), Vec<Diagnostic>>,
)>() + 2 * size_of::<&Vec<crate::ast::ModuleUse>>()
    + 2 * size_of::<bool>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<Result<(), Vec<Diagnostic>>>();

pub(in crate::workspace_graph::expected_projection) fn selected_estimates<'a>(
    program: &Program,
    authored: &'a Index<'a>,
    programs: &'a [Program],
    maximum: Option<usize>,
    mode: u8,
    first_imports: bool,
    registry: Option<&private_layout::registry::LayoutRegistry<'_>>,
) -> Result<(SyntheticBuilderCosts, SyntheticBuilderCosts), Vec<Diagnostic>> {
    super::super::reserve_builder_structure(SELECT)?;
    if program.module_uses.is_empty() {
        super::paired::estimates(
            program,
            authored,
            programs,
            maximum,
            mode,
            first_imports,
            registry,
        )
    } else {
        estimates(
            program,
            authored,
            programs,
            maximum,
            mode,
            first_imports,
            registry,
        )
    }
}

#[cfg(test)]
#[path = "modular_pair_tests.rs"]
mod tests;
