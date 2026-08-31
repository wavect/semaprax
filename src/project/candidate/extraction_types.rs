//! Separate Copy-boundary and internal-owner admission for extraction.

use super::*;
use crate::hir::DeclarationKind;
use crate::workspace_graph::WorkspaceGraphProjectionModule;

pub(super) struct Types<'a> {
    revision: &'a ProjectRevision,
    program: &'a Program,
    module: &'a WorkspaceGraphProjectionModule,
    checked: BTreeSet<ResolvedType>,
    owned: BTreeSet<ResolvedType>,
    checked_nodes: usize,
    projected_nodes: usize,
}

impl<'a> Types<'a> {
    pub(super) fn new(
        revision: &'a ProjectRevision,
        program: &'a Program,
        target: &str,
    ) -> Result<Self> {
        let mut selected = None;
        for module in revision.semantic.image_modules() {
            for _function in module
                .functions()
                .iter()
                .filter(|function| function.id.as_str() == target)
            {
                if module.path() != program.path || module.module() != program.module {
                    return Err(invalid(
                        "extraction checked function has a different source owner",
                    ));
                }
                if selected.replace(module).is_some() {
                    return Err(invalid("extraction checked function identity is ambiguous"));
                }
            }
        }
        Ok(Self {
            revision,
            program,
            module: selected.ok_or_else(|| invalid("extraction checked function is absent"))?,
            checked: BTreeSet::new(),
            owned: BTreeSet::new(),
            checked_nodes: 0,
            projected_nodes: 0,
        })
    }

    pub(super) fn check(&mut self, ty: &ResolvedType) -> Result<()> {
        if scalar(ty).is_some() {
            return Ok(());
        }
        let ResolvedType::Nominal { arguments, .. } = ty else {
            return Err(invalid(
                "extraction requires direct scalar or checked Copy nominal values",
            ));
        };
        if arguments.len() > intent::MAX_AGGREGATE_TYPE_ARGUMENTS {
            return Err(limit(
                "extraction nominal type arguments exceed their bound",
            ));
        }
        if arguments
            .iter()
            .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
        {
            return Err(invalid(
                "extraction nominal arguments require direct i64 or bool",
            ));
        }
        if self.checked.contains(ty) {
            return Ok(());
        }
        charge(&mut self.checked_nodes, 1 + arguments.len())?;
        let (kind, facts) = self.module.value_type_facts(ty).ok_or_else(|| {
            invalid("extraction nominal value has no retained checked type facts")
        })?;
        if !matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
            || !facts.copy
            || !facts.sized
            || facts.needs_drop
            || facts.contains_resource
        {
            return Err(invalid("extraction nominal values require checked sized Copy records or variants without owned cleanup or resources"));
        }
        self.checked.insert(ty.clone());
        Ok(())
    }

    pub(super) fn ast(&mut self, ty: &ResolvedType) -> Result<Type> {
        self.check(ty)?;
        if let Some(scalar) = scalar(ty) {
            return Ok(scalar);
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            unreachable!("check admitted scalar or nominal type");
        };
        // Bound the actual added annotations separately from distinct checked
        // type facts, before allocating argument JSON or AST type vectors.
        charge(&mut self.projected_nodes, 1 + arguments.len())?;
        let arguments = arguments
            .iter()
            .map(|argument| match argument {
                ResolvedType::I64 => json!("i64"),
                ResolvedType::Bool => json!("bool"),
                _ => unreachable!("check admitted direct scalar type arguments"),
            })
            .collect::<Vec<_>>();
        intent::nominal_type_plan(
            self.revision,
            self.program,
            declaration.as_str(),
            &json!(arguments),
        )
    }

    /// Internal owners never become captures or helper results. Their facts
    /// come from the same compiler owner as ordinary source admission.
    pub(super) fn internal(&mut self, ty: &ResolvedType, mode: OwnershipMode) -> Result<bool> {
        if mode == OwnershipMode::Value {
            self.check(ty)?;
            return Ok(false);
        }
        if mode != OwnershipMode::Own {
            return Err(invalid(
                "extraction cannot relocate borrowed or shared values",
            ));
        }
        if self.owned.contains(ty) {
            return Ok(true);
        }
        let facts = match ty {
            ResolvedType::String | ResolvedType::Bytes => hir::DeclarationIndex::default()
                .type_facts(ty)
                .ok_or_else(|| invalid("extraction internal owner has no compiler type facts"))?,
            ResolvedType::Nominal { arguments, .. } if arguments.is_empty() => {
                let (kind, facts) = self.module.value_type_facts(ty).ok_or_else(|| {
                    invalid("extraction internal nominal owner has no retained type facts")
                })?;
                if !matches!(kind, DeclarationKind::Record | DeclarationKind::Variant) {
                    return Err(invalid("extraction internal owners exclude classes and resources"));
                }
                facts.clone()
            }
            _ => return Err(invalid("extraction internal owners require String, Bytes or monomorphic record/variant types")),
        };
        if facts.copy || !facts.sized || !facts.needs_drop || facts.contains_resource {
            return Err(invalid(
                "extraction internal owners require checked sized resource-free owned data",
            ));
        }
        charge(&mut self.checked_nodes, 1)?;
        self.owned.insert(ty.clone());
        Ok(true)
    }
}

fn charge(nodes: &mut usize, count: usize) -> Result<()> {
    *nodes = nodes
        .checked_add(count)
        .ok_or_else(|| limit("extraction nominal type inventory overflow"))?;
    if *nodes > MAX_NODES {
        return Err(limit("extraction nominal type inventory exceeds its bound"));
    }
    Ok(())
}

fn scalar(ty: &ResolvedType) -> Option<Type> {
    match ty {
        ResolvedType::I64 => Some(Type::I64),
        ResolvedType::I32 => Some(Type::I32),
        ResolvedType::U8 => Some(Type::U8),
        ResolvedType::Usize => Some(Type::Usize),
        ResolvedType::Bool => Some(Type::Bool),
        _ => None,
    }
}
