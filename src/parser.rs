use std::path::Path;

use crate::ast::{
    BinaryOp, Expr, ExprKind, FieldDeclaration, FieldInitializer, Function, ImportDeclaration,
    ImportFailure, InterfaceDeclaration, MatchArm, MatchPattern, MatchPatternField, Param,
    ParamMode, Program, ResourceLifecycleDeclaration, ResourceLifecycleKind, Span, Statement, Type,
    TypeDeclaration, TypeDeclarationKind, UnaryOp, VariantCaseDeclaration,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::{lex, Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    path: String,
}

impl Parser {
    pub fn new(source: &str, path: &Path) -> Result<Self, Diagnostic> {
        let path = path.display().to_string();
        Ok(Self {
            tokens: lex(source, &path)?,
            cursor: 0,
            path,
        })
    }

    pub fn parse(mut self) -> Result<Program, Diagnostic> {
        self.keyword("module")?;
        let (module, _) = self.qualified_ident("module name")?;
        self.take(&TokenKind::Semicolon);

        let permits = if self.at_keyword("permit") {
            self.bump();
            self.effect_set()?
        } else {
            Vec::new()
        };

        let mut types = Vec::new();
        let mut interfaces = Vec::new();
        let mut functions = Vec::new();
        while !self.at(&TokenKind::Eof) {
            let stable_id = self.stable_id_attribute()?;
            if self.at_keyword("resource") {
                types.push(self.resource(&module, stable_id)?);
            } else if self.at_keyword("record") {
                types.push(self.record(&module, stable_id)?);
            } else if self.at_keyword("variant") {
                types.push(self.variant(&module, stable_id)?);
            } else if self.at_keyword("interface") {
                interfaces.push(self.interface(&module, stable_id)?);
            } else {
                functions.push(self.function(&module, stable_id)?);
            }
        }
        if functions.is_empty() {
            return Err(self.error_here("SPX-P101", "a module must declare at least one function"));
        }
        Ok(Program {
            path: self.path,
            module,
            permits,
            types,
            interfaces,
            functions,
        })
    }

    fn stable_id_attribute(&mut self) -> Result<Option<String>, Diagnostic> {
        if !self.take(&TokenKind::At) {
            return Ok(None);
        }
        let (attribute, _) = self.ident("attribute name")?;
        if attribute != "id" {
            return Err(self.error_here(
                "SPX-P102",
                format!("unknown attribute `@{attribute}`; only `@id` is supported"),
            ));
        }
        self.expect(&TokenKind::LParen, "`(` after @id")?;
        let id = match &self.bump().kind {
            TokenKind::String(value) => value.clone(),
            _ => return Err(self.error_previous("SPX-P103", "@id expects a string literal")),
        };
        self.expect(&TokenKind::RParen, "`)` after stable id")?;
        Ok(Some(id))
    }

    fn resource(
        &mut self,
        module: &str,
        stable_id: Option<String>,
    ) -> Result<TypeDeclaration, Diagnostic> {
        let start = self.keyword("resource")?.span;
        let (name, name_span) = self.ident("resource name")?;
        let explicit_id = stable_id.is_some();
        let stable_id = stable_id.unwrap_or_else(|| format!("auto:resource:{module}.{name}"));
        let (lifecycles, end) = if self.take(&TokenKind::Semicolon) {
            (Vec::new(), self.tokens[self.cursor.saturating_sub(1)].span)
        } else {
            self.expect(&TokenKind::LBrace, "`{` before resource lifecycle")?;
            let mut lifecycles = Vec::new();
            while !self.at(&TokenKind::RBrace) {
                if self.at(&TokenKind::Eof) {
                    return Err(
                        self.error_here("SPX-P106", "expected `}` after resource lifecycle")
                    );
                }
                let lifecycle_id = self.stable_id_attribute()?;
                let lifecycle_start = self.keyword("drop")?.span;
                let kind = if self.at_keyword("trivial") {
                    self.bump();
                    ResourceLifecycleKind::Trivial
                } else if self.at_keyword("import") {
                    self.bump();
                    let import_key = match self.bump().kind.clone() {
                        TokenKind::String(value) => value,
                        _ => {
                            return Err(self.error_previous(
                                "SPX-P106",
                                "expected logical import string after `drop import`",
                            ));
                        }
                    };
                    ResourceLifecycleKind::Imported { import_key }
                } else {
                    return Err(
                        self.error_here("SPX-P106", "expected `trivial` or `import` after `drop`")
                    );
                };
                let lifecycle_end = self
                    .expect(&TokenKind::Semicolon, "`;` after resource lifecycle")?
                    .span;
                lifecycles.push(ResourceLifecycleDeclaration {
                    stable_id: lifecycle_id,
                    kind,
                    span: lifecycle_start.merge(lifecycle_end),
                });
            }
            let end = self
                .expect(&TokenKind::RBrace, "`}` after resource lifecycle")?
                .span;
            (lifecycles, end)
        };
        Ok(TypeDeclaration {
            stable_id,
            explicit_id,
            name,
            name_span,
            kind: TypeDeclarationKind::Resource { lifecycles },
            span: start.merge(end),
        })
    }

    fn interface(
        &mut self,
        module: &str,
        stable_id: Option<String>,
    ) -> Result<InterfaceDeclaration, Diagnostic> {
        let start = self.keyword("interface")?.span;
        let (name, name_span) = self.ident("interface name")?;
        let explicit_id = stable_id.is_some();
        let stable_id = stable_id.unwrap_or_else(|| format!("auto:interface:{module}.{name}"));
        self.keyword("permits")?;
        let permits = self.effect_set()?;
        self.expect(&TokenKind::LBrace, "`{` before interface imports")?;
        let mut imports = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            if self.at(&TokenKind::Eof) {
                return Err(self.error_here("SPX-P106", "expected `}` after interface imports"));
            }
            let import_id = self.stable_id_attribute()?;
            let import_start = self.keyword("import")?.span;
            self.keyword("fn")?;
            let (import_name, import_name_span) = self.ident("import name")?;
            self.expect(&TokenKind::LParen, "`(` after import name")?;
            let mut params = Vec::new();
            if !self.at(&TokenKind::RParen) {
                loop {
                    let (param_name, span) = self.ident("import parameter name")?;
                    self.expect(&TokenKind::Colon, "`:` after import parameter name")?;
                    let mode = if self.at_keyword("own") {
                        self.bump();
                        ParamMode::Own
                    } else if self.at_keyword("borrow") {
                        self.bump();
                        ParamMode::Borrow
                    } else if self.at_keyword("shared") {
                        self.bump();
                        ParamMode::Shared
                    } else {
                        ParamMode::Value
                    };
                    let ty = self.ty()?;
                    params.push(Param {
                        name: param_name,
                        mode,
                        ty,
                        span,
                    });
                    if !self.take(&TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(&TokenKind::RParen, "`)` after import parameters")?;
            self.expect(&TokenKind::Arrow, "`->` before import result")?;
            self.keyword("unit")?;
            self.keyword("effects")?;
            let effects = self.effect_set()?;
            self.keyword("failure")?;
            let failure = if self.at_keyword("infallible") {
                self.bump();
                ImportFailure::Infallible
            } else if self.at_keyword("status") {
                self.bump();
                let domain_id = match self.bump().kind.clone() {
                    TokenKind::String(value) => value,
                    _ => {
                        return Err(self.error_previous(
                            "SPX-P106",
                            "expected status-domain string after `failure status`",
                        ));
                    }
                };
                ImportFailure::Status { domain_id }
            } else {
                return Err(self.error_here(
                    "SPX-P106",
                    "expected `infallible` or `status` after `failure`",
                ));
            };
            self.keyword("consumes")?;
            let (consumes, consumes_span) = self.ident("consumed parameter name")?;
            self.keyword("always")?;
            let end = self
                .expect(&TokenKind::Semicolon, "`;` after import contract")?
                .span;
            let import_explicit_id = import_id.is_some();
            let import_stable_id =
                import_id.unwrap_or_else(|| format!("auto:import:{stable_id}.{import_name}"));
            imports.push(ImportDeclaration {
                stable_id: import_stable_id,
                explicit_id: import_explicit_id,
                name: import_name,
                name_span: import_name_span,
                params,
                effects,
                failure,
                consumes,
                consumes_span,
                span: import_start.merge(end),
            });
        }
        let end = self
            .expect(&TokenKind::RBrace, "`}` after interface imports")?
            .span;
        Ok(InterfaceDeclaration {
            stable_id,
            explicit_id,
            name,
            name_span,
            permits,
            imports,
            span: start.merge(end),
        })
    }

    fn record(
        &mut self,
        module: &str,
        stable_id: Option<String>,
    ) -> Result<TypeDeclaration, Diagnostic> {
        let start = self.keyword("record")?.span;
        let (name, name_span) = self.ident("record name")?;
        let explicit_id = stable_id.is_some();
        let stable_id = stable_id.unwrap_or_else(|| format!("auto:record:{module}.{name}"));
        self.expect(&TokenKind::LBrace, "`{` before record fields")?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            if self.at(&TokenKind::Eof) {
                return Err(self.error_here("SPX-P106", "expected `}` after record fields"));
            }
            let field_id = self.stable_id_attribute()?;
            let (field_name, field_name_span) = self.ident("record field name")?;
            self.expect(&TokenKind::Colon, "`:` after record field name")?;
            let ty = self.ty()?;
            let end = self
                .expect(&TokenKind::Comma, "`,` after record field")?
                .span;
            let field_explicit_id = field_id.is_some();
            let field_stable_id =
                field_id.unwrap_or_else(|| format!("auto:field:{stable_id}.{field_name}"));
            fields.push(FieldDeclaration {
                stable_id: field_stable_id,
                explicit_id: field_explicit_id,
                name: field_name,
                name_span: field_name_span,
                ty,
                span: field_name_span.merge(end),
            });
        }
        let end = self
            .expect(&TokenKind::RBrace, "`}` after record fields")?
            .span;
        Ok(TypeDeclaration {
            stable_id,
            explicit_id,
            name,
            name_span,
            kind: TypeDeclarationKind::Record { fields },
            span: start.merge(end),
        })
    }

    fn variant(
        &mut self,
        module: &str,
        stable_id: Option<String>,
    ) -> Result<TypeDeclaration, Diagnostic> {
        let start = self.keyword("variant")?.span;
        let (name, name_span) = self.ident("variant name")?;
        let explicit_id = stable_id.is_some();
        let stable_id = stable_id.unwrap_or_else(|| format!("auto:variant:{module}.{name}"));
        self.expect(&TokenKind::LBrace, "`{` before variant cases")?;
        let mut cases = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            if self.at(&TokenKind::Eof) {
                return Err(self.error_here("SPX-P106", "expected `}` after variant cases"));
            }
            let case_id = self.stable_id_attribute()?;
            let (case_name, case_name_span) = self.ident("variant case name")?;
            let case_explicit_id = case_id.is_some();
            let case_stable_id =
                case_id.unwrap_or_else(|| format!("auto:case:{stable_id}.{case_name}"));
            let mut fields = Vec::new();
            if self.take(&TokenKind::LBrace) {
                while !self.at(&TokenKind::RBrace) {
                    let field_id = self.stable_id_attribute()?;
                    let (field_name, field_name_span) = self.ident("variant payload field name")?;
                    self.expect(&TokenKind::Colon, "`:` after variant payload field name")?;
                    let ty = self.ty()?;
                    let end = self
                        .expect(&TokenKind::Comma, "`,` after variant payload field")?
                        .span;
                    let field_explicit_id = field_id.is_some();
                    let field_stable_id = field_id.unwrap_or_else(|| {
                        format!("auto:case-field:{case_stable_id}.{field_name}")
                    });
                    fields.push(FieldDeclaration {
                        stable_id: field_stable_id,
                        explicit_id: field_explicit_id,
                        name: field_name,
                        name_span: field_name_span,
                        ty,
                        span: field_name_span.merge(end),
                    });
                }
                self.expect(&TokenKind::RBrace, "`}` after variant payload fields")?;
            }
            let end = self
                .expect(&TokenKind::Comma, "`,` after variant case")?
                .span;
            cases.push(VariantCaseDeclaration {
                stable_id: case_stable_id,
                explicit_id: case_explicit_id,
                name: case_name,
                name_span: case_name_span,
                fields,
                span: case_name_span.merge(end),
            });
        }
        let end = self
            .expect(&TokenKind::RBrace, "`}` after variant cases")?
            .span;
        Ok(TypeDeclaration {
            stable_id,
            explicit_id,
            name,
            name_span,
            kind: TypeDeclarationKind::Variant { cases },
            span: start.merge(end),
        })
    }

    fn function(
        &mut self,
        module: &str,
        stable_id: Option<String>,
    ) -> Result<Function, Diagnostic> {
        let start = self.current().span;
        self.keyword("fn")?;
        let (name, name_span) = self.ident("function name")?;
        let explicit_id = stable_id.is_some();
        let stable_id = stable_id.unwrap_or_else(|| format!("auto:{module}.{name}"));
        self.expect(&TokenKind::LParen, "`(` after function name")?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                let (param_name, span) = self.ident("parameter name")?;
                self.expect(&TokenKind::Colon, "`:` after parameter name")?;
                let mode = if self.at_keyword("own") {
                    self.bump();
                    ParamMode::Own
                } else if self.at_keyword("borrow") {
                    self.bump();
                    ParamMode::Borrow
                } else if self.at_keyword("shared") {
                    self.bump();
                    ParamMode::Shared
                } else {
                    ParamMode::Value
                };
                let ty = self.ty()?;
                params.push(Param {
                    name: param_name,
                    mode,
                    ty,
                    span,
                });
                if !self.take(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen, "`)` after parameters")?;
        self.expect(&TokenKind::Arrow, "`->` before return type")?;
        let return_type = self.ty()?;

        let effects = if self.at_keyword("uses") {
            self.bump();
            self.effect_set()?
        } else {
            Vec::new()
        };
        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        loop {
            if self.at_keyword("requires") {
                self.bump();
                requires.push(self.expression_with_record_literals(0, false)?);
            } else if self.at_keyword("ensures") {
                self.bump();
                ensures.push(self.expression_with_record_literals(0, false)?);
            } else {
                break;
            }
        }
        let body = self.block("function body")?;
        let end = body.span;
        Ok(Function {
            stable_id,
            explicit_id,
            name,
            name_span,
            params,
            return_type,
            effects,
            requires,
            ensures,
            body,
            span: start.merge(end),
        })
    }

    fn expression(&mut self, minimum_precedence: u8) -> Result<Expr, Diagnostic> {
        self.expression_with_record_literals(minimum_precedence, true)
    }

    fn expression_with_record_literals(
        &mut self,
        minimum_precedence: u8,
        allow_record_literals: bool,
    ) -> Result<Expr, Diagnostic> {
        let mut left = self.prefix(allow_record_literals)?;
        while let Some(op) = self.binary_op() {
            let precedence = op.precedence();
            if precedence < minimum_precedence {
                break;
            }
            self.bump();
            let right =
                self.expression_with_record_literals(precedence + 1, allow_record_literals)?;
            let span = left.span.merge(right.span);
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn prefix(&mut self, allow_record_literals: bool) -> Result<Expr, Diagnostic> {
        let token = self.bump().clone();
        let mut expression = match token.kind {
            TokenKind::Int(value) => Expr {
                kind: ExprKind::Int(value),
                span: token.span,
            },
            TokenKind::Ident(value) if value == "true" || value == "false" => Expr {
                kind: ExprKind::Bool(value == "true"),
                span: token.span,
            },
            TokenKind::Ident(value) if value == "if" => self.if_expression(token.span)?,
            TokenKind::Ident(value) if value == "match" => self.match_expression(token.span)?,
            TokenKind::Ident(value) => Expr {
                kind: ExprKind::Var(value),
                span: token.span,
            },
            TokenKind::Minus | TokenKind::Bang => {
                let op = if matches!(token.kind, TokenKind::Minus) {
                    UnaryOp::Neg
                } else {
                    UnaryOp::Not
                };
                let value = self.expression_with_record_literals(7, allow_record_literals)?;
                let span = token.span.merge(value.span);
                Expr {
                    kind: ExprKind::Unary {
                        op,
                        value: Box::new(value),
                    },
                    span,
                }
            }
            TokenKind::LParen => {
                let inner = self.expression(0)?;
                let end = self
                    .expect(&TokenKind::RParen, "`)` after expression")?
                    .span;
                Expr {
                    kind: inner.kind,
                    span: token.span.merge(end),
                }
            }
            TokenKind::LBrace => self.block_after_open(token.span)?,
            _ => {
                return Err(
                    Diagnostic::error("SPX-P201", "expected an expression", token.span)
                        .at_path(&self.path),
                );
            }
        };

        loop {
            if self.take(&TokenKind::ColonColon) {
                let ExprKind::Var(type_name) = expression.kind else {
                    return Err(self.error_previous(
                        "SPX-P204",
                        "variant construction requires a named variant qualifier",
                    ));
                };
                let type_span = expression.span;
                let (case_name, case_span) = self.ident("variant case name after `::`")?;
                self.expect(&TokenKind::LBrace, "`{` after variant case name")?;
                let fields = self.field_initializers("variant payload field")?;
                let end = self
                    .expect(&TokenKind::RBrace, "`}` after variant payload")?
                    .span;
                expression = Expr {
                    kind: ExprKind::ConstructVariant {
                        type_name,
                        type_span,
                        case_name,
                        case_span,
                        fields,
                    },
                    span: type_span.merge(end),
                };
                continue;
            }

            if self.at_keyword("with") {
                let start = expression.span;
                self.bump();
                self.expect(&TokenKind::LBrace, "`{` after `with`")?;
                let mut fields = Vec::new();
                while !self.at(&TokenKind::RBrace) {
                    let (name, name_span) = self.ident("record replacement field name")?;
                    self.expect(&TokenKind::Colon, "`:` after record replacement field")?;
                    let value = self.expression(0)?;
                    let field_span = name_span.merge(value.span);
                    fields.push(FieldInitializer {
                        name,
                        name_span,
                        value,
                        span: field_span,
                    });
                    if !self.take(&TokenKind::Comma) {
                        break;
                    }
                    if self.at(&TokenKind::RBrace) {
                        break;
                    }
                }
                let end = self
                    .expect(&TokenKind::RBrace, "`}` after record replacements")?
                    .span;
                expression = Expr {
                    kind: ExprKind::UpdateRecord {
                        base: Box::new(expression),
                        fields,
                    },
                    span: start.merge(end),
                };
                continue;
            }

            if allow_record_literals && self.at(&TokenKind::LBrace) {
                let Some(type_name) = expression_path(&expression) else {
                    break;
                };
                let type_span = expression.span;
                let start = expression.span;
                self.bump();
                let mut fields = Vec::new();
                while !self.at(&TokenKind::RBrace) {
                    let (name, name_span) = self.ident("record initializer field name")?;
                    let value = if self.take(&TokenKind::Colon) {
                        self.expression(0)?
                    } else {
                        Expr {
                            kind: ExprKind::Var(name.clone()),
                            span: name_span,
                        }
                    };
                    let field_span = name_span.merge(value.span);
                    fields.push(FieldInitializer {
                        name,
                        name_span,
                        value,
                        span: field_span,
                    });
                    if !self.take(&TokenKind::Comma) {
                        break;
                    }
                    if self.at(&TokenKind::RBrace) {
                        break;
                    }
                }
                let end = self
                    .expect(&TokenKind::RBrace, "`}` after record initializer")?
                    .span;
                expression = Expr {
                    kind: ExprKind::ConstructRecord {
                        type_name,
                        type_span,
                        fields,
                    },
                    span: start.merge(end),
                };
                continue;
            }

            if self.take(&TokenKind::LParen) {
                let ExprKind::Var(name) = expression.kind else {
                    return Err(
                        self.error_previous("SPX-P202", "only named functions can be called")
                    );
                };
                let mut args = Vec::new();
                if !self.at(&TokenKind::RParen) {
                    loop {
                        args.push(self.expression(0)?);
                        if !self.take(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let end = self
                    .expect(&TokenKind::RParen, "`)` after call arguments")?
                    .span;
                expression = Expr {
                    kind: ExprKind::Call { name, args },
                    span: expression.span.merge(end),
                };
                continue;
            }

            if self.take(&TokenKind::Dot) {
                let (field, field_span) = self.ident("field name after `.`")?;
                let start = expression.span;
                expression = Expr {
                    kind: ExprKind::Project {
                        base: Box::new(expression),
                        field,
                        field_span,
                    },
                    span: start.merge(field_span),
                };
                continue;
            }
            break;
        }
        Ok(expression)
    }

    fn block(&mut self, description: &str) -> Result<Expr, Diagnostic> {
        let start = self
            .expect(&TokenKind::LBrace, &format!("`{{` before {description}"))?
            .span;
        self.block_after_open(start)
    }

    fn block_after_open(&mut self, start: Span) -> Result<Expr, Diagnostic> {
        let mut statements = Vec::new();
        while self.at_keyword("let") {
            let statement_start = self.bump().span;
            let (name, name_span) = self.ident("local binding name")?;
            self.expect(&TokenKind::Eq, "`=` in local binding")?;
            let value = self.expression(0)?;
            let end = self
                .expect(&TokenKind::Semicolon, "`;` after local binding")?
                .span;
            statements.push(Statement::Let {
                name,
                name_span,
                value,
                span: statement_start.merge(end),
            });
        }
        if self.at(&TokenKind::RBrace) {
            return Err(self.error_here("SPX-P203", "block requires a final value expression"));
        }
        let tail = self.expression(0)?;
        self.take(&TokenKind::Semicolon);
        let end = self.expect(&TokenKind::RBrace, "`}` after block")?.span;
        Ok(Expr {
            kind: ExprKind::Block {
                statements,
                tail: Box::new(tail),
            },
            span: start.merge(end),
        })
    }

    fn if_expression(&mut self, start: Span) -> Result<Expr, Diagnostic> {
        let condition = self.expression_with_record_literals(0, false)?;
        let then_branch = self.block("`if` condition")?;
        self.keyword("else")?;
        let else_branch = self.block("`else`")?;
        let span = start.merge(else_branch.span);
        Ok(Expr {
            kind: ExprKind::If {
                condition: Box::new(condition),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            },
            span,
        })
    }

    fn match_expression(&mut self, start: Span) -> Result<Expr, Diagnostic> {
        let scrutinee = self.expression_with_record_literals(0, false)?;
        self.expect(&TokenKind::LBrace, "`{` before match arms")?;
        let mut arms = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            if self.at(&TokenKind::Eof) {
                return Err(self.error_here("SPX-P205", "expected `}` after match arms"));
            }
            let pattern = self.match_pattern()?;
            self.expect(&TokenKind::FatArrow, "`=>` after match pattern")?;
            let value = self.expression(0)?;
            let arm_span = pattern.span().merge(value.span);
            arms.push(MatchArm {
                pattern,
                value,
                span: arm_span,
            });
            self.expect(&TokenKind::Comma, "`,` after match arm")?;
        }
        let end = self
            .expect(&TokenKind::RBrace, "`}` after match arms")?
            .span;
        Ok(Expr {
            kind: ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span: start.merge(end),
        })
    }

    fn match_pattern(&mut self) -> Result<MatchPattern, Diagnostic> {
        let (type_name, type_span) = self.ident("variant name or `_` in match pattern")?;
        if type_name == "_" {
            return Ok(MatchPattern::Wildcard { span: type_span });
        }
        self.expect(
            &TokenKind::ColonColon,
            "`::` after variant name in match pattern",
        )?;
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
        Ok(MatchPattern::Variant {
            type_name,
            type_span,
            case_name,
            case_span,
            fields,
            span: type_span.merge(end),
        })
    }

    fn field_initializers(
        &mut self,
        description: &str,
    ) -> Result<Vec<FieldInitializer>, Diagnostic> {
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            let (name, name_span) = self.ident(description)?;
            self.expect(&TokenKind::Colon, "`:` after variant payload field")?;
            let value = self.expression(0)?;
            fields.push(FieldInitializer {
                name,
                name_span,
                span: name_span.merge(value.span),
                value,
            });
            if !self.take(&TokenKind::Comma) {
                break;
            }
        }
        Ok(fields)
    }

    fn effect_set(&mut self) -> Result<Vec<String>, Diagnostic> {
        self.expect(&TokenKind::LBrace, "`{` before effect set")?;
        let mut effects = Vec::new();
        if !self.at(&TokenKind::RBrace) {
            loop {
                let (effect, _) = self.qualified_ident("effect name")?;
                effects.push(effect);
                if !self.take(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RBrace, "`}` after effect set")?;
        Ok(effects)
    }

    fn ty(&mut self) -> Result<Type, Diagnostic> {
        let (name, _) = self.qualified_ident("type")?;
        match name.as_str() {
            "i64" => Ok(Type::I64),
            "bool" => Ok(Type::Bool),
            _ => Ok(Type::Named(name)),
        }
    }

    fn binary_op(&self) -> Option<BinaryOp> {
        Some(match self.current().kind {
            TokenKind::Plus => BinaryOp::Add,
            TokenKind::Minus => BinaryOp::Sub,
            TokenKind::Star => BinaryOp::Mul,
            TokenKind::Slash => BinaryOp::Div,
            TokenKind::Percent => BinaryOp::Rem,
            TokenKind::EqEq => BinaryOp::Eq,
            TokenKind::BangEq => BinaryOp::Ne,
            TokenKind::Lt => BinaryOp::Lt,
            TokenKind::Le => BinaryOp::Le,
            TokenKind::Gt => BinaryOp::Gt,
            TokenKind::Ge => BinaryOp::Ge,
            TokenKind::AndAnd => BinaryOp::And,
            TokenKind::OrOr => BinaryOp::Or,
            _ => return None,
        })
    }

    fn keyword(&mut self, expected: &str) -> Result<Token, Diagnostic> {
        if self.at_keyword(expected) {
            Ok(self.bump().clone())
        } else {
            Err(self.error_here("SPX-P104", format!("expected `{expected}`")))
        }
    }

    fn ident(&mut self, description: &str) -> Result<(String, Span), Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Ident(value) => Ok((value, token.span)),
            _ => Err(
                Diagnostic::error("SPX-P105", format!("expected {description}"), token.span)
                    .at_path(&self.path),
            ),
        }
    }

    fn qualified_ident(&mut self, description: &str) -> Result<(String, Span), Diagnostic> {
        let (mut value, start) = self.ident(description)?;
        let mut span = start;
        while self.take(&TokenKind::Dot) {
            let (part, part_span) = self.ident(description)?;
            value.push('.');
            value.push_str(&part);
            span = span.merge(part_span);
        }
        Ok((value, span))
    }

    fn expect(&mut self, expected: &TokenKind, description: &str) -> Result<Token, Diagnostic> {
        if self.at(expected) {
            Ok(self.bump().clone())
        } else {
            Err(self.error_here("SPX-P106", format!("expected {description}")))
        }
    }

    fn take(&mut self, expected: &TokenKind) -> bool {
        if self.at(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn at(&self, expected: &TokenKind) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(expected)
    }

    fn at_keyword(&self, expected: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Ident(value) if value == expected)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn bump(&mut self) -> &Token {
        let index = self.cursor;
        if !matches!(self.tokens[index].kind, TokenKind::Eof) {
            self.cursor += 1;
        }
        &self.tokens[index]
    }

    fn error_here(&self, code: &'static str, message: impl Into<String>) -> Diagnostic {
        Diagnostic::error(code, message, self.current().span).at_path(&self.path)
    }

    fn error_previous(&self, code: &'static str, message: impl Into<String>) -> Diagnostic {
        let index = self.cursor.saturating_sub(1);
        Diagnostic::error(code, message, self.tokens[index].span).at_path(&self.path)
    }
}

fn expression_path(expression: &Expr) -> Option<String> {
    match &expression.kind {
        ExprKind::Var(name) => Some(name.clone()),
        ExprKind::Project { base, field, .. } => {
            let mut path = expression_path(base)?;
            path.push('.');
            path.push_str(field);
            Some(path)
        }
        _ => None,
    }
}
