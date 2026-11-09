//! Reuse immutable directory DATA while a separate callee certificate is live.
use super::super::super::shared_local_callees::{Context, Costs, Failure};
use super::super::joint::modular_census as joint;
use super::{
    size_of, Directory, Header, ImportDeclaration, Index, InterfaceDeclaration, Program, Proof,
    Source, SELECTOR,
};
type Error = Failure;

#[allow(
    clippy::manual_range_contains,
    reason = "keep explicitly invoiced scalar comparisons"
)]
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

#[cfg(any())]
pub(in crate::workspace_graph::expected_projection) fn indexed_loan<const N: usize>(
    context: Context<'_, '_>,
    imports: bool,
    name: &str,
    arity: usize,
) -> (bool, bool) {
    let mut proof = Proof::default();
    let mut directory = Directory {
        rows: [Header::EMPTY; N],
        len: 0,
        last: None,
    };
    directory.populate(context.program, imports, &mut proof);
    directory.sort(&mut proof);
    let loan = super::super::joint::Lookup::target(
        &mut directory,
        context.authored,
        name,
        arity,
        &mut proof,
    );
    (loan.auxiliary, loan.local.is_some())
}
