//! New proof storage and every receiving bank are paid before use.
use super::{
    size_of, AuthoredDeclaration, Cell, Certificate, ClassifierLedger, Context, Costs, Function,
    Headers, Index, Program, Type, WORK,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workspace_graph::expected_projection) enum Failure {
    Physical,
    Arithmetic,
    Foreign,
    Work,
}

pub(super) struct Invoice {
    bytes: Cell<usize>,
    remaining: Cell<usize>,
    ledger: ClassifierLedger,
}

// One initial gateway and one renewed gateway after every successful payment.
// A terminal failed gateway cannot be reused. The final unused bank is retained.
fn gateway() -> usize {
    4 * size_of::<&Invoice>()
        + 4 * size_of::<usize>()
        + 2 * size_of::<Option<usize>>()
        + 2 * size_of::<Result<(), Failure>>()
        + 3 * size_of::<bool>()
        + 4 * size_of::<&Cell<usize>>()
        + ClassifierLedger::current_receiving_controls()
}

impl Invoice {
    pub(super) fn setup_bytes() -> usize {
        3 * size_of::<Self>()
            + 2 * size_of::<Context<'_, '_>>()
            + 3 * size_of::<Costs>()
            + 3 * size_of::<Result<Costs, Failure>>()
            + 2 * size_of::<Result<Costs, Vec<crate::diagnostic::Diagnostic>>>()
            + 2 * size_of::<Result<Self, Failure>>()
            + 3 * size_of::<Cell<usize>>()
            + ClassifierLedger::capture_into_controls()
            + ClassifierLedger::insertion_terminal_controls()
            + gateway()
    }

    pub(super) fn acquire() -> Result<Self, Failure> {
        let bytes = Self::setup_bytes();
        if !crate::bounded_output::reserve_active_required(bytes) {
            return Err(Failure::Physical);
        }
        let mut ledger = ClassifierLedger::empty_receiving();
        ledger.capture_current_into();
        Ok(Self {
            bytes: Cell::new(bytes),
            remaining: Cell::new(WORK),
            ledger,
        })
    }

    // A distinct same-invocation constructor pays its own actual/formal and
    // returned owners before formation; the original certificate is unchanged.
    pub(super) fn acquire_with<T>() -> Result<Self, Failure> {
        let bytes = Self::setup_bytes()
            .checked_add(size_of::<T>())
            .ok_or(Failure::Arithmetic)?;
        if !crate::bounded_output::reserve_active_required(bytes) {
            return Err(Failure::Physical);
        }
        let mut ledger = ClassifierLedger::empty_receiving();
        ledger.capture_current_into();
        Ok(Self {
            bytes: Cell::new(bytes),
            remaining: Cell::new(WORK),
            ledger,
        })
    }

    pub(super) fn pay<T>(&self) -> Result<(), Failure> {
        self.pay_bytes(size_of::<T>())
    }

    fn pay_bytes(&self, bytes: usize) -> Result<(), Failure> {
        if !self.ledger.current() {
            return Err(Failure::Foreign);
        }
        let bytes = bytes.checked_add(gateway()).ok_or(Failure::Arithmetic)?;
        let next = self
            .bytes
            .get()
            .checked_add(bytes)
            .ok_or(Failure::Arithmetic)?;
        if !crate::bounded_output::reserve_active_required(bytes) {
            return Err(Failure::Physical);
        }
        self.bytes.set(next);
        Ok(())
    }

    pub(super) fn pay_node<K, V>(&self) -> Result<(), Failure> {
        self.pay::<(Option<usize>, usize, Result<(), Failure>)>()?;
        let bytes =
            crate::cleanup_plan::compiler_btree_node_upper::<K, V>().ok_or(Failure::Arithmetic)?;
        self.pay_bytes(bytes)
    }

    pub(super) fn work(&self, units: usize) -> Result<(), Failure> {
        self.pay::<(
            &Self,
            usize,
            &Cell<usize>,
            usize,
            Option<usize>,
            usize,
            Result<(), Failure>,
        )>()?;
        let Some(next) = self.remaining.get().checked_sub(units) else {
            self.remaining.set(0);
            return Err(Failure::Work);
        };
        self.remaining.set(next);
        Ok(())
    }

    pub(super) fn bytes(&self) -> usize {
        self.bytes.get()
    }
}

