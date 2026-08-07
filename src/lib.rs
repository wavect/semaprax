#![allow(clippy::result_large_err)]

//! SEMAPRAX v0.1 compiler library.
//!
//! The source projection is for humans. The semantic graph is the agent API.

pub mod ast;
pub mod codegen;
pub mod diagnostic;
pub mod format;
pub mod graph;
pub mod lexer;
pub mod parser;
pub mod patch;
pub mod verify;

use std::path::Path;

use ast::Program;
use diagnostic::Diagnostic;

pub fn parse(source: &str, path: impl AsRef<Path>) -> Result<Program, Diagnostic> {
    parser::Parser::new(source, path.as_ref()).and_then(parser::Parser::parse)
}

pub fn check(source: &str, path: impl AsRef<Path>) -> Result<Program, Vec<Diagnostic>> {
    let program = parse(source, path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        Err(diagnostics)
    } else {
        Ok(program)
    }
}

pub fn compile_file(path: impl AsRef<Path>) -> Result<Program, Vec<Diagnostic>> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I001",
            format!("cannot read {}: {error}", path.display()),
        )]
    })?;
    check(&source, path)
}
