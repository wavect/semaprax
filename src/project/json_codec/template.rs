//! Expand compiler-owned markers once; authored replacement bytes stay opaque.

pub(super) fn expand(source: &str, replacements: &[(&str, &str)]) -> String {
    let mut output = String::with_capacity(source.len());
    let mut remaining = source;
    while let Some((offset, marker, value)) = replacements
        .iter()
        .filter_map(|(marker, value)| {
            assert!(!marker.is_empty(), "codec template marker must be nonempty");
            remaining
                .find(*marker)
                .map(|offset| (offset, *marker, *value))
        })
        .min_by_key(|(offset, _, _)| *offset)
    {
        output.push_str(&remaining[..offset]);
        output.push_str(value);
        remaining = &remaining[offset + marker.len()..];
    }
    output.push_str(remaining);
    output
}

/// `_` is an ordinary local name in SEMAPRAX, not a discard pattern. Give
/// generated effect statements fresh local names without rewriting literals,
/// comments, authored source or subsequent marker-like replacement text.
pub(super) fn discard_bindings(
    source: &str,
    authored: &str,
    path: &str,
) -> Result<String, crate::diagnostic::Diagnostic> {
    use crate::lexer::TokenKind;
    // A named `_` use is not a discard. Preserve the whole fragment in that
    // case so this source-only repair never changes binding/reference scope.
    let program = crate::parse(source, path)?;
    for function in &program.functions {
        let mut pending = function
            .requires
            .iter()
            .chain(&function.ensures)
            .chain(std::iter::once(&function.body))
            .collect::<Vec<_>>();
        while let Some(expression) = pending.pop() {
            if matches!(&expression.kind, crate::ast::ExprKind::Var(name) if name == "_") {
                return Ok(source.to_owned());
            }
            if let crate::ast::ExprKind::Block { statements, .. } = &expression.kind {
                if statements.iter().any(|statement| {
                    matches!(statement, crate::ast::Statement::Assign { name, .. } if name == "_")
                }) {
                    return Ok(source.to_owned());
                }
            }
            let mut child = 0;
            while let Some(nested) = expression.child(child) {
                pending.push(nested);
                child += 1;
            }
        }
    }
    let tokens = crate::lexer::lex(source, path)?;
    let authored_tokens = crate::lexer::lex(authored, path)?;
    let mut occupied = tokens
        .iter()
        .chain(&authored_tokens)
        .filter_map(|token| match &token.kind {
            TokenKind::Ident(name) => Some(name.clone()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    let mut next = 0usize;
    for window in tokens.windows(3) {
        if !matches!(&window[0].kind, TokenKind::Ident(name) if name == "let")
            || !matches!(&window[1].kind, TokenKind::Ident(name) if name == "_")
            || window[2].kind != TokenKind::Eq
        {
            continue;
        }
        let name = loop {
            let candidate = format!("json_discard_effect_{next}");
            next += 1;
            if occupied.insert(candidate.clone()) {
                break candidate;
            }
        };
        let span = window[1].span;
        output.push_str(&source[cursor..span.start]);
        output.push_str(&name);
        cursor = span.end;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn replacements_are_opaque_and_unknown_template_text_is_preserved() {
        assert_eq!(
            expand(
                "__NAME__ / __ID__ / __BOUND__ / __UNKNOWN__ / é",
                &[
                    ("__NAME__", "__ID__"),
                    ("__ID__", "a.__BOUND__"),
                    ("__BOUND__", "16")
                ]
            ),
            "__ID__ / a.__BOUND__ / 16 / __UNKNOWN__ / é"
        );
    }

    #[test]
    fn ordinary_marker_expansion_preserves_existing_template_bytes() {
        let source = "__ID__::__NAME__ = __BOUND__; __NAME__";
        let replacements = [
            ("__NAME__", "Record"),
            ("__ID__", "app.record"),
            ("__BOUND__", "64"),
        ];
        let previous = replacements
            .iter()
            .fold(source.to_owned(), |text, (marker, value)| {
                text.replace(*marker, value)
            });
        assert_eq!(expand(source, &replacements), previous);
        assert_eq!(expand(source, &[]), source);
    }

    #[test]
    fn generated_discards_are_fresh_and_literal_comment_bytes_remain_opaque() {
        let source = "module t; fn main()->i64 { let json_discard_effect_0=1; let _=true; let _=false; let text=\"let _=true\"; // let _=false\n0 }";
        let authored = "module t; fn json_discard_effect_1()->i64{0}";
        let revised = super::discard_bindings(source, authored, "t.spx").unwrap();
        assert!(revised.contains("let json_discard_effect_2=true"));
        assert!(revised.contains("let json_discard_effect_3=false"));
        assert!(revised.contains("\"let _=true\""));
        assert!(revised.contains("// let _=false"));
        crate::check(&revised, "t.spx").unwrap();
        assert_eq!(
            revised,
            super::discard_bindings(source, authored, "t.spx").unwrap()
        );
    }

    #[test]
    fn non_discard_bindings_and_authored_markers_keep_their_exact_bytes() {
        let source = "module t; @id(\"let _=__NAME__\") fn main()->i64 { let kept=7; kept }";
        assert_eq!(
            super::discard_bindings(source, source, "t.spx").unwrap(),
            source
        );
        let referenced = "module t; fn main()->i64 { let _=7; _ }";
        assert_eq!(
            super::discard_bindings(referenced, "", "t.spx").unwrap(),
            referenced
        );
        let assigned = "module t; fn main()->i64 { let _=7; _=8; 0 }";
        assert_eq!(
            super::discard_bindings(assigned, "", "t.spx").unwrap(),
            assigned
        );
        let wildcard = "module t; fn main()->i64 { let _=true; match 1 { _ => 7, } }";
        let revised = super::discard_bindings(wildcard, "", "t.spx").unwrap();
        assert!(revised.contains("match 1 { _ => 7, }"));
        assert!(revised.contains("let json_discard_effect_0=true"));
    }
}
