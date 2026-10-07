//! Fix hints for habits carried over from other languages.
//!
//! The grammar is closed and small, so the first thing a newcomer or a coding
//! agent writes is often a construct that does not exist here: `return`, `for`,
//! an expression statement, a tuple, or a `Some(x)` pattern. Left
//! alone, each surfaces as a bare ``expected `}` after block`` and costs another
//! edit-check cycle to diagnose. These helpers recognise the habit at the point
//! where the grammar already rejects it and attach the fix.
//!
//! They never change what parses. Every hinted diagnostic keeps the stable code
//! the grammar produced for that input, and no hint admits new syntax: each
//! recogniser fires only on a token sequence that was already an error.

use crate::ast::{Expr, ExprKind, MatchPattern};
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;

const RETURN_MESSAGE: &str = "`return` is not admitted; a block's value is its final expression";
const RETURN_HELP: &str =
    "delete `return` and the trailing `;` so the value is the block's last expression";
const LOOP_MESSAGE: &str = "only `while` loops are admitted";
const LOOP_HELP: &str =
    "write `while <condition> { <statements>; <tail> }`; the condition controls repetition and \
                         the required body tail is discarded";
const EXPRESSION_STATEMENT_HELP: &str = "a block is statements followed by exactly one final value \
                                         expression; discard an intermediate call with `let _ = …;` \
                                         or move it to the end of the block";
/// `println("…");` as a statement: the habit is printing, not the statement.
const PRINT_NAMES: [&str; 6] = ["print", "println", "printf", "puts", "console_log", "echo"];
const PRINT_HELP: &str = "there is no print routine; bind the text, then `let view = \
                          string_as_str(text); let written = stdout_write(str_as_bytes(view));` \
                          under `permit { process.stdout.write }` and `uses { process.stdout.write }`";
const WHILE_BODY_HELP: &str = "end the `while` body with a final expression; its value is \
                               discarded because the condition controls repetition";
const FOR_BODY_HELP: &str = "end the `for` body with a final expression, such as `0`; its value \
                             is discarded";
const BRANCH_HELP: &str =
    "a statement `if` yields no value, so the enclosing block still ends with \
                           a final expression after it; to make the `if` the value, end every \
                           branch with a value and add an `else` branch";
const FUNCTION_BODY_HELP: &str = "a function's value is its final expression; there is no `return`";
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
const TERNARY_HELP: &str =
    "there is no `? :` operator; `if` is an expression: `if <condition> { a } else { b }`";
const CAST_HELP: &str =
    "there are no casts or numeric conversions; keep a computation in one integer \
                         type and suffix literals to match it, such as `5i32` or `5usize`";
const BREAK_HELP: &str = "there is no `break` or `continue`; put the exit test in the `while` \
                          condition, for example with a `let mut done = false;` flag";
/// `|x| …` or `|| …` where an expression was expected: closure syntax.
pub(super) const CLOSURE_HELP: &str =
    "an anonymous function is `fn(x: i64) -> i64 { x + 1 }`; parameter and result types are required";
const USE_HELP: &str = "`use` imports one declaration of a project module: `use function \
                        @id(\"stable.id\") from other.module as name;`; compiler-owned functions such \
                        as `string_concat` need no import";
/// `[1, 2, 3]`: array literals hold only bytes.
pub(super) const ARRAY_LITERAL_HELP: &str = "array literals hold bytes: `[1u8, 2u8]`; a list of other \
                                             values is a `Vec`: `vec_push<i64>(vec_with_capacity<i64>(3usize), 1)`";
const IF_LET_HELP: &str = "there is no `if let` or `while let`; `match` the value: `match o { \
                           Option::Some { value: v } => v, Option::None {} => 0, }`";
const RANGE_PATTERN_HELP: &str = "range patterns are not admitted; bind the value and guard the \
                                  arm: `n if n >= 0 && n <= 5 => …,`";
/// `i++` or `i--`: increment operators from C-family languages.
pub(super) const INCREMENT_HELP: &str =
    "there is no `++` or `--`; write `i = i + 1;` with `i` declared `let mut`";
