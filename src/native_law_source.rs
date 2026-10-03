//! Native source projection for the bounded LAW-02 declaration profile.
//!
//! A law module is an ordinary, explicitly selected Project source.  This
//! parser deliberately has no filesystem behaviour: callers supply both the
//! bytes and their Project-relative path.  `LAWS.spx` is therefore only a
//! helpful convention, never discovery authority.

use crate::assurance_manifest::law_set::{
    ContractKind, EvidenceRequirement, LawDefinition, LawModule, LawSelector, RelationalBinder,
    MAX_LAWS, MAX_REFERENCES,
};
use crate::ast::Span;
use crate::diagnostic::Diagnostic;
use crate::lexer::{lex, Token, TokenKind};
use std::collections::BTreeSet;

const MAX_BINDERS: usize = MAX_REFERENCES;

/// One declared, typed subject binder.  Binders make the source proposition
/// readable and checkable; LAW-01's selector remains the normalized closed
/// proposition paired with the persistent contract identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LawBinder {
    pub name: String,
    pub ty: ScalarType,
    pub span: Span,
}

/// The scalar types admitted in the initial native-law profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarType {
    I64,
    I32,
    U8,
    Usize,
    Bool,
    Char,
    F32,
    F64,
}

impl ScalarType {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "i64" => Self::I64,
            "i32" => Self::I32,
            "u8" => Self::U8,
            "usize" => Self::Usize,
            "bool" => Self::Bool,
            "char" => Self::Char,
            "f32" => Self::F32,
            "f64" => Self::F64,
            _ => return None,
        })
    }

    pub(crate) fn source(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::I32 => "i32",
            Self::U8 => "u8",
            Self::Usize => "usize",
            Self::Bool => "bool",
            Self::Char => "char",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

/// A contract clause or a separately stated scalar relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeLawSubject {
    Contract {
        subject_id: String,
        clause: ContractKind,
    },
    ScalarRelational,
}

/// A declaration whose subject and proposition have one persistent law identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeLawDeclaration {
    pub law_id: String,
    pub subject: NativeLawSubject,
    pub binders: Vec<LawBinder>,
    /// Canonical scalar expression text.  This is a proposition, never an
    /// executable assertion or a call surface.
    pub proposition: String,
    pub evidence: EvidenceRequirement,
    pub span: Span,
}

/// Native declarations from one explicitly configured source module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeLawModule {
    pub module_id: String,
    pub source_path: String,
    pub laws: Vec<NativeLawDeclaration>,
}

impl NativeLawModule {
    /// Convert this source projection to LAW-01's single typed policy input.
    /// Native declarations currently introduce neither assumptions nor law
    /// dependencies; their absence is represented by empty, canonical lists.
    pub fn law_module(&self) -> LawModule {
        LawModule {
            module_id: self.module_id.clone(),
            source_path: self.source_path.clone(),
            assumptions: Vec::new(),
            laws: self
                .laws
                .iter()
                .map(|law| LawDefinition {
                    law_id: law.law_id.clone(),
                    selector: match &law.subject {
                        NativeLawSubject::Contract { subject_id, clause } => {
                            LawSelector::Contract {
                                declaration_id: subject_id.clone(),
                                clause: clause.clone(),
                                proposition: law.proposition.clone(),
                            }
                        }
                        NativeLawSubject::ScalarRelational => LawSelector::ScalarRelational {
                            binders: law
                                .binders
                                .iter()
                                .map(|binder| RelationalBinder {
                                    name: binder.name.clone(),
                                    scalar_type: binder.ty.source().to_owned(),
                                })
                                .collect(),
                            proposition: law.proposition.clone(),
                        },
                    },
                    assumption_ids: Vec::new(),
                    requires_laws: Vec::new(),
                    evidence: law.evidence.clone(),
                })
                .collect(),
        }
    }
}

/// Parse a native-law source file.  The accepted grammar is deliberately
/// small and maps without interpretation to LAW-01:
///
/// ```text
/// module example.laws;
/// @id("example.add.nonnegative")
/// law contract "example.add" ensures (left: i64, right: i64)
///     left + right >= left
///     evidence smt_proved;
/// ```
pub fn parse(source: &str, path: &str) -> Result<NativeLawModule, Diagnostic> {
    let tokens = lex(source, path)?;
    Parser {
        source,
        path,
        tokens,
        cursor: 0,
    }
    .parse()
}

