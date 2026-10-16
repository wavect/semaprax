//! Invocation-owned immutable function presentations on the actual byte owner.
//! This is storage acquisition, not a semantic or work-admission capability.
use super::*;
use crate::ast::program_source::{FunctionRef, ProgramSource};
use crate::ast::{Function, TypeDeclarationKind};
use std::alloc::Layout;

type Cell<'p> = crate::ast::program_source::FunctionCell<'p>;
impl IdentityReservation {
    pub(crate) fn function_cells<'p>(
        &mut self,
        program: &'p dyn ProgramSource,
    ) -> Result<Vec<Cell<'p>>, Refused> {
        self.reserve(
            size_of::<Vec<Cell<'p>>>()
                + size_of::<Result<Vec<Cell<'p>>, Refused>>()
                + size_of::<Layout>()
                + size_of::<Result<Layout, std::alloc::LayoutError>>()
                + size_of::<usize>()
                + size_of::<Option<usize>>()
                + size_of::<crate::ast::program_source::FunctionSlices<'p>>()
                + size_of::<&dyn ProgramSource>(),
        )?;
        let mut count = program.function_refs().len();
        for declaration in program.types() {
            self.reserve(
                size_of::<&crate::ast::TypeDeclaration>()
                    + size_of::<Option<&crate::ast::TypeDeclaration>>()
                    + size_of::<Option<usize>>()
                    + size_of::<usize>(),
            )?;
            if let TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
                count = count.checked_add(methods.len()).ok_or_else(|| {
                    poison(&self.budget, usize::MAX);
                    Refused::Arithmetic
                })?;
            }
        }
        let layout = Layout::array::<Cell<'p>>(count).map_err(|_| {
            poison(&self.budget, usize::MAX);
            Refused::Arithmetic
        })?;
        self.reserve(layout.size())?;
        let mut cells = Vec::new();
        cells.try_reserve_exact(count).map_err(|_| {
            poison(&self.budget, 0);
            Refused::Allocation
        })?;
        if cells.capacity() != count {
            let excess = cells
                .capacity()
                .saturating_sub(count)
                .checked_mul(size_of::<Cell<'p>>())
                .ok_or_else(|| {
                    poison(&self.budget, usize::MAX);
                    Refused::Arithmetic
                })?;
            self.reserve(excess)?;
            poison(&self.budget, 0);
            return Err(Refused::Capacity);
        }
        // No later reserve, growth, sorting, replacement or mutation is allowed
        // after this inventory begins lending cells to declaration maps.
        self.reserve(
            size_of::<crate::ast::program_source::FunctionSlices<'p>>()
                + size_of::<
                    crate::ast::program_source::function_alias::presentation::FunctionIter<'p>,
                >(),
        )?;
        let mut source = program.function_refs().iter();
        loop {
            // Both next's return and its receiving local are prepaid, including
            // the final None. Reserve before asking the iterator for a view.
            self.reserve(
                size_of::<FunctionRef<'p>>() // constructor/view value
                    + size_of::<FunctionRef<'p>>() // receiving binding
                    + size_of::<Cell<'p>>() // pushed tuple operand
                    + size_of::<Option<FunctionRef<'p>>>() // get return
                    + size_of::<Option<FunctionRef<'p>>>() // next's local
                    + size_of::<Option<FunctionRef<'p>>>() // next return
                    + size_of::<Option<FunctionRef<'p>>>(), // let-Some operand
            )?;
            let Some(function) = source.next() else {
                break;
            };
            cells.push((None, function));
        }
        for declaration in program.types() {
            if let TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
                self.reserve(
                    size_of::<std::slice::Iter<'p, Function>>()
                        + size_of::<Option<&'p crate::ast::TypeDeclaration>>(),
                )?;
                for method in methods {
                    self.reserve(
                        size_of::<FunctionRef<'p>>()
                            + size_of::<FunctionRef<'p>>()
                            + size_of::<Cell<'p>>()
                            + size_of::<Option<&Function>>()
                            + size_of::<&Function>(),
                    )?;
                    cells.push((Some(declaration), FunctionRef::ordinary(method)));
                }
            }
        }
        Ok(cells)
    }
}