const MACRO_HELP: &str = "there are no macros; build text with `string_concat(a, b)` and \
                          `string_from_i64(n)`, and print with `stdout_write` (`semaprax help \
                          language strings`)";
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

    /// A range's second dot otherwise looks like a missing projected field.
    /// Keep the established foreign-loop diagnostic without intercepting an
    /// admitted vector traversal or unrelated expression errors.
    /// Range `for` loops are a distinct subset: they remain P105 with a
    /// range-specific hint, while generic `for`/`loop` stay P106.
    pub(super) fn range_for_hint(&self, mut diagnostic: Diagnostic) -> Diagnostic {
        if diagnostic.code == "SPX-P105"
            && self
                .cursor
                .checked_sub(2)
                .and_then(|index| self.tokens.get(index..self.cursor))
                .is_some_and(|tokens| tokens.iter().all(|token| token.kind == TokenKind::Dot))
        {
            diagnostic.message = "range `for` loops are not admitted; use `while`".to_owned();
            diagnostic.with_help(LOOP_HELP)
        } else {
            diagnostic
        }
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
        let next = self.tokens.get(self.cursor + 1).map(|token| &token.kind)?;
        if matches!(word.as_str(), "break" | "continue")
            && matches!(next, TokenKind::Semicolon | TokenKind::RBrace)
        {
            return Some(
                self.error_here("SPX-P106", format!("`{word}` is not admitted"))
                    .with_help(BREAK_HELP),
            );
        }
        if matches!(next, TokenKind::Semicolon) {
            return Some(
                self.error_here("SPX-P106", "expected `}` after block")
                    .with_help(EXPRESSION_STATEMENT_HELP),
            );
        }
        let (message, help) = match word.as_str() {
            "return" => (RETURN_MESSAGE, RETURN_HELP),
            "for" | "loop" => (LOOP_MESSAGE, LOOP_HELP),
            _ => return None,
        };
        let begins_operand = matches!(
            next,
            TokenKind::Ident(_)
                | TokenKind::Int(_)
                | TokenKind::IntMinMagnitude
                | TokenKind::Int32(_)
                | TokenKind::Int32MinMagnitude
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
    pub(super) fn expression_statement(&self, tail: &Expr) -> Diagnostic {
        let help = match &tail.kind {
            ExprKind::Call { name, .. } if PRINT_NAMES.contains(&name.as_str()) => PRINT_HELP,
            _ => EXPRESSION_STATEMENT_HELP,
        };
        self.error_here("SPX-P106", "expected `}` after block")
            .with_help(help)
    }

    /// Give a block that ended without a value the help that fits the position
    /// it was parsed in. Inner blocks attach their own help first and keep it.
    pub(super) fn attach_block_help(diagnostic: Diagnostic, description: &str) -> Diagnostic {
        if diagnostic.code != "SPX-P203" || diagnostic.help.is_some() {
            return diagnostic;
        }
        let help = match description {
            "`while` body" => WHILE_BODY_HELP,
            "`for` body" => FOR_BODY_HELP,
            "`if` condition" | "`else`" => BRANCH_HELP,
            "function body" => FUNCTION_BODY_HELP,
            _ => return diagnostic,
        };
        diagnostic.with_help(help)
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
        if let Some(help) = self.foreign_operator_help() {
            return diagnostic.with_help(help);
        }
        if self.at(&TokenKind::Bang) {
            return diagnostic.with_help(MACRO_HELP);
        }
        if let Some(noun) = description.strip_prefix("`,` after ") {
            if noun == "variant case" && self.at(&TokenKind::LParen) {
                return diagnostic.with_help(
                    "variant cases name their fields: `Circle { @id(\"shape.circle.radius\") \
                     radius: i64, },`; a case without data is `Empty {},`",
                );
            }
            if self.at(&TokenKind::RBrace) {
                return diagnostic.with_help(format!(
                    "every {noun} ends with `,`, including the last one before `}}`"
                ));
            }
            return diagnostic.with_help(format!("every {noun} ends with `,`"));
        }
        let previous_is_let = self
            .cursor
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .is_some_and(|token| matches!(&token.kind, TokenKind::Ident(word) if word == "let"));
        match description {
            "`{` before `if` condition" | "`{` before `while` body" if previous_is_let => {
                diagnostic.with_help(IF_LET_HELP)
            }
            "`=>` after match pattern" if self.at(&TokenKind::Dot) => {
                diagnostic.with_help(RANGE_PATTERN_HELP)
            }
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

    /// An operator spelled the way other languages spell it, found where the
    /// expression it should continue has already ended: `c ? a : b` (the `?`
    /// parses as propagation), `x as i64`, or the word operators `and`/`or`.
    fn foreign_operator_help(&self) -> Option<&'static str> {
        let previous = self
            .cursor
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map(|token| &token.kind);
        if previous == Some(&TokenKind::Question) && !self.at(&TokenKind::RBrace) {
            return Some(TERNARY_HELP);
        }
        match &self.current().kind {
            TokenKind::Ident(word) if word == "as" => Some(CAST_HELP),
            TokenKind::Ident(word) if word == "and" => Some("logical and is `&&`"),
            TokenKind::Ident(word) if word == "or" => Some("logical or is `||`"),
            _ => None,
        }
    }

    /// `use std::io;` or `use crate::x;`: a path import from another language.
    pub(super) fn use_path_help(diagnostic: Diagnostic) -> Diagnostic {
        diagnostic.with_help(USE_HELP)
    }
}
