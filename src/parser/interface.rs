use super::*;
use crate::ast::Type;

impl Parser {
    pub(super) fn interface(
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
            let index_selected = native_rust && self.at_keyword("selected");
            if index_selected {
                self.bump();
            }
            self.keyword("fn")?;
            let (import_name, import_name_span) = self.ident("import name")?;
            let mut params = Vec::new();
            if !index_selected {
                self.expect(&TokenKind::LParen, "`(` after import name")?;
            }
            if !index_selected && !self.at(&TokenKind::RParen) {
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
            if !index_selected {
                self.expect(&TokenKind::RParen, "`)` after import parameters")?;
                self.expect(&TokenKind::Arrow, "`->` before import result")?;
            }
            let result = if index_selected || self.at_keyword("unit") {
                if !index_selected {
                    self.bump();
                }
                ImportResult::Unit
            } else if native_rust && self.at_keyword("i64") {
                self.bump();
                ImportResult::I64
            } else if native_rust && self.at_keyword("bool") {
                self.bump();
                ImportResult::Bool
            } else if native_rust {
                let ty = self.ty()?;
                match ty {
                    Type::String => ImportResult::OwnedString,
                    ref ty if ImportResult::container_for_type(ty).is_some() => {
                        ImportResult::container_for_type(ty).unwrap()
                    }
                    Type::Named { name, arguments } if arguments.is_empty() => {
                        ImportResult::OwnedResource { name }
                    }
                    _ => {
                        return Err(self.error_previous(
                            "SPX-P106",
                            "native Rust result requires an opaque resource, string, Option<string>, or Result<string, i64>",
                        ))
                    }
                }
            } else {
                return Err(self.error_here("SPX-P106", "expected admitted import result type"));
            };
            let rust_path = if native_rust && self.at_keyword("from") {
                self.bump();
                match self.bump().kind.clone() {
                    TokenKind::String(value) => Some(value),
                    _ => {
                        return Err(self.error_previous(
                            "SPX-P106",
                            "expected Rust API path string after `from`",
                        ))
                    }
                }
            } else {
                None
            };
            if index_selected && rust_path.is_none() {
                return Err(
                    self.error_here("SPX-P106", "selected Rust import requires `from` path")
                );
            }
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
                index_selected,
                selected_signature: None,
                selected_index_digest: None,
                selected_receiver: None,
                rust_path,
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
}
