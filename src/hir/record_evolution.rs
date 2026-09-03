//! Selected checked-declaration TypeFacts for source-only record evolution.
//! No whole-project index is retained and no Copy rule is duplicated here.
use super::*;

const MAX_DECLARATIONS: usize = 4096;
const MAX_VISITS: usize = 1_048_576;
const MAX_DEPTH: usize = 256;
const MAX_BYTES: usize = 16 * 1024 * 1024;

struct Budget {
    visits: usize,
    bytes: usize,
}

impl Budget {
    fn charge(&mut self, bytes: usize, depth: usize) -> Result<(), Diagnostic> {
        self.visits = self.visits.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes).saturating_add(256);
        if self.visits > MAX_VISITS || self.bytes > MAX_BYTES || depth > MAX_DEPTH {
            return Err(capacity());
        }
        Ok(())
    }
}

fn capacity() -> Diagnostic {
    Diagnostic::io(
        "SPX-G226",
        "selected record TypeFacts closure exceeds its bound",
    )
}

impl DeclarationIndex {
    /// Reconstruct only the selected nominal dependency closure from immutable
    /// checked declarations. Callers must supply the exact retained source
    /// inventory; this is not an admission API for authored or forged HIR.
    pub(crate) fn record_evolution_type_facts(
        selected: &DeclarationId,
        declarations: &BTreeMap<&str, &ResolvedTypeDeclaration>,
    ) -> Result<Option<TypeFacts>, Diagnostic> {
        Self::record_evolution_concrete_type_facts(
            &ResolvedType::Nominal {
                declaration: selected.clone(),
                arguments: Vec::new(),
            },
            declarations,
        )
    }

    /// Reconstruct checked facts for one exact concrete record/variant type.
    /// This is retained-HIR analysis only and grants no source or target authority.
    pub(crate) fn record_evolution_concrete_type_facts(
        ty: &ResolvedType,
        declarations: &BTreeMap<&str, &ResolvedTypeDeclaration>,
    ) -> Result<Option<TypeFacts>, Diagnostic> {
        let ResolvedType::Nominal { declaration, .. } = ty else {
            return Ok(None);
        };
        let mut index = compiler_prelude_declarations()?;
        let mut visited = BTreeSet::new();
        let mut budget = Budget {
            visits: 0,
            bytes: 0,
        };
        if !retain_declaration(
            declaration,
            declarations,
            &mut index,
            &mut visited,
            &mut budget,
            0,
        )? {
            return Ok(None);
        }
        // The ordinary owner computes concrete generic substitutions, layout,
        // Copy/resource/drop flags, and recursive-type rejection. Its layout
        // output is bounded here even when this query has no outer builder.
        let (facts, overflowed) =
            crate::bounded_output::with_limit(MAX_BYTES, || index.type_facts(ty));
        if overflowed {
            return Err(capacity());
        }
        Ok(facts)
    }
}

fn retain_declaration(
    id: &DeclarationId,
    declarations: &BTreeMap<&str, &ResolvedTypeDeclaration>,
    index: &mut DeclarationIndex,
    visited: &mut BTreeSet<DeclarationId>,
    budget: &mut Budget,
    depth: usize,
) -> Result<bool, Diagnostic> {
    budget.charge(id.as_str().len(), depth)?;
    if crate::prelude::is_compiler_owned_id(id.as_str()) {
        return Ok(index.declaration(id).is_some());
    }
    if !visited.insert(id.clone()) {
        return Ok(true);
    }
    if visited.len() > MAX_DECLARATIONS {
        return Err(capacity());
    }
    let Some(declaration) = declarations.get(id.as_str()).copied() else {
        return Ok(false);
    };
    if declaration.id != *id {
        return Ok(false);
    }
    budget.charge(declaration.name.len(), depth)?;
    for parameter in &declaration.type_parameters {
        budget.charge(parameter.name.len(), depth)?;
    }
    let kind = match &declaration.kind {
        ResolvedTypeDeclarationKind::Record { fields } => {
            for field in fields {
                if !retain_field(field, declarations, index, visited, budget, depth + 1)? {
                    return Ok(false);
                }
            }
            DeclarationKind::Record
        }
        ResolvedTypeDeclarationKind::Variant { cases } => {
            for case in cases {
                budget.charge(case.id.as_str().len() + case.name.len(), depth)?;
                for field in &case.fields {
                    if !retain_field(field, declarations, index, visited, budget, depth + 1)? {
                        return Ok(false);
                    }
                }
            }
            DeclarationKind::Variant
        }
        ResolvedTypeDeclarationKind::Class { .. }
        | ResolvedTypeDeclarationKind::Resource { .. } => return Ok(false),
    };
    // No display-name lookup is used by type_facts: all edges retain checked
    // declaration IDs, including foreign-module and compiler-prelude types.
    index.insert_top_level(
        declaration.name.clone(),
        id.clone(),
        kind,
        IdentityOrigin::Explicit,
    );
    index
        .type_parameters
        .insert(id.clone(), declaration.type_parameters.clone());
    match &declaration.kind {
        ResolvedTypeDeclarationKind::Record { fields } => {
            index.record_fields.insert(id.clone(), fields.clone());
        }
        ResolvedTypeDeclarationKind::Variant { cases } => {
            index.variant_cases.insert(id.clone(), cases.clone());
        }
        _ => unreachable!("unsupported dependency rejected above"),
    }
    Ok(true)
}

fn retain_field(
    field: &ResolvedFieldDeclaration,
    declarations: &BTreeMap<&str, &ResolvedTypeDeclaration>,
    index: &mut DeclarationIndex,
    visited: &mut BTreeSet<DeclarationId>,
    budget: &mut Budget,
    depth: usize,
) -> Result<bool, Diagnostic> {
    budget.charge(field.id.as_str().len() + field.name.len(), depth)?;
    retain_type(&field.ty, declarations, index, visited, budget, depth)
}

fn retain_type(
    ty: &ResolvedType,
    declarations: &BTreeMap<&str, &ResolvedTypeDeclaration>,
    index: &mut DeclarationIndex,
    visited: &mut BTreeSet<DeclarationId>,
    budget: &mut Budget,
    depth: usize,
) -> Result<bool, Diagnostic> {
    budget.charge(0, depth)?;
    match ty {
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            for argument in arguments {
                if !retain_type(argument, declarations, index, visited, budget, depth + 1)? {
                    return Ok(false);
                }
            }
            retain_declaration(declaration, declarations, index, visited, budget, depth + 1)
        }
        ResolvedType::TypeParameter { owner, .. } => {
            budget.charge(owner.as_str().len(), depth)?;
            Ok(true)
        }
        _ => Ok(true),
    }
}
