//! Reuse immutable directory DATA while a separate callee certificate is live.
use super::super::super::shared_local_callees::{Context, Costs, Failure};
use super::super::joint::{self, Loan, Lookup};
use super::*;
type Error = Failure;

pub(in crate::workspace_graph::expected_projection) fn count(
    context: Context<'_, '_>,
    imports: bool,
) -> Result<(Proof, Costs), Error> {
    let program = context.program;
    let mut selector = Proof {
        roots: 0,
        bytes: SELECTOR.saturating_add(
            2 * size_of::<(Context<'_, '_>, bool, Result<(Proof, Costs), Error>)>(),
        ),
    };
    let mut length = program.functions.len();
    if imports {
        length = length.saturating_add(program.module_uses.len());
    }
    let mut interfaces = program.interfaces.iter();
    loop {
        bill!(selector; 2 * &mut std::slice::Iter<'_, InterfaceDeclaration>);
        bill!(selector; (Option<&InterfaceDeclaration>, &InterfaceDeclaration, &Vec<ImportDeclaration>, &Vec<ImportDeclaration>, usize, usize, usize));
        let Some(interface) = interfaces.next() else {
            break;
        };
        length = length.saturating_add(interface.imports.len());
    }
    // The separate monomorphized frames retain only the selected capacity.
    // Small and over-capacity programs keep the complete original scan; the
    // selector is still honestly added, never refunded or described as free.
    if length >= 64 && length <= 128 {
        bill!(selector; 2 * (&Program, &Index<'_>, bool, usize, Proof));
        indexed::<128>(context, imports, selector.bytes)
    } else if length >= 129 && length <= 512 {
        bill!(selector; 2 * (&Program, &Index<'_>, bool, usize, Proof));
        indexed::<512>(context, imports, selector.bytes)
    } else {
        joint::linear(context, imports, selector.bytes)
    }
}

impl<'p, const N: usize> Lookup<'p> for Directory<'p, N> {
    fn target(
        &mut self,
        authored: &Index<'_>,
        name: &str,
        arity: usize,
        proof: &mut Proof,
    ) -> Loan<'p> {
        #[cfg(test)]
        note(Event::Query);
        if compiler_dispatch(name, proof) {
            return Loan::NONE;
        }
        // Last position is raw immutable-header DATA only. It cannot bypass
        // Names, multiplicity or any selected target check below. No checked
        // call result, arity verdict or provider association is retained.
        bill!(proof; (Option<usize>, Option<usize>, usize, bool));
        let mut found = None;
        if let Some(last) = self.last {
            bill!(proof; usize);
            bill!(proof; 2 * (&[Header<'_>; N], usize, &Header<'_>));
            bill!(proof; 4 * &str);
            bill!(proof; 2 * bool);
            if str::eq(self.rows[last].name, name) {
                bill!(proof; Option<usize>);
                found = Some(last);
                #[cfg(test)]
                note(Event::LastHit);
            }
        }
        bill!(proof; (Option<usize>, usize, usize));
        let lower = match found {
            Some(position) => position,
            None => {
                bill!(proof; 2 * (&Self, &str, &mut Proof, Option<usize>));
                bill!(proof; (Option<usize>, usize));
                let Some(position) = self.find(name, proof) else {
                    return Loan::NONE;
                };
                bill!(proof; Option<usize>);
                self.last = Some(position);
                position
            }
        };
        bill!(proof; (usize, usize, bool));
        if lower + 1 < self.len {
            bill!(proof; 2 * (&[Header<'_>; N], usize, &Header<'_>));
            bill!(proof; 4 * &str);
            bill!(proof; 2 * bool);
            if str::eq(self.rows[lower + 1].name, name) {
                return Loan::NONE;
            }
        }
        bill!(proof; 2 * (&[Header<'_>; N], usize, &Header<'_>));
        bill!(proof; &Source<'_>);
        // A unique raw header is not a checked target. Preserve all original
        // live provider/identity/generic/arity predicates here, per query.
        bill!(proof; 2 * Loan<'_>);
        let local = match &self.rows[lower].source {
            Source::Function(function) => Some(*function),
            _ => None,
        };
        let function = match &self.rows[lower].source {
            Source::Empty => return Loan::NONE,
            Source::Interface(import) => {
                proof.charge(import);
                return Loan::NONE;
            }
            Source::Function(function) => {
                bill!(proof; (&&Function, &Function));
                *function
            }
            Source::Use(item) => {
                bill!(proof; &&ModuleUse);
                if item.kind != ModuleUseKind::Function {
                    return Loan::NONE;
                }
                bill!(proof; 2 * (&Index<'_>, &str, Option<&AuthoredDeclaration<'_>>));
                let Some(header) = authored.get(item.persistent_id.as_str()) else {
                    return Loan::NONE;
                };
                proof.charge(&header);
                if header.module != item.target_module {
                    return Loan::NONE;
                }
                let Some(function) = header.function else {
                    return Loan::NONE;
                };
                proof.charge(&function);
                if function.stable_id != item.persistent_id {
                    return Loan::NONE;
                }
                function
            }
        };
        bill!(proof; &Function);
        proof.charge(&function);
        let admitted = function.type_parameters.is_empty() && function.params.len() == arity;
        proof.charge(&admitted);
        Loan {
            auxiliary: admitted,
            local: if admitted { local } else { None },
        }
    }
}

fn indexed<const N: usize>(
    context: Context<'_, '_>,
    imports: bool,
    selector: usize,
) -> Result<(Proof, Costs), Error> {
    let program = context.program;
    let mut proof = Proof {
        roots: 0,
        bytes: selector,
    };
    bill!(proof; 2 * Directory<'_, N>);
    bill!(proof; ([Header<'_>; N], Header<'_>, Source<'_>, &Program, &Index<'_>, bool, usize, Option<usize>));
    let mut directory = Directory {
        rows: [Header::EMPTY; N],
        len: 0,
        last: None,
    };
    bill!(proof; 2 * (&mut Directory<'_, N>, &Program, bool, &mut Proof));
    directory.populate(program, imports, &mut proof);
    bill!(proof; 2 * (&mut Directory<'_, N>, &mut Proof));
    directory.sort(&mut proof);
    joint::count_with_lookup(context, imports, proof, &mut directory)
}
