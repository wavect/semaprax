//! One declaration's borrowed sorted ordinals: no payload clone or authority.
//! Small lists stay inline; large lists acquire exact backing before sorting.
use crate::ast::TypeParameterDeclaration;
use crate::diagnostic::Diagnostic;

const INLINE: usize = 16;
enum Ordinals {
    Inline { values: [usize; INLINE], len: usize },
    Heap(Vec<usize>),
}
impl Ordinals {
    fn values(&self) -> &[usize] {
        match self {
            Self::Inline { values, len } => &values[..*len],
            Self::Heap(values) => values,
        }
    }
    fn values_mut(&mut self) -> &mut [usize] {
        match self {
            Self::Inline { values, len } => &mut values[..*len],
            Self::Heap(values) => values,
        }
    }
}
pub(super) struct BorrowedNames<'a> {
    parameters: &'a [TypeParameterDeclaration],
    ordinals: Ordinals,
    #[cfg(test)]
    comparisons: std::cell::Cell<usize>,
}
fn refused(requested: usize) -> Vec<Diagnostic> {
    crate::bounded_output::refuse_active_required(requested);
    vec![crate::workspace_graph::graph_error(
        "SPX-G171",
        "generic parameter name index exceeds its actual backing reservation",
    )]
}
impl<'a> BorrowedNames<'a> {
    pub(super) fn new(parameters: &'a [TypeParameterDeclaration]) -> Result<Self, Vec<Diagnostic>> {
        if !crate::bounded_output::reserve_active_required(0) {
            return Err(refused(0));
        }
        let count = parameters.len();
        let ordinals = if count <= INLINE {
            let mut values = [0; INLINE];
            for (ordinal, value) in values[..count].iter_mut().enumerate() {
                *value = ordinal;
            }
            Ordinals::Inline { values, len: count }
        } else {
            let payload = count
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or_else(|| refused(usize::MAX))?;
            let bytes = std::mem::size_of::<Self>()
                .checked_add(payload)
                .ok_or_else(|| refused(usize::MAX))?;
            if !crate::bounded_output::reserve_active_required(bytes) {
                return Err(refused(bytes));
            }
            let mut values = Vec::new();
            values
                .try_reserve_exact(count)
                .map_err(|_| refused(payload))?;
            if values.capacity() != count {
                return Err(refused(payload));
            }
            values.extend(0..count);
            Ordinals::Heap(values)
        };
        let mut result = Self {
            parameters,
            ordinals,
            #[cfg(test)]
            comparisons: std::cell::Cell::new(0),
        };
        #[cfg(test)]
        let comparisons = &result.comparisons;
        result
            .ordinals
            .values_mut()
            .sort_unstable_by(|left, right| {
                #[cfg(test)]
                comparisons.set(comparisons.get() + 1);
                parameters[*left].name.cmp(&parameters[*right].name)
            });
        Ok(result)
    }
    pub(super) fn contains(&self, name: &str) -> bool {
        self.ordinals
            .values()
            .binary_search_by(|ordinal| {
                #[cfg(test)]
                self.comparisons.set(self.comparisons.get() + 1);
                self.parameters[*ordinal].name.as_str().cmp(name)
            })
            .is_ok()
    }
}
#[cfg(test)]
#[path = "parameter_names/tests.rs"]
mod tests;

#[path = "parameter_names/receiving.rs"]
pub(super) mod receiving;
