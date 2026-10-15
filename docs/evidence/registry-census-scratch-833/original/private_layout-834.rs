//! Invocation-local private layout closure; never a source import or graph edge.
use super::super::{graph_error, reserve_builder_structure, AuthoredDeclaration};
use crate::ast::{ModuleUseKind, Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use std::collections::BTreeMap;
use std::fmt::Write;
#[path = "private_layout/authority.rs"]
pub(in crate::workspace_graph) mod authority;
#[path = "private_layout/ledger.rs"]
mod ledger;
#[path = "private_layout/names.rs"]
mod names;
#[path = "private_layout/registry.rs"]
pub(in crate::workspace_graph) mod registry;
#[path = "private_layout/roots.rs"]
mod roots;
#[path = "private_layout/shape.rs"]
mod shape;
#[path = "private_layout/sorting.rs"]
mod sorting;
use authority::view::Lookup;
use shape::{debit, explicit_only, scalar, validate_declaration, validate_root, Absence};

#[path = "private_layout/owner_index.rs"]
mod owner_index;

pub(super) struct Entry<'a> {
    pub(super) module: &'a str,
    pub(super) declaration: &'a TypeDeclaration,
    pub(super) alias: String,
}
pub(in crate::workspace_graph) struct LayoutClosure<'a> {
    pub(super) entries: Vec<Entry<'a>>,
    owners: Option<owner_index::Owners<'a>>,
}
struct Frame<'a> {
    module: &'a str,
    declaration: &'a TypeDeclaration,
    next: usize,
}
const DEPTH: usize = crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH;
const WORK: usize = 1_048_576;

