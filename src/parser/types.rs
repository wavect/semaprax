//! Type syntax: named and primitive types, `[u8; N]` arrays, `Slice<u8>`,
//! generic parameter lists, and explicit type-argument lists. Split out of
//! `parser.rs` so the grammar root stays under its module-size budget; the
//! methods are ordinary `Parser` methods and share its private state.

use crate::ast::{Type, TypeParameterDeclaration};
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;

impl Parser {
    pub(super) fn ty(&mut self) -> Result<Type, Diagnostic> {
        if let Some(diagnostic) = self.unit_type() {
            return Err(diagnostic);
        }
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

    pub(super) fn type_parameters(&mut self) -> Result<Vec<TypeParameterDeclaration>, Diagnostic> {
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

    pub(super) fn type_arguments(&mut self) -> Result<Vec<Type>, Diagnostic> {
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
}
