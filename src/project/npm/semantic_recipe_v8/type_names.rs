//! Collision-only source aliases. Presentation names are authenticated recipe
//! data, never nominal identity or filesystem/publication authority.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    LinkedDeclarationFact, LinkedOwnedDataParts, LinkedScalarFunction, ResolvedProgram,
    ResolvedTypeDeclaration,
};

use super::{ensure_bound, package_error};

// Wire: PREFIX + canonical JSON [[stable_id, original_name], ...] + LF.
// Rows cover every authored type in strict stable-ID order; row ordinal fixes
// its source alias. No header is emitted or admitted without a name collision.
const PREFIX: &str = "// semaprax-owned-data-type-names.v1 ";
type Rows = Vec<[String; 2]>;

fn alias(index: usize) -> String {
    format!("SpxRecipeType{index}")
}

/// Called with the complete stable-ID-sorted authored type inventory. Replacing
/// every authored name only in the collision case prevents an original name
/// from colliding with an alias; compiler prelude names remain untouched.
pub(super) fn apply_aliases(
    authored: &[&ResolvedTypeDeclaration],
    names: &mut BTreeMap<String, String>,
) -> Result<String, Diagnostic> {
    let mut seen = BTreeSet::new();
    if authored.iter().all(|ty| seen.insert(ty.name.as_str())) {
        return Ok(String::new());
    }
    let mut rows = Vec::new();
    for (index, ty) in authored.iter().enumerate() {
        validate_name(&ty.name)?;
        rows.push([ty.id.as_str().to_owned(), ty.name.clone()]);
        names.insert(ty.id.as_str().to_owned(), alias(index));
    }
    render_header(&rows)
}

fn render_header(rows: &[[String; 2]]) -> Result<String, Diagnostic> {
    let mut header = String::from(PREFIX);
    header.push('[');
    for (index, [id, name]) in rows.iter().enumerate() {
        if index != 0 {
            header.push(',');
        }
        header.push('[');
        header.push_str(&quote_json(id));
        header.push(',');
        header.push_str(&quote_json(name));
        header.push(']');
        ensure_bound(&header)?;
    }
    header.push_str("]\n");
    ensure_bound(&header)?;
    Ok(header)
}

/// The caller checks the entire 1 MiB recipe before this parser allocates. The
/// closed array-of-pairs grammar has no object keys and cannot hide duplicate
/// keys. Sorting, lexical validity, and exact JSON spelling are checked before
/// the source parser or reconstruction sees the names.
pub(super) fn read_header(recipe: &str) -> Result<(Option<Rows>, &str), Diagnostic> {
    ensure_bound(recipe)?;
    let Some(rest) = recipe.strip_prefix(PREFIX) else {
        return Ok((None, recipe));
    };
    let (json, body) = rest
        .split_once('\n')
        .ok_or_else(|| package_error("owned-data type-name header is unterminated"))?;
    let rows: Rows = serde_json::from_str(json)
        .map_err(|_| package_error("owned-data type-name header has an invalid shape"))?;
    let mut previous = None;
    let mut names = BTreeSet::new();
    let mut collision = false;
    for [id, name] in &rows {
        if previous.is_some_and(|prior: &str| prior >= id.as_str())
            || crate::prelude::is_compiler_owned_id(id)
        {
            return Err(package_error(
                "owned-data type-name identities are not canonical",
            ));
        }
        previous = Some(id.as_str());
        validate_name(name)?;
        collision |= !names.insert(name.as_str());
    }
    if !collision {
        return Err(package_error(
            "owned-data type-name header has no name collision",
        ));
    }
    let canonical = render_header(&rows)?;
    if recipe.strip_prefix(&canonical) != Some(body) {
        return Err(package_error(
            "owned-data type-name header is not canonical",
        ));
    }
    Ok((Some(rows), body))
}

fn validate_name(name: &str) -> Result<(), Diagnostic> {
    if crate::prelude::is_reserved_type_name(name) {
        return Err(package_error(
            "owned-data type-name header uses a reserved type name",
        ));
    }
    // Parser::ident accepts exactly Ident, including source's contextual
    // keywords. Exact token text also rejects leading/trailing trivia/comments.
    let tokens = crate::lexer::lex(name, "semaprax-owned-data-type-name")
        .map_err(|_| package_error("owned-data type-name header has an invalid identifier"))?;
    if !matches!(tokens.as_slice(), [first, last]
        if matches!(&first.kind, crate::lexer::TokenKind::Ident(value) if value == name)
            && last.kind == crate::lexer::TokenKind::Eof)
    {
        return Err(package_error(
            "owned-data type-name header has an invalid identifier",
        ));
    }
    Ok(())
}

pub(super) fn restore(
    mut program: ResolvedProgram,
    rows: Rows,
) -> Result<ResolvedProgram, Diagnostic> {
    let mut authored = program
        .types
        .iter_mut()
        .filter(|ty| !crate::prelude::is_compiler_owned_id(ty.id.as_str()))
        .collect::<Vec<_>>();
    authored.sort_by(|left, right| left.id.cmp(&right.id));
    if authored.len() != rows.len() {
        return Err(package_error(
            "owned-data type-name header changes the type inventory",
        ));
    }
    for (index, (ty, [id, name])) in authored.into_iter().zip(rows).enumerate() {
        if ty.id.as_str() != id || ty.name != alias(index) {
            return Err(package_error(
                "owned-data type-name header changes a type identity or alias",
            ));
        }
        ty.name = name;
    }

    let facts = program
        .declarations
        .workspace_declarations()
        .into_iter()
        .filter(|fact| !crate::prelude::is_compiler_owned_id(fact.id.as_str()))
        .map(|fact| {
            (
                fact.id,
                LinkedDeclarationFact {
                    kind: fact.kind,
                    origin: fact.identity_origin,
                    owner: fact.owner,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let order = program
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| (function.id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let functions = program
        .functions
        .into_iter()
        .map(|function| {
            let origin = facts
                .get(&function.id)
                .ok_or_else(|| package_error("owned-data recipe function identity is unavailable"))?
                .origin;
            Ok(LinkedScalarFunction { function, origin })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    program
        .types
        .retain(|ty| !crate::prelude::is_compiler_owned_id(ty.id.as_str()));
    let mut restored = crate::hir::link_owned_data_api_workspace(
        program.module,
        program.entrypoint,
        functions,
        LinkedOwnedDataParts {
            permits: program.permits,
            types: program.types,
            interfaces: program.interfaces,
            declaration_facts: facts,
            function_templates: program.function_templates,
            function_instances: program.function_instances,
        },
    )
    .map_err(|_| package_error("owned-data type-name restoration does not validate"))?;
    // The linker has a canonical stable-ID order; recipes historically number
    // function display aliases by the supplied order instead. Preserve that
    // already-authenticated order without mutating identities or expressions.
    if restored
        .functions
        .iter()
        .any(|function| !order.contains_key(&function.id))
    {
        return Err(package_error(
            "owned-data type-name restoration changes the function inventory",
        ));
    }
    restored
        .functions
        .sort_by_key(|function| order.get(&function.id).copied());
    crate::hir::validate(&restored)
        .map_err(|_| package_error("owned-data type-name restoration does not validate"))?;
    Ok(restored)
}

#[cfg(test)]
mod tests;
