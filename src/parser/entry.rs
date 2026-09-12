//! Parser entry points: from source text, and from source text while keeping
//! the comments the lexer collected so they can travel alongside the program to
//! the canonical formatter.

use std::path::Path;

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::lexer::{self, Comments, Token, TokenKind};

use super::Parser;

impl Parser {
    pub fn new(source: &str, path: &Path) -> Result<Self, Diagnostic> {
        let path = path.display().to_string();
        let tokens = lexer::lex(source, &path)?;
        reject_token_nesting(&tokens, &path)?;
        Ok(Self {
            tokens,
            cursor: 0,
            path,
        })
    }

    /// Lex `source`, keep its comments, and parse the tokens. The program is
    /// exactly what [`crate::parse`] returns; the comments are the lexer's
    /// trivia in source order.
    pub(crate) fn parse_with_comments(
        source: &str,
        path: &Path,
    ) -> Result<(Program, Comments), Diagnostic> {
        let path = path.display().to_string();
        let (tokens, comments) = lexer::lex_with_comments(source, &path)?;
        reject_token_nesting(&tokens, &path)?;
        let program = Parser {
            tokens,
            cursor: 0,
            path,
        }
        .parse()?;
        Ok((program, comments))
    }
}

/// Bounds pre-parse "nesting" before any AST exists, so a pathological input
/// cannot exhaust the recursive-descent parser's own stack before there is a
/// tree for [`super::depth::validate_program`] to walk. `(`/`)`, `{`/`}`, and
/// `[`/`]` are always paired in any syntactically valid program, so a blind
/// running counter over them is sound. `<`/`>` are not: SEMAPRAX uses `<` both
/// to open a generic-argument list (`Container<T>`) and as the plain
/// less-than comparison operator, and a comparison's `<` need never be
/// followed by any `>` at all (see issue #247). Counting every `<` as an
/// opener matched only by a literal `>`, as this used to, means a file of
/// many independent, syntactically shallow comparisons (`a < b`, `c < d`,
/// ... never closed by a `>` anywhere) accumulates against the same budget
/// as real nesting, even though none of them nest.
///
/// So `<`/`>` get their own tentative counter, `generic_depth`, that only
/// accumulates while the token run since the last unmatched `<` still looks
/// like a generic-argument list: an identifier (optionally `.`-qualified)
/// followed immediately by `<`, and then only `Ident`/`.`/`,` tokens and
/// further nested `<`/`>` until it closes. The instant a token appears that
/// cannot occur inside a type-argument list, the run is proven to have been
/// a comparison (or otherwise not a generic), and it stops contributing:
/// `generic_depth` resets to 0 without requiring a matching `>`. Reaching a
/// `)`, `}`, or `]` with `generic_depth` still nonzero also resets it, because
/// a real generic-argument list always closes its `>` before the bracket that
/// encloses it (see `type_arguments` in `parser/types.rs`), so an enclosing
/// close proves the open `<` wasn't one.
///
/// This is a lexer-level heuristic, not a re-parse, so it can still miscount
/// in one direction: many bare comparisons joined only by commas inside a
/// single unclosed bracketed list (e.g. `f(a < b, c < d, e < f, ...)` with no
/// keyword, literal, or other token breaking the run before its closing `)`)
/// still accumulate together, because commas and identifiers are exactly what
/// a real multi-argument generic list also contains. This is far narrower
/// than the defect it replaces -- it requires that many flat comparisons
/// share one unclosed comma list, not merely one file -- and true generic
/// nesting (`Container<Container<Container<...>>>`) is still rejected
/// correctly (see the `token_level_precheck_still_refuses_genuinely_nested_generic_brackets`
/// regression), which is the property this check exists to guarantee.
fn reject_token_nesting(tokens: &[Token], path: &str) -> Result<(), Diagnostic> {
    let mut delimiters = 0usize;
    let mut generic_depth = 0usize;
    let mut unary_chain = 0usize;
    let mut previous_is_ident = false;
    for token in tokens {
        match token.kind {
            TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket => {
                delimiters += 1;
            }
            TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket => {
                delimiters = delimiters.saturating_sub(1);
                generic_depth = 0;
            }
            TokenKind::Lt if generic_depth > 0 || previous_is_ident => {
                generic_depth += 1;
            }
            TokenKind::Gt if generic_depth > 0 => {
                generic_depth -= 1;
            }
            TokenKind::Ident(_) | TokenKind::Dot | TokenKind::Comma => {}
            _ => {
                generic_depth = 0;
            }
        }
        unary_chain = if matches!(token.kind, TokenKind::Minus | TokenKind::Bang) {
            unary_chain + 1
        } else {
            0
        };
        if delimiters > super::depth::MAX_SOURCE_NESTING
            || generic_depth > super::depth::MAX_SOURCE_NESTING
            || unary_chain > super::depth::MAX_SOURCE_NESTING
        {
            return Err(Diagnostic::error(
                "SPX-P207",
                format!(
                    "source nesting depth exceeds the admitted maximum ({})",
                    super::depth::MAX_SOURCE_NESTING
                ),
                token.span,
            )
            .at_path(path)
            .with_help("split the expression or block into named helper functions"));
        }
        previous_is_ident = matches!(token.kind, TokenKind::Ident(_));
    }
    Ok(())
}