/// Canonically format a native-law module.  Comments are deliberately not
/// logical content and are not included in the LAW-01 projection.
pub fn canonical(module: &NativeLawModule) -> String {
    let mut output = format!("module {};\n", module.module_id);
    for law in &module.laws {
        output.push('\n');
        output.push_str("@id(\"");
        output.push_str(&law.law_id);
        output.push_str("\")\n");
        match &law.subject {
            NativeLawSubject::Contract { subject_id, clause } => {
                output.push_str("law contract \"");
                output.push_str(subject_id);
                output.push_str("\" ");
                output.push_str(match clause {
                    ContractKind::Precondition => "requires",
                    ContractKind::Postcondition => "ensures",
                });
            }
            NativeLawSubject::ScalarRelational => output.push_str("law relational"),
        }
        output.push_str(" (");
        for (index, binder) in law.binders.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            output.push_str(&binder.name);
            output.push_str(": ");
            output.push_str(binder.ty.source());
        }
        output.push_str(")\n    ");
        output.push_str(&law.proposition);
        output.push_str("\n    evidence ");
        output.push_str(match law.evidence {
            EvidenceRequirement::RuntimeGuarded => "runtime_guarded",
            EvidenceRequirement::CompilerProved => "compiler_proved",
            EvidenceRequirement::ModelChecked => "model_checked",
            EvidenceRequirement::SmtProved => "smt_proved",
            EvidenceRequirement::TheoremProved => "theorem_proved",
        });
        output.push_str(";\n");
    }
    output
}