impl<'a> LayoutClosure<'a> {
    pub(in crate::workspace_graph) fn signature_owner(
        &self,
        caller: &Program,
        target: &AuthoredDeclaration<'_>,
        ty: &Type,
        authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
        programs: &[Program],
    ) -> Result<Option<bool>, Vec<Diagnostic>> {
        match &self.owners {
            Some(owners) => owners.owner(caller, target, ty, authored, programs),
            None => Ok(None),
        }
    }
    pub(in crate::workspace_graph) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(in crate::workspace_graph) fn derive(
        caller: &Program,
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        programs: &'a [Program],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut remaining = WORK;
        Self::derive_with_work(caller, authored, programs, &mut remaining, None)
    }
    fn derive_with_work(
        caller: &Program,
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        programs: &'a [Program],
        remaining: &mut usize,
        ledger: Option<&std::rc::Rc<ledger::Ledger>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if explicit_only(caller, authored, programs, remaining)? != Absence::Possible {
            return Ok(Self {
                entries: Vec::new(),
                owners: None,
            });
        }
        // Borrowed root traversal precedes naming. Explicit-only programs do
        // not acquire the new identifier census or private String backing.
        reserve_builder_structure(DEPTH * std::mem::size_of::<Frame<'_>>())?;
        let mut frames = Vec::with_capacity(DEPTH);
        if frames.capacity() != DEPTH {
            return Err(refusal());
        }
        let mut count = 0usize;
        visit(
            caller,
            authored,
            programs,
            &mut frames,
            remaining,
            |_, _, _| {
                count = count.checked_add(1usize).ok_or_else(refusal)?;
                Ok(())
            },
        )?;
        if count == 0 {
            return Ok(Self {
                entries: Vec::new(),
                owners: None,
            });
        }
        let mut ordinal = names::first(caller)?;
        debit(remaining, programs.len())?;
        let capacity = programs
            .iter()
            .try_fold(0usize, |n, p| n.checked_add(p.types.len()))
            .ok_or_else(refusal)?;
        let capacity = capacity.min(count);
        reserve_builder_structure(
            capacity
                .checked_mul(std::mem::size_of::<Entry<'_>>())
                .ok_or_else(refusal)?,
        )?;
        let mut entries: Vec<Entry<'a>> = Vec::with_capacity(capacity);
        if frames.capacity() != DEPTH || entries.capacity() != capacity {
            return Err(refusal());
        }
        visit(
            caller,
            authored,
            programs,
            &mut frames,
            remaining,
            |module, declaration, remaining| {
                let scan = entries
                    .iter()
                    .try_fold(0usize, |n, e| {
                        n.checked_add(
                            e.declaration
                                .stable_id
                                .len()
                                .checked_add(declaration.stable_id.len())?,
                        )
                    })
                    .ok_or_else(refusal)?;
                *remaining = remaining.checked_sub(scan).ok_or_else(refusal)?;
                if entries
                    .iter()
                    .any(|e| e.declaration.stable_id == declaration.stable_id)
                {
                    return Ok(());
                }
                if entries.len() == capacity {
                    return Err(refusal());
                }
                let alias = private_name(ordinal)?;
                ordinal = ordinal.checked_add(1).ok_or_else(refusal)?;
                entries.push(Entry {
                    module,
                    declaration,
                    alias,
                });
                Ok(())
            },
        )?;
        sorting::sort(&mut entries, remaining, |a, b, work| {
            sorting::identity(&a.declaration.stable_id, &b.declaration.stable_id, work)
        })?;
        let owners =
            owner_index::Owners::build(caller, authored, programs, &entries, remaining, ledger)?;
        Ok(Self {
            entries,
            owners: Some(owners),
        })
    }
    pub(super) fn add_costs(
        &self,
        caller: &Program,
        programs: &[Program],
        raw: &mut super::StructuralCost,
        runtime: &mut super::StructuralCost,
        identities: &mut usize,
    ) -> Result<(), Vec<Diagnostic>> {
        for e in &self.entries {
            super::ast_type_declaration_cost(e.declaration, raw)?;
            *identities = super::checked_builder_sum(
                *identities,
                super::identity_slots::ast_type_declaration_identity_slots(e.declaration)?,
            )?;
            runtime.string(&e.alias)?;
            super::super::generic_type_import::rewrite_declaration_runtime_cost_with_layout(
                e.declaration,
                e.module,
                caller,
                programs,
                runtime,
                Some(self),
            )?;
        }
        Ok(())
    }
    pub(super) fn append_types(
        &self,
        synthetic: &mut Program,
        caller: &Program,
        programs: &[Program],
    ) -> Result<(), Vec<Diagnostic>> {
        if self.entries.is_empty() {
            return Ok(());
        }
        let length = synthetic
            .types
            .len()
            .checked_add(self.entries.len())
            .ok_or_else(refusal)?;
        if length > synthetic.types.capacity() {
            // Charge the full replacement while the cloned original directory
            // remains live, not only the added declaration count.
            reserve_builder_structure(
                length
                    .checked_mul(std::mem::size_of::<TypeDeclaration>())
                    .ok_or_else(refusal)?,
            )?;
            synthetic
                .types
                .try_reserve_exact(length - synthetic.types.len())
                .map_err(|_| refusal())?;
            if synthetic.types.capacity() != length {
                return Err(refusal());
            }
        }
        for e in &self.entries {
            let mut declaration = e.declaration.clone();
            declaration.name = crate::bounded_output::budgeted_clone(&e.alias);
            super::super::generic_type_import::rewrite_declaration_with_layout(
                &mut declaration,
                e.module,
                caller,
                programs,
                Some(self),
            )?;
            synthetic.types.push(declaration);
        }
        Ok(())
    }
    /// Bind the copies `append_types` placed at the end of `synthetic` to this
    /// closure for diagnostic presentation, after checking each is the exact
    /// appended declaration. The view borrows the closure and both programs,
    /// so success allocates no slot backing or string copy.
    pub(in crate::workspace_graph) fn presentation<'s>(
        &'s self,
        caller: &'s Program,
        synthetic: &'s dyn crate::ast::program_source::ProgramSource,
    ) -> Result<Option<crate::source_verify::private_names::PrivateNames<'s>>, Vec<Diagnostic>>
    {
        if self.entries.is_empty() {
            return Ok(None);
        }
        let ledger = &self.owners.as_ref().ok_or_else(refusal)?.ledger;
        if ledger.failed.get() {
            return Err(refusal());
        }
        let mut work = ledger.remaining.get();
        let result = (|| {
            let base = synthetic
                .types()
                .len()
                .checked_sub(self.entries.len())
                .ok_or_else(presentation_mismatch)?;
            for (entry, copy) in self.entries.iter().zip(
                synthetic
                    .types()
                    .suffix(base)
                    .ok_or_else(presentation_mismatch)?,
            ) {
                debit(&mut work, 1)?;
                let name = sorting::identity(&copy.name, &entry.alias, &mut work)?;
                let identity =
                    sorting::identity(&copy.stable_id, &entry.declaration.stable_id, &mut work)?;
                if name != std::cmp::Ordering::Equal || identity != std::cmp::Ordering::Equal {
                    return Err(presentation_mismatch());
                }
            }
            Ok(base)
        })();
        ledger.remaining.set(work);
        if result.is_err() {
            ledger.failed.set(true);
            ledger.remaining.set(0);
        }
        let base = result?;
        Ok(Some(
            crate::source_verify::private_names::PrivateNames::new(synthetic, caller, base, self),
        ))
    }
    pub(super) fn alias<'b>(&'b self, caller: &'b Program, id: &str) -> Option<&'b str> {
        caller
            .module_uses
            .iter()
            .find(|u| u.kind == ModuleUseKind::Type && u.persistent_id == id)
            .map(|u| u.alias.as_str())
            .or_else(|| {
                caller
                    .types
                    .iter()
                    .find(|t| t.stable_id == id)
                    .map(|t| t.name.as_str())
            })
            .or_else(|| {
                self.entries
                    .binary_search_by(|e| e.declaration.stable_id.as_str().cmp(id))
                    .ok()
                    .map(|index| self.entries[index].alias.as_str())
            })
    }
}
impl crate::source_verify::private_names::Providers for LayoutClosure<'_> {
    fn provider(&self, index: usize) -> Option<(&str, &TypeDeclaration)> {
        self.entries.get(index).map(|e| (e.module, e.declaration))
    }
}
fn private_name(ordinal: u64) -> Result<String, Vec<Diagnostic>> {
    let mut digits = 1;
    let mut number = ordinal;
    while number >= 10 {
        digits += 1;
        number /= 10;
    }
    let bytes = "_spx_layout_".len() + digits;
    reserve_builder_structure(bytes)?;
    let mut name = String::new();
    name.try_reserve_exact(bytes).map_err(|_| refusal())?;
    if name.capacity() != bytes {
        return Err(refusal());
    }
    write!(&mut name, "_spx_layout_{ordinal}").map_err(|_| refusal())?;
    Ok(name)
}

