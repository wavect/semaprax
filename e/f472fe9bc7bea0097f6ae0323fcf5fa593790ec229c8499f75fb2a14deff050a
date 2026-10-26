//! Distinct local constructor proof in an imported module; no import discount.
//! No auxiliary-carrier count, name index or pointer alone grants identity scope.
use super::{
    compiler, local, ordinary, Context, Costs, Failure, Function, Index, Invoice, Program,
    MAX_HEADERS,
};
type Error = Failure;

// These loans can only be formed after the full ordinary/local source check.
// The surrounding Session owns their exact immutable Context and Invoice.
struct CheckedFunction<'p> {
    function: &'p Function,
    arity: usize,
}

pub(in crate::workspace_graph::expected_projection) struct Session<'p, 'a> {
    context: Context<'p, 'a>,
    invoice: Invoice,
    slots: usize,
    exhausted: bool,
    excluded: bool,
    owner: Option<CheckedFunction<'p>>,
    target: Option<CheckedFunction<'p>>,
}
type Eligibility<'p> = (
    &'p Program,
    &'p Program,
    &'p Vec<Function>,
    &'p Vec<Function>,
    &'p Vec<crate::ast::InterfaceDeclaration>,
    &'p Vec<crate::ast::InterfaceDeclaration>,
    &'p Vec<crate::ast::ProtocolDeclaration>,
    &'p Vec<crate::ast::ProtocolDeclaration>,
    &'p Vec<crate::ast::ProtocolImplementation>,
    &'p Vec<crate::ast::ProtocolImplementation>,
    &'p Vec<crate::ast::SessionProtocolDeclaration>,
    &'p Vec<crate::ast::SessionProtocolDeclaration>,
    &'p Vec<crate::ast::AgentDeclaration>,
    &'p Vec<crate::ast::AgentDeclaration>,
    usize,
    usize,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
);
type Acquire<'p, 'a> = (
    Eligibility<'p>,
    Context<'p, 'a>,
    Context<'p, 'a>,
    Session<'p, 'a>,
    Session<'p, 'a>,
    Option<Session<'p, 'a>>,
    Option<Session<'p, 'a>>,
    Result<Invoice, Failure>,
    Result<Invoice, Failure>,
    usize,
    usize,
    Option<usize>,
    Result<usize, Failure>,
    bool,
);

impl<'p, 'a> Session<'p, 'a> {
    pub(in crate::workspace_graph::expected_projection) fn eligible(program: &Program) -> bool {
        // Each constructor independently excludes ALL competing import aliases,
        // including on retained-only passes; lookup's flag is not that proof.
        program.functions.len() <= MAX_HEADERS
            && program.interfaces.is_empty()
            && program.protocols.is_empty()
            && program.implementations.is_empty()
            && program.session_protocols.is_empty()
            && program.agents.is_empty()
    }

    pub(in crate::workspace_graph::expected_projection) fn checkpoint(
        current: &Option<Self>,
    ) -> usize {
        current.as_ref().map_or(0, |held| held.slots)
    }

    // Closed caller prerequisite: owner and target come from this Context's
    // actual Program.functions traversal/unique Source::Function lookup. This
    // authenticates that loan; an arbitrary fabricated Index plus copied
    // Function is not a membership witness for this internal entry.
    pub(in crate::workspace_graph::expected_projection) fn observe(
        current: &mut Option<Self>,
        context: Context<'p, 'a>,
        owner: &'p Function,
        target: &'p Function,
        arity: usize,
    ) -> Result<(), Error> {
        let result = (|| {
            if current.is_none() {
                let invoice = Invoice::acquire_with::<Acquire<'p, 'a>>()?;
                *current = Some(Self {
                    context,
                    invoice,
                    slots: 0,
                    exhausted: false,
                    excluded: !Self::eligible(context.program),
                    owner: None,
                    target: None,
                });
            }
            let held = current.as_mut().expect("fresh local certificate lender");
            held.invoice.pay::<(
                &mut Option<Self>,
                &mut Option<Self>,
                &mut Self,
                &mut Self,
                Context<'p, 'a>,
                Context<'p, 'a>,
                &Program,
                &Program,
                &Program,
                &Program,
                &Index<'a>,
                &Index<'a>,
                &Index<'a>,
                &Index<'a>,
                &[Program],
                &[Program],
                &[Program],
                &[Program],
                &Function,
                &Function,
                &Function,
                &Function,
                usize,
                usize,
                Result<(), Failure>,
                Result<(), Failure>,
                bool,
                bool,
                bool,
                bool,
                Option<usize>,
                usize,
                CheckedFunction<'p>,
                CheckedFunction<'p>,
                Option<CheckedFunction<'p>>,
                Option<CheckedFunction<'p>>,
                &Option<CheckedFunction<'p>>,
                &Option<CheckedFunction<'p>>,
                &CheckedFunction<'p>,
                &CheckedFunction<'p>,
                &CheckedFunction<'p>,
                &Option<CheckedFunction<'p>>,
                &Vec<crate::ast::Param>,
                &Vec<crate::ast::Param>,
                usize,
                usize,
                Result<(), Error>,
                Result<(), Error>,
                &Function,
                &Function,
                &Function,
                &Function,
                &Function,
                &Function,
                &Function,
                &Function,
                bool,
                bool,
                bool,
                bool,
            )>()?;
            if !std::ptr::eq(held.context.program, context.program)
                || !std::ptr::eq(held.context.authored, context.authored)
                || !std::ptr::eq(held.context.programs, context.programs)
            {
                return Err(Failure::Foreign);
            }
            if held.excluded || held.exhausted {
                return Ok(());
            }
            // Pointer equality is only lookup of a SEALED constructor loan,
            // never its creation. No result escapes this exact lender/context;
            // both immutable source owners remain borrowed until finish.
            if !matches!(&held.owner, Some(checked) if std::ptr::eq(checked.function, owner)) {
                if !ordinary(owner, &held.invoice)?
                    || !local(context, owner, &held.invoice)?
                    || !unaliased(context, owner, &held.invoice)?
                {
                    return Ok(());
                }
                held.owner = Some(CheckedFunction {
                    function: owner,
                    arity: owner.params.len(),
                });
            }
            if !matches!(&held.target, Some(checked) if std::ptr::eq(checked.function, target)) {
                if !ordinary(target, &held.invoice)?
                    || !local(context, target, &held.invoice)?
                    || compiler(&target.name, &held.invoice)?
                    || !unaliased(context, target, &held.invoice)?
                {
                    return Ok(());
                }
                held.target = Some(CheckedFunction {
                    function: target,
                    arity: target.params.len(),
                });
            }
            let Some(target) = &held.target else {
                unreachable!("authenticated target loan");
            };
            if target.arity == arity {
                held.slots = held.slots.checked_add(1).ok_or(Failure::Arithmetic)?;
            }
            Ok(())
        })();
        match result {
            Err(Failure::Work) => {
                if let Some(held) = current {
                    held.slots = 0;
                    held.exhausted = true;
                }
                Ok(())
            }
            other => other,
        }
    }

