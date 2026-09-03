//! Match-pattern parsing: literal, wildcard, binding, or-pattern, record, and
//! variant patterns. Split out of `parser.rs` so the grammar root stays under
//! its module-size budget; the methods are ordinary `Parser` methods and share
//! its private state.

use crate::ast::{MatchPattern, MatchPatternField, PatternLiteral, Span};
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;

impl Parser {
    pub(super) fn match_pattern(&mut self) -> Result<MatchPattern, Diagnostic> {
        let first = self.match_pattern_atom()?;
        if !self.take(&TokenKind::Pipe) {
            return Ok(first);
        }
        // Refutable Match v1: `a | b | c` over literal alternatives. Only
        // literal atoms parse here; same-type and non-nesting rules are
        // enforced by the resolvers with SPX-M105.
        let mut alternatives = vec![first];
        let mut last_span;
        loop {
            let next = self.match_pattern_atom()?;
            last_span = next.span();
            alternatives.push(next);
            if !self.take(&TokenKind::Pipe) {
                break;
            }
        }
        let span = alternatives[0].span().merge(last_span);
        Ok(MatchPattern::Or { alternatives, span })
    }

    fn match_pattern_atom(&mut self) -> Result<MatchPattern, Diagnostic> {
        // Refutable Match v1: negative integer literals fold their sign at
        // parse time so patterns stay exact constants like expression
        // literals; `-9223372036854775808` stays unrepresentable exactly as
        // in the expression grammar (SPX-P003 at the lexer).
        if self.take(&TokenKind::Minus) {
            let minus_span = self.previous_span();
            let token = self.bump().clone();
            let negated = |value: i128, minimum: i128, span: Span| -> Result<i128, Diagnostic> {
                let folded = -value;
                if folded < minimum {
                    return Err(Diagnostic::error(
                        "SPX-P206",
                        "negative literal pattern is outside its integer range",
                        span,
                    )
                    .at_path(&self.path));
                }
                Ok(folded)
            };
            let value = match token.kind {
                TokenKind::Int(value) => PatternLiteral::Int(negated(
                    i128::from(value),
                    i128::from(i64::MIN),
                    token.span,
                )? as i64),
                TokenKind::Int32(value) => PatternLiteral::Int32(negated(
                    i128::from(value),
                    i128::from(i32::MIN),
                    token.span,
                )? as i32),
                TokenKind::Usize(_) => {
                    return Err(Diagnostic::error(
                        "SPX-T260",
                        "usize literals cannot be negative",
                        minus_span.merge(token.span),
                    )
                    .at_path(&self.path))
                }
                _ => {
                    return Err(Diagnostic::error(
                        "SPX-P206",
                        "`-` must precede an integer literal in a match pattern",
                        token.span,
                    )
                    .at_path(&self.path))
                }
            };
            let span = minus_span.merge(token.span);
            return Ok(MatchPattern::Literal { value, span });
        }
        let token = self.bump().clone();
        let pattern = match token.kind {
            TokenKind::Ident(name) if name == "_" => MatchPattern::Wildcard { span: token.span },
            TokenKind::Ident(name) if name == "true" || name == "false" => {
                MatchPattern::Literal {
                    value: PatternLiteral::Bool(name == "true"),
                    span: token.span,
                }
            }
            TokenKind::Ident(name) => {
                if self.take(&TokenKind::LBrace) {
                    let fields = self.record_match_pattern_fields()?;
                    let end = self
                        .expect(&TokenKind::RBrace, "`}` after record pattern")?
                        .span;
                    MatchPattern::Record {
                        type_name: name,
                        type_span: token.span,
                        fields,
                        span: token.span.merge(end),
                    }
                } else if self.take(&TokenKind::ColonColon) {
                    let (case_name, case_span) = self.ident("variant case name in match pattern")?;
                    self.expect(&TokenKind::LBrace, "`{` after variant case pattern")?;
                    let mut fields = Vec::new();
                    while !self.at(&TokenKind::RBrace) {
                        let (name, name_span) = self.ident("variant pattern field name")?;
                        let (binding, binding_span) = if self.take(&TokenKind::Colon) {
                            self.ident("variant pattern binding name")?
                        } else {
                            (name.clone(), name_span)
                        };
                        fields.push(MatchPatternField {
                            name,
                            name_span,
                            binding,
                            binding_span,
                            span: name_span.merge(binding_span),
                        });
                        if !self.take(&TokenKind::Comma) {
                            break;
                        }
                    }
                    let end = self
                        .expect(&TokenKind::RBrace, "`}` after variant pattern")?
                        .span;
                    MatchPattern::Variant {
                        type_name: name,
                        type_span: token.span,
                        case_name,
                        case_span,
                        fields,
                        span: token.span.merge(end),
                    }
                } else {
                    // Refutable Match v1: an irrefutable whole-scrutinee
                    // binding arm.
                    MatchPattern::Binding {
                        name,
                        span: token.span,
                    }
                }
            }
            TokenKind::Int(value) => MatchPattern::Literal {
                value: PatternLiteral::Int(value),
                span: token.span,
            },
            TokenKind::Int32(value) => MatchPattern::Literal {
                value: PatternLiteral::Int32(value),
                span: token.span,
            },
            TokenKind::Uint8(value) => MatchPattern::Literal {
                value: PatternLiteral::Uint8(value),
                span: token.span,
            },
            TokenKind::Usize(value) => MatchPattern::Literal {
                value: PatternLiteral::Usize(value),
                span: token.span,
            },
            TokenKind::Char(value) => MatchPattern::Literal {
                value: PatternLiteral::Char(value),
                span: token.span,
            },
            _ => {
                return Err(Diagnostic::error(
                    "SPX-P206",
                    "match patterns admit `_`, bindings, aggregate patterns, and integer/char/bool literals",
                    token.span,
                )
                .at_path(&self.path))
            }
        };
        Ok(pattern)
    }

    fn record_match_pattern_fields(
        &mut self,
    ) -> Result<Vec<crate::ast::RecordMatchPatternField>, Diagnostic> {
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            let (name, name_span) = self.ident("record pattern field name")?;
            let pattern = if self.take(&TokenKind::Colon) {
                let (pattern_name, pattern_span) = self.ident("record field pattern")?;
                if pattern_name == "_" {
                    crate::ast::RecordMatchFieldPattern::Wildcard { span: pattern_span }
                } else if self.take(&TokenKind::LBrace) {
                    let nested_fields = self.record_match_pattern_fields()?;
                    let end = self
                        .expect(&TokenKind::RBrace, "`}` after nested record pattern")?
                        .span;
                    crate::ast::RecordMatchFieldPattern::Record {
                        type_name: pattern_name,
                        type_span: pattern_span,
                        fields: nested_fields,
                        span: pattern_span.merge(end),
                    }
                } else {
                    crate::ast::RecordMatchFieldPattern::Binding {
                        name: pattern_name,
                        span: pattern_span,
                    }
                }
            } else {
                crate::ast::RecordMatchFieldPattern::Binding {
                    name: name.clone(),
                    span: name_span,
                }
            };
            let span = name_span.merge(pattern.span());
            fields.push(crate::ast::RecordMatchPatternField {
                name,
                name_span,
                pattern,
                span,
            });
            if !self.take(&TokenKind::Comma) {
                break;
            }
        }
        Ok(fields)
    }
}
