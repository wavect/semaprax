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

fn reject_token_nesting(tokens: &[Token], path: &str) -> Result<(), Diagnostic> {
    let mut delimiters = 0usize;
    let mut unary_chain = 0usize;
    for token in tokens {
        match token.kind {
            TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket | TokenKind::Lt => {
                delimiters += 1;
            }
            TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket | TokenKind::Gt => {
                delimiters = delimiters.saturating_sub(1);
            }
            _ => {}
        }
        unary_chain = if matches!(token.kind, TokenKind::Minus | TokenKind::Bang) {
            unary_chain + 1
        } else {
            0
        };
        if delimiters > super::depth::MAX_SOURCE_NESTING
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
    }
    Ok(())
}
