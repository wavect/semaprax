//! Invocation-local nested byte proof. References bind this census to one
//! immutable function; shape discovery is distinct from late provenance replay.
use super::{DeclarationIndex, Diagnostic, Region, ResolvedExpr, ResolvedFunction};
#[cfg(test)]
use crate::hir::{
    ControlTarget, ExpressionId, FunctionExecutionId, ResolvedExprKind, ResolvedProgram,
    ResolvedStatement,
};
mod check;
mod index;
mod intervals;
mod shape;
pub(crate) mod slot;
mod storage;
pub(crate) use slot::Slot;
mod walk;
use shape::Shape;
use storage::{Refused, Storage};
use walk::{Event, Walker};

pub(super) enum Failure {
    Semantic(Diagnostic),
    Storage(Refused),
}
impl From<Refused> for Failure {
    fn from(value: Refused) -> Self {
        Self::Storage(value)
    }
}
impl From<Diagnostic> for Failure {
    fn from(value: Diagnostic) -> Self {
        Self::Semantic(value)
    }
}
impl Failure {
    fn semantic(message: &'static str) -> Self {
        Self::Semantic(Diagnostic::io_static("SPX-H006", message))
    }
    fn diagnostic(self) -> Diagnostic {
        match self {
            Self::Semantic(error) => error,
            Self::Storage(Refused::Detached) => Diagnostic::io_static(
                "SPX-H006",
                "borrowed byte traversal census owner is detached",
            ),
            Self::Storage(_) => Diagnostic::io_static(
                "SPX-H006",
                "borrowed byte traversal census storage reservation refused",
            ),
        }
    }
}
#[derive(Clone, Copy)]
struct Row<'a> {
    shape: Shape<'a>,
    start: usize,
    end: usize,
    body_start: usize,
    body_end: usize,
}
/// No Clone/serialization: authority is lent only for these exact immutable
/// expression references, never recovered from an ID or an attached marker.
pub(crate) struct Census<'a> {
    context: Option<&'a dyn crate::cleanup::ReplayContext>,
    function: &'a ResolvedFunction,
    rows: Vec<Row<'a>>,
    storage: Storage,
}
impl<'a> Census<'a> {
    pub(crate) fn acquire(
        context: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
    ) -> Result<Self, Diagnostic> {
        slot::controls(0)?;
        if !context
            .functions()
            .iter()
            .any(|member| std::ptr::eq(member, function))
            && !context
                .function_instances()
                .iter()
                .any(|member| std::ptr::eq(&member.function, function))
        {
            return Err(Failure::semantic(
                "borrowed byte traversal function is not a current context member",
            )
            .diagnostic());
        }
        slot::controls(std::mem::size_of::<Result<Self, Diagnostic>>())?;
        let mut census = Self::build(function, None).map_err(Failure::diagnostic)?;
        census.context = Some(context);
        Ok(census)
    }
    pub(super) fn validate(
        declarations: &DeclarationIndex,
        function: &'a ResolvedFunction,
    ) -> Result<(), Diagnostic> {
        Self::build(function, Some(declarations))
            .map(|_| ())
            .map_err(Failure::diagnostic)
    }
    fn build(
        function: &'a ResolvedFunction,
        declarations: Option<&DeclarationIndex>,
    ) -> Result<Self, Failure> {
        let storage = Storage::acquire()?;
        storage
            .reserve(std::mem::size_of::<Self>() + std::mem::size_of::<Result<Self, Failure>>())?;
        let mut walker = Walker::acquire(&storage)?;
        let index = index::Index::build(function, &storage, &mut walker)?;
        let mut rows = storage.vector(index.regions)?;
        walker.function(function, |event| {
            if let Event::Enter { expression, .. } = event {
                if let Some(shape) = shape::shape(expression)? {
                    let node = index.node(expression)?;
                    let body = index.node(shape.authored_body)?;
                    storage.push(
                        &mut rows,
                        Row {
                            shape,
                            start: node.start,
                            end: node.end,
                            body_start: body.start,
                            body_end: body.end,
                        },
                    )?;
                }
            }
            Ok(())
        })?;
        check::validate(function, &rows, &index, declarations, &storage, &mut walker)?;
        rows.sort_unstable_by(|a, b| a.shape.root.id.cmp(&b.shape.root.id));
        Ok(Self {
            context: None,
            function,
            rows,
            storage,
        })
    }
    pub(crate) fn region(
        &self,
        context: &dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        expression: &'a ResolvedExpr,
    ) -> Result<Option<Region<'a>>, Diagnostic> {
        self.storage
            .reserve(0)
            .map_err(|error| Failure::Storage(error).diagnostic())?;
        if !self
            .context
            .is_some_and(|held| std::ptr::addr_eq(held, context))
            || !std::ptr::eq(self.function, function)
        {
            return Err(Failure::semantic(
                "borrowed byte traversal census function differs from its current receiver",
            )
            .diagnostic());
        }
        // The private rows are ordered by exact function preorder. Expression
        // IDs are not authority: every selected row must retain pointer identity.
        // Public callers enumerate the same CFG expressions; lookup is indexed
        // by a private sorted association rather than retaining owned HIR copies.
        let row = self
            .rows
            .binary_search_by(|row| row.shape.root.id.cmp(&expression.id))
            .ok()
            .map(|index| &self.rows[index])
            .filter(|row| std::ptr::eq(row.shape.root, expression));
        if row.is_none() && shape::shape(expression)?.is_some() {
            return Err(Failure::semantic(
                "borrowed byte traversal region is not a current census member",
            )
            .diagnostic());
        }
        Ok(row.map(|row| Region {
            receiver: row.shape.receiver,
            authored_body: row.shape.authored_body,
            view: row.shape.view,
            payload: row.shape.payload,
        }))
    }
}
#[cfg(test)]
mod tests;
