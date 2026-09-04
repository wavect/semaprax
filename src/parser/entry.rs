//! Parser entry points: from source text, and from source text while keeping
//! the comments the lexer collected so they can travel alongside the program to
//! the canonical formatter.

use std::path::Path;

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::lexer::{self, Comments};

use super::Parser;

impl Parser {
    pub fn new(source: &str, path: &Path) -> Result<Self, Diagnostic> {
        let path = path.display().to_string();
        Ok(Self {
            tokens: lexer::lex(source, &path)?,
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
        let program = Parser {
            tokens,
            cursor: 0,
            path,
        }
        .parse()?;
        Ok((program, comments))
    }
}
