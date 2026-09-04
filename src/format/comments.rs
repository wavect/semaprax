//! Comment preservation for the canonical formatter.
//!
//! The grammar never sees `//` comments; the lexer drops them as trivia. For
//! years that meant `semaprax fmt` deleted every comment a person wrote. This
//! module puts them back by *position*, not by attaching them to the AST: the
//! lexer records where each comment was, this module maps each position onto
//! the canonical structure, and the writer asks for the comments that belong
//! before, after, or at the end of each item it emits.
//!
//! The placement rules are few and deterministic:
//!
//! - A comment before the `module` line stays at the top of the file.
//! - A comment on its own line before an item leads that item and is printed
//!   above it, above its `@id` attribute. If it is on the line right after the
//!   previous item, it trails that item instead, so the blank line canonical
//!   formatting inserts between declarations never separates a comment from
//!   the item it followed.
//! - A comment that trails a token on the same line trails the item that token
//!   belongs to and is printed on its own line right after that item.
//! - A comment inside an item's signature, contracts, or a single-line
//!   expression is printed above that item.
//! - A comment after the last statement of a block is printed before the
//!   closing brace; one after the last declaration is printed at the end of
//!   the file.
//!
//! Items are module uses, declarations, record and class fields, variant
//! cases, resource lifecycles, class methods, block statements, and block
//! tails. The text after `//` is kept verbatim except for trailing whitespace,
//! so formatting twice yields the same bytes and a canonical file with
//! comments passes `fmt --check`. [Canonical comments
//! v1](../../docs/CANONICAL-COMMENTS-V1.md) owns the contract.

use std::collections::BTreeMap;

use crate::ast::{Expr, ExprKind, Program, Span, Statement, TypeDeclarationKind};
pub use crate::lexer::{Comment, Comments};

/// Where every comment of one source file goes in its canonical rendering.
#[derive(Debug, Default)]
pub struct Placement {
    header: Vec<String>,
    leading: BTreeMap<usize, Vec<String>>,
    trailing: BTreeMap<usize, Vec<String>>,
    closing: BTreeMap<usize, Vec<String>>,
    file_end: Vec<String>,
}

/// One formatted item: its source span and, for blocks and type bodies, the
/// sequence of items inside it.
struct Item {
    start: usize,
    end: usize,
    body: Option<Sequence>,
}

/// Sibling items in source order. `close` is the offset of the closing brace,
/// or `None` for the top level of the file.
struct Sequence {
    items: Vec<Item>,
    open: usize,
    close: Option<usize>,
}

impl Placement {
    /// Map every comment onto the program's canonical structure.
    #[must_use]
    pub fn new(program: &Program, comments: &Comments) -> Self {
        let root = root_sequence(program);
        let mut placement = Self::default();
        for comment in &comments.items {
            if comment.offset < comments.first_token_offset {
                placement.header.push(comment.text.clone());
            } else {
                placement.place(&root, comment);
            }
        }
        placement
    }