fn presentation_mismatch() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G173",
        "private layout presentation does not match its appended synthetic declarations",
    )]
}

fn refusal() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G172",
        "private layout closure exceeds its independently bounded nominal profile",
    )]
}

fn visit<'a, A: Lookup<'a>>(
    caller: &Program,
    authored: &A,
    programs: &'a [Program],
    frames: &mut Vec<Frame<'a>>,
    remaining: &mut usize,
    mut found: impl FnMut(&'a str, &'a TypeDeclaration, &mut usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    roots::visit(
        caller,
        authored,
        programs,
        remaining,
        |module, root, signature, remaining| {
            let root_private = signature && private(caller, module, &root.stable_id, remaining)?;
            if !root_private
                && !needs_private(caller, module, root, authored, programs, frames, remaining)?
            {
                return Ok(());
            }
            let outcome = matches!(root.kind, TypeDeclarationKind::Variant { .. });
            validate_root(root, module, authored, programs, remaining)?;
            let mut fields = 0usize;
            let mut leaves = 0usize;
            validate_declaration(root, &mut fields, remaining)?;
            if root_private {
                found(module, root, remaining)?;
            }
            frames.push(Frame {
                module,
                declaration: root,
                next: 0,
            });
            while let Some(frame) = frames.last_mut() {
                *remaining = remaining.checked_sub(1).ok_or_else(refusal)?;
                if let TypeDeclarationKind::Variant { cases } = &frame.declaration.kind {
                    debit(
                        remaining,
                        cases.iter().map(|c| c.fields.len()).sum::<usize>(),
                    )?;
                }
                let child = field_type(frame.declaration, frame.next);
                frame.next += 1;
                let module = frame.module;
                let Some(ty) = child else {
                    frames.pop();
                    continue;
                };
                if scalar(ty) {
                    continue;
                }
                if matches!(ty, Type::String | Type::Bytes)
                    || (!outcome && crate::map_ops::ast_collection(ty))
                {
                    leaves = leaves.checked_add(1).ok_or_else(refusal)?;
                    if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                        return Err(refusal());
                    }
                    continue;
                }
                let Type::Named { name, arguments } = ty else {
                    return Err(refusal());
                };
                let id = identity(module, name, programs, remaining)?;
                if id == Some(crate::prelude::VEC_ID) || (id.is_none() && name == "Vec") {
                    debit(remaining, caller.types.len() + caller.module_uses.len())?;
                    if caller.types.iter().any(|t| t.name == "Vec")
                        || caller.module_uses.iter().any(|u| {
                            u.kind == ModuleUseKind::Type
                                && u.alias == "Vec"
                                && u.persistent_id != crate::prelude::VEC_ID
                        })
                    {
                        return Err(refusal());
                    }
                    leaves = leaves.checked_add(1).ok_or_else(refusal)?;
                    if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                        return Err(refusal());
                    }
                    let [element] = arguments.as_slice() else {
                        return Err(refusal());
                    };
                    if crate::vec_ops::ast_vec_element_is_admitted(element)
                        || *element == Type::String
                    {
                        continue;
                    }
                    let Type::Named { name, arguments } = element else {
                        return Err(refusal());
                    };
                    if !arguments.is_empty() {
                        return Err(refusal());
                    }
                    let id = identity(module, name, programs, remaining)?.ok_or_else(refusal)?;
                    let target = authored.get(id).ok_or_else(refusal)?;
                    let declaration = target.ty.ok_or_else(refusal)?;
                    debit(
                        remaining,
                        match &declaration.kind {
                            TypeDeclarationKind::Record { fields } => fields.len() * 2,
                            _ => 1,
                        },
                    )?;
                    if !target.explicit
                        || !declaration.explicit_id
                        || declaration.extends.is_some()
                        || !super::type_rewrite::record_element(declaration)
                    {
                        return Err(refusal());
                    }
                    if private(caller, target.module, id, remaining)? {
                        found(target.module, declaration, remaining)?;
                    }
                    continue;
                }
                if !arguments.is_empty() {
                    return Err(refusal());
                }
                let id = id.ok_or_else(refusal)?;
                let target = authored.get(id).ok_or_else(refusal)?;
                let declaration = target.ty.ok_or_else(refusal)?;
                debit(remaining, frames.len())?;
                if !target.explicit
                    || !declaration.explicit_id
                    || !declaration.type_parameters.is_empty()
                    || !declaration.invariants().is_empty()
                    || declaration.extends.is_some()
                    || !matches!(declaration.kind, TypeDeclarationKind::Record { .. })
                    || frames.iter().any(|f| f.declaration.stable_id == id)
                    || frames.len() == DEPTH
                {
                    return Err(refusal());
                }
                if private(caller, target.module, id, remaining)? {
                    found(target.module, declaration, remaining)?;
                }
                validate_declaration(declaration, &mut fields, remaining)?;
                frames.push(Frame {
                    module: target.module,
                    declaration,
                    next: 0,
                });
            }
            Ok(())
        },
    )
}
// Selection only: traverse borrowed declaration references to determine whether
// this particular root needs inference. Full shape validation follows separately.
fn needs_private<'a, A: Lookup<'a>>(
    caller: &Program,
    module: &'a str,
    root: &'a TypeDeclaration,
    authored: &A,
    programs: &'a [Program],
    frames: &mut Vec<Frame<'a>>,
    work: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    frames.clear();
    frames.push(Frame {
        module,
        declaration: root,
        next: 0,
    });
    while let Some(frame) = frames.last_mut() {
        debit(work, 1)?;
        if let TypeDeclarationKind::Variant { cases } = &frame.declaration.kind {
            debit(work, cases.iter().map(|c| c.fields.len()).sum())?;
        }
        let child = field_type(frame.declaration, frame.next);
        frame.next += 1;
        let module = frame.module;
        let Some(Type::Named { name, arguments }) = child else {
            if child.is_none() {
                frames.pop();
            }
            continue;
        };
        let id = identity(module, name, programs, work)?;
        let (name, arguments) =
            if id == Some(crate::prelude::VEC_ID) || (id.is_none() && name == "Vec") {
                let [Type::Named { name, arguments }] = arguments.as_slice() else {
                    continue;
                };
                (name, arguments)
            } else {
                (name, arguments)
            };
        if !arguments.is_empty() {
            continue;
        }
        let Some(id) = identity(module, name, programs, work)? else {
            continue;
        };
        let target = authored.get(id).ok_or_else(refusal)?;
        if private(caller, target.module, id, work)? {
            frames.clear();
            return Ok(true);
        }
        let Some(declaration) = target.ty else {
            continue;
        };
        debit(work, frames.len())?;
        if frames.iter().any(|f| f.declaration.stable_id == id) {
            continue;
        }
        if frames.len() == DEPTH {
            return Err(refusal());
        }
        frames.push(Frame {
            module: target.module,
            declaration,
            next: 0,
        });
    }
    Ok(false)
}