pub(super) type Acquire<'p, 'a, 'b> = (
    Context<'p, 'a>,
    Context<'p, 'a>,
    &'b Invoice,
    &'b Invoice,
    Certificate<'p, 'a, 'b>,
    Certificate<'p, 'a, 'b>,
    Result<Certificate<'p, 'a, 'b>, Failure>,
    usize,
    &'p Vec<Function>,
    usize,
    bool,
    &'p Vec<crate::ast::InterfaceDeclaration>,
    bool,
    // Other unsupported program surfaces are borrowed in place; no cloning.
    &'p Program,
    usize,
    bool,
);
pub(super) type Consume<'p, 'a, 'b> = (
    Certificate<'p, 'a, 'b>,
    Certificate<'p, 'a, 'b>,
    Context<'p, 'a>,
    Context<'p, 'a>,
    &'b Invoice,
    &'b Invoice,
    &'p Program,
    &'p Program,
    &'a Index<'a>,
    &'a Index<'a>,
    &'a [Program],
    &'a [Program],
    bool,
    bool,
    bool,
    bool,
    Costs,
    Costs,
    Result<Costs, Failure>,
);
pub(super) type Header<'a> = (
    &'a Function,
    &'a Function,
    &'a str,
    &'a str,
    &'a Headers<'a>,
    &'a Headers<'a>,
    Option<&'a Option<&'a Function>>,
    Option<&'a Option<&'a Function>>,
    Option<&'a Function>,
    Option<&'a Function>,
    bool,
    &'a mut Headers<'a>,
    &'a mut Headers<'a>,
    Option<Option<&'a Function>>,
    Option<Option<&'a Function>>,
    (),
    (),
);
pub(super) type Alias<'a> = (
    &'a crate::ast::ModuleUse,
    &'a crate::ast::ModuleUse,
    &'a mut Headers<'a>,
    &'a mut Headers<'a>,
    &'a str,
    &'a str,
    Option<&'a mut Option<&'a Function>>,
    Option<&'a mut Option<&'a Function>>,
    &'a mut Option<&'a Function>,
    Option<&'a Function>,
    bool,
);
pub(super) type FunctionPass<'a> = (
    &'a crate::ast::Function,
    &'a crate::ast::Function,
    &'a Vec<crate::ast::Param>,
    usize,
    usize,
    bool,
    bool,
    usize,
    &'a Program,
    &'a [Program],
    &'a Index<'a>,
    Option<&'a AuthoredDeclaration<'a>>,
    &'a AuthoredDeclaration<'a>,
    Option<&'a crate::ast::Function>,
    &'a crate::ast::Function,
    bool,
    bool,
    bool,
    bool,
    Option<usize>,
    Result<Option<usize>, Failure>,
);
pub(super) type Query<'a> = (
    &'a Headers<'a>,
    &'a Headers<'a>,
    &'a str,
    &'a str,
    Option<&'a Option<&'a crate::ast::Function>>,
    Option<&'a Option<&'a crate::ast::Function>>,
    &'a crate::ast::Function,
    &'a crate::ast::Function,
    &'a Vec<crate::ast::Param>,
    usize,
    usize,
    bool,
    bool,
    &'a Index<'a>,
    &'a Index<'a>,
    &'a str,
    &'a str,
    Option<&'a AuthoredDeclaration<'a>>,
    &'a AuthoredDeclaration<'a>,
    &'a Program,
    &'a [Program],
    usize,
    bool,
    bool,
    bool,
    bool,
);

// Separate helper entries and terminal iterator advances, including empty banks.
pub(super) type Ordinary<'a> = (
    &'a Function,
    &'a Function,
    &'a Invoice,
    &'a Invoice,
    &'a Type,
    &'a Type,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    &'a Vec<crate::ast::Param>,
    std::slice::Iter<'a, crate::ast::Param>,
    Result<bool, Failure>,
    Result<bool, Failure>,
);
pub(super) type Parameter<'a> = (
    &'a mut std::slice::Iter<'a, crate::ast::Param>,
    &'a mut std::slice::Iter<'a, crate::ast::Param>,
    Option<&'a crate::ast::Param>,
    Option<&'a crate::ast::Param>,
    &'a crate::ast::Param,
    &'a Type,
    &'a Type,
    bool,
    bool,
);
pub(super) type Local<'p, 'a> = (
    Context<'p, 'a>,
    Context<'p, 'a>,
    &'p Function,
    &'p Function,
    &'a Invoice,
    &'a Invoice,
    std::slice::Iter<'a, Program>,
    bool,
    bool,
    &'a Index<'a>,
    &'a Index<'a>,
    &'p str,
    &'p str,
    Option<&'a AuthoredDeclaration<'a>>,
    Option<&'a AuthoredDeclaration<'a>>,
    &'a AuthoredDeclaration<'a>,
    Option<&'a Function>,
    &'a Function,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    bool,
    bool,
    bool,
    bool,
    Result<bool, Failure>,
    Result<bool, Failure>,
);
pub(super) type Provider<'a> = (
    &'a mut std::slice::Iter<'a, Program>,
    &'a mut std::slice::Iter<'a, Program>,
    Option<&'a Program>,
    Option<&'a Program>,
    &'a Program,
    &'a Program,
    &'a Program,
    bool,
    bool,
);

pub(super) type Advance<'a, T> = (
    &'a mut std::slice::Iter<'a, T>,
    &'a mut std::slice::Iter<'a, T>,
    Option<&'a T>,
    Option<&'a T>,
    &'a T,
    bool,
);
// Each lookup keeps distinct producer return, is_some receiver and moved
// operand slots. Reserve the closed full dispatch bank even after an early hit.
type CompilerResults = (
    Option<crate::stdin_stream_ops::PureOp>,
    Option<crate::string_ops::StringOp>,
    Option<crate::str_ops::StrOp>,
    Option<crate::vec_ops::VecOp>,
    Option<crate::box_ops::BoxOp>,
    Option<crate::iterator_ops::IteratorOp>,
    Option<crate::list_ops::ListOp>,
    Option<crate::map_ops::MapOp>,
    Option<crate::byte_ops::ByteOp>,
    Option<crate::host_io_ops::HostIoOp>,
    Option<crate::hir::ResolvedHostCommandOperation>,
);
pub(super) type Compiler<'a> = (
    &'a str,
    &'a str,
    &'a Invoice,
    &'a Invoice,
    [(&'a str, &'a str, bool); 17],
    CompilerResults,
    CompilerResults,
    CompilerResults,
    Result<bool, Failure>,
    Result<bool, Failure>,
);
