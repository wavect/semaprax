//! Parsing the two source-level bounded traversal spellings.

use crate::ast::Statement;
use crate::diagnostic::Diagnostic;

use super::Parser;

pub(super) fn parse(parser: &mut Parser) -> Result<Statement, Diagnostic> {
    let start = parser.keyword("for")?.span;
    let consuming = parser.at_keyword("own");
    if consuming {
        parser.bump();
    }
    let (item, item_span) = parser.ident("loop item binding")?;
    parser.keyword("in")?;
    let values = parser
        .expression_with_record_literals(0, false)
        .map_err(|diagnostic| parser.range_for_hint(diagnostic))?;
    let body = parser.block("`for` body")?;
    let span = start.merge(body.span);
    Ok(if consuming {
        Statement::ForOwn {
            item,
            item_span,
            values: Box::new(values),
            body: Box::new(body),
            span,
        }
    } else {
        Statement::For {
            item,
            item_span,
            values: Box::new(values),
            body: Box::new(body),
            span,
        }
    })
}
