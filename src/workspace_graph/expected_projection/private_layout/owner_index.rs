//! Invocation-held nominal ownedness; names and stored IDs are never authority.
use super::{
    debit, field_type, identity, private, refusal, reserve_builder_structure, AuthoredDeclaration,
    Entry, DEPTH,
};
use crate::ast::{Function, ModuleUseKind, Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use std::collections::BTreeMap;
use std::rc::Rc;
struct Fact<'a> {
    module: &'a str,
    declaration: &'a TypeDeclaration,
    owns: bool,
}
struct Frame<'a> {
    module: &'a str,
    declaration: &'a TypeDeclaration,
    next: usize,
    owns: bool,
}
struct Site<'a> {
    provider: &'a Program,
    function: &'a Function,
    ty: &'a Type,
    ordinal: usize,
    owns: Option<bool>,
}
pub(super) struct Owners<'a> {
    caller: &'a Program,
    programs: &'a [Program],
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    sites: Vec<Site<'a>>,
    pub(super) ledger: Rc<super::ledger::Ledger>,
}
impl<'a> Owners<'a> {
    pub(super) fn build(
        caller: &Program,
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        programs: &'a [Program],
        entries: &[Entry<'a>],
        work: &mut usize,
        ledger: Option<&Rc<super::ledger::Ledger>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        debit(work, programs.len())?;
        let caller = programs
            .iter()
            .find(|p| std::ptr::eq(*p, caller))
            .ok_or_else(refusal)?;
        debit(work, programs.len())?;
        let capacity = programs
            .iter()
            .try_fold(0usize, |n, p| n.checked_add(p.types.len()))
            .ok_or_else(refusal)?;
        reserve_builder_structure(
            std::mem::size_of::<Self>()
                .checked_add(std::mem::size_of::<Vec<Fact<'_>>>())
                .and_then(|n| n.checked_add(std::mem::size_of::<Vec<Frame<'_>>>()))
                .and_then(|n| n.checked_add(capacity.checked_mul(std::mem::size_of::<Fact<'_>>())?))
                .and_then(|n| n.checked_add(DEPTH.checked_mul(std::mem::size_of::<Frame<'_>>())?))
                .ok_or_else(refusal)?,
        )?;
        let mut facts: Vec<Fact<'a>> = Vec::with_capacity(capacity);
        let mut frames: Vec<Frame<'a>> = Vec::with_capacity(DEPTH);
        if facts.capacity() != capacity || frames.capacity() != DEPTH {
            return Err(refusal());
        }
        for entry in entries {
            if existing(&facts, entry.declaration, work)?.is_some() {
                continue;
            }
            frames.push(Frame {
                module: entry.module,
                declaration: entry.declaration,
                next: 0,
                owns: false,
            });
            while let Some(frame) = frames.last_mut() {
                debit(work, 1)?;
                if let TypeDeclarationKind::Variant { cases } = &frame.declaration.kind {
                    debit(work, cases.len())?;
                    let fields = cases
                        .iter()
                        .try_fold(0usize, |n, case| n.checked_add(case.fields.len()))
                        .ok_or_else(refusal)?;
                    debit(work, fields)?;
                }
                let child = field_type(frame.declaration, frame.next);
                frame.next = frame.next.checked_add(1).ok_or_else(refusal)?;
                let module = frame.module;
                let Some(ty) = child else {
                    let complete = frames.pop().ok_or_else(refusal)?;
                    if facts.len() == capacity {
                        return Err(refusal());
                    }
                    facts.push(Fact {
                        module: complete.module,
                        declaration: complete.declaration,
                        owns: complete.owns,
                    });
                    if let Some(parent) = frames.last_mut() {
                        parent.owns |= complete.owns;
                    }
                    continue;
                };
                if super::scalar(ty) {
                    continue;
                }
                if matches!(ty, Type::String | Type::Bytes) || crate::map_ops::ast_collection(ty) {
                    frame.owns = true;
                    continue;
                }
                let Type::Named { name, arguments } = ty else {
                    return Err(refusal());
                };
                let id = identity(module, name, programs, work)?;
                if id == Some(crate::prelude::VEC_ID) || (id.is_none() && name == "Vec") {
                    // The complete closure walk already authenticated exact compiler Vec
                    // identity, arity, flat element layout and bounds for this same field.
                    frame.owns = true;
                    continue;
                }
                if !arguments.is_empty() {
                    return Err(refusal());
                }
                let target = authored.get(id.ok_or_else(refusal)?).ok_or_else(refusal)?;
                let declaration = target.ty.ok_or_else(refusal)?;
                exact_declaration(target, declaration, programs, work)?;
                if let Some(owns) = existing(&facts, declaration, work)? {
                    frames.last_mut().ok_or_else(refusal)?.owns |= owns;
                    continue;
                }
                debit(work, frames.len())?;
                if frames.len() == DEPTH
                    || frames
                        .iter()
                        .any(|f| std::ptr::eq(f.declaration, declaration))
                {
                    return Err(refusal());
                }
                frames.push(Frame {
                    module: target.module,
                    declaration,
                    next: 0,
                    owns: false,
                });
            }
        }
        // Unique stable IDs make the sorted directory deterministic without merge scratch.
        debit(work, facts.len())?;
        let max = facts
            .iter()
            .map(|f| f.declaration.stable_id.len())
            .max()
            .unwrap_or(0);
        super::sorting::sort(&mut facts, work, |a, b, work| {
            super::sorting::identity(&a.declaration.stable_id, &b.declaration.stable_id, work)
        })?;
        debit(
            work,
            facts
                .len()
                .checked_mul(max.checked_add(1).ok_or_else(refusal)?)
                .ok_or_else(refusal)?,
        )?;
        if facts
            .windows(2)
            .any(|pair| pair[0].declaration.stable_id == pair[1].declaration.stable_id)
        {
            return Err(refusal());
        }
        let mut site_capacity = 0usize;
        debit(work, caller.module_uses.len())?;
        for import in &caller.module_uses {
            if import.kind != ModuleUseKind::Function {
                continue;
            }
            let target = authored
                .get(import.persistent_id.as_str())
                .ok_or_else(refusal)?;
            let function = target.function.ok_or_else(refusal)?;
            site_capacity = site_capacity
                .checked_add(function.params.len().checked_add(1).ok_or_else(refusal)?)
                .ok_or_else(refusal)?;
        }
        reserve_builder_structure(
            site_capacity
                .checked_mul(std::mem::size_of::<Site<'_>>())
                .ok_or_else(refusal)?,
        )?;
        let mut sites: Vec<Site<'a>> = Vec::with_capacity(site_capacity);
        if sites.capacity() != site_capacity {
            return Err(refusal());
        }
        for import in &caller.module_uses {
            debit(work, 1)?;
            if import.kind != ModuleUseKind::Function {
                continue;
            }
            let target = authored
                .get(import.persistent_id.as_str())
                .ok_or_else(refusal)?;
            let function = target.function.ok_or_else(refusal)?;
            debit(
                work,
                sites
                    .len()
                    .checked_add(programs.len())
                    .ok_or_else(refusal)?,
            )?;
            if sites
                .iter()
                .any(|site| std::ptr::eq(site.function, function))
            {
                continue;
            }
            let provider = programs
                .iter()
                .find(|p| p.module == target.module && p.path == target.path)
                .ok_or_else(refusal)?;
            debit(work, provider.functions.len())?;
            if !target.explicit
                || !function.explicit_id
                || function.stable_id != import.persistent_id
                || target.module != import.target_module
                || !provider
                    .functions
                    .iter()
                    .any(|held| std::ptr::eq(held, function))
            {
                return Err(refusal());
            }
            if !function.type_parameters.is_empty()
                || function.yields.is_some()
                || function.follows.is_some()
            {
                continue;
            }
            for (ordinal, ty) in std::iter::once(&function.return_type)
                .chain(function.params.iter().map(|p| &p.ty))
                .enumerate()
            {
                debit(work, 1)?;
                let owns = site_owner(caller, provider, ty, authored, programs, &facts, work)?;
                if sites.len() == site_capacity {
                    return Err(refusal());
                }
                sites.push(Site {
                    provider,
                    function,
                    ty,
                    ordinal,
                    owns,
                });
            }
        }
        // Sorting is fallible at each comparison; no partial directory escapes.
        super::sorting::sort(&mut sites, work, |a, b, work| {
            let order =
                super::sorting::identity(&a.function.stable_id, &b.function.stable_id, work)?;
            if order != std::cmp::Ordering::Equal {
                return Ok(order);
            }
            debit(work, 1)?;
            Ok(a.ordinal.cmp(&b.ordinal))
        })?;
        Ok(Self {
            caller,
            programs,
            authored,
            sites,
            ledger: match ledger {
                Some(ledger) => Rc::clone(ledger),
                None => super::ledger::Ledger::new(*work)?,
            },
        })
    }
    pub(super) fn build_retained(
        caller: &Program,
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        programs: &'a [Program],
        entries: &[Entry<'a>],
        work: &mut usize,
        ledger: Option<&Rc<super::ledger::Ledger>>,
        scratch: &mut Scratch<'a>,
    ) -> Result<Self, Vec<Diagnostic>> {
        debit(work, programs.len())?;
        let caller = programs
            .iter()
            .find(|p| std::ptr::eq(*p, caller))
            .ok_or_else(refusal)?;
        debit(work, programs.len())?;
        let capacity = programs
            .iter()
            .try_fold(0usize, |n, p| n.checked_add(p.types.len()))
            .ok_or_else(refusal)?;
        reserve_builder_structure(
            2 * std::mem::size_of::<Self>()
                + std::mem::size_of::<Result<(), Vec<Diagnostic>>>()
                + std::mem::size_of::<Result<Self, Vec<Diagnostic>>>()
                + std::mem::size_of::<&mut Scratch<'a>>()
                + std::mem::size_of::<&mut Vec<Fact<'a>>>()
                + std::mem::size_of::<&mut Vec<Frame<'a>>>(),
        )?;
        if scratch.facts.capacity() == 0 {
            scratch.acquire(capacity)?;
        }
        if scratch.facts.capacity() != capacity
            || scratch.frames.capacity() != DEPTH
            || !scratch.facts.is_empty()
            || !scratch.frames.is_empty()
        {
            return Err(refusal());
        }
        let facts = &mut scratch.facts;
        let frames = &mut scratch.frames;
        for entry in entries {
            if existing(facts, entry.declaration, work)?.is_some() {
                continue;
            }
            frames.push(Frame {
                module: entry.module,
                declaration: entry.declaration,
                next: 0,
                owns: false,
            });
            while let Some(frame) = frames.last_mut() {
                debit(work, 1)?;
                if let TypeDeclarationKind::Variant { cases } = &frame.declaration.kind {
                    debit(work, cases.len())?;
                    let fields = cases
                        .iter()
                        .try_fold(0usize, |n, case| n.checked_add(case.fields.len()))
                        .ok_or_else(refusal)?;
                    debit(work, fields)?;
                }
                let child = field_type(frame.declaration, frame.next);
                frame.next = frame.next.checked_add(1).ok_or_else(refusal)?;
                let module = frame.module;
                let Some(ty) = child else {
                    let complete = frames.pop().ok_or_else(refusal)?;
                    if facts.len() == capacity {
                        return Err(refusal());
                    }
                    facts.push(Fact {
                        module: complete.module,
                        declaration: complete.declaration,
                        owns: complete.owns,
                    });
                    if let Some(parent) = frames.last_mut() {
                        parent.owns |= complete.owns;
                    }
                    continue;
                };
                if super::scalar(ty) {
                    continue;
                }
                if matches!(ty, Type::String | Type::Bytes) || crate::map_ops::ast_collection(ty) {
                    frame.owns = true;
                    continue;
                }
                let Type::Named { name, arguments } = ty else {
                    return Err(refusal());
                };
                let id = identity(module, name, programs, work)?;
                if id == Some(crate::prelude::VEC_ID) || (id.is_none() && name == "Vec") {
                    // The complete closure walk already authenticated exact compiler Vec
                    // identity, arity, flat element layout and bounds for this same field.
                    frame.owns = true;
                    continue;
                }
                if !arguments.is_empty() {
                    return Err(refusal());
                }
                let target = authored.get(id.ok_or_else(refusal)?).ok_or_else(refusal)?;
                let declaration = target.ty.ok_or_else(refusal)?;
                exact_declaration(target, declaration, programs, work)?;
                if let Some(owns) = existing(facts, declaration, work)? {
                    frames.last_mut().ok_or_else(refusal)?.owns |= owns;
                    continue;
                }
                debit(work, frames.len())?;
                if frames.len() == DEPTH
                    || frames
                        .iter()
                        .any(|f| std::ptr::eq(f.declaration, declaration))
                {
                    return Err(refusal());
                }
                frames.push(Frame {
                    module: target.module,
                    declaration,
                    next: 0,
                    owns: false,
                });
            }
        }
        // Unique stable IDs make the sorted directory deterministic without merge scratch.
        debit(work, facts.len())?;
        let max = facts
            .iter()
            .map(|f| f.declaration.stable_id.len())
            .max()
            .unwrap_or(0);
        super::sorting::sort(facts, work, |a, b, work| {
            super::sorting::identity(&a.declaration.stable_id, &b.declaration.stable_id, work)
        })?;
        debit(
            work,
            facts
                .len()
                .checked_mul(max.checked_add(1).ok_or_else(refusal)?)
                .ok_or_else(refusal)?,
        )?;
        if facts
            .windows(2)
            .any(|pair| pair[0].declaration.stable_id == pair[1].declaration.stable_id)
        {
            return Err(refusal());
        }
        let mut site_capacity = 0usize;
        debit(work, caller.module_uses.len())?;
        for import in &caller.module_uses {
            if import.kind != ModuleUseKind::Function {
                continue;
            }
            let target = authored
                .get(import.persistent_id.as_str())
                .ok_or_else(refusal)?;
            let function = target.function.ok_or_else(refusal)?;
            site_capacity = site_capacity
                .checked_add(function.params.len().checked_add(1).ok_or_else(refusal)?)
                .ok_or_else(refusal)?;
        }
        reserve_builder_structure(
            site_capacity
                .checked_mul(std::mem::size_of::<Site<'_>>())
                .ok_or_else(refusal)?,
        )?;
        let mut sites: Vec<Site<'a>> = Vec::with_capacity(site_capacity);
        if sites.capacity() != site_capacity {
            return Err(refusal());
        }
        for import in &caller.module_uses {
            debit(work, 1)?;
            if import.kind != ModuleUseKind::Function {
                continue;
            }
            let target = authored
                .get(import.persistent_id.as_str())
                .ok_or_else(refusal)?;
            let function = target.function.ok_or_else(refusal)?;
            debit(
                work,
                sites
                    .len()
                    .checked_add(programs.len())
                    .ok_or_else(refusal)?,
            )?;
            if sites
                .iter()
                .any(|site| std::ptr::eq(site.function, function))
            {
                continue;
            }
            let provider = programs
                .iter()
                .find(|p| p.module == target.module && p.path == target.path)
                .ok_or_else(refusal)?;
            debit(work, provider.functions.len())?;
            if !target.explicit
                || !function.explicit_id
                || function.stable_id != import.persistent_id
                || target.module != import.target_module
                || !provider
                    .functions
                    .iter()
                    .any(|held| std::ptr::eq(held, function))
            {
                return Err(refusal());
            }
            if !function.type_parameters.is_empty()
                || function.yields.is_some()
                || function.follows.is_some()
            {
                continue;
            }
            for (ordinal, ty) in std::iter::once(&function.return_type)
                .chain(function.params.iter().map(|p| &p.ty))
                .enumerate()
            {
                debit(work, 1)?;
                let owns = site_owner(caller, provider, ty, authored, programs, facts, work)?;
                if sites.len() == site_capacity {
                    return Err(refusal());
                }
                sites.push(Site {
                    provider,
                    function,
                    ty,
                    ordinal,
                    owns,
                });
            }
        }
        // Sorting is fallible at each comparison; no partial directory escapes.
        super::sorting::sort(&mut sites, work, |a, b, work| {
            let order =
                super::sorting::identity(&a.function.stable_id, &b.function.stable_id, work)?;
            if order != std::cmp::Ordering::Equal {
                return Ok(order);
            }
            debit(work, 1)?;
            Ok(a.ordinal.cmp(&b.ordinal))
        })?;
        let owner = Self {
            caller,
            programs,
            authored,
            sites,
            ledger: match ledger {
                Some(ledger) => Rc::clone(ledger),
                None => super::ledger::Ledger::new(*work)?,
            },
        };
        // These buffers lend no child references to the next derivation.
        facts.clear();
        frames.clear();
        Ok(owner)
    }
    pub(super) fn owner(
        &self,
        caller: &Program,
        target: &AuthoredDeclaration<'_>,
        ty: &Type,
        authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
        programs: &[Program],
    ) -> Result<Option<bool>, Vec<Diagnostic>> {
        if self.ledger.failed.get() {
            return Err(refusal());
        }
        let mut work = self.ledger.remaining.get();
        let result = (|| {
            if !std::ptr::eq(caller, self.caller)
                || !std::ptr::eq(programs, self.programs)
                || !std::ptr::eq(authored, self.authored)
            {
                return Err(refusal());
            }
            let function = target.function.ok_or_else(refusal)?;
            debit(&mut work, 1)?;
            let key = (
                function.stable_id.as_str(),
                signature_ordinal(function, ty).ok_or_else(refusal)?,
            );
            let (mut low, mut high) = (0, self.sites.len());
            while low < high {
                let mid = low + (high - low) / 2;
                let site = &self.sites[mid];
                debit(
                    &mut work,
                    function
                        .stable_id
                        .len()
                        .checked_add(site.function.stable_id.len())
                        .and_then(|n| n.checked_add(1))
                        .ok_or_else(refusal)?,
                )?;
                match site_key(site).cmp(&key) {
                    std::cmp::Ordering::Less => low = mid + 1,
                    std::cmp::Ordering::Greater => high = mid,
                    std::cmp::Ordering::Equal => {
                        if !std::ptr::eq(site.function, function)
                            || !std::ptr::eq(site.ty, ty)
                            || !target.explicit
                            || target.module != site.provider.module
                            || target.path != site.provider.path
                        {
                            return Err(refusal());
                        }
                        return Ok(site.owns);
                    }
                }
            }
            Err(refusal())
        })();
        if result.is_err() {
            self.ledger.failed.set(true);
            work = 0;
        }
        self.ledger.remaining.set(work);
        result
    }
}
fn site_key<'a>(site: &Site<'a>) -> (&'a str, usize) {
    (site.function.stable_id.as_str(), site.ordinal)
}
// Safe address arithmetic selects only a candidate ordinal; the final borrowed
// field equality authenticates it before use. No unknown address is dereferenced.
fn signature_ordinal(function: &Function, ty: &Type) -> Option<usize> {
    if std::ptr::eq(&function.return_type, ty) {
        return Some(0);
    }
    let first = &function.params.first()?.ty as *const Type as usize;
    let offset = (ty as *const Type as usize).checked_sub(first)?;
    let stride = std::mem::size_of::<crate::ast::Param>();
    if stride == 0 || offset % stride != 0 {
        return None;
    }
    let index = offset / stride;
    if !std::ptr::eq(&function.params.get(index)?.ty, ty) {
        return None;
    }
    index.checked_add(1)
}

fn existing(
    facts: &[Fact<'_>],
    declaration: &TypeDeclaration,
    work: &mut usize,
) -> Result<Option<bool>, Vec<Diagnostic>> {
    for fact in facts {
        debit(
            work,
            fact.declaration
                .stable_id
                .len()
                .checked_add(declaration.stable_id.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(refusal)?,
        )?;
        if fact.declaration.stable_id == declaration.stable_id {
            if !std::ptr::eq(fact.declaration, declaration) {
                return Err(refusal());
            }
            return Ok(Some(fact.owns));
        }
    }
    Ok(None)
}
fn exact_declaration(
    target: &AuthoredDeclaration<'_>,
    declaration: &TypeDeclaration,
    programs: &[Program],
    work: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    debit(work, programs.len())?;
    let provider = programs
        .iter()
        .find(|p| p.module == target.module && p.path == target.path)
        .ok_or_else(refusal)?;
    debit(work, provider.types.len())?;
    if !target.explicit
        || !declaration.explicit_id
        || !provider.types.iter().any(|d| std::ptr::eq(d, declaration))
    {
        return Err(refusal());
    }
    Ok(())
}

fn site_owner<'a>(
    caller: &Program,
    provider: &Program,
    ty: &Type,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    facts: &[Fact<'a>],
    work: &mut usize,
) -> Result<Option<bool>, Vec<Diagnostic>> {
    let Type::Named { name, arguments } = ty else {
        return Ok(None);
    };
    let resolved = identity(&provider.module, name, programs, work)?;
    let vector = resolved == Some(crate::prelude::VEC_ID) || (resolved.is_none() && name == "Vec");
    let id = if vector {
        let [Type::Named { name, arguments }] = arguments.as_slice() else {
            return Ok(None);
        };
        if !arguments.is_empty() {
            return Err(refusal());
        }
        identity(&provider.module, name, programs, work)?.ok_or_else(refusal)?
    } else {
        if !arguments.is_empty() {
            return Ok(None);
        }
        let Some(id) = resolved else { return Ok(None) };
        id
    };
    if !private(caller, &provider.module, id, work)? {
        return Ok(None);
    }
    let target = authored.get(id).ok_or_else(refusal)?;
    let declaration = target.ty.ok_or_else(refusal)?;
    exact_declaration(target, declaration, programs, work)?;
    let mut lo = 0;
    let mut hi = facts.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let fact = &facts[mid];
        debit(
            work,
            id.len()
                .checked_add(fact.declaration.stable_id.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(refusal)?,
        )?;
        match fact.declaration.stable_id.as_str().cmp(id) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => {
                if fact.module != target.module || !std::ptr::eq(fact.declaration, declaration) {
                    return Err(refusal());
                }
                if vector {
                    let count = match &declaration.kind {
                        crate::ast::TypeDeclarationKind::Record { fields } => fields.len(),
                        _ => return Err(refusal()),
                    };
                    debit(work, count.checked_mul(2).ok_or_else(refusal)?)?;
                    if !super::super::type_rewrite::record_element(declaration) {
                        return Err(refusal());
                    }
                }
                return Ok(Some(vector || fact.owns));
            }
        }
    }
    Err(refusal())
}

#[cfg(test)]
#[path = "owner_index/tests.rs"]
mod tests;

/// Registry-owned physical backing; source custody is checked by its owner
/// before any child borrows these fields. No remaining child allowance is held.
pub(super) struct Scratch<'a> {
    facts: Vec<Fact<'a>>,
    frames: Vec<Frame<'a>>,
}
impl<'a> Scratch<'a> {
    pub(super) fn initializer_controls() -> usize {
        std::mem::size_of::<Self>()
            + std::mem::size_of::<Vec<Fact<'a>>>()
            + std::mem::size_of::<Vec<Frame<'a>>>()
    }
    pub(super) fn empty() -> Self {
        Self {
            facts: Vec::new(),
            frames: Vec::new(),
        }
    }
    fn acquire(&mut self, capacity: usize) -> Result<(), Vec<Diagnostic>> {
        reserve_builder_structure(
            3 * std::mem::size_of::<Result<(), Vec<Diagnostic>>>()
                + std::mem::size_of::<&mut Self>()
                + std::mem::size_of::<Vec<Fact<'a>>>()
                + std::mem::size_of::<Vec<Frame<'a>>>(),
        )?;
        reserve_builder_structure(
            capacity
                .checked_mul(std::mem::size_of::<Fact<'a>>())
                .and_then(|n| n.checked_add(DEPTH.checked_mul(std::mem::size_of::<Frame<'a>>())?))
                .ok_or_else(refusal)?,
        )?;
        self.facts = Vec::with_capacity(capacity);
        self.frames = Vec::with_capacity(DEPTH);
        if self.facts.capacity() != capacity || self.frames.capacity() != DEPTH {
            return Err(refusal());
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn state(&self) -> ([usize; 2], [usize; 2], bool) {
        (
            [self.facts.as_ptr() as usize, self.frames.as_ptr() as usize],
            [self.facts.capacity(), self.frames.capacity()],
            self.facts.is_empty() && self.frames.is_empty(),
        )
    }
}
