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
const MODULE_HELP: &str = "a file starts with `module dotted.name;`, then its `@id`-annotated \
                           declarations";
const RETURN_TYPE_HELP: &str = "every function declares its result type after `->`; there is no \
                                unit or implicit result, so return `i64` or `bool`";
const UNIT_TYPE_HELP: &str =
    "there is no unit type; return `i64` (conventionally `0`) or `bool` instead of `()`";
const LET_VALUE_HELP: &str =
    "every `let` binds a value at its declaration; there is no uninitialised binding";
const CONDITION_ASSIGN_HELP: &str =
    "comparison is `==`; a single `=` is assignment, which is a statement and never a condition";
const INDEX_HELP: &str = "there is no indexing syntax; read a byte with `byte_get(view, index)`, which \
                          returns `Option<u8>`, after `array_as_slice(array)` or `bytes_as_slice(bytes)`";

impl Parser {
    /// The mandatory `module dotted.name;` header. A file pasted from another
    /// language, or a snippet saved without its first line, otherwise fails
    /// with a bare ``expected `module` `` that names the rule but not the fix.
    pub(super) fn module_header(&mut self) -> Result<(), Diagnostic> {
        self.keyword("module")
            .map(drop)
            .map_err(|diagnostic| diagnostic.with_help(MODULE_HELP))
    }

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

    /// A declaration keyword from another language where `fn` or a type
    /// declaration was expected.
    pub(super) fn foreign_declaration(&self) -> Option<Diagnostic> {
        let TokenKind::Ident(word) = &self.current().kind else {
            return None;
        };
        let help = match word.as_str() {
            "struct" => "a product type is `record Name { @id(\"…\") field: Type, }`",
            "enum" => {
                "a sum type is `variant Name { @id(\"…\") Case, @id(\"…\") Case { @id(\"…\") field: Type, }, }`"
            }
            "pub" | "public" | "export" => {
                "declarations are reachable through their `@id`; there is no visibility keyword"
            }
            "const" | "static" | "let" | "var" => {
                "there are no module-level values; declare `fn name() -> i64 { value }` and call it"
            }
            "trait" => "`class Child : Parent` inherits methods and `protocol` declares method requirements",
            "type" | "typedef" => "type aliases are not admitted; write the type at each use",
            _ => return None,
        };
        Some(self.error_here("SPX-P104", "expected `fn`").with_help(help))
    }

    /// `x += 1;` and friends where a statement was expected.
    pub(super) fn compound_assignment(&self) -> Option<Diagnostic> {
        let TokenKind::Ident(name) = &self.current().kind else {
            return None;
        };
        let operator = match self.tokens.get(self.cursor + 1).map(|token| &token.kind)? {
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            _ => return None,
        };
        let follows_eq = matches!(
            self.tokens.get(self.cursor + 2).map(|token| &token.kind),
            Some(TokenKind::Eq)
        );
        follows_eq.then(|| {
            Diagnostic::error(
                "SPX-P201",
                "compound assignment is not admitted",
                self.tokens[self.cursor + 1]
                    .span
                    .merge(self.tokens[self.cursor + 2].span),
            )
            .at_path(&self.path)
            .with_help(format!(
                "write `{name} = {name} {operator} …;`; assignment is a statement with a plain `=`"
            ))
        })
    }

    /// `()` where a type was expected.
    pub(super) fn unit_type(&self) -> Option<Diagnostic> {
        (self.at(&TokenKind::LParen)
            && matches!(
                self.tokens.get(self.cursor + 1).map(|token| &token.kind),
                Some(TokenKind::RParen)
            ))
        .then(|| {
            self.error_here("SPX-P105", "expected type")
                .with_help(UNIT_TYPE_HELP)
        })
    }

    /// Attach the fix for the common ways an `expected …` rejection arises.
    pub(super) fn decorate_expected(
        &self,
        diagnostic: Diagnostic,
        description: &str,
    ) -> Diagnostic {
        if self.at(&TokenKind::LBracket) {
            return diagnostic.with_help(INDEX_HELP);
        }
        if let Some(noun) = description.strip_prefix("`,` after ") {
            if self.at(&TokenKind::RBrace) {
                return diagnostic.with_help(format!(
                    "every {noun} ends with `,`, including the last one before `}}`"
                ));
            }
            return diagnostic;
        }
        match description {
            "`->` before return type" => diagnostic.with_help(RETURN_TYPE_HELP),
            "`=` in local binding" if self.at(&TokenKind::Semicolon) => {
                diagnostic.with_help(LET_VALUE_HELP)
            }
            "`{` before `if` condition" | "`{` before `while` body" if self.at(&TokenKind::Eq) => {
                diagnostic.with_help(CONDITION_ASSIGN_HELP)
            }
            _ => diagnostic,
        }
    }

    /// A parenthesised expression followed by `,`: a tuple literal.
    pub(super) fn tuple_literal(&self) -> Option<Diagnostic> {
        self.at(&TokenKind::Comma).then(|| {
            self.error_here("SPX-P106", "expected `)` after expression")
                .with_help(TUPLE_HELP)
        })
    }
}
