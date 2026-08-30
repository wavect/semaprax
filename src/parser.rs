use std::path::Path;

use crate::ast::{
    BinaryOp, Expr, ExprKind, FieldDeclaration, FieldInitializer, FieldTarget, Function,
    ImportDeclaration, ImportFailure, ImportResult, InterfaceDeclaration, MatchArm, MatchPattern,
    MatchPatternField, ModuleUse, ModuleUseKind, Param, ParamMode, PatternLiteral, Program,
    ProtocolDeclaration, ProtocolImplementation, ProtocolImplementationMember, ProtocolMethod,
    ResourceLifecycleDeclaration, ResourceLifecycleKind, Span, Statement, Type, TypeDeclaration,
    TypeDeclarationKind, TypeParameterDeclaration, UnaryOp, VariantCaseDeclaration,
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
        let mut protocols = Vec::new();
        let mut implementations = Vec::new();
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
            } else if self.at_keyword("class") {
                types.push(self.class(&module, stable_id)?);
            } else if self.at_keyword("interface") {
                interfaces.push(self.interface(&module, stable_id)?);
            } else if self.at_keyword("protocol") {
                if protocols.len() >= crate::static_protocol::MAX_IMPLEMENTATIONS {
                    return Err(
                        self.error_here("SPX-Q109", "too many static protocol declarations")
                    );
                }
                protocols.push(self.protocol(&module, stable_id)?);
            } else if self.at_keyword("impl") {
                if implementations.len() >= crate::static_protocol::MAX_IMPLEMENTATIONS {
                    return Err(
                        self.error_here("SPX-Q109", "too many static protocol implementations")
                    );
                }
                implementations.push(self.protocol_implementation(stable_id)?);
            } else {
                functions.push(self.function(&module, stable_id)?);
            }
        }
        self.reject_duplicate_protocols(&protocols)?;
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
            protocols,
            implementations,
            functions,
        })
    }

    /// Protocol Projection v1 fail-closed structural gates: protocol names are
    /// unique module-wide, method names are unique inside one protocol, every
    /// protocol declares at least one method, and protocol/method stable ids
    /// never repeat. (There is no `impl` syntax in v1, so implements cycles
    /// cannot be expressed; conformance stays explicitly empty.)
    fn reject_duplicate_protocols(
        &self,
        protocols: &[ProtocolDeclaration],
    ) -> Result<(), Diagnostic> {
        let mut names = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::new();
        for protocol in protocols {
            if !names.insert(protocol.name.as_str()) {
                return Err(Diagnostic::error(
                    "SPX-P120",
                    format!("duplicate protocol declaration name `{}`", protocol.name),
                    protocol.name_span,
                )
                .at_path(&self.path));
            }
            if protocol.methods.is_empty() {
                return Err(Diagnostic::error(
                    "SPX-P123",
                    format!(
                        "protocol `{}` must declare at least one method",
                        protocol.name
                    ),
                    protocol.span,
                )
                .at_path(&self.path));
            }
            let mut methods = std::collections::BTreeSet::new();
            for method in &protocol.methods {
                if !methods.insert(method.name.as_str()) {
                    return Err(Diagnostic::error(
                        "SPX-P121",
                        format!(
                            "duplicate method name `{}` in protocol `{}`",
                            method.name, protocol.name
                        ),
                        method.name_span,
                    )
                    .at_path(&self.path));
                }
                if !ids.insert(method.stable_id.as_str()) {
                    return Err(Diagnostic::error(
                        "SPX-P122",
                        format!(
                            "duplicate protocol identity `{}` in protocol `{}`",
                            method.stable_id, protocol.name
                        ),
                        method.name_span,
                    )
                    .at_path(&self.path));
                }
            }
            if !ids.insert(protocol.stable_id.as_str()) {
                return Err(Diagnostic::error(
                    "SPX-P122",
                    format!("duplicate protocol identity `{}`", protocol.stable_id),
                    protocol.name_span,
                )
                .at_path(&self.path));
            }
        }
        Ok(())
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
            extends: None,
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

    /// Explicit source-owned static method bindings by persistent identity.
    fn protocol_implementation(
        &mut self,
        stable_id: Option<String>,
    ) -> Result<ProtocolImplementation, Diagnostic> {
        let start = self.keyword("impl")?.span;
        let stable_id = stable_id.ok_or_else(|| {
            self.error_previous("SPX-Q106", "static protocol impl requires an explicit @id")
        })?;
        let protocol_id = self.protocol_binding_id()?;
        self.keyword("for")?;
        let receiver_id = self.protocol_binding_id()?;
        self.expect(&TokenKind::LBrace, "`{` before static protocol bindings")?;
        let mut members = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            if members.len() >= crate::static_protocol::MAX_IMPLEMENTATION_MEMBERS {
                return Err(self.error_here("SPX-Q109", "too many static protocol member bindings"));
            }
            let member_start = self.current().span;
            let method_id = self.protocol_binding_id()?;
            self.expect(&TokenKind::Eq, "`=` between static protocol member IDs")?;
            let function_id = self.protocol_binding_id()?;
            let end = self
                .expect(&TokenKind::Semicolon, "`;` after static protocol binding")?
                .span;
            members.push(ProtocolImplementationMember {
                method_id,
                function_id,
                span: member_start.merge(end),
            });
        }
        let end = self
            .expect(&TokenKind::RBrace, "`}` after static protocol bindings")?
            .span;
        Ok(ProtocolImplementation {
            stable_id,
            explicit_id: true,
            protocol_id,
            receiver_id,
            members,
            span: start.merge(end),
        })
    }

    fn protocol_binding_id(&mut self) -> Result<String, Diagnostic> {
        match &self.bump().kind {
            TokenKind::String(value) => Ok(value.clone()),
            _ => Err(self.error_previous(
                "SPX-Q106",
                "static protocol binding requires a stable-ID string",
            )),
        }
    }

    fn protocol(
        &mut self,
        module: &str,
        stable_id: Option<String>,
    ) -> Result<ProtocolDeclaration, Diagnostic> {
        let start = self.keyword("protocol")?.span;
        let (name, name_span) = self.ident("protocol name")?;
        let explicit_id = stable_id.is_some();
        let stable_id = stable_id.unwrap_or_else(|| format!("auto:protocol:{module}.{name}"));
        self.expect(&TokenKind::LBrace, "`{` before protocol methods")?;
        let mut methods = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            if methods.len() >= crate::static_protocol::MAX_IMPLEMENTATION_MEMBERS {
                return Err(self.error_here("SPX-Q109", "too many protocol requirements"));
            }
            if self.at(&TokenKind::Eof) {
                return Err(self.error_here("SPX-P106", "expected `}` after protocol methods"));
            }
            let method_id = self.stable_id_attribute()?;
            let method_start = self.keyword("fn")?.span;
            let (method_name, method_name_span) = self.ident("protocol method name")?;
            self.expect(&TokenKind::LParen, "`(` after protocol method name")?;
            let mut params = Vec::new();
            if !self.at(&TokenKind::RParen) {
                loop {
                    let (param_name, span) = self.ident("protocol method parameter name")?;
                    if params.len() >= crate::static_protocol::MAX_METHOD_PARAMETERS {
                        return Err(
                            self.error_here("SPX-Q109", "too many protocol method parameters")
                        );
                    }
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
            let end = self
                .expect(&TokenKind::Semicolon, "`;` after protocol method signature")?
                .span;
            let method_explicit_id = method_id.is_some();
            let method_stable_id =
                method_id.unwrap_or_else(|| format!("auto:method:{stable_id}.{method_name}"));
            methods.push(ProtocolMethod {
                stable_id: method_stable_id,
                explicit_id: method_explicit_id,
                name: method_name,
                name_span: method_name_span,
                params,
                return_type,
                span: method_start.merge(end),
            });
        }
        let end = self
            .expect(&TokenKind::RBrace, "`}` after protocol methods")?
            .span;
        Ok(ProtocolDeclaration {
            stable_id,
            explicit_id,
            name,
            name_span,
            methods,
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
            extends: None,
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
            extends: None,
            span: start.merge(end),
        })
    }

    fn class(
        &mut self,
        module: &str,
        stable_id: Option<String>,
    ) -> Result<TypeDeclaration, Diagnostic> {
        let start = self.keyword("class")?.span;
        let (name, name_span) = self.ident("class name")?;
        let type_parameters = self.type_parameters()?;
        // Class Inheritance v1: one optional named parent `class C : P { .. }`.
        let extends = if self.take(&TokenKind::Colon) {
            Some(self.ty()?)
        } else {
            None
        };
        let explicit_id = stable_id.is_some();
        let stable_id = stable_id.unwrap_or_else(|| format!("auto:class:{module}.{name}"));
        self.expect(&TokenKind::LBrace, "`{` before class members")?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            if self.at(&TokenKind::Eof) {
                return Err(self.error_here("SPX-P106", "expected `}` after class members"));
            }
            let member_id = self.stable_id_attribute()?;
            if self.at_keyword("fn") {
                methods.push(self.function(module, member_id)?);
            } else {
                let (field_name, field_name_span) = self.ident("class field name")?;
                self.expect(&TokenKind::Colon, "`:` after class field name")?;
                let ty = self.ty()?;
                let end = self
                    .expect(&TokenKind::Comma, "`,` after class field")?
                    .span;
                let field_explicit = member_id.is_some();
                let field_stable =
                    member_id.unwrap_or_else(|| format!("auto:field:{stable_id}.{field_name}"));
                fields.push(FieldDeclaration {
                    stable_id: field_stable,
                    explicit_id: field_explicit,
                    name: field_name,
                    name_span: field_name_span,
                    ty,
                    span: field_name_span.merge(end),
                });
            }
        }
        let end = self
            .expect(&TokenKind::RBrace, "`}` after class members")?
            .span;
        Ok(TypeDeclaration {
            stable_id,
            explicit_id,
            name,
            name_span,
            type_parameters,
            kind: TypeDeclarationKind::Class { fields, methods },
            extends,
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
            TokenKind::Usize(value) => Expr {
                kind: ExprKind::Usize(value),
                span: token.span,
            },
            TokenKind::LBracket => self.byte_array_literal(token.span)?,
            TokenKind::String(value) => Expr {
                kind: ExprKind::String(value),
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
                if op == UnaryOp::Neg && matches!(value.kind, ExprKind::Usize(_)) {
                    return Err(Diagnostic::error(
                        "SPX-T260",
                        "usize literals cannot be negative",
                        span,
                    )
                    .at_path(&self.path));
                }
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
                expression = self.dot_suffix(expression)?;
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

    /// Parses one `.` postfix suffix (field projection or method call) after
    /// the receiver has already been parsed. Kept out of the recursive
    /// postfix loop so deep expression nesting does not pay for its locals.
    fn dot_suffix(&mut self, expression: Expr) -> Result<Expr, Diagnostic> {
        let (field, field_span) = self.ident("field name after `.`")?;
        let start = expression.span;
        // Allow generic type arguments after method name like `obj.method<T>(...)`
        let method_type_arguments =
            if self.at(&TokenKind::Lt) && self.looks_like_generic_function_call() {
                self.type_arguments()?
            } else {
                Vec::new()
            };
        if self.at(&TokenKind::LParen) {
            self.bump();
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
                .expect(&TokenKind::RParen, "`)` after method arguments")?
                .span;
            if matches!(&expression.kind, ExprKind::Var(name) if name == "super") {
                return Ok(Expr {
                    kind: ExprKind::SuperMethod {
                        method: field,
                        method_span: field_span,
                        args,
                    },
                    span: start.merge(end),
                });
            }
            return Ok(Expr {
                kind: ExprKind::MethodCall {
                    receiver: Box::new(expression),
                    method: field,
                    method_span: field_span,
                    type_arguments: method_type_arguments,
                    args,
                },
                span: start.merge(end),
            });
        }
        if !method_type_arguments.is_empty() {
            return Err(self.error_previous(
                "SPX-P202",
                "generic arguments require a call; use `.method::<T>(...)`",
            ));
        }
        Ok(Expr {
            kind: ExprKind::Project {
                base: Box::new(expression),
                field,
                field_span,
            },
            span: start.merge(field_span),
        })
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
            } else if self.at_unsafe_statement() {
                statements.push(self.unsafe_statement()?);
            } else if self.at_keyword("while") {
                statements.push(self.while_statement()?);
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
        // Class Inheritance v1: optional declared type `let name: T = value;`.
        let declared = if self.take(&TokenKind::Colon) {
            Some(self.ty()?)
        } else {
            None
        };
        self.expect(&TokenKind::Eq, "`=` in local binding")?;
        let value = self.expression(0)?;
        let end = self
            .expect(&TokenKind::Semicolon, "`;` after local binding")?
            .span;
        Ok(Statement::Let {
            name,
            name_span,
            mutable,
            declared,
            value,
            span: statement_start.merge(end),
        })
    }

    fn at_assign_statement(&self) -> bool {
        if !matches!(&self.current().kind, TokenKind::Ident(_)) {
            return false;
        }
        if self.tokens.get(self.cursor + 1).map(|token| &token.kind) == Some(&TokenKind::Eq) {
            return true;
        }
        // Field Mutation v1: `<binding>.<field> = ...`. Deeper chains also
        // enter the assignment statement so they can be rejected there.
        let mut index = self.cursor + 1;
        while matches!(
            self.tokens.get(index).map(|token| &token.kind),
            Some(TokenKind::Dot)
        ) && matches!(
            self.tokens.get(index + 1).map(|token| &token.kind),
            Some(TokenKind::Ident(_))
        ) {
            match self.tokens.get(index + 2).map(|token| &token.kind) {
                Some(TokenKind::Eq) => return true,
                Some(TokenKind::Dot) => index += 2,
                _ => return false,
            }
        }
        false
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
        // Field Mutation v1 admits exactly one direct field level; deeper
        // place chains stay outside the slice.
        let field = if self.take(&TokenKind::Dot) {
            let (field_name, field_span) = self.ident("assignment field name")?;
            if self.at(&TokenKind::Dot) {
                return Err(self.error_here(
                    "SPX-U111",
                    "nested place chains like `a.b.c = ...` are outside field mutation v1",
                ));
            }
            Some(FieldTarget {
                name: field_name,
                span: field_span,
            })
        } else {
            None
        };
        self.expect(&TokenKind::Eq, "`=` in assignment")?;
        let value = self.expression(0)?;
        let end = self
            .expect(&TokenKind::Semicolon, "`;` after assignment")?
            .span;
        Ok(Statement::Assign {
            name,
            name_span,
            field,
            value,
            span: name_span.merge(end),
        })
    }

    /// Bounded While-Loops v1: `while <condition> { <body> }`. The condition
    /// excludes record literals exactly like `if` conditions; the body is an
    /// ordinary block whose discarded value is checked by the verifier.
    fn while_statement(&mut self) -> Result<Statement, Diagnostic> {
        let start = self.keyword("while")?.span;
        let condition = self.expression_with_record_literals(0, false)?;
        let body = self.block("`while` body")?;
        let span = start.merge(body.span);
        Ok(Statement::While {
            condition: Box::new(condition),
            body: Box::new(body),
            span,
        })
    }

    /// Unsafe Boundary Mechanics v1: an unsafe boundary statement is
    /// `@audit("<summary>") unsafe { ... }`. The audit attribute is mandatory
    /// (`SPX-N102`) and its summary must be a non-empty string (`SPX-N103`);
    /// the body is parsed as an ordinary block.
    fn at_unsafe_statement(&self) -> bool {
        if self.at_keyword("unsafe")
            && self.tokens.get(self.cursor + 1).map(|t| &t.kind) == Some(&TokenKind::LBrace)
        {
            return true;
        }
        // Any statement-position attribute is a boundary annotation attempt;
        // `@` cannot begin an expression, so this keeps diagnostics precise.
        self.at(&TokenKind::At)
            && matches!(
                self.tokens.get(self.cursor + 1).map(|t| &t.kind),
                Some(TokenKind::Ident(_))
            )
    }

    fn unsafe_statement(&mut self) -> Result<Statement, Diagnostic> {
        if !self.take(&TokenKind::At) {
            return Err(self.error_here(
                "SPX-N102",
                "unsafe blocks require an audit summary annotation: `@audit(\"...\")`",
            ));
        }
        let (attribute, attribute_span) = self.ident("attribute name")?;
        if attribute != "audit" {
            return Err(self.error_here(
                "SPX-P102",
                format!("unknown attribute `@{attribute}`; only `@id` and `@audit` are supported"),
            ));
        }
        self.expect(&TokenKind::LParen, "`(` after @audit")?;
        let audit = match &self.bump().kind {
            TokenKind::String(value) => value.clone(),
            _ => {
                return Err(
                    self.error_previous("SPX-N103", "@audit expects a string literal summary")
                );
            }
        };
        let audit_end = self
            .expect(&TokenKind::RParen, "`)` after @audit summary")?
            .span;
        if audit.is_empty() {
            return Err(Diagnostic::error(
                "SPX-N103",
                "@audit summary must be a non-empty string",
                attribute_span.merge(audit_end),
            )
            .at_path(&self.path));
        }
        self.keyword("unsafe")?;
        let body = self.block("unsafe block body")?;
        let span = attribute_span.merge(body.span);
        Ok(Statement::Unsafe {
            audit,
            audit_span: attribute_span.merge(audit_end),
            body: Box::new(body),
            span,
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
        let mode = self.contextual_match_mode()?;
        let scrutinee = self.expression_with_record_literals(0, false)?;
        self.expect(&TokenKind::LBrace, "`{` before match arms")?;
        let mut arms = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            if self.at(&TokenKind::Eof) {
                return Err(self.error_here("SPX-P205", "expected `}` after match arms"));
            }
            let pattern = self.match_pattern()?;
            // Refutable Match v1: `pattern if guard => value`. The guard
            // expression ends at `=>` because no operator consumes FatArrow.
            let guard = if self.at_keyword("if") {
                self.bump();
                Some(Box::new(self.expression(0)?))
            } else {
                None
            };
            self.expect(&TokenKind::FatArrow, "`=>` after match pattern")?;
            let value = self.expression(0)?;
            let pattern_end = guard
                .as_ref()
                .map_or(pattern.span(), |guard| pattern.span().merge(guard.span));
            let arm_span = pattern_end.merge(value.span);
            arms.push(MatchArm {
                pattern,
                guard,
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
                mode,
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span: start.merge(end),
        })
    }

    /// Parse `own`/`borrow` as contextual match-mode words only when the next
    /// token begins a distinct expression and cannot legally continue the
    /// legacy identifier expression. In particular, `match own { ... }` and
    /// `match own(value) { ... }` remain matches over identifiers/calls named
    /// `own`; the same rule applies to `borrow`.
    fn contextual_match_mode(&mut self) -> Result<crate::ast::MatchMode, Diagnostic> {
        use crate::ast::MatchMode;

        let candidate = if self.at_keyword("own") {
            Some(MatchMode::Own)
        } else if self.at_keyword("borrow") {
            Some(MatchMode::Borrow)
        } else {
            None
        };
        let Some(mode) = candidate else {
            return Ok(MatchMode::Value);
        };
        if !self.next_token_starts_unambiguous_match_scrutinee() {
            return Ok(MatchMode::Value);
        }

        let mode_span = self.bump().span;
        if self.at_keyword("own") || self.at_keyword("borrow") {
            return Err(Diagnostic::error(
                "SPX-P207",
                "a match expression accepts exactly one ownership mode: `own` or `borrow`",
                mode_span.merge(self.current().span),
            )
            .at_path(&self.path));
        }
        Ok(mode)
    }

    fn next_token_starts_unambiguous_match_scrutinee(&self) -> bool {
        match self.tokens.get(self.cursor + 1).map(|token| &token.kind) {
            // `with` is a legal postfix continuation of the legacy
            // identifier scrutinee: `match own with { field: value } { ... }`.
            Some(TokenKind::Ident(value)) => value != "with",
            Some(
                TokenKind::Int(_)
                | TokenKind::Int32(_)
                | TokenKind::Float(_)
                | TokenKind::Char(_)
                | TokenKind::Uint8(_)
                | TokenKind::Usize(_)
                | TokenKind::String(_)
                | TokenKind::LBracket
                | TokenKind::Minus
                | TokenKind::Bang,
            ) => true,
            _ => false,
        }
    }

    fn match_pattern(&mut self) -> Result<MatchPattern, Diagnostic> {
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

    fn fixed_array_count(&mut self) -> Result<u32, Diagnostic> {
        let token = self.bump().clone();
        let value = match token.kind {
            TokenKind::Int(value) if value >= 0 => u32::try_from(value).ok(),
            _ => None,
        }
        .filter(|value| *value <= 65_536)
        .ok_or_else(|| {
            Diagnostic::error(
                "SPX-T261",
                "fixed byte-array length must be a decimal constant in 0..=65536",
                token.span,
            )
            .at_path(&self.path)
        })?;
        Ok(value)
    }

    fn byte_array_literal(&mut self, start: Span) -> Result<Expr, Diagnostic> {
        if self.at(&TokenKind::RBracket) {
            let end = self.bump().span;
            return Ok(Expr {
                kind: ExprKind::ArrayU8(Vec::new()),
                span: start.merge(end),
            });
        }
        let first = self.bump().clone();
        let TokenKind::Uint8(first_value) = first.kind else {
            return Err(Diagnostic::error(
                "SPX-T262",
                "fixed byte-array literal elements must be exact `u8` literals",
                first.span,
            )
            .at_path(&self.path));
        };
        if self.take(&TokenKind::Semicolon) {
            let count = self.fixed_array_count()?;
            let end = self
                .expect(
                    &TokenKind::RBracket,
                    "`]` after repeated byte-array literal",
                )?
                .span;
            return Ok(Expr {
                kind: ExprKind::RepeatArrayU8 {
                    value: first_value,
                    count,
                },
                span: start.merge(end),
            });
        }
        let mut values = vec![first_value];
        while self.take(&TokenKind::Comma) {
            if self.at(&TokenKind::RBracket) {
                break;
            }
            let token = self.bump().clone();
            let TokenKind::Uint8(value) = token.kind else {
                return Err(Diagnostic::error(
                    "SPX-T262",
                    "fixed byte-array literal elements must be exact `u8` literals",
                    token.span,
                )
                .at_path(&self.path));
            };
            values.push(value);
            if values.len() > 65_536 {
                return Err(Diagnostic::error(
                    "SPX-T261",
                    "fixed byte-array literal length exceeds 65536 bytes",
                    start.merge(token.span),
                )
                .at_path(&self.path));
            }
        }
        let end = self
            .expect(&TokenKind::RBracket, "`]` after byte-array literal")?
            .span;
        Ok(Expr {
            kind: ExprKind::ArrayU8(values),
            span: start.merge(end),
        })
    }

    fn ty(&mut self) -> Result<Type, Diagnostic> {
        if self.take(&TokenKind::LBracket) {
            let (element, _) = self.qualified_ident("fixed-array element type")?;
            if element != "u8" {
                return Err(self.error_here(
                    "SPX-T268",
                    "Portable Indexed Byte Data v1 admits only fixed `[u8; N]` arrays",
                ));
            }
            self.expect(&TokenKind::Semicolon, "`;` after fixed-array element type")?;
            let length = self.fixed_array_count()?;
            self.expect(&TokenKind::RBracket, "`]` after fixed-array length")?;
            return Ok(Type::ArrayU8(length));
        }
        let (name, _) = self.qualified_ident("type")?;
        if name == "Slice" {
            self.expect(&TokenKind::Lt, "`<` after `Slice`")?;
            let (element, _element_span) = self.qualified_ident("slice element type")?;
            if element != "u8" {
                return Err(self.error_here(
                    "SPX-T268",
                    "Portable Indexed Byte Data v1 admits only `Slice<u8>`",
                ));
            }
            self.expect(&TokenKind::Gt, "`>` after `Slice<u8`")?;
            return Ok(Type::SliceU8);
        }
        let is_primitive = matches!(
            name.as_str(),
            "i64"
                | "i32"
                | "u8"
                | "usize"
                | "char"
                | "f32"
                | "f64"
                | "bool"
                | "string"
                | "str"
                | "Bytes"
        );
        if is_primitive && self.at(&TokenKind::Lt) {
            return Err(self.error_here(
                "SPX-P106",
                format!("primitive type `{name}` does not accept generic arguments"),
            ));
        }
        match name.as_str() {
            "i64" => Ok(Type::I64),
            "i32" => Ok(Type::I32),
            "u8" => Ok(Type::U8),
            "usize" => Ok(Type::Usize),
            "char" => Ok(Type::Char),
            "f32" => Ok(Type::F32),
            "f64" => Ok(Type::F64),
            "bool" => Ok(Type::Bool),
            "string" => Ok(Type::String),
            "Bytes" => Ok(Type::Bytes),
            "str" => Ok(Type::Str),
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

    /// The span of the token immediately before the cursor. The cursor never
    /// sits at index zero while parsing, so this is always a real token.
    fn previous_span(&self) -> Span {
        self.tokens[self.cursor.saturating_sub(1)].span
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
