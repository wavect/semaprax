use std::path::Path;

use crate::ast::{
    BinaryOp, Expr, ExprKind, FieldDeclaration, FieldInitializer, Function, ImportDeclaration,
    ImportFailure, ImportResult, InterfaceDeclaration, MatchArm, MatchPattern, MatchPatternField,
    ModuleUse, ModuleUseKind, Param, ParamMode, Program, ResourceLifecycleDeclaration,
    ResourceLifecycleKind, Span, Statement, Type, TypeDeclaration, TypeDeclarationKind,
    TypeParameterDeclaration, UnaryOp, VariantCaseDeclaration,
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

        let mut module_uses = Vec::new();
        while self.at_keyword("use") {
            module_uses.push(self.module_use()?);
        }

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
            if self.at_keyword("use") {
                return Err(self.error_here(
                    "SPX-G170",
                    "workspace module uses must appear immediately after the module declaration",
                ));
            }
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
            module_uses,
            permits,
            types,
            interfaces,
            functions,
        })
    }

    fn module_use(&mut self) -> Result<ModuleUse, Diagnostic> {
        self.module_use_inner().map_err(|mut diagnostic| {
            diagnostic.code = "SPX-G170";
            diagnostic
        })
    }

    fn module_use_inner(&mut self) -> Result<ModuleUse, Diagnostic> {
        let start = self.keyword("use")?.span;
        let kind = if self.at_keyword("function") {
            self.bump();
            ModuleUseKind::Function
        } else if self.at_keyword("type") {
            self.bump();
            ModuleUseKind::Type
        } else {
            return Err(self.error_here(
                "SPX-G170",
                "workspace module use expects `function` or `type`",
            ));
        };
        if !self.take(&TokenKind::At) {
            return Err(self.error_here(
                "SPX-G170",
                "workspace module use requires `@id(\"<persistent-id>\")`",
            ));
        }
        let (attribute, _) = self.ident("workspace module use attribute")?;
        if attribute != "id" {
            return Err(self.error_here(
                "SPX-G170",
                "workspace module use requires the `@id` attribute",
            ));
        }
        self.expect(&TokenKind::LParen, "`(` after workspace module use @id")?;
        let persistent_id = match &self.bump().kind {
            TokenKind::String(value) => value.clone(),
            _ => {
                return Err(self.error_previous(
                    "SPX-G170",
                    "workspace module use @id expects a string literal",
                ));
            }
        };
        self.expect(&TokenKind::RParen, "`)` after workspace module use @id")?;
        self.keyword("from")?;
        let (target_module, _) = self.qualified_ident("workspace module use target")?;
        self.keyword("as")?;
        let (alias, _) = self.ident("workspace module use alias")?;
        let end = self
            .expect(&TokenKind::Semicolon, "`;` after workspace module use")?
            .span;
        Ok(ModuleUse {
            kind,
            persistent_id,
            target_module,
            alias,
            span: start.merge(end),
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
        let type_parameters = self.type_parameters()?;
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
            type_parameters,
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
            let native_rust = if self.at_keyword("rust") {
                self.bump();
                true
            } else {
                false
            };
            self.keyword("fn")?;
            let (import_name, import_name_span) = self.ident("import name")?;
            self.expect(&TokenKind::LParen, "`(` after import name")?;
            let mut params = Vec::new();
            if !self.at(&TokenKind::RParen) {
                loop {
                    let (param_name, span) = self.ident("import parameter name")?;
                    self.reject_mut_parameter(&param_name, span)?;
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
            let result = if self.at_keyword("unit") {
                self.bump();
                ImportResult::Unit
            } else if native_rust && self.at_keyword("i64") {
                self.bump();
                ImportResult::I64
            } else if native_rust && self.at_keyword("bool") {
                self.bump();
                ImportResult::Bool
            } else {
                return Err(self.error_here("SPX-P106", "expected admitted import result type"));
            };
            self.keyword("effects")?;
            let effects = self.effect_set()?;
            let failure = {
                if native_rust && !self.at_keyword("failure") {
                    return Err(self.error_here("SPX-P106", "expected keyword `failure`"));
                }
                self.keyword("failure")?;
                if self.at_keyword("infallible") {
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
                }
            };
            let (consumes, consumes_span) = if native_rust {
                (String::new(), import_start)
            } else {
                self.keyword("consumes")?;
                let consumed = self.ident("consumed parameter name")?;
                self.keyword("always")?;
                consumed
            };
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
                native_rust,
                params,
                result,
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
        let type_parameters = self.type_parameters()?;
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
            type_parameters,
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
        let type_parameters = self.type_parameters()?;
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
            type_parameters,
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
        let type_parameters = self.type_parameters()?;
        let explicit_id = stable_id.is_some();
        let stable_id = stable_id.unwrap_or_else(|| format!("auto:{module}.{name}"));
        self.expect(&TokenKind::LParen, "`(` after function name")?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                let (param_name, span) = self.ident("parameter name")?;
                self.reject_mut_parameter(&param_name, span)?;
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
            type_parameters,
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
            TokenKind::Int32(value) => Expr {
                kind: ExprKind::Int32(value),
                span: token.span,
            },
            TokenKind::Char(value) => Expr {
                kind: ExprKind::Char(value),
                span: token.span,
            },
            TokenKind::Uint8(value) => Expr {
                kind: ExprKind::Uint8(value),
                span: token.span,
            },
            TokenKind::Float(literal) => Expr {
                kind: if literal.wide {
                    ExprKind::Float64(literal.value.to_bits())
                } else {
                    ExprKind::Float32((literal.value as f32).to_bits())
                },
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
            let type_arguments = if self.at(&TokenKind::Lt)
                && matches!(&expression.kind, ExprKind::Var(_))
                && (self.looks_like_generic_variant_qualifier()
                    || self.looks_like_generic_function_call()
                    || (allow_record_literals && self.looks_like_generic_record_qualifier()))
            {
                self.type_arguments()?
            } else {
                Vec::new()
            };
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
                        type_arguments,
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
                        type_arguments,
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
                    kind: ExprKind::Call {
                        name,
                        type_arguments,
                        args,
                    },
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
            if self.at(&TokenKind::Question) {
                let start = expression.span;
                let end = self.bump().span;
                expression = Expr {
                    kind: ExprKind::Try {
                        operand: Box::new(expression),
                    },
                    span: start.merge(end),
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
        loop {
            if self.at_keyword("let") {
                statements.push(self.let_statement()?);
            } else if self.at_assign_statement() {
                statements.push(self.assign_statement()?);
            } else {
                break;
            }
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

    fn let_statement(&mut self) -> Result<Statement, Diagnostic> {
        let statement_start = self.bump().span;
        let mut mutable = false;
        if self.at_keyword("mut") {
            self.bump();
            mutable = true;
            if self.at_keyword("mut") {
                return Err(self.error_here(
                    "SPX-U104",
                    "duplicate `mut` modifier; write `let mut` exactly once",
                ));
            }
        }
        let (name, name_span) = self.ident("local binding name")?;
        self.expect(&TokenKind::Eq, "`=` in local binding")?;
        let value = self.expression(0)?;
        let end = self
            .expect(&TokenKind::Semicolon, "`;` after local binding")?
            .span;
        Ok(Statement::Let {
            name,
            name_span,
            mutable,
            value,
            span: statement_start.merge(end),
        })
    }

    fn at_assign_statement(&self) -> bool {
        matches!(&self.current().kind, TokenKind::Ident(_))
            && self.tokens.get(self.cursor + 1).map(|token| &token.kind) == Some(&TokenKind::Eq)
    }

    /// Explicit Mutation v1 admits no mutable parameters: parameters are
    /// immutable, so a leading `mut` in a parameter position is rejected.
    fn reject_mut_parameter(&mut self, name: &str, span: Span) -> Result<(), Diagnostic> {
        if name == "mut" {
            return Err(Diagnostic::error(
                "SPX-U103",
                "`mut` is only allowed on local `let` bindings; parameters are immutable",
                span,
            )
            .at_path(&self.path));
        }
        Ok(())
    }

    fn assign_statement(&mut self) -> Result<Statement, Diagnostic> {
        let (name, name_span) = self.ident("assignable binding name")?;
        self.expect(&TokenKind::Eq, "`=` in assignment")?;
        let value = self.expression(0)?;
        let end = self
            .expect(&TokenKind::Semicolon, "`;` after assignment")?
            .span;
        Ok(Statement::Assign {
            name,
            name_span,
            value,
            span: name_span.merge(end),
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
        if self.take(&TokenKind::LBrace) {
            let fields = self.record_match_pattern_fields()?;
            let end = self
                .expect(&TokenKind::RBrace, "`}` after record pattern")?
                .span;
            return Ok(MatchPattern::Record {
                type_name,
                type_span,
                fields,
                span: type_span.merge(end),
            });
        }
        self.expect(
            &TokenKind::ColonColon,
            "`{` after record name or `::` after variant name in match pattern",
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
            "i64" if !self.at(&TokenKind::Lt) => Ok(Type::I64),
            "i32" if !self.at(&TokenKind::Lt) => Ok(Type::I32),
            "u8" if !self.at(&TokenKind::Lt) => Ok(Type::U8),
            "char" if !self.at(&TokenKind::Lt) => Ok(Type::Char),
            "f32" if !self.at(&TokenKind::Lt) => Ok(Type::F32),
            "f64" if !self.at(&TokenKind::Lt) => Ok(Type::F64),
            "bool" if !self.at(&TokenKind::Lt) => Ok(Type::Bool),
            _ => Ok(Type::Named {
                name,
                arguments: self.type_arguments()?,
            }),
        }
    }

    fn type_parameters(&mut self) -> Result<Vec<TypeParameterDeclaration>, Diagnostic> {
        if !self.take(&TokenKind::Lt) {
            return Ok(Vec::new());
        }
        if self.at(&TokenKind::Gt) {
            return Err(self.error_here("SPX-P106", "generic parameter list cannot be empty"));
        }
        let mut parameters = Vec::new();
        loop {
            if !matches!(self.current().kind, TokenKind::Ident(_)) {
                return Err(self.error_here("SPX-P106", "expected generic type parameter"));
            }
            let (name, span) = self.ident("generic type parameter")?;
            parameters.push(TypeParameterDeclaration { name, span });
            if self.at(&TokenKind::Gt) {
                break;
            }
            if !self.take(&TokenKind::Comma) || self.at(&TokenKind::Gt) {
                return Err(self.error_here(
                    "SPX-P106",
                    "generic type parameters require comma-separated names without a trailing comma",
                ));
            }
        }
        self.expect(&TokenKind::Gt, "`>` after generic type parameters")?;
        Ok(parameters)
    }

    fn type_arguments(&mut self) -> Result<Vec<Type>, Diagnostic> {
        if !self.take(&TokenKind::Lt) {
            return Ok(Vec::new());
        }
        if self.at(&TokenKind::Gt) {
            return Err(self.error_here("SPX-P106", "generic argument list cannot be empty"));
        }
        let mut arguments = Vec::new();
        loop {
            if !matches!(self.current().kind, TokenKind::Ident(_)) {
                return Err(self.error_here("SPX-P106", "expected generic type argument"));
            }
            arguments.push(self.ty()?);
            if self.at(&TokenKind::Gt) {
                break;
            }
            if !self.take(&TokenKind::Comma) || self.at(&TokenKind::Gt) {
                return Err(self.error_here(
                    "SPX-P106",
                    "generic type arguments require comma-separated types without a trailing comma",
                ));
            }
        }
        self.expect(&TokenKind::Gt, "`>` after generic type arguments")?;
        Ok(arguments)
    }

    fn looks_like_generic_variant_qualifier(&self) -> bool {
        self.looks_like_generic_qualifier(TokenKind::ColonColon)
    }

    fn looks_like_generic_record_qualifier(&self) -> bool {
        self.looks_like_generic_qualifier(TokenKind::LBrace)
    }

    fn looks_like_generic_function_call(&self) -> bool {
        self.looks_like_generic_qualifier(TokenKind::LParen)
    }

    fn looks_like_generic_qualifier(&self, terminator: TokenKind) -> bool {
        if self.tokens.get(self.cursor).map(|token| &token.kind) != Some(&TokenKind::Lt) {
            return false;
        }
        let malformed_qualifier = self.looks_like_malformed_generic_qualifier(&terminator);
        let mut cursor = self.cursor + 1;
        if self.tokens.get(cursor).map(|token| &token.kind) == Some(&TokenKind::Gt) {
            return malformed_qualifier;
        }
        loop {
            let Some(next) = self.generic_type_end(cursor) else {
                return malformed_qualifier;
            };
            cursor = next;
            match self.tokens.get(cursor).map(|token| &token.kind) {
                Some(TokenKind::Comma) => cursor += 1,
                Some(TokenKind::Gt) => {
                    return self
                        .tokens
                        .get(cursor + 1)
                        .is_some_and(|next| next.kind == terminator);
                }
                _ => return malformed_qualifier,
            }
        }
    }

    fn looks_like_malformed_generic_qualifier(&self, terminator: &TokenKind) -> bool {
        let mut depth = 0_usize;
        for (offset, token) in self.tokens[self.cursor..].iter().enumerate() {
            match token.kind {
                TokenKind::Lt => depth += 1,
                TokenKind::Gt => {
                    let Some(next_depth) = depth.checked_sub(1) else {
                        return false;
                    };
                    depth = next_depth;
                    if depth == 0 {
                        return self
                            .tokens
                            .get(self.cursor + offset + 1)
                            .is_some_and(|next| &next.kind == terminator);
                    }
                }
                TokenKind::Ident(_) | TokenKind::Dot | TokenKind::Comma => {}
                _ => return false,
            }
        }
        false
    }

    fn generic_type_end(&self, mut cursor: usize) -> Option<usize> {
        if !matches!(self.tokens.get(cursor)?.kind, TokenKind::Ident(_)) {
            return None;
        }
        cursor += 1;
        while self.tokens.get(cursor).map(|token| &token.kind) == Some(&TokenKind::Dot) {
            cursor += 1;
            if !matches!(self.tokens.get(cursor)?.kind, TokenKind::Ident(_)) {
                return None;
            }
            cursor += 1;
        }
        if self.tokens.get(cursor).map(|token| &token.kind) != Some(&TokenKind::Lt) {
            return Some(cursor);
        }
        cursor += 1;
        if self.tokens.get(cursor).map(|token| &token.kind) == Some(&TokenKind::Gt) {
            return None;
        }
        loop {
            cursor = self.generic_type_end(cursor)?;
            match self.tokens.get(cursor).map(|token| &token.kind) {
                Some(TokenKind::Comma) => cursor += 1,
                Some(TokenKind::Gt) => return Some(cursor + 1),
                _ => return None,
            }
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
