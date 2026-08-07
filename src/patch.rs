use std::collections::BTreeSet;
use std::path::Path;

use crate::ast::{Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::{format, graph, lexer, parse, verify};

#[derive(Debug)]
struct Rename {
    stable_id: String,
    new_name: String,
}

#[derive(Debug)]
struct SemanticPatch {
    base: String,
    renames: Vec<Rename>,
    no_new_effects: bool,
}

pub fn apply(source_path: &Path, patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    let source = std::fs::read_to_string(source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I201",
            format!("cannot read {}: {error}", source_path.display()),
        )]
    })?;
    let patch_source = std::fs::read_to_string(patch_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("cannot read {}: {error}", patch_path.display()),
        )]
    })?;
    let patch = parse_patch(&patch_source)?;
    let before = parse(&source, source_path).map_err(|error| vec![error])?;
    let revision = graph::revision(&before);
    if revision != patch.base {
        return Err(vec![Diagnostic::io(
            "SPX-G409",
            format!(
                "stale semantic patch: expected graph {}, current graph {revision}",
                patch.base
            ),
        )
        .with_help("regenerate the patch against the current semantic graph")]);
    }

    let before_effects = effect_set(&before);
    let mut replacements = Vec::new();
    let tokens =
        lexer::lex(&source, &source_path.display().to_string()).map_err(|error| vec![error])?;
    for rename in &patch.renames {
        if !is_identifier(&rename.new_name) {
            return Err(vec![Diagnostic::io(
                "SPX-G103",
                format!("`{}` is not a valid symbol name", rename.new_name),
            )]);
        }
        if let Some(function) = before
            .functions
            .iter()
            .find(|function| function.stable_id == rename.stable_id)
        {
            if !function.explicit_id {
                return Err(vec![Diagnostic::io(
                    "SPX-G104",
                    format!(
                        "`{}` needs an explicit @id before it can be renamed",
                        function.name
                    ),
                )]);
            }
            for pair in tokens.windows(2) {
                if matches!(&pair[0].kind, lexer::TokenKind::Ident(name) if name == &function.name)
                    && matches!(pair[1].kind, lexer::TokenKind::LParen)
                {
                    replacements.push((
                        pair[0].span.start,
                        pair[0].span.end,
                        rename.new_name.clone(),
                    ));
                }
            }
            continue;
        }
        if let Some(resource) = before.types.iter().find(|declaration| {
            declaration.stable_id == rename.stable_id
                && matches!(declaration.kind, TypeDeclarationKind::Resource)
        }) {
            if !resource.explicit_id {
                return Err(vec![Diagnostic::io(
                    "SPX-G104",
                    format!(
                        "`{}` needs an explicit @id before it can be renamed",
                        resource.name
                    ),
                )]);
            }
            for (start, end) in resource_type_positions(&before, &tokens, resource) {
                replacements.push((start, end, rename.new_name.clone()));
            }
            continue;
        }
        return Err(vec![Diagnostic::io(
            "SPX-G404",
            format!("stable id `{}` does not exist", rename.stable_id),
        )]);
    }
    replacements.sort_by_key(|replacement| replacement.0);
    replacements.dedup_by_key(|replacement| (replacement.0, replacement.1));
    let mut changed = source;
    for (start, end, replacement) in replacements.into_iter().rev() {
        changed.replace_range(start..end, &replacement);
    }

    let after = parse(&changed, source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&after);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    if patch.no_new_effects && !effect_set(&after).is_subset(&before_effects) {
        return Err(vec![Diagnostic::io(
            "SPX-G105",
            "semantic patch violates requirement `no-new-effects`",
        )]);
    }
    let canonical = format::canonical(&after);
    let parent = source_path.parent().unwrap_or_else(|| Path::new("."));
    let name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("module.spx");
    let temporary = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&temporary, canonical).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I203",
            format!("cannot stage semantic patch: {error}"),
        )]
    })?;
    std::fs::rename(&temporary, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I204",
            format!("cannot atomically commit semantic patch: {error}"),
        )]
    })?;
    Ok(graph::revision(&after))
}

fn parse_patch(source: &str) -> Result<SemanticPatch, Vec<Diagnostic>> {
    let mut base = None;
    let mut renames = Vec::new();
    let mut no_new_effects = false;
    for (line_index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<_> = line.split_whitespace().collect();
        match words.as_slice() {
            ["base", revision] => base = Some((*revision).to_owned()),
            ["rename", stable_id, "to", new_name] => renames.push(Rename {
                stable_id: (*stable_id).to_owned(),
                new_name: (*new_name).to_owned(),
            }),
            ["require", "no-new-effects"] => no_new_effects = true,
            _ => {
                return Err(vec![Diagnostic::io(
                    "SPX-G101",
                    format!(
                        "invalid semantic patch instruction on line {}: {line}",
                        line_index + 1
                    ),
                )]);
            }
        }
    }
    let Some(base) = base else {
        return Err(vec![Diagnostic::io(
            "SPX-G102",
            "semantic patch is missing a `base <revision>` instruction",
        )]);
    };
    Ok(SemanticPatch {
        base,
        renames,
        no_new_effects,
    })
}

fn effect_set(program: &crate::ast::Program) -> BTreeSet<&str> {
    program
        .functions
        .iter()
        .flat_map(|function| function.effects.iter().map(String::as_str))
        .collect()
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn resource_type_positions(
    program: &crate::ast::Program,
    tokens: &[lexer::Token],
    resource: &TypeDeclaration,
) -> BTreeSet<(usize, usize)> {
    let mut positions = BTreeSet::from([(resource.name_span.start, resource.name_span.end)]);
    let resource_type = Type::Named(resource.name.clone());

    for declaration in &program.types {
        let TypeDeclarationKind::Record { fields } = &declaration.kind else {
            continue;
        };
        for field in fields {
            if field.ty == resource_type {
                insert_named_type_token(
                    &mut positions,
                    tokens,
                    field.name_span.end,
                    field.span.end,
                    &resource.name,
                );
            }
        }
    }

    for function in &program.functions {
        for param in &function.params {
            if param.ty != resource_type {
                continue;
            }
            let end = tokens
                .iter()
                .find(|token| {
                    token.span.start >= param.span.end
                        && matches!(
                            token.kind,
                            lexer::TokenKind::Comma | lexer::TokenKind::RParen
                        )
                })
                .map_or(function.body.span.start, |token| token.span.start);
            insert_named_type_token(&mut positions, tokens, param.span.end, end, &resource.name);
        }

        if function.return_type == resource_type {
            if let Some(arrow) = tokens.iter().find(|token| {
                token.span.start >= function.name_span.end
                    && token.span.end <= function.body.span.start
                    && matches!(token.kind, lexer::TokenKind::Arrow)
            }) {
                insert_named_type_token(
                    &mut positions,
                    tokens,
                    arrow.span.end,
                    function.body.span.start,
                    &resource.name,
                );
            }
        }
    }

    positions
}

fn insert_named_type_token(
    positions: &mut BTreeSet<(usize, usize)>,
    tokens: &[lexer::Token],
    start: usize,
    end: usize,
    name: &str,
) {
    if let Some(token) = tokens.iter().find(|token| {
        token.span.start >= start
            && token.span.end <= end
            && matches!(&token.kind, lexer::TokenKind::Ident(candidate) if candidate == name)
    }) {
        positions.insert((token.span.start, token.span.end));
    }
}
