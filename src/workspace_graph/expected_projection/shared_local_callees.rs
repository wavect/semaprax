//! One certified local callee occurrence, never an expression/nominal discount.
//!
//! Constructor-family audit for the final *uncached* retained-HIR forecast:
//! - resolve_expr's Function-ID lookup clones the held DeclarationId, then
//!   FinishCall moves it into the HIR node; ids::DeclarationText::Shared clones Arc.
//! - DeclarationIndex::functions_by_name clones the declaration's held ID. Its
//!   declaration/type entries are per declaration, not fresh per call.
//! - nominal/type-result slots remain completely charged; this proof accepts
//!   only concrete nonnominal signatures and never changes BASE's three slots.
//! - call_index::PersistentCallIndex clones callee IDs into sites/maps/sets.
//!   Its independent expression.id.as_str().to_owned() stays charged as ExprID.
//! - HIR validation borrows the callee/index and its held parameter views;
//!   semantic validation, work, error construction and indexes are unchanged.
//! - cleanup/inventory_builder traverses Call's arguments, not a fresh callee
//!   text owner; all expression and cleanup-owner slots remain charged.
//! - cleanup_plan/build/finish_call clones callee into PropagatedCall.
//! - workspace retained call edges and call-site text verification are *import*
//!   consumers (expected_projection's function_uses/aliases); Use is excluded.
//!   graph/call_inventory's local sets clone DeclarationId too.
//!
//! JSON is not shared ID storage: graph.rs renders an independent String set
//! as well as quoted output. Neither is part of this retained-HIR certificate;
//! all serialization, writer and output allowances remain unchanged. The final
//! uncached core retains WorkspaceModule (workspace_graph.rs retain branch);
//! source_callables.rs's JSON route uses the unchanged mode-zero full prebound.
//! quote_json/CappedString pays writes, not that temporary String set.
//! No writer, output limit, serialized/cache term, or cached mode is discounted.
//! The mode-six caller below only removes a proven callee slot from the old
//! identity text bound; its sixteen-copy factor and every other term survive.

use super::super::AuthoredDeclaration;
use crate::ast::{Expr, ExprKind, Function, Program, Statement, Type};
use crate::bounded_output::classifier_string::ClassifierLedger;
use std::{cell::Cell, collections::BTreeMap, mem::size_of};

#[path = "shared_local_callees/invoice.rs"]
mod invoice;
#[cfg(test)]
#[path = "shared_local_callees/tests.rs"]
mod tests;
#[path = "shared_local_callees/walk.rs"]
mod walk;
use invoice::{Failure, Invoice};

type Index<'a> = BTreeMap<&'a str, AuthoredDeclaration<'a>>;
type Headers<'a> = BTreeMap<&'a str, Option<&'a Function>>;
const MAX_HEADERS: usize = 512;
const MAX_NAMES: usize = 128;
const MAX_FRAMES: usize = 129;
const WORK: usize = 1_048_576;

#[derive(Clone, Copy)]
pub(super) struct Context<'p, 'a> {
    pub(super) program: &'p Program,
    pub(super) authored: &'a Index<'a>,
    pub(super) programs: &'a [Program],
}

/// A short-lived immutable source loan, consumed before the forecast publishes.
/// The exact lender stays borrowed; a reused stack address cannot impersonate it.
pub(super) struct Certificate<'p, 'a, 'b> {
    context: Context<'p, 'a>,
    invoice: &'b Invoice,
    slots: usize,
}

pub(super) struct Costs {
    pub(super) slots: usize,
    pub(super) bytes: usize,
}

fn plain(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::String
            | Type::Bytes
            | Type::Str
            | Type::SliceU8
    )
}