    /// `true` when the program had no comments, so the writer's hooks emit
    /// nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.header.is_empty()
            && self.leading.is_empty()
            && self.trailing.is_empty()
            && self.closing.is_empty()
            && self.file_end.is_empty()
    }

    fn place(&mut self, sequence: &Sequence, comment: &Comment) {
        for (index, item) in sequence.items.iter().enumerate() {
            if comment.offset < item.start {
                let sticks_to_previous = index > 0 && sticks_to_previous(comment);
                if sticks_to_previous {
                    let previous = sequence.items[index - 1].start;
                    self.trailing
                        .entry(previous)
                        .or_default()
                        .push(comment.text.clone());
                } else {
                    self.leading
                        .entry(item.start)
                        .or_default()
                        .push(comment.text.clone());
                }
                return;
            }
            if comment.offset < item.end {
                if let Some(body) = &item.body {
                    let inside = comment.offset > body.open
                        && body.close.is_some_and(|close| comment.offset < close);
                    if inside {
                        self.place(body, comment);
                        return;
                    }
                }
                self.leading
                    .entry(item.start)
                    .or_default()
                    .push(comment.text.clone());
                return;
            }
        }
        match (sequence.close, sequence.items.last()) {
            (None, _) => self.file_end.push(comment.text.clone()),
            (Some(_), Some(last)) if sticks_to_previous(comment) => {
                self.trailing
                    .entry(last.start)
                    .or_default()
                    .push(comment.text.clone());
            }
            (Some(close), _) => self
                .closing
                .entry(close)
                .or_default()
                .push(comment.text.clone()),
        }
    }

    pub(super) fn header(&self, output: &mut impl std::fmt::Write) {
        write_comments(output, &self.header, 0);
    }

    pub(super) fn leading(&self, output: &mut impl std::fmt::Write, start: usize, depth: usize) {
        if let Some(comments) = self.leading.get(&start) {
            write_comments(output, comments, depth);
        }
    }

    pub(super) fn trailing(&self, output: &mut impl std::fmt::Write, start: usize, depth: usize) {
        if let Some(comments) = self.trailing.get(&start) {
            write_comments(output, comments, depth);
        }
    }

    pub(super) fn closing(&self, output: &mut impl std::fmt::Write, close: usize, depth: usize) {
        if let Some(comments) = self.closing.get(&close) {
            write_comments(output, comments, depth);
        }
    }

    pub(super) fn file_end(&self, output: &mut impl std::fmt::Write) {
        write_comments(output, &self.file_end, 0);
    }
}

/// A comment that trails a token on its line, or sits on the line right after
/// the previous token, belongs to the item before it. A free function rather
/// than a method: other crates include this file by path, where `Comment` is
/// a foreign type.
fn sticks_to_previous(comment: &Comment) -> bool {
    !comment.own_line || comment.line == comment.previous_token_line + 1
}

fn write_comments(output: &mut impl std::fmt::Write, comments: &[String], depth: usize) {
    for text in comments {
        for _ in 0..depth {
            output.write_str("    ").unwrap();
        }
        writeln!(output, "//{text}").unwrap();
    }
}

/// The canonical source projection with the file's comments restored.
#[must_use]
pub fn canonical_with_comments(program: &Program, comments: &Comments) -> String {
    let placement = Placement::new(program, comments);
    let mut output = String::new();
    super::write_canonical_commented(program, &placement, &mut output);
    output
}

fn root_sequence(program: &Program) -> Sequence {
    let mut items = Vec::new();
    items.extend(program.module_uses.iter().map(|use_| leaf(use_.span)));
    items.extend(program.types.iter().map(|declaration| Item {
        start: declaration.span.start,
        end: declaration.span.end,
        body: Some(Sequence {
            items: type_members(&declaration.kind),
            open: declaration.name_span.end,
            close: Some(declaration.span.end.saturating_sub(1)),
        }),
    }));
    items.extend(
        program
            .interfaces
            .iter()
            .map(|interface| leaf(interface.span)),
    );
    items.extend(program.protocols.iter().map(|protocol| leaf(protocol.span)));
    items.extend(
        program
            .implementations
            .iter()
            .map(|implementation| leaf(implementation.span)),
    );
    items.extend(
        program
            .functions
            .iter()
            .map(|function| function_item(function.span, &function.body)),
    );
    items.sort_by_key(|item| item.start);
    Sequence {
        items,
        open: 0,
        close: None,
    }
}

fn type_members(kind: &TypeDeclarationKind) -> Vec<Item> {
    let mut items: Vec<Item> = match kind {
        TypeDeclarationKind::Resource { lifecycles } => lifecycles
            .iter()
            .map(|lifecycle| leaf(lifecycle.span))
            .collect(),
        TypeDeclarationKind::Record { fields } => {
            fields.iter().map(|field| leaf(field.span)).collect()
        }
        TypeDeclarationKind::Variant { cases } => {
            cases.iter().map(|case| leaf(case.span)).collect()
        }
        TypeDeclarationKind::Class { fields, methods } => fields
            .iter()
            .map(|field| leaf(field.span))
            .chain(
                methods
                    .iter()
                    .map(|method| function_item(method.span, &method.body)),
            )
            .collect(),
    };
    items.sort_by_key(|item| item.start);
    items
}

fn function_item(span: Span, body: &Expr) -> Item {
    Item {
        start: span.start,
        end: span.end,
        body: block_sequence(body),
    }
}