struct Parser<'a> {
    source: &'a str,
    path: &'a str,
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser<'_> {
    fn parse(mut self) -> Result<NativeLawModule, Diagnostic> {
        self.keyword("module")?;
        let module_id = self.qualified("module name")?;
        self.expect(TokenKind::Semicolon, "`;` after module declaration")?;
        let mut laws = Vec::new();
        let mut ids = BTreeSet::new();
        while !self.at(&TokenKind::Eof) {
            if laws.len() == MAX_LAWS {
                return Err(self.error(
                    "SPX-LW111",
                    "native law source exceeds the law declaration limit",
                ));
            }
            let start = self.current().span;
            self.expect(TokenKind::At, "`@id` before a law declaration")?;
            self.keyword("id")?;
            self.expect(TokenKind::LParen, "`(` after `@id`")?;
            let law_id = self.string("law stable ID")?;
            self.expect(TokenKind::RParen, "`)` after law stable ID")?;
            if !ids.insert(law_id.clone()) {
                return Err(self.error("SPX-LW110", "duplicate native law stable ID"));
            }
            self.keyword("law")?;
            let subject = if self.at_keyword("contract") {
                self.bump();
                let subject_id = self.string("subject declaration @id")?;
                let clause = if self.at_keyword("requires") {
                    self.bump();
                    ContractKind::Precondition
                } else if self.at_keyword("ensures") {
                    self.bump();
                    ContractKind::Postcondition
                } else {
                    return Err(self.error(
                        "SPX-LW110",
                        "law contract subject requires `requires` or `ensures`",
                    ));
                };
                NativeLawSubject::Contract { subject_id, clause }
            } else if self.at_keyword("relational") {
                self.bump();
                NativeLawSubject::ScalarRelational
            } else {
                return Err(self.error(
                    "SPX-LW110",
                    "law subject must be `contract` or `relational`",
                ));
            };
            let binders = self.binders()?;
            let proposition_start = self.current().span.start;
            let (proposition_end, evidence) = self.proposition_and_evidence()?;
            let proposition = canonical_proposition(
                &self.source[proposition_start..proposition_end],
                self.path,
                &binders,
                &subject,
            )?;
            laws.push(NativeLawDeclaration {
                law_id,
                subject,
                binders,
                proposition,
                evidence,
                span: Span {
                    start: start.start,
                    end: self.previous().span.end,
                    line: start.line,
                    column: start.column,
                },
            });
        }
        if laws.is_empty() {
            return Err(self.error(
                "SPX-LW110",
                "a native law source must declare at least one law",
            ));
        }
        Ok(NativeLawModule {
            module_id,
            source_path: self.path.to_owned(),
            laws,
        })
    }

    fn binders(&mut self) -> Result<Vec<LawBinder>, Diagnostic> {
        self.expect(TokenKind::LParen, "`(` before typed law binders")?;
        let mut binders = Vec::new();
        let mut names = BTreeSet::new();
        if self.at(&TokenKind::RParen) {
            return Err(self.error(
                "SPX-LW110",
                "law declarations require at least one typed binder",
            ));
        }
        loop {
            if binders.len() == MAX_BINDERS {
                return Err(self.error(
                    "SPX-LW111",
                    "native law declaration exceeds the binder limit",
                ));
            }
            let (name, span) = self.ident("law binder")?;
            if !names.insert(name.clone()) {
                return Err(self.error("SPX-LW110", "duplicate native law binder"));
            }
            self.expect(TokenKind::Colon, "`:` after law binder")?;
            let (type_name, _) = self.ident("scalar binder type")?;
            let ty = ScalarType::parse(&type_name).ok_or_else(|| {
                self.error("SPX-LW110", "native law binders support scalar types only")
            })?;
            binders.push(LawBinder { name, ty, span });
            if self.at(&TokenKind::RParen) {
                self.bump();
                break;
            }
            self.expect(TokenKind::Comma, "`,` between law binders")?;
        }
        Ok(binders)
    }

    fn proposition_and_evidence(&mut self) -> Result<(usize, EvidenceRequirement), Diagnostic> {
        let mut depth = 0usize;
        loop {
            if self.at(&TokenKind::Eof) {
                return Err(self.error("SPX-LW110", "native law declaration is missing `evidence`"));
            }
            if depth == 0 && self.at_keyword("evidence") {
                let end = self.previous().span.end;
                self.bump();
                let (value, _) = self.ident("law evidence requirement")?;
                let evidence = match value.as_str() {
                    "runtime_guarded" => EvidenceRequirement::RuntimeGuarded,
                    "compiler_proved" => EvidenceRequirement::CompilerProved,
                    "model_checked" => EvidenceRequirement::ModelChecked,
                    "smt_proved" => EvidenceRequirement::SmtProved,
                    "theorem_proved" => EvidenceRequirement::TheoremProved,
                    _ => {
                        return Err(
                            self.error("SPX-LW110", "unknown native law evidence requirement")
                        )
                    }
                };
                self.expect(TokenKind::Semicolon, "`;` after law evidence requirement")?;
                return Ok((end, evidence));
            }
            match self.current().kind {
                TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket => depth += 1,
                TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket if depth > 0 => {
                    depth -= 1
                }
                _ => {}
            }
            self.bump();
        }
    }

    fn keyword(&mut self, expected: &str) -> Result<(), Diagnostic> {
        if self.at_keyword(expected) {
            self.bump();
            Ok(())
        } else {
            Err(self.error(
                "SPX-LW110",
                format!("expected `{expected}` in native law declaration"),
            ))
        }
    }

    fn qualified(&mut self, description: &str) -> Result<String, Diagnostic> {
        let (mut value, _) = self.ident(description)?;
        while self.at(&TokenKind::Dot) {
            self.bump();
            let (segment, _) = self.ident(description)?;
            value.push('.');
            value.push_str(&segment);
        }
        Ok(value)
    }

    fn ident(&mut self, description: &str) -> Result<(String, Span), Diagnostic> {
        match &self.current().kind {
            TokenKind::Ident(value) => {
                let value = value.clone();
                let span = self.bump().span;
                Ok((value, span))
            }
            _ => Err(self.error("SPX-LW110", format!("expected {description}"))),
        }
    }

    fn string(&mut self, description: &str) -> Result<String, Diagnostic> {
        match &self.current().kind {
            TokenKind::String(value) => {
                let value = value.clone();
                self.bump();
                Ok(value)
            }
            _ => Err(self.error("SPX-LW110", format!("expected {description}"))),
        }
    }

    fn expect(&mut self, expected: TokenKind, description: &str) -> Result<(), Diagnostic> {
        if self.at(&expected) {
            self.bump();
            Ok(())
        } else {
            Err(self.error("SPX-LW110", format!("expected {description}")))
        }
    }

    fn at_keyword(&self, expected: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Ident(value) if value == expected)
    }

    fn at(&self, expected: &TokenKind) -> bool {
        &self.current().kind == expected
    }

    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }
    fn previous(&self) -> &Token {
        &self.tokens[self.cursor.saturating_sub(1)]
    }
    fn bump(&mut self) -> &Token {
        let index = self.cursor;
        if !self.at(&TokenKind::Eof) {
            self.cursor += 1;
        }
        &self.tokens[index]
    }
    fn error(&self, code: &'static str, message: impl Into<String>) -> Diagnostic {
        Diagnostic::error(code, message, self.current().span).at_path(self.path)
    }
}