fn private(
    caller: &Program,
    module: &str,
    id: &str,
    remaining: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    *remaining = remaining
        .checked_sub(caller.module_uses.len() + 1)
        .ok_or_else(refusal)?;
    Ok(private_without_work(caller, module, id))
}
fn private_without_work(caller: &Program, module: &str, id: &str) -> bool {
    caller.module != module
        && !caller
            .module_uses
            .iter()
            .any(|u| u.kind == ModuleUseKind::Type && u.persistent_id == id)
}
fn field_type(declaration: &TypeDeclaration, index: usize) -> Option<&Type> {
    match &declaration.kind {
        TypeDeclarationKind::Record { fields } => fields.get(index).map(|f| &f.ty),
        TypeDeclarationKind::Variant { cases } => cases
            .iter()
            .flat_map(|c| &c.fields)
            .nth(index)
            .map(|f| &f.ty),
        TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. } => None,
    }
}
// Borrow the persistent identity; no cloned lookup key or hidden prelude walk.
pub(super) fn identity<'a>(
    module: &str,
    name: &str,
    programs: &'a [Program],
    remaining: &mut usize,
) -> Result<Option<&'a str>, Vec<Diagnostic>> {
    *remaining = remaining.checked_sub(programs.len()).ok_or_else(refusal)?;
    let provider = programs
        .iter()
        .find(|p| p.module == module)
        .ok_or_else(refusal)?;
    *remaining = remaining
        .checked_sub(provider.types.len() + provider.module_uses.len())
        .ok_or_else(refusal)?;
    Ok(provider
        .types
        .iter()
        .find(|t| t.name == name)
        .map(|t| t.stable_id.as_str())
        .or_else(|| {
            provider
                .module_uses
                .iter()
                .find(|u| u.kind == ModuleUseKind::Type && u.alias == name)
                .map(|u| u.persistent_id.as_str())
        }))
}