fn ordinary(function: &Function, invoice: &Invoice) -> Result<bool, Failure> {
    invoice.pay::<invoice::Ordinary<'_>>()?;
    let header = function.explicit_id
        && function.type_parameters.is_empty()
        && function.yields.is_none()
        && function.follows.is_none()
        && plain(&function.return_type);
    if !header {
        return Ok(false);
    }
    let mut parameters = function.params.iter();
    loop {
        invoice.pay::<invoice::Parameter<'_>>()?;
        let Some(parameter) = parameters.next() else {
            break;
        };
        invoice.work(1)?;
        if !plain(&parameter.ty) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn compiler(name: &str, invoice: &Invoice) -> Result<bool, Failure> {
    invoice.pay::<invoice::Compiler<'_>>()?;
    Ok(name == crate::vec_ops::string_slot::NAME
        || crate::stdin_stream_ops::pure_by_name(name).is_some()
        || crate::vec_ops::field_unique::literal_call(name)
        || name == crate::vec_ops::field_update::NAME
        || name == crate::vec_field::NAME
        || name == crate::literal_format::NAME
        || crate::string_ops::by_name(name).is_some()
        || crate::str_ops::by_name(name).is_some()
        || crate::vec_ops::by_name(name).is_some()
        || crate::box_ops::by_name(name).is_some()
        || crate::iterator_ops::by_name(name).is_some()
        || crate::list_ops::by_name(name).is_some()
        || crate::map_ops::by_name(name).is_some()
        || crate::byte_ops::by_name(name).is_some()
        || crate::host_io_ops::by_name(name).is_some()
        || crate::command_io_ops::by_name(name).is_some())
}

// Exact current declarations and provider context, not equality of copied text.
fn local<'p, 'a>(
    context: Context<'p, 'a>,
    function: &Function,
    invoice: &Invoice,
) -> Result<bool, Failure> {
    invoice.pay::<invoice::Local<'_, '_>>()?;
    let mut programs = context.programs.iter();
    let mut present = false;
    loop {
        invoice.pay::<invoice::Provider<'_>>()?;
        let Some(program) = programs.next() else {
            break;
        };
        invoice.work(1)?;
        if std::ptr::eq(program, context.program) {
            present = true;
            break;
        }
    }
    if !present {
        return Ok(false);
    }
    invoice.work(
        function
            .stable_id
            .len()
            .saturating_add(context.authored.len())
            .saturating_add(1),
    )?;
    let Some(target) = context.authored.get(function.stable_id.as_str()) else {
        return Ok(false);
    };
    let Some(held) = target.function else {
        return Ok(false);
    };
    Ok(target.kind == super::super::AuthoredKind::Function
        && target.explicit
        && target.module == context.program.module
        && target.path == context.program.path
        && std::ptr::eq(held, function))
}

impl<'p, 'a, 'b> Certificate<'p, 'a, 'b> {
    fn acquire(context: Context<'p, 'a>, invoice: &'b Invoice) -> Result<Self, Failure> {
        invoice.pay::<invoice::Acquire<'_, '_, '_>>()?;
        let mut slots = 0usize;
        // These surfaces have their own scopes/lowerings. They never receive a
        // guessed certificate, including aliases absent from a filtered output.
        if context.program.functions.len() > MAX_HEADERS
            || !context.program.interfaces.is_empty()
            || !context.program.protocols.is_empty()
            || !context.program.implementations.is_empty()
            || !context.program.session_protocols.is_empty()
            || !context.program.agents.is_empty()
        {
            return Ok(Self {
                context,
                invoice,
                slots,
            });
        }
        invoice.pay::<(
            Headers<'_>,
            std::slice::Iter<'_, Function>,
            std::slice::Iter<'_, Function>,
            std::slice::Iter<'_, crate::ast::ModuleUse>,
            &Vec<Function>,
            &Vec<Function>,
            &Vec<crate::ast::ModuleUse>,
        )>()?;
        let mut headers = Headers::new();
        let mut functions = context.program.functions.iter();
        loop {
            invoice.pay::<invoice::Advance<'_, Function>>()?;
            let Some(function) = functions.next() else {
                break;
            };
            invoice.pay::<invoice::Header<'_>>()?;
            invoice.work(function.name.len().saturating_add(1))?;
            let previous = headers.get(function.name.as_str());
            let value = if previous.is_some() {
                None
            } else {
                Some(function)
            };
            invoice.pay_node::<&str, Option<&Function>>()?;
            headers.insert(function.name.as_str(), value);
        }
        // An authored alias remains excluded even when a retained-only output
        // omits its stub. No provider Use or Interface can become Source::Function.
        let mut aliases = context.program.module_uses.iter();
        loop {
            invoice.pay::<invoice::Advance<'_, crate::ast::ModuleUse>>()?;
            let Some(item) = aliases.next() else {
                break;
            };
            invoice.pay::<invoice::Alias<'_>>()?;
            invoice.work(item.alias.len().saturating_add(1))?;
            if let Some(value) = headers.get_mut(item.alias.as_str()) {
                *value = None;
            }
        }
        let mut functions = context.program.functions.iter();
        loop {
            invoice.pay::<invoice::Advance<'_, Function>>()?;
            let Some(function) = functions.next() else {
                break;
            };
            invoice.pay::<invoice::FunctionPass<'_>>()?;
            invoice.work(function.params.len().saturating_add(1))?;
            if !ordinary(function, invoice)? || !local(context, function, invoice)? {
                continue;
            }
            if let Some(count) = walk::count(context, function, &headers, invoice)? {
                slots = slots.checked_add(count).ok_or(Failure::Arithmetic)?;
            }
        }
        Ok(Self {
            context,
            invoice,
            slots,
        })
    }

    fn consume(self, context: Context<'p, 'a>, invoice: &Invoice) -> Result<Costs, Failure> {
        self.invoice.pay::<invoice::Consume<'_, '_, '_>>()?;
        if !std::ptr::eq(self.invoice, invoice)
            || !std::ptr::eq(self.context.program, context.program)
            || !std::ptr::eq(self.context.authored, context.authored)
            || !std::ptr::eq(self.context.programs, context.programs)
        {
            return Err(Failure::Foreign);
        }
        Ok(Costs {
            slots: self.slots,
            bytes: invoice.bytes(),
        })
    }
}

pub(super) fn costs(context: Context<'_, '_>) -> Result<Costs, Vec<crate::diagnostic::Diagnostic>> {
    let result = (|| {
        let invoice = Invoice::acquire()?;
        match Certificate::acquire(context, &invoice) {
            Ok(certificate) => certificate.consume(context, &invoice),
            // Optional bounded source work never authorizes a partial count.
            Err(Failure::Work) => Ok(Costs {
                slots: 0,
                bytes: invoice.bytes(),
            }),
            Err(error) => Err(error),
        }
    })();
    result.map_err(|_| {
        vec![super::super::limit_error(
            "builder_bytes",
            super::super::active_builder_limit(),
        )]
    })
}