    pub(in crate::workspace_graph::expected_projection) fn rollback(
        current: &mut Option<Self>,
        before: usize,
    ) -> Result<(), Error> {
        if let Some(held) = current {
            held.invoice.pay::<(
                &mut Self,
                &mut Self,
                &mut Option<Self>,
                &mut Option<Self>,
                usize,
                usize,
                bool,
                bool,
                Result<(), Failure>,
                Result<(), Error>,
                Result<(), Error>,
            )>()?;
            if held.exhausted {
                held.slots = 0;
            } else if before <= held.slots {
                held.slots = before;
            } else {
                return Err(Failure::Foreign);
            }
        }
        Ok(())
    }

    #[allow(
        clippy::let_and_return,
        reason = "retain separately invoiced receiving and returned Result owners"
    )]
    pub(in crate::workspace_graph::expected_projection) fn finish(
        current: Option<Self>,
        context: Context<'p, 'a>,
    ) -> Result<Costs, Error> {
        let Some(held) = current else {
            return Ok(Costs { slots: 0, bytes: 0 });
        };
        let result = (|| {
            held.invoice.pay::<(
                Self,
                Self,
                Context<'p, 'a>,
                Context<'p, 'a>,
                &Invoice,
                &Invoice,
                Costs,
                Costs,
                Result<Costs, Failure>,
                Result<Costs, Error>,
                Result<Costs, Error>,
                &Program,
                &Program,
                &Program,
                &Program,
                &Index<'a>,
                &Index<'a>,
                &Index<'a>,
                &Index<'a>,
                &[Program],
                &[Program],
                &[Program],
                &[Program],
                bool,
                bool,
                bool,
            )>()?;
            if !std::ptr::eq(held.context.program, context.program)
                || !std::ptr::eq(held.context.authored, context.authored)
                || !std::ptr::eq(held.context.programs, context.programs)
            {
                return Err(Failure::Foreign);
            }
            Ok(Costs {
                slots: held.slots,
                bytes: held.invoice.bytes(),
            })
        })();
        result
    }
}

// A local Source::Function loan cannot borrow an alias's resolution priority.
// This scan is independent of include_imports and of the alias target's kind,
// provider existence, arity or validity. Every matching spelling closes proof.
fn unaliased(
    context: Context<'_, '_>,
    function: &Function,
    invoice: &Invoice,
) -> Result<bool, Failure> {
    invoice.pay::<(
        Context<'_, '_>,
        Context<'_, '_>,
        &Function,
        &Function,
        &Invoice,
        &Invoice,
        &Vec<crate::ast::ModuleUse>,
        &Vec<crate::ast::ModuleUse>,
        &[crate::ast::ModuleUse],
        &[crate::ast::ModuleUse],
        &[crate::ast::ModuleUse],
        &[crate::ast::ModuleUse],
        std::slice::Iter<'_, crate::ast::ModuleUse>,
        std::slice::Iter<'_, crate::ast::ModuleUse>,
        Result<bool, Failure>,
        Result<bool, Failure>,
    )>()?;
    let mut aliases = context.program.module_uses.iter();
    loop {
        invoice.pay::<(
            &Invoice,
            &Invoice,
            &mut std::slice::Iter<'_, crate::ast::ModuleUse>,
            &mut std::slice::Iter<'_, crate::ast::ModuleUse>,
            Option<&crate::ast::ModuleUse>,
            Option<&crate::ast::ModuleUse>,
            &crate::ast::ModuleUse,
            &crate::ast::ModuleUse,
            &String,
            &String,
            &String,
            &String,
            &String,
            &String,
            &String,
            &String,
            &str,
            &str,
            &str,
            &str,
            &str,
            &str,
            &str,
            &str,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            bool,
            bool,
            Result<(), Failure>,
            Result<(), Failure>,
        )>()?;
        let Some(alias) = aliases.next() else {
            return Ok(true);
        };
        invoice.work(
            alias
                .alias
                .len()
                .saturating_add(function.name.len())
                .saturating_add(1),
        )?;
        if str::eq(alias.alias.as_str(), function.name.as_str()) {
            return Ok(false);
        }
    }
}