#[cfg(test)]
#[path = "private_layout/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "private_layout/function_tests.rs"]
mod function_tests;

#[cfg(test)]
#[path = "private_layout/function_runtime.rs"]
mod function_runtime;

// A function-only nominal signature is checked through the same borrowed closure
// walk before admission; this creates no aliases or stored authority.
#[cfg(test)]
pub(in crate::workspace_graph) fn signature_owner<'a>(
    caller: &Program,
    module: &'a str,
    ty: &Type,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> Option<bool> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    let mut work = WORK;
    let candidate = identity(module, name, programs, &mut work).ok()?;
    let vector =
        candidate == Some(crate::prelude::VEC_ID) || (candidate.is_none() && name == "Vec");
    let id = if vector {
        let [Type::Named { name, arguments }] = arguments.as_slice() else {
            return None;
        };
        if !arguments.is_empty() {
            return None;
        }
        identity(module, name, programs, &mut work).ok()??
    } else {
        if !arguments.is_empty() {
            return None;
        }
        candidate?
    };
    let root = authored.get(id)?.ty?;
    if !private(caller, module, id, &mut work).ok()? {
        return None;
    }
    reserve_builder_structure(DEPTH.checked_mul(std::mem::size_of::<Frame<'_>>())?).ok()?;
    let mut frames = Vec::with_capacity(DEPTH);
    if frames.capacity() != DEPTH {
        return None;
    }
    let mut seen = false;
    visit(
        caller,
        authored,
        programs,
        &mut frames,
        &mut work,
        |_, declaration, _| {
            if std::ptr::eq(root, declaration) {
                seen = true;
            }
            Ok(())
        },
    )
    .ok()?;
    if !seen {
        return None;
    }
    let mut owns = vector;
    frames.push(Frame {
        module: authored.get(id)?.module,
        declaration: root,
        next: 0,
    });
    while let Some(frame) = frames.last_mut() {
        debit(&mut work, 1).ok()?;
        let ty = field_type(frame.declaration, frame.next);
        frame.next = frame.next.checked_add(1)?;
        let owner = frame.module;
        let Some(ty) = ty else {
            frames.pop();
            continue;
        };
        if scalar(ty) {
            continue;
        }
        if matches!(ty, Type::String | Type::Bytes) || crate::map_ops::ast_collection(ty) {
            owns = true;
            continue;
        }
        let Type::Named { name, arguments } = ty else {
            return None;
        };
        let id = identity(owner, name, programs, &mut work).ok()?;
        if id == Some(crate::prelude::VEC_ID) || (id.is_none() && name == "Vec") {
            owns = true;
            continue;
        }
        if !arguments.is_empty() || frames.len() == DEPTH {
            return None;
        }
        let child = authored.get(id?)?;
        frames.push(Frame {
            module: child.module,
            declaration: child.ty?,
            next: 0,
        });
    }
    Some(owns)
}
pub(in crate::workspace_graph) fn signature_is_private(
    caller: &Program,
    module: &str,
    ty: &Type,
    programs: &[Program],
) -> bool {
    let Type::Named { name, arguments } = ty else {
        return false;
    };
    let mut work = WORK;
    let resolved = match identity(module, name, programs, &mut work) {
        Ok(id) => id,
        Err(_) => return true,
    };
    let id = if resolved == Some(crate::prelude::VEC_ID) || (resolved.is_none() && name == "Vec") {
        let [element] = arguments.as_slice() else {
            return true;
        };
        let Type::Named { name, arguments } = element else {
            return false;
        };
        if !arguments.is_empty() {
            return true;
        }
        match identity(module, name, programs, &mut work) {
            Ok(Some(id)) => id,
            _ => return true,
        }
    } else {
        if !arguments.is_empty() {
            return true;
        }
        let Some(id) = resolved else { return false };
        id
    };
    private(caller, module, id, &mut work).unwrap_or(true)
}

#[cfg(test)]
#[path = "private_layout/capacity_before.rs"]
mod capacity_before;
#[cfg(test)]
#[path = "private_layout/capacity_tests.rs"]
mod capacity_tests;