fn block_sequence(body: &Expr) -> Option<Sequence> {
    let ExprKind::Block { statements, tail } = &body.kind else {
        return None;
    };
    let mut items: Vec<Item> = statements.iter().map(statement_item).collect();
    items.push(leaf(tail.span));
    Some(Sequence {
        items,
        open: body.span.start,
        close: Some(body.span.end.saturating_sub(1)),
    })
}

fn statement_item(statement: &Statement) -> Item {
    match statement {
        Statement::Let { span, .. } | Statement::Assign { span, .. } => leaf(*span),
        Statement::While { body, span, .. } | Statement::Unsafe { body, span, .. } => Item {
            start: span.start,
            end: span.end,
            body: block_sequence(body),
        },
    }
}

fn leaf(span: Span) -> Item {
    Item {
        start: span.start,
        end: span.end,
        body: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn formatted(source: &str) -> String {
        let (program, comments) = crate::parse_with_comments(source, Path::new("c.spx")).unwrap();
        canonical_with_comments(&program, &comments)
    }

    #[test]
    fn comments_survive_and_formatting_is_idempotent() {
        let source = "// License header\n// second line\nmodule app.c;\n\n// leads add\n@id(\"c.add\")\nfn add(a: i64, b: i64) -> i64 // trails the signature\n{\n    // leads let\n    let total = a + b; // trails let\n    let unused = 0;\n    // between statements, sticks to unused\n\n    // leads the tail after a blank line\n    total\n    // before the closing brace\n}\n// sticks to add\n\n// leads main\n@id(\"app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n// end of file\n";
        let once = formatted(source);
        let expected = "// License header\n// second line\nmodule app.c;\n\n// leads add\n// trails the signature\n@id(\"c.add\")\nfn add(a: i64, b: i64) -> i64\n{\n    // leads let\n    let total = a + b;\n    // trails let\n    let unused = 0;\n    // between statements, sticks to unused\n    // leads the tail after a blank line\n    total\n    // before the closing brace\n}\n// sticks to add\n\n// leads main\n@id(\"app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n// end of file\n";
        assert_eq!(once, expected);
        assert_eq!(formatted(&once), once, "formatting must be idempotent");
        // Without comments the writer is byte-identical to the plain canonical form.
        let (program, _) = crate::parse_with_comments(source, Path::new("c.spx")).unwrap();
        let stripped = crate::format::canonical(&program);
        assert!(!stripped.contains("//"));
        assert_eq!(
            formatted(&stripped),
            stripped,
            "a comment-free file formats to itself"
        );
    }

    #[test]
    fn nested_blocks_fields_and_methods_keep_their_comments() {
        let source = "module app.n;\n\n@id(\"n.point\")\nrecord Point {\n    // horizontal\n    @id(\"n.point.x\")\n    x: i64,\n    @id(\"n.point.y\")\n    y: i64, // vertical\n}\n\n@id(\"n.counter\")\nclass Counter {\n    @id(\"n.counter.value\")\n    value: i64,\n\n    // the only method\n    @id(\"n.counter.get\")\n    fn get(self: Counter) -> i64\n{\n        self.value\n    }\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    let mut i = 0;\n    while i < 3 {\n        // loop body\n        i = i + 1;\n        i < 3 // keep looping\n    }\n    i\n}\n";
        let once = formatted(source);
        let expected = "module app.n;\n\n@id(\"n.point\")\nrecord Point {\n    // horizontal\n    @id(\"n.point.x\")\n    x: i64,\n    @id(\"n.point.y\")\n    y: i64,\n    // vertical\n}\n\n@id(\"n.counter\")\nclass Counter {\n    @id(\"n.counter.value\")\n    value: i64,\n\n    // the only method\n    @id(\"n.counter.get\")\n    fn get(self: Counter) -> i64\n{\n        self.value\n    }\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    let mut i = 0;\n    while i < 3 {\n        // loop body\n        i = i + 1;\n        i < 3\n        // keep looping\n    }\n    i\n}\n";
        assert_eq!(once, expected);
        assert_eq!(formatted(&once), once);
    }
}