fn canonical_proposition(
    source: &str,
    path: &str,
    binders: &[LawBinder],
    subject: &NativeLawSubject,
) -> Result<String, Diagnostic> {
    let result = if matches!(
        subject,
        NativeLawSubject::Contract {
            clause: ContractKind::Postcondition,
            ..
        }
    ) {
        binders.iter().find(|binder| binder.name == "result")
    } else {
        None
    };
    let parameters = binders
        .iter()
        .filter(|binder| binder.name != "result" || result.is_none())
        .map(|binder| format!("{}: {}", binder.name, binder.ty.source()))
        .collect::<Vec<_>>()
        .join(", ");
    let (return_type, clause, body) = match result {
        Some(binder) => (
            binder.ty.source(),
            "ensures",
            match binder.ty {
                ScalarType::I64 => "0",
                ScalarType::I32 => "0i32",
                ScalarType::U8 => "0u8",
                ScalarType::Usize => "0usize",
                ScalarType::Bool => "false",
                ScalarType::Char => "'a'",
                ScalarType::F32 => "0.0f32",
                ScalarType::F64 => "0.0f64",
            },
        ),
        None => ("i64", "requires", "0"),
    };
    let source = format!("module law.selector;\n@id(\"law.selector\")\nfn selected({parameters}) -> {return_type}\n {clause} {source}\n{{ {body} }}\n@id(\"law.main\") fn main() -> i64 {{ 0 }}\n");
    let program = crate::parse(&source, path).map_err(|_| {
        Diagnostic::io(
            "SPX-LW110",
            "native law proposition must be an admitted pure scalar expression",
        )
        .at_path(path)
    })?;
    let expression = match result {
        Some(_) => &program.functions[0].ensures[0],
        None => &program.functions[0].requires[0],
    };
    scalar_expression(expression, path, binders)?;
    crate::hir::resolve(&program).map_err(|_| {
        Diagnostic::io(
            "SPX-LW110",
            "native law proposition must be a typed boolean scalar expression",
        )
        .at_path(path)
    })?;
    Ok(crate::format::expr(expression, 0))
}

fn scalar_expression(
    expression: &crate::ast::Expr,
    path: &str,
    binders: &[LawBinder],
) -> Result<(), Diagnostic> {
    use crate::ast::ExprKind;
    let mut pending = vec![expression];
    let mut visited = 0usize;
    while let Some(expression) = pending.pop() {
        visited += 1;
        if visited > 1024 {
            return Err(Diagnostic::io(
                "SPX-LW111",
                "native law proposition exceeds the scalar expression limit",
            )
            .at_path(path));
        }
        match &expression.kind {
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Uint8(_)
            | ExprKind::Usize(_)
            | ExprKind::Char(_)
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_) => {}
            ExprKind::Var(name) if binders.iter().any(|binder| binder.name == *name) => {}
            ExprKind::Var(_) => {
                return Err(Diagnostic::error(
                    "SPX-LW110",
                    "native law proposition refers to a binder that was not declared",
                    expression.span,
                )
                .at_path(path))
            }
            ExprKind::Unary { value, .. } => pending.push(value),
            ExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            _ => {
                return Err(Diagnostic::error(
                    "SPX-LW110",
                    "native law proposition supports pure scalar expressions only",
                    expression.span,
                )
                .at_path(path))
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"module arithmetic.laws;

@id("arithmetic.add.monotonic")
law contract "arithmetic.add" ensures (left: i64, right: i64)
    left + right >= left
    evidence smt_proved;
"#;

    #[test]
    fn parses_formats_and_lowers_a_typed_contract_law() {
        let module = parse(VALID, "LAWS.spx").unwrap();
        assert_eq!(module.laws[0].proposition, "left + right >= left");
        assert_eq!(canonical(&module), VALID);
        let lowered = module.law_module();
        assert_eq!(lowered.source_path, "LAWS.spx");
        assert!(matches!(
            lowered.laws[0].selector,
            LawSelector::Contract {
                clause: ContractKind::Postcondition,
                ..
            }
        ));
    }

    #[test]
    fn rejects_effectful_calls_with_a_stable_diagnostic() {
        let source = VALID.replace("left + right >= left", "observe(left)");
        assert_eq!(parse(&source, "LAWS.spx").unwrap_err().code, "SPX-LW110");
    }

    #[test]
    fn rejects_missing_explicit_stable_id() {
        let source = VALID.replacen("@id(\"arithmetic.add.monotonic\")\n", "", 1);
        assert_eq!(parse(&source, "LAWS.spx").unwrap_err().code, "SPX-LW110");
    }
}
