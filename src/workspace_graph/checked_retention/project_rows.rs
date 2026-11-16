//! Move the three authored HIR selections into the project projection.
use super::{graph_error, hir, CheckedModuleLease, Diagnostic, Retained, Selection, View};
use std::{mem::size_of, ops::Range};

type Types = hir::ResolvedTypeDeclaration;
type Functions = hir::ResolvedFunction;
type Templates = hir::ResolvedFunctionTemplate;
type Owned = (Vec<Types>, Vec<Functions>, Vec<Templates>);
type Views<'a> = (View<'a, Types>, View<'a, Functions>, View<'a, Templates>);

pub(in crate::workspace_graph) struct ProjectRows(Rows);

enum Rows {
    Owned(Owned),
    Checked(CheckedModuleLease, Selections),
}

// The original Project receiver pays for three Vec headers. Share one lease
// and use a single discriminant for the three index shapes within that space.
const _: () = assert!(size_of::<ProjectRows>() == size_of::<Owned>());

enum Selections {
    Ranges(Range<usize>, Range<usize>, Range<usize>),
    Types(Box<[usize]>, Range<usize>, Range<usize>),
    Functions(Range<usize>, Box<[usize]>, Range<usize>),
    Templates(Range<usize>, Range<usize>, Box<[usize]>),
    TypesFunctions(Box<[usize]>, Box<[usize]>, Range<usize>),
    TypesTemplates(Box<[usize]>, Range<usize>, Box<[usize]>),
    FunctionsTemplates(Range<usize>, Box<[usize]>, Box<[usize]>),
    Ordinals(Box<[usize]>, Box<[usize]>, Box<[usize]>),
}

impl ProjectRows {
    pub(in crate::workspace_graph) fn empty() -> Self {
        Self(Rows::Owned((Vec::new(), Vec::new(), Vec::new())))
    }

    pub(in crate::workspace_graph) fn new(
        types: Retained<Types>,
        functions: Retained<Functions>,
        templates: Retained<Templates>,
    ) -> Result<Self, Vec<Diagnostic>> {
        match (types, functions, templates) {
            (Retained::Owned(types), Retained::Owned(functions), Retained::Owned(templates)) => {
                Ok(Self(Rows::Owned((types, functions, templates))))
            }
            (
                Retained::Checked {
                    owner,
                    selection: types,
                    ..
                },
                Retained::Checked {
                    owner: functions_owner,
                    selection: functions,
                    ..
                },
                Retained::Checked {
                    owner: templates_owner,
                    selection: templates,
                    ..
                },
            ) if owner.same_backing(&functions_owner) && owner.same_backing(&templates_owner) => {
                Ok(Self(Rows::Checked(
                    owner,
                    Selections::new(types, functions, templates),
                )))
            }
            _ => Err(vec![graph_error(
                "SPX-G173",
                "project HIR selections disagree on their workspace owner",
            )]),
        }
    }

    pub(in crate::workspace_graph) fn views(&self) -> Views<'_> {
        match &self.0 {
            Rows::Owned((types, functions, templates)) => (
                View::slice(types),
                View::slice(functions),
                View::slice(templates),
            ),
            Rows::Checked(owner, selections) => selections.views(owner),
        }
    }
}

impl Selections {
    fn new(types: Selection, functions: Selection, templates: Selection) -> Self {
        use Selection::{Ordinals, Range};
        match (types, functions, templates) {
            (Range(types), Range(functions), Range(templates)) => {
                Self::Ranges(types, functions, templates)
            }
            (Ordinals(types), Range(functions), Range(templates)) => {
                Self::Types(types, functions, templates)
            }
            (Range(types), Ordinals(functions), Range(templates)) => {
                Self::Functions(types, functions, templates)
            }
            (Range(types), Range(functions), Ordinals(templates)) => {
                Self::Templates(types, functions, templates)
            }
            (Ordinals(types), Ordinals(functions), Range(templates)) => {
                Self::TypesFunctions(types, functions, templates)
            }
            (Ordinals(types), Range(functions), Ordinals(templates)) => {
                Self::TypesTemplates(types, functions, templates)
            }
            (Range(types), Ordinals(functions), Ordinals(templates)) => {
                Self::FunctionsTemplates(types, functions, templates)
            }
            (Ordinals(types), Ordinals(functions), Ordinals(templates)) => {
                Self::Ordinals(types, functions, templates)
            }
        }
    }

