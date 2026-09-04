//! Fix hints for habits carried over from other languages.
//!
//! The grammar is closed and small, so the first thing a newcomer or a coding
//! agent writes is often a construct that does not exist here: `return`, `for`,
//! `else if`, an expression statement, a tuple, or a `Some(x)` pattern. Left
//! alone, each surfaces as a bare ``expected `}` after block`` and costs another
//! edit-check cycle to diagnose. These helpers recognise the habit at the point
//! where the grammar already rejects it and attach the fix.
//!
//! They never change what parses. Every hinted diagnostic keeps the stable code
//! the grammar produced for that input, and no hint admits new syntax: each
//! recogniser fires only on a token sequence that was already an error.

use crate::ast::MatchPattern;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;

const RETURN_MESSAGE: &str = "`return` is not admitted; a block's value is its final expression";
const RETURN_HELP: &str =
    "delete `return` and the trailing `;` so the value is the block's last expression";
const LOOP_MESSAGE: &str = "only `while` loops are admitted";
const LOOP_HELP: &str =
    "write `while <condition> { <statements>; <condition> }`; the body's final \
                         expression is the bool that decides whether to loop again";
const EXPRESSION_STATEMENT_HELP: &str = "a block is statements followed by exactly one final value \
                                         expression; discard an intermediate call with `let _ = …;` \
                                         or move it to the end of the block";
const WHILE_BODY_HELP: &str = "end the `while` body with the bool that decides whether to loop \
                               again, usually the loop condition repeated";
const BRANCH_HELP: &str = "`if` is an expression: end each branch with its value, for example the \
                           binding the branch assigned";
const FUNCTION_BODY_HELP: &str = "a function's value is its final expression; there is no `return`";
const ELSE_IF_MESSAGE: &str = "`else if` is not admitted";
const ELSE_IF_HELP: &str =
    "nest the second `if` inside the else block: `else { if <condition> { … } else { … } }`";
const MISSING_ELSE_HELP: &str = "`if` is an expression and always has an `else` branch";
const CALL_PATTERN_HELP: &str =
    "variant patterns name the case and its fields: `Option::Some { value: v }`, not `Some(v)`";
const TUPLE_HELP: &str = "tuples are not admitted; declare a `record` with named fields";

impl Parser {
    /// `return <expr>`, `for <name> …`, or `loop {` where a statement or the
    /// block's tail expression was expected. The word is an ordinary
    /// identifier to the lexer, so this fires only when the following token
    /// could not continue an expression rooted at that identifier; a binding
    /// that happens to be called `return` still parses as before.
    pub(super) fn foreign_statement(&self) -> Option<Diagnostic> {
        let TokenKind::Ident(word) = &self.current().kind else {
            return None;
        };
        let (message, help) = match word.as_str() {
            "return" => (RETURN_MESSAGE, RETURN_HELP),
            "for" | "loop" => (LOOP_MESSAGE, LOOP_HELP),
            _ => return None,
        };
        let next = self.tokens.get(self.cursor + 1).map(|token| &token.kind)?;
        let begins_operand = matches!(
            next,
            TokenKind::Ident(_)
                | TokenKind::Int(_)
                | TokenKind::Int32(_)
                | TokenKind::Float(_)
                | TokenKind::Char(_)
                | TokenKind::Uint8(_)
                | TokenKind::Usize(_)
                | TokenKind::String(_)
                | TokenKind::LBracket
                | TokenKind::LBrace
                | TokenKind::Bang
        );
        begins_operand.then(|| self.error_here("SPX-P106", message).with_help(help))
    }

    /// A tail expression was terminated with `;` and the block continues, which
    /// is how every other language spells an expression statement.
    pub(super) fn expression_statement(&self) -> Diagnostic {
        self.error_here("SPX-P106", "expected `}` after block")
            .with_help(EXPRESSION_STATEMENT_HELP)
    }

    /// Give a block that ended without a value the help that fits the position
    /// it was parsed in. Inner blocks attach their own help first and keep it.
    pub(super) fn attach_block_help(diagnostic: Diagnostic, description: &str) -> Diagnostic {
        if diagnostic.code != "SPX-P203" || diagnostic.help.is_some() {
            return diagnostic;
        }
        let help = match description {
            "`while` body" => WHILE_BODY_HELP,
            "`if` condition" | "`else`" => BRANCH_HELP,
            "function body" => FUNCTION_BODY_HELP,
            _ => return diagnostic,
        };
        diagnostic.with_help(help)
    }

    /// `else` followed by `if` instead of a block.
    pub(super) fn else_if(&self) -> Option<Diagnostic> {
        self.at_keyword("if").then(|| {
            self.error_here("SPX-P106", ELSE_IF_MESSAGE)
                .with_help(ELSE_IF_HELP)
        })
    }

    /// The `else` branch is missing from an `if`.
    pub(super) fn missing_else(diagnostic: Diagnostic) -> Diagnostic {
        diagnostic.with_help(MISSING_ELSE_HELP)
    }

    /// A bare binding pattern immediately followed by `(`: the Rust and ML
    /// spelling of a payload pattern.
    pub(super) fn call_pattern(&self, pattern: &MatchPattern) -> Option<Diagnostic> {
        (matches!(pattern, MatchPattern::Binding { .. }) && self.at(&TokenKind::LParen)).then(
            || {
                self.error_here("SPX-P106", "expected `=>` after match pattern")
                    .with_help(CALL_PATTERN_HELP)
            },
        )
    }

    /// A parenthesised expression followed by `,`: a tuple literal.
    pub(super) fn tuple_literal(&self) -> Option<Diagnostic> {
        self.at(&TokenKind::Comma).then(|| {
            self.error_here("SPX-P106", "expected `)` after expression")
                .with_help(TUPLE_HELP)
        })
    }
}
