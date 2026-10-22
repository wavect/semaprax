//! Immutable header DATA, not checked-call authority. Every query still checks
//! the exact selected import's provider/ID/header and full spelling equality.
use super::{AuthoredDeclaration, Names, Proof, call_identity, compiler_dispatch};
use crate::ast::{Expr, ExprKind, ModuleUseKind, Program, Statement};
use crate::ast::{Function, ImportDeclaration, InterfaceDeclaration, ModuleUse};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::mem::size_of;
type Index<'a> = BTreeMap<&'a str, AuthoredDeclaration<'a>>;

// This is structural input accounting, like the original Proof::charge. The
// census helpers are not a second replay budget or a reservation/authority API.
macro_rules! bill {
    ($proof:ident; $n:literal * $ty:ty) => {
        $proof.bytes = $proof.bytes.saturating_add($n * size_of::<$ty>());
    };
    ($proof:ident; $ty:ty) => {
        $proof.bytes = $proof.bytes.saturating_add(size_of::<$ty>());
    };
}
#[derive(Clone, Copy)]
enum Source<'a> {
    Empty,
    Function(&'a Function),
    Interface(&'a ImportDeclaration),
    Use(&'a ModuleUse),
}
#[derive(Clone, Copy)]
struct Header<'a> {
    name: &'a str,
    source: Source<'a>,
}
impl Header<'_> {
    const EMPTY: Self = Self {
        name: "",
        source: Source::Empty,
    };
}
struct Directory<'a, const N: usize> {
    rows: [Header<'a>; N],
    len: usize,
    last: Option<usize>,
}
// Full new wrapper/formal/return slots and the two Vec::len boundaries.
// The old count and all of its original invoice remain complete on fallback.
pub(super) const SELECTOR: usize = 2 * size_of::<(&Program, &Index<'_>, bool, Proof)>()
    + size_of::<Proof>()
    + 4 * size_of::<usize>()
    + 4 * size_of::<bool>()
    + 2 * size_of::<(&Vec<Function>, usize)>()
    + 2 * size_of::<(&Vec<ModuleUse>, usize)>()
    + 2 * size_of::<&Vec<InterfaceDeclaration>>()
    + 4 * size_of::<&[InterfaceDeclaration]>()
    + 2 * size_of::<std::slice::Iter<'_, InterfaceDeclaration>>();

#[cfg(test)]
pub(super) const EMPTY_SELECTOR: usize = SELECTOR
    + 2 * size_of::<&mut std::slice::Iter<'_, InterfaceDeclaration>>()
    + size_of::<(
        Option<&InterfaceDeclaration>,
        &InterfaceDeclaration,
        &Vec<ImportDeclaration>,
        &Vec<ImportDeclaration>,
        usize,
        usize,
        usize,
    )>();

#[allow(
    clippy::manual_range_contains,
    reason = "keep the explicitly invoiced scalar comparisons"
)]
pub(in crate::workspace_graph::expected_projection) fn count(
    program: &Program,
    authored: &Index<'_>,
    imports: bool,
) -> Proof {
    let mut selector = Proof {
        roots: 0,
        bytes: SELECTOR,
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
        indexed::<128>(program, authored, imports, selector.bytes)
    } else if length >= 129 && length <= 512 {
        bill!(selector; 2 * (&Program, &Index<'_>, bool, usize, Proof));
        indexed::<512>(program, authored, imports, selector.bytes)
    } else {
        let mut result = super::count_linear(program, authored, imports);
        result.bytes = result.bytes.saturating_add(selector.bytes);
        result
    }
}

impl<'a, const N: usize> Directory<'a, N> {
    fn populate(&mut self, program: &'a Program, imports: bool, proof: &mut Proof) {
        #[cfg(test)]
        note(Event::Build);
        macro_rules! append {
            ($name:expr, $source:expr) => {{
                // Enum/row initializer and array-place mutation are real new
                // values, not donors borrowed from the original loop invoice.
                bill!(proof; (Header<'_>, Source<'_>, ));
                bill!(proof; 2 * (&mut [Header<'_>; N], usize, &mut Header<'_>));
                bill!(proof; 2 * (&String, &str));
                #[cfg(test)]
                note(Event::Row);
                self.rows[self.len] = Header { name: $name, source: $source };
                bill!(proof; usize);
                self.len += 1;
            }};
        }
        bill!(proof; 2 * &Vec<InterfaceDeclaration>);
        bill!(proof; 4 * &[InterfaceDeclaration]);
        bill!(proof; 2 * std::slice::Iter<'_, InterfaceDeclaration>);
        let mut interfaces = program.interfaces.iter();
        loop {
            bill!(proof; 2 * &mut std::slice::Iter<'_, InterfaceDeclaration>);
            bill!(proof; (Option<&InterfaceDeclaration>, &InterfaceDeclaration));
            let Some(interface) = interfaces.next() else {
                break;
            };
            bill!(proof; 2 * &Vec<ImportDeclaration>);
            bill!(proof; 4 * &[ImportDeclaration]);
            bill!(proof; 2 * std::slice::Iter<'_, ImportDeclaration>);
            let mut entries = interface.imports.iter();
            loop {
                bill!(proof; 2 * &mut std::slice::Iter<'_, ImportDeclaration>);
                bill!(proof; (Option<&ImportDeclaration>, &ImportDeclaration));
                let Some(entry) = entries.next() else { break };
                append!(entry.name.as_str(), Source::Interface(entry));
            }
        }
        bill!(proof; 2 * &Vec<Function>);
        bill!(proof; 4 * &[Function]);
        bill!(proof; 2 * std::slice::Iter<'_, Function>);
        let mut functions = program.functions.iter();
        loop {
            bill!(proof; 2 * &mut std::slice::Iter<'_, Function>);
            bill!(proof; (Option<&Function>, &Function));
            let Some(function) = functions.next() else {
                break;
            };
            append!(function.name.as_str(), Source::Function(function));
        }
        if imports {
            bill!(proof; 2 * &Vec<ModuleUse>);
            bill!(proof; 4 * &[ModuleUse]);
            bill!(proof; 2 * std::slice::Iter<'_, ModuleUse>);
            let mut uses = program.module_uses.iter();
            loop {
                bill!(proof; 2 * &mut std::slice::Iter<'_, ModuleUse>);
                bill!(proof; (Option<&ModuleUse>, &ModuleUse));
                let Some(item) = uses.next() else { break };
                append!(item.alias.as_str(), Source::Use(item));
            }
        }
    }
    fn compare(&self, left: usize, right: usize, proof: &mut Proof) -> Ordering {
        bill!(proof; 4 * (&[Header<'_>; N], usize));
        bill!(proof; 4 * &Header<'_>);
        bill!(proof; 4 * &str);
        bill!(proof; 2 * Ordering);
        #[cfg(test)]
        note(Event::SortCompare);
        str::cmp(self.rows[left].name, self.rows[right].name)
    }
    fn swap(&mut self, left: usize, right: usize, proof: &mut Proof) {
        // Explicit Copy moves: one saved row and the two receiving values.
        // No opaque sort/swap primitive or additional backing is introduced.
        bill!(proof; 3 * Header<'_>);
        bill!(proof; 2 * (&[Header<'_>; N], usize, &Header<'_>));
        let saved = self.rows[left];
        bill!(proof; 2 * (&[Header<'_>; N], usize, &Header<'_>));
        bill!(proof; 2 * (&mut [Header<'_>; N], usize, &mut Header<'_>));
        self.rows[left] = self.rows[right];
        bill!(proof; 2 * (&mut [Header<'_>; N], usize, &mut Header<'_>));
        self.rows[right] = saved;
        #[cfg(test)]
        note(Event::Swap);
    }
    fn sift(&mut self, mut root: usize, end: usize, proof: &mut Proof) {
        loop {
            bill!(proof; (usize, usize, usize, bool));
            let mut child = root * 2 + 1;
            if child >= end {
                break;
            }
            bill!(proof; (usize, usize, bool));
            if child + 1 < end {
                bill!(proof; 2 * (&Self, usize, usize, &mut Proof, Ordering));
                bill!(proof; (Ordering, bool, bool));
                bill!(proof; 4 * &Ordering);
                if self.compare(child, child + 1, proof) == Ordering::Less {
                    child += 1
                }
            }
            bill!(proof; 2 * (&Self, usize, usize, &mut Proof, Ordering));
            bill!(proof; (Ordering, bool, bool));
            bill!(proof; 4 * &Ordering);
            if self.compare(root, child, proof) != Ordering::Less {
                break;
            }
            bill!(proof; 2 * (&mut Self, usize, usize, &mut Proof));
            self.swap(root, child, proof);
            root = child;
        }
    }
    fn sort(&mut self, proof: &mut Proof) {
        // Explicit bounded heapsort, with no opaque sorting closure/temp Vec.
        bill!(proof; (usize, usize));
        let mut parent = self.len / 2;
        loop {
            bill!(proof; (usize, usize, bool));
            if parent == 0 {
                break;
            }
            parent -= 1;
            bill!(proof; 2 * (&mut Self, usize, usize, &mut Proof));
            self.sift(parent, self.len, proof);
        }
        let mut end = self.len;
        loop {
            bill!(proof; (usize, usize, bool));
            if end <= 1 {
                break;
            }
            end -= 1;
            bill!(proof; 2 * (&mut Self, usize, usize, &mut Proof));
            self.swap(0, end, proof);
            bill!(proof; 2 * (&mut Self, usize, usize, &mut Proof));
            self.sift(0, end, proof);
        }
    }
    fn find(&self, name: &str, proof: &mut Proof) -> Option<usize> {
        bill!(proof; (usize, usize));
        let mut lower = 0;
        let mut upper = self.len;
        loop {
            bill!(proof; (usize, usize, bool));
            if lower == upper {
                break;
            }
            bill!(proof; (usize, usize, usize));
            let middle = lower + (upper - lower) / 2;
            bill!(proof; 2 * (&[Header<'_>; N], usize, &Header<'_>));
            bill!(proof; 4 * &str);
            bill!(proof; 2 * Ordering);
            #[cfg(test)]
            note(Event::QueryCompare);
            match str::cmp(self.rows[middle].name, name) {
                Ordering::Less => lower = middle + 1,
                _ => upper = middle,
            }
        }
        bill!(proof; (usize, usize, bool));
        if lower == self.len {
            return None;
        }
        bill!(proof; 2 * (&[Header<'_>; N], usize, &Header<'_>));
        bill!(proof; 4 * &str);
        bill!(proof; 2 * bool);
        if !str::eq(self.rows[lower].name, name) {
            return None;
        }
        bill!(proof; (Option<usize>, usize));
        Some(lower)
    }
    fn target(
        &mut self,
        authored: &Index<'_>,
        name: &str,
        arity: usize,
        proof: &mut Proof,
    ) -> bool {
        #[cfg(test)]
        note(Event::Query);
        if compiler_dispatch(name, proof) {
            return false;
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
                    return false;
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
                return false;
            }
        }
        bill!(proof; 2 * (&[Header<'_>; N], usize, &Header<'_>));
        bill!(proof; &Source<'_>);
        // A unique raw header is not a checked target. Preserve all original
        // live provider/identity/generic/arity predicates here, per query.
        let function = match &self.rows[lower].source {
            Source::Empty => return false,
            Source::Interface(import) => {
                proof.charge(import);
                return false;
            }
            Source::Function(function) => {
                bill!(proof; (&&Function, &Function));
                *function
            }
            Source::Use(item) => {
                bill!(proof; &&ModuleUse);
                if item.kind != ModuleUseKind::Function {
                    return false;
                }
                bill!(proof; 2 * (&Index<'_>, &str, Option<&AuthoredDeclaration<'_>>));
                let Some(header) = authored.get(item.persistent_id.as_str()) else {
                    return false;
                };
                proof.charge(&header);
                if header.module != item.target_module {
                    return false;
                }
                let Some(function) = header.function else {
                    return false;
                };
                proof.charge(&function);
                if function.stable_id != item.persistent_id {
                    return false;
                }
                function
            }
        };
        bill!(proof; &Function);
        proof.charge(&function);
        let admitted = function.type_parameters.is_empty() && function.params.len() == arity;
        proof.charge(&admitted);
        admitted
    }
}

fn indexed<const N: usize>(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    imports: bool,
    selector: usize,
) -> Proof {
    let mut proof = Proof {
        roots: 0,
        bytes: selector,
    };
    proof.charge(&(program, authored, imports));
    proof.bytes = proof.bytes.saturating_add(std::mem::size_of::<Proof>());
    // Full directory field/init/caller footprint is recorded before formation.
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
    for function in &program.functions {
        proof.charge(&function);
        // Generic owner bodies can be materialized repeatedly; no discount.
        if !function.type_parameters.is_empty() {
            continue;
        }
        let mut names = Names {
            entries: [None; 128],
            len: 0,
        };
        proof.charge(&names);
        let mut complete = function.params.iter().all(|p| {
            proof.charge(&p);
            names.insert(&p.name, &mut proof)
        });
        for root in call_identity::roots(function) {
            // Fixed walker scratch is retained for this complete pass, including
            // failed proofs. Each visited formal/reference is charged too.
            proof.bytes = proof
                .bytes
                .saturating_add(std::mem::size_of::<[Option<(&Expr, usize)>; 129]>());
            complete &= call_identity::walk(root, &mut |expression| {
                proof.charge(&expression);
                match &expression.kind {
                    ExprKind::Block { statements, .. } => statements.iter().all(|s| match s {
                        Statement::Let { name, .. } | Statement::Assign { name, .. } => {
                            names.insert(name, &mut proof)
                        }
                        Statement::For { item, .. } | Statement::ForOwn { item, .. } => {
                            names.insert(item, &mut proof)
                        }
                        Statement::ForBorrow { item, index, .. } => {
                            names.insert(item, &mut proof)
                                && index
                                    .as_ref()
                                    .is_none_or(|i| names.insert(&i.name, &mut proof))
                        }
                        _ => true,
                    }),
                    ExprKind::Closure { params, .. } => params.iter().all(|p| {
                        proof.charge(&p);
                        names.insert(&p.name, &mut proof)
                    }),
                    ExprKind::Match { arms, .. } => arms
                        .iter()
                        .all(|a| names.pattern(&a.pattern, 0, &mut proof)),
                    _ => true,
                }
            });
        }
        proof.charge(&complete);
        if !complete {
            continue;
        }
        let before = proof.roots;
        proof.charge(&before);
        for root in call_identity::roots(function) {
            if !visit(
                root,
                false,
                true,
                0,
                &names,
                &mut directory,
                authored,
                &mut proof,
            ) {
                proof.roots = before;
                break;
            }
        }
    }
    proof
}

#[allow(clippy::too_many_arguments)]
fn visit<const N: usize>(
    expression: &Expr,
    free: bool,
    outside: bool,
    depth: usize,
    names: &Names<'_>,
    directory: &mut Directory<'_, N>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    proof: &mut Proof,
) -> bool {
    bill!(proof; &mut Directory<'_, N>);
    proof.charge(&(
        expression,
        free,
        outside,
        depth,
        names,
        &mut *directory,
        authored,
    ));
    if depth == 128 {
        return false;
    }
    if let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &expression.kind
    {
        if free && outside && type_arguments.is_empty() && !names.contains(name, proof) && {
            bill!(proof; 2 * (&mut Directory<'_, N>, &Index<'_>, &str, usize, &mut Proof, bool));
            directory.target(authored, name, args.len(), proof)
        } {
            proof.roots = proof.roots.saturating_add(1);
        }
    }
    let outside = outside && !matches!(expression.kind, ExprKind::Closure { .. });
    let mut index = 0;
    while let Some(child) = expression.child(index) {
        let free = match &expression.kind {
            ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::If { .. }
            | ExprKind::Call { .. } => true,
            ExprKind::Block { tail, .. } => std::ptr::eq(child, tail.as_ref()),
            _ => false,
        };
        proof.charge(&(index, child, free, outside));
        if !visit(
            child,
            free,
            outside,
            depth + 1,
            names,
            directory,
            authored,
            proof,
        ) {
            return false;
        }
        index += 1;
    }
    let complete = true;
    proof.charge(&complete);
    complete
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
struct Census {
    builds: usize,
    rows: usize,
    sort_compares: usize,
    swaps: usize,
    queries: usize,
    query_compares: usize,
    last_hits: usize,
}
#[cfg(test)]
enum Event {
    Build,
    Row,
    SortCompare,
    Swap,
    Query,
    QueryCompare,
    LastHit,
}
#[cfg(test)]
thread_local! { static CENSUS: std::cell::Cell<Census> = const { std::cell::Cell::new(Census { builds:0, rows:0, sort_compares:0, swaps:0, queries:0, query_compares:0, last_hits:0 }) }; }
#[cfg(test)]
fn note(event: Event) {
    CENSUS.with(|cell| {
        let mut value = cell.get();
        let count = match event {
            Event::Build => &mut value.builds,
            Event::Row => &mut value.rows,
            Event::SortCompare => &mut value.sort_compares,
            Event::Swap => &mut value.swaps,
            Event::Query => &mut value.queries,
            Event::QueryCompare => &mut value.query_compares,
            Event::LastHit => &mut value.last_hits,
        };
        *count = count
            .checked_add(1)
            .expect("bounded test event counter overflow");
        cell.set(value);
    });
}
#[cfg(test)]
fn take_census() -> Census {
    CENSUS.with(|cell| cell.replace(Census::default()))
}
#[cfg(test)]
#[path = "call_carriers_directory_tests.rs"]
mod tests;
