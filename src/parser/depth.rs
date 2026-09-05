use crate::ast::{Program, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;

pub(crate) const MAX_SOURCE_NESTING: usize = 128;

pub(super) fn validate_program(program: Program) -> Result<Program, Diagnostic> {
    let mut roots = Vec::new();
    for function in program
        .functions
        .iter()
        .chain(
            program
                .types
                .iter()
                .flat_map(|declaration| match &declaration.kind {
                    TypeDeclarationKind::Class { methods, .. } => methods.iter(),
                    _ => [].iter(),
                }),
        )
    {
        roots.extend(function.requires.iter());
        roots.extend(function.ensures.iter());
        roots.push(&function.body);
    }
    let mut pending = roots
        .into_iter()
        .map(|expression| (expression, 1usize))
        .collect::<Vec<_>>();
    while let Some((expression, depth)) = pending.pop() {
        if depth > MAX_SOURCE_NESTING {
            return Err(Diagnostic::error(
                "SPX-P207",
                format!("source nesting depth exceeds the admitted maximum ({MAX_SOURCE_NESTING})"),
                expression.span,
            )
            .at_path(&program.path)
            .with_help("split the expression or block into named helper functions"));
        }
        let mut index = 0;
        while let Some(child) = expression.child(index) {
            pending.push((child, depth + 1));
            index += 1;
        }
    }
    Ok(program)
}