    fn views<'a>(&'a self, owner: &'a hir::ResolvedProgram) -> Views<'a> {
        let types = &owner.types;
        let functions = &owner.functions;
        let templates = &owner.function_templates;
        match self {
            Self::Ranges(t, f, p) => (range(types, t), range(functions, f), range(templates, p)),
            Self::Types(t, f, p) => (
                View::indexed(types, t),
                range(functions, f),
                range(templates, p),
            ),
            Self::Functions(t, f, p) => (
                range(types, t),
                View::indexed(functions, f),
                range(templates, p),
            ),
            Self::Templates(t, f, p) => (
                range(types, t),
                range(functions, f),
                View::indexed(templates, p),
            ),
            Self::TypesFunctions(t, f, p) => (
                View::indexed(types, t),
                View::indexed(functions, f),
                range(templates, p),
            ),
            Self::TypesTemplates(t, f, p) => (
                View::indexed(types, t),
                range(functions, f),
                View::indexed(templates, p),
            ),
            Self::FunctionsTemplates(t, f, p) => (
                range(types, t),
                View::indexed(functions, f),
                View::indexed(templates, p),
            ),
            Self::Ordinals(t, f, p) => (
                View::indexed(types, t),
                View::indexed(functions, f),
                View::indexed(templates, p),
            ),
        }
    }
}

fn range<'a, T>(source: &'a [T], indices: &Range<usize>) -> View<'a, T> {
    View::slice(&source[indices.clone()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::incremental::checked_hir::tests::cold;

    #[test]
    fn every_project_selection_shape_moves_without_allocating_and_keeps_ordinal_order() {
        for sparse in 0..8 {
            let (cache, _, owner) = cold();
            let types = Retained::Checked {
                owner: owner.clone(),
                selection: if sparse & 1 == 0 {
                    Selection::Range(0..0)
                } else {
                    Selection::Ordinals(Box::default())
                },
                read: |program| &program.types,
            };
            let functions = Retained::Checked {
                owner: owner.clone(),
                selection: if sparse & 2 == 0 {
                    Selection::Range(0..2)
                } else {
                    Selection::Ordinals(Box::new([0, 2]))
                },
                read: |program| &program.functions,
            };
            let templates = Retained::Checked {
                owner: owner.clone(),
                selection: if sparse & 4 == 0 {
                    Selection::Range(0..0)
                } else {
                    Selection::Ordinals(Box::default())
                },
                read: |program| &program.function_templates,
            };
            let source = owner.functions.as_ptr();
            let (rows, allocation) = crate::test_allocation_observer::measure(|| {
                ProjectRows::new(types, functions, templates).unwrap()
            });
            assert_eq!((allocation.allocated, allocation.freed), (0, 0));
            assert!(!allocation.invalid);
            drop(cache);
            drop(owner);
            let (types, functions, templates) = rows.views();
            assert_eq!(types.len(), 0);
            assert_eq!(templates.len(), 0);
            assert_eq!(functions.len(), 2);
            assert_eq!(functions.get(0).unwrap() as *const _, source);
            let second = if sparse & 2 == 0 {
                "local.middle"
            } else {
                "local.last"
            };
            assert_eq!(functions.get(1).unwrap().id.as_str(), second);
            assert!(functions.get(2).is_none());
        }
    }

    #[test]
    fn unequal_owners_do_not_coalesce_even_with_equal_checked_hir() {
        let (_, _, left) = cold();
        let (_, _, right) = cold();
        assert_eq!(*left, *right);
        let result = ProjectRows::new(
            Retained::select(left.clone(), |program| &program.types, |_| true).unwrap(),
            Retained::select(right, |program| &program.functions, |_| true).unwrap(),
            Retained::select(left, |program| &program.function_templates, |_| true).unwrap(),
        );
        let Err(diagnostics) = result else {
            panic!("different allocations cannot own the same grouped selection");
        };
        assert_eq!(diagnostics[0].code, "SPX-G173");
    }
}
