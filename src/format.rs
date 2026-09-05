use crate::ast::{
    BinaryOp, Expr, ExprKind, ImportFailure, MatchPattern, ModuleUseKind, Program,
    ResourceLifecycleKind, Statement, TypeDeclarationKind, UnaryOp,
};
use std::collections::HashMap;
use std::fmt::Write as _;
// Explicit paths: other crates include this file by `#[path]`, where a bare
// `mod` would resolve beside the file instead of under `format/`.
#[path = "format/capacity.rs"]
mod capacity;
#[path = "format/comments.rs"]
pub mod comments;
use capacity::{legacy_canonical_temporary_bytes, legacy_expr_temporary_bytes};
/// Canonical `f64` literal text: shortest round-trip decimal that always
/// re-parses as a floating-point literal (it keeps a fraction or exponent).
pub(crate) fn canonical_f64_bits(bits: u64) -> String {
    let text = format!("{}", f64::from_bits(bits));
    if text.contains('.')
        || text.contains('e')
        || !text.chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        text
    } else {
        format!("{text}.0")
    }
}
/// Canonical `f32` literal text in the same style, without the suffix.
pub(crate) fn canonical_f32_bits(bits: u32) -> String {
    let text = format!("{}", f32::from_bits(bits));
    if text.contains('.')
        || text.contains('e')
        || !text.chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        text
    } else {
        format!("{text}.0")
    }
}
/// Canonical `char` literal text for one Unicode scalar value. Printable
/// ASCII (except quote and backslash) and the named escapes project directly;
/// every other scalar projects as lowercase `\u{...}` so the round trip is
/// exact.
pub(crate) fn canonical_char(value: u32) -> String {
    const ESCAPES: &[(u32, &str)] = &[
        (0x00, "\\0"),
        (0x09, "\\t"),
        (0x0A, "\\n"),
        (0x0D, "\\r"),
        (0x27, "\\'"),
        (0x5C, "\\\\"),
    ];
    let mut text = String::from("'");
    if let Some((_, escape)) = ESCAPES.iter().find(|(scalar, _)| *scalar == value) {
        text.push_str(escape);
    } else if (0x20..=0x7E).contains(&value) {
        text.push(char::from_u32(value).expect("printable ASCII is a scalar value"));
    } else {
        text.push_str(&format!("\\u{{{:x}}}", value));
    }
    text.push('\'');
    text
}
pub(crate) fn canonical_string(value: &str) -> String {
    let mut text = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => text.push_str("\\\\"),
            '"' => text.push_str("\\\""),
            '\n' => text.push_str("\\n"),
            '\r' => text.push_str("\\r"),
            '\t' => text.push_str("\\t"),
            _ if (ch as u32) < 0x20 || ch == '\u{7f}' => {
                write!(text, "\\u{{{:x}}}", ch as u32).expect("writing to String cannot fail");
            }
            _ => text.push(ch),
        }
    }
    text.push('"');
    text
}
fn write_string_escaped(output: &mut impl std::fmt::Write, value: &str) {
    for ch in value.chars() {
        match ch {
            '\\' => output.write_str("\\\\").unwrap(),
            '"' => output.write_str("\\\"").unwrap(),
            '\n' => output.write_str("\\n").unwrap(),
            '\r' => output.write_str("\\r").unwrap(),
            '\t' => output.write_str("\\t").unwrap(),
            _ if (ch as u32) < 0x20 || ch == '\u{7f}' => {
                write!(output, "\\u{{{:x}}}", ch as u32).unwrap();
            }
            _ => output.write_char(ch).unwrap(),
        }
    }
}
enum ExprFormatFrame<'a> {
    Expr(&'a Expr, u8),
    MeasureEnd(usize, u8, usize),
    CallArgs(&'a [Expr], usize),
    BinaryRight(&'a Expr, BinaryOp, bool),
    Block(&'a [Statement], &'a Expr, usize),
    BlockNext(&'a [Statement], &'a Expr, usize),
    BlockNextAfterUnsafe(&'a [Statement], &'a Expr, usize),
    WhileBody(&'a Expr),
    BlockNextAfterWhile(&'a [Statement], &'a Expr, usize),
    IfThen(&'a Expr, &'a Expr),
    IfElse(&'a Expr),
    Fields(&'a [crate::ast::FieldInitializer], usize, &'static str),
    MatchArms(&'a [crate::ast::MatchArm], usize),
    MatchArmValue(&'a Expr),
    TryEnd(bool),
    PostfixFields(&'a [crate::ast::FieldInitializer]),
    ProjectField(&'a str),
    MethodCallSuffix(&'a str, &'a [crate::ast::Type], &'a [Expr], bool),
    Close(char),
}
enum PatternFormatFrame<'a> {
    Enter(&'a str, &'a [crate::ast::RecordMatchPatternField]),
    Fields(&'a [crate::ast::RecordMatchPatternField], usize),
}
enum ContainsRecordFrame<'a> {
    Enter(&'a Expr),
    Children(&'a Expr, usize),
}

enum TypeFormatFrame<'a> {
    Type(&'a crate::ast::Type),
    Arguments(&'a [crate::ast::Type], usize),
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct PrivateScratchCapacity {
    expression_slots: usize,
    contains_record_slots: usize,
    type_slots: usize,
    pattern_slots: usize,
    bytes: usize,
}
#[allow(dead_code)]
impl PrivateScratchCapacity {
    pub(crate) fn bytes(self) -> usize {
        self.bytes
    }

    #[cfg(test)]
    pub(crate) fn slots(self) -> [usize; 4] {
        [
            self.expression_slots,
            self.contains_record_slots,
            self.type_slots,
            self.pattern_slots,
        ]
    }
}

#[allow(dead_code)]
pub(crate) fn private_scratch_capacity(
    expression_depth: usize,
    type_depth: usize,
    pattern_depth: usize,
) -> Option<PrivateScratchCapacity> {
    // The measured formatter retains one end marker beside each authored
    // expression and its ordinary continuation frame.
    let expression_slots = expression_depth.checked_mul(3)?.checked_add(4)?;
    let contains_record_slots = expression_depth.checked_add(1)?;
    let type_slots = type_depth.checked_add(1)?;
    let pattern_slots = pattern_depth.checked_add(1)?;
    let bytes = expression_slots
        .checked_mul(std::mem::size_of::<ExprFormatFrame<'_>>())?
        .checked_add(
            contains_record_slots.checked_mul(std::mem::size_of::<ContainsRecordFrame<'_>>())?,
        )?
        .checked_add(type_slots.checked_mul(std::mem::size_of::<TypeFormatFrame<'_>>())?)?
        .checked_add(pattern_slots.checked_mul(std::mem::size_of::<PatternFormatFrame<'_>>())?)?;
    Some(PrivateScratchCapacity {
        expression_slots,
        contains_record_slots,
        type_slots,
        pattern_slots,
        bytes,
    })
}

#[derive(Clone, Copy)]
enum ScratchStackKind {
    Expression,
    ContainsRecord,
    Type,
    Pattern,
}

impl ScratchStackKind {
    #[cfg(test)]
    fn index(self) -> usize {
        match self {
            Self::Expression => 0,
            Self::ContainsRecord => 1,
            Self::Type => 2,
            Self::Pattern => 3,
        }
    }
}

thread_local! {
    static PRIVATE_SCRATCH_CAPACITY: std::cell::Cell<Option<PrivateScratchCapacity>> = const { std::cell::Cell::new(None) };
    #[cfg(test)]
    static PRIVATE_SCRATCH_HIGH_WATER: std::cell::Cell<[(usize, usize); 4]> = const { std::cell::Cell::new([(0, 0); 4]) };
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn reset_private_scratch_high_water() {
    PRIVATE_SCRATCH_HIGH_WATER.with(|water| water.set([(0, 0); 4]));
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn private_scratch_high_water() -> [(usize, usize); 4] {
    PRIVATE_SCRATCH_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
fn note_private_scratch_high_water(kind: ScratchStackKind, len: usize, capacity: usize) {
    PRIVATE_SCRATCH_HIGH_WATER.with(|water| {
        let mut values = water.get();
        let entry = &mut values[kind.index()];
        entry.0 = entry.0.max(len);
        entry.1 = entry.1.max(capacity);
        water.set(values);
    });
}

#[cfg(not(test))]
fn note_private_scratch_high_water(_: ScratchStackKind, _: usize, _: usize) {}

struct FormatFrameStack<T> {
    values: Vec<T>,
    limit: Option<usize>,
    kind: ScratchStackKind,
}

impl<T> FormatFrameStack<T> {
    fn new(initial: T, kind: ScratchStackKind) -> Self {
        let limit = PRIVATE_SCRATCH_CAPACITY.with(|capacity| {
            capacity.get().map(|capacity| match kind {
                ScratchStackKind::Expression => capacity.expression_slots,
                ScratchStackKind::ContainsRecord => capacity.contains_record_slots,
                ScratchStackKind::Type => capacity.type_slots,
                ScratchStackKind::Pattern => capacity.pattern_slots,
            })
        });
        let mut values = Vec::with_capacity(limit.unwrap_or(1));
        if let Some(limit) = limit {
            assert_eq!(
                values.capacity(),
                limit,
                "private formatter Vec capacity drift"
            );
        }
        values.push(initial);
        note_private_scratch_high_water(kind, values.len(), values.capacity());
        Self {
            values,
            limit,
            kind,
        }
    }

    fn push(&mut self, value: T) {
        if let Some(limit) = self.limit {
            assert!(
                self.values.len() < limit,
                "private formatter scratch census underflow"
            );
        }
        self.values.push(value);
        note_private_scratch_high_water(self.kind, self.values.len(), self.values.capacity());
    }

    fn pop(&mut self) -> Option<T> {
        self.values.pop()
    }
}

pub fn canonical(program: &Program) -> String {
    let _ = crate::bounded_output::reserve_active(legacy_canonical_temporary_bytes(program));
    let mut output = crate::bounded_output::CappedString::new();
    write_canonical(program, &mut output);
    output.into_string()
}

/// Write the canonical source projection into a caller-owned bounded sink.
/// Native/private builders use this to count and reserve the exact final
/// capacity before materializing a retained source String.
pub(crate) fn write_canonical(program: &Program, output: &mut impl std::fmt::Write) {
    write_canonical_commented(program, &comments::Placement::default(), output);
}

/// The canonical projection with the comments of `placement` restored at the
/// items they belong to; an empty placement renders plain canonical bytes.
pub(crate) fn write_canonical_commented(
    program: &Program,
    placement: &comments::Placement,
    output: &mut impl std::fmt::Write,
) {
    placement.header(output);
    writeln!(output, "module {};", program.module).unwrap();
    for module_use in &program.module_uses {
        placement.leading(output, module_use.span.start, 0);
        let kind = match module_use.kind {
            ModuleUseKind::Function => "function",
            ModuleUseKind::Type => "type",
            ModuleUseKind::Protocol => "protocol",
        };
        write!(output, "use {kind} @id(\"").unwrap();
        write_escaped(output, &module_use.persistent_id);
        writeln!(
            output,
            "\") from {} as {};",
            module_use.target_module, module_use.alias
        )
        .unwrap();
        placement.trailing(output, module_use.span.start, 0);
    }
    if !program.permits.is_empty() {
        write!(output, "\npermit {{ ").unwrap();
        write_joined(output, &program.permits, ", ");
        writeln!(output, " }}").unwrap();
    }
    for declaration in &program.types {
        writeln!(output).unwrap();
        placement.leading(output, declaration.span.start, 0);
        if declaration.explicit_id {
            write!(output, "@id(\"").unwrap();
            write_escaped(output, &declaration.stable_id);
            writeln!(output, "\")").unwrap();
        }
        match &declaration.kind {
            TypeDeclarationKind::Resource { lifecycles } => {
                if lifecycles.is_empty() {
                    write!(output, "resource {}", declaration.name).unwrap();
                    write_type_parameters(output, &declaration.type_parameters);
                    writeln!(output, ";").unwrap();
                    continue;
                }
                write!(output, "resource {}", declaration.name).unwrap();
                write_type_parameters(output, &declaration.type_parameters);
                writeln!(output, " {{").unwrap();
                for lifecycle in lifecycles {
                    placement.leading(output, lifecycle.span.start, 1);
                    if let Some(stable_id) = &lifecycle.stable_id {
                        write!(output, "    @id(\"").unwrap();
                        write_escaped(output, stable_id);
                        writeln!(output, "\")").unwrap();
                    }
                    match &lifecycle.kind {
                        ResourceLifecycleKind::Trivial => {
                            writeln!(output, "    drop trivial;").unwrap();
                        }
                        ResourceLifecycleKind::Imported { import_key } => {
                            write!(output, "    drop import \"").unwrap();
                            write_escaped(output, import_key);
                            writeln!(output, "\";").unwrap();
                        }
                    }
                    placement.trailing(output, lifecycle.span.start, 1);
                }
                placement.closing(output, declaration.span.end.saturating_sub(1), 1);
                writeln!(output, "}}").unwrap();
            }
            TypeDeclarationKind::Record { fields } => {
                write!(output, "record {}", declaration.name).unwrap();
                write_type_parameters(output, &declaration.type_parameters);
                writeln!(output, " {{").unwrap();
                for field in fields {
                    placement.leading(output, field.span.start, 1);
                    if field.explicit_id {
                        write!(output, "    @id(\"").unwrap();
                        write_escaped(output, &field.stable_id);
                        writeln!(output, "\")").unwrap();
                    }
                    write!(output, "    {}: ", field.name).unwrap();
                    write_type(output, &field.ty);
                    writeln!(output, ",").unwrap();
                    placement.trailing(output, field.span.start, 1);
                }
                placement.closing(output, declaration.span.end.saturating_sub(1), 1);
                writeln!(output, "}}").unwrap();
            }
            TypeDeclarationKind::Variant { cases } => {
                write!(output, "variant {}", declaration.name).unwrap();
                write_type_parameters(output, &declaration.type_parameters);
                writeln!(output, " {{").unwrap();
                for case in cases {
                    placement.leading(output, case.span.start, 1);
                    if case.explicit_id {
                        write!(output, "    @id(\"").unwrap();
                        write_escaped(output, &case.stable_id);
                        writeln!(output, "\")").unwrap();
                    }
                    if case.fields.is_empty() {
                        writeln!(output, "    {},", case.name).unwrap();
                        placement.trailing(output, case.span.start, 1);
                        continue;
                    }
                    writeln!(output, "    {} {{", case.name).unwrap();
                    for field in &case.fields {
                        placement.leading(output, field.span.start, 2);
                        if field.explicit_id {
                            write!(output, "        @id(\"").unwrap();
                            write_escaped(output, &field.stable_id);
                            writeln!(output, "\")").unwrap();
                        }
                        write!(output, "        {}: ", field.name).unwrap();
                        write_type(output, &field.ty);
                        writeln!(output, ",").unwrap();
                        placement.trailing(output, field.span.start, 2);
                    }
                    placement.closing(output, case.span.end.saturating_sub(2), 2);
                    writeln!(output, "    }},").unwrap();
                    placement.trailing(output, case.span.start, 1);
                }
                placement.closing(output, declaration.span.end.saturating_sub(1), 1);
                writeln!(output, "}}").unwrap();
            }
            TypeDeclarationKind::Class { fields, methods } => {
                write!(output, "class {}", declaration.name).unwrap();
                write_type_parameters(output, &declaration.type_parameters);
                if let Some(parent) = &declaration.extends {
                    write!(output, " : ").unwrap();
                    write_type(output, parent);
                }
                if fields.is_empty() && methods.is_empty() {
                    writeln!(output, " {{ }}").unwrap();
                    continue;
                }
                writeln!(output, " {{").unwrap();
                for field in fields {
                    placement.leading(output, field.span.start, 1);
                    if field.explicit_id {
                        write!(output, "    @id(\"").unwrap();
                        write_escaped(output, &field.stable_id);
                        writeln!(output, "\")").unwrap();
                    }
                    write!(output, "    {}: ", field.name).unwrap();
                    write_type(output, &field.ty);
                    writeln!(output, ",").unwrap();
                    placement.trailing(output, field.span.start, 1);
                }
                for method in methods {
                    writeln!(output).unwrap();
                    placement.leading(output, method.span.start, 1);
                    if method.explicit_id {
                        write!(output, "    @id(\"").unwrap();
                        write_escaped(output, &method.stable_id);
                        writeln!(output, "\")").unwrap();
                    }
                    write!(output, "    fn {}", method.name).unwrap();
                    write_type_parameters(output, &method.type_parameters);
                    output.write_char('(').unwrap();
                    for (index, param) in method.params.iter().enumerate() {
                        if index > 0 {
                            output.write_str(", ").unwrap();
                        }
                        write!(output, "{}: {}", param.name, param.mode.source_prefix()).unwrap();
                        write_type(output, &param.ty);
                    }
                    output.write_str(") -> ").unwrap();
                    write_type(output, &method.return_type);
                    writeln!(output).unwrap();
                    if !method.effects.is_empty() {
                        write!(output, "        uses {{ ").unwrap();
                        write_joined(output, &method.effects, ", ");
                        writeln!(output, " }}").unwrap();
                    }
                    write_indented_function_body(output, &method.body, 2, placement);
                    placement.trailing(output, method.span.start, 1);
                }
                placement.closing(output, declaration.span.end.saturating_sub(1), 1);
                writeln!(output, "}}").unwrap();
            }
        }
        placement.trailing(output, declaration.span.start, 0);
    }
    for interface in &program.interfaces {
        writeln!(output).unwrap();
        placement.leading(output, interface.span.start, 0);
        if interface.explicit_id {
            write!(output, "@id(\"").unwrap();
            write_escaped(output, &interface.stable_id);
            writeln!(output, "\")").unwrap();
        }
        writeln!(output, "interface {}", interface.name).unwrap();
        write!(output, "    permits {{ ").unwrap();
        write_joined(output, &interface.permits, ", ");
        writeln!(output, " }}").unwrap();
        writeln!(output, "{{").unwrap();
        for import in &interface.imports {
            if import.explicit_id {
                write!(output, "    @id(\"").unwrap();
                write_escaped(output, &import.stable_id);
                writeln!(output, "\")").unwrap();
            }
            write!(
                output,
                "    import {}fn {}(",
                if import.native_rust { "rust " } else { "" },
                import.name
            )
            .unwrap();
            for (index, param) in import.params.iter().enumerate() {
                if index > 0 {
                    output.write_str(", ").unwrap();
                }
                write!(output, "{}: {}", param.name, param.mode.source_prefix()).unwrap();
                write_type(output, &param.ty);
            }
            writeln!(output, ") -> {}", import.result).unwrap();
            write!(output, "        effects {{ ").unwrap();
            write_joined(output, &import.effects, ", ");
            writeln!(output, " }}").unwrap();
            match &import.failure {
                ImportFailure::Infallible => {
                    writeln!(
                        output,
                        "        failure infallible{}",
                        if import.native_rust { ";" } else { "" }
                    )
                    .unwrap();
                }
                ImportFailure::Status { domain_id } => {
                    write!(output, "        failure status \"").unwrap();
                    write_escaped(output, domain_id);
                    writeln!(output, "\"{}", if import.native_rust { ";" } else { "" }).unwrap();
                }
            }
            if !import.native_rust {
                writeln!(output, "        consumes {} always;", import.consumes).unwrap();
            }
        }
        writeln!(output, "}}").unwrap();
        placement.trailing(output, interface.span.start, 0);
    }
    for protocol in &program.protocols {
        writeln!(output).unwrap();
        placement.leading(output, protocol.span.start, 0);
        if protocol.explicit_id {
            write!(output, "@id(\"").unwrap();
            write_escaped(output, &protocol.stable_id);
            writeln!(output, "\")").unwrap();
        }
        writeln!(output, "protocol {} {{", protocol.name).unwrap();
        for method in &protocol.methods {
            if method.explicit_id {
                write!(output, "    @id(\"").unwrap();
                write_escaped(output, &method.stable_id);
                writeln!(output, "\")").unwrap();
            }
            write!(output, "    fn {}(", method.name).unwrap();
            for (index, param) in method.params.iter().enumerate() {
                if index > 0 {
                    output.write_str(", ").unwrap();
                }
                write!(output, "{}: {}", param.name, param.mode.source_prefix()).unwrap();
                write_type(output, &param.ty);
            }
            output.write_str(") -> ").unwrap();
            write_type(output, &method.return_type);
            writeln!(output, ";").unwrap();
        }
        writeln!(output, "}}").unwrap();
        placement.trailing(output, protocol.span.start, 0);
    }
    for implementation in &program.implementations {
        writeln!(output).unwrap();
        placement.leading(output, implementation.span.start, 0);
        if implementation.explicit_id {
            write!(output, "@id(\"").unwrap();
            write_escaped(output, &implementation.stable_id);
            writeln!(output, "\")").unwrap();
        }
        write!(output, "impl \"").unwrap();
        write_escaped(output, &implementation.protocol_id);
        write!(output, "\" for \"").unwrap();
        write_escaped(output, &implementation.receiver_id);
        writeln!(output, "\" {{").unwrap();
        for member in &implementation.members {
            write!(output, "    \"").unwrap();
            write_escaped(output, &member.method_id);
            write!(output, "\" = \"").unwrap();
            write_escaped(output, &member.function_id);
            writeln!(output, "\";").unwrap();
        }
        writeln!(output, "}}").unwrap();
        placement.trailing(output, implementation.span.start, 0);
    }
    for function in &program.functions {
        writeln!(output).unwrap();
        placement.leading(output, function.span.start, 0);
        if function.explicit_id {
            write!(output, "@id(\"").unwrap();
            write_escaped(output, &function.stable_id);
            writeln!(output, "\")").unwrap();
        }
        write!(output, "fn {}", function.name).unwrap();
        write_type_parameters(output, &function.type_parameters);
        output.write_char('(').unwrap();
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                output.write_str(", ").unwrap();
            }
            write!(output, "{}: {}", param.name, param.mode.source_prefix()).unwrap();
            write_type(output, &param.ty);
        }
        output.write_str(") -> ").unwrap();
        write_type(output, &function.return_type);
        writeln!(output).unwrap();
        if !function.effects.is_empty() {
            write!(output, "    uses {{ ").unwrap();
            write_joined(output, &function.effects, ", ");
            writeln!(output, " }}").unwrap();
        }
        for contract in &function.requires {
            write!(output, "    requires ").unwrap();
            write_record_literal_delimited_expr(output, contract);
            writeln!(output).unwrap();
        }
        for contract in &function.ensures {
            write!(output, "    ensures ").unwrap();
            write_record_literal_delimited_expr(output, contract);
            writeln!(output).unwrap();
        }
        write_function_body(output, &function.body, placement);
        placement.trailing(output, function.span.start, 0);
    }
    placement.file_end(output);
}

#[allow(dead_code)]
pub(crate) fn write_canonical_with_scratch(
    program: &Program,
    output: &mut impl std::fmt::Write,
    capacity: PrivateScratchCapacity,
) {
    struct Restore(Option<PrivateScratchCapacity>);
    impl Drop for Restore {
        fn drop(&mut self) {
            PRIVATE_SCRATCH_CAPACITY.with(|slot| slot.set(self.0));
        }
    }
    let previous = PRIVATE_SCRATCH_CAPACITY.with(|slot| slot.replace(Some(capacity)));
    let _restore = Restore(previous);
    write_canonical(program, output);
}

pub fn expr(value: &Expr, parent_precedence: u8) -> String {
    let _ = crate::bounded_output::reserve_active(legacy_expr_temporary_bytes(
        value,
        parent_precedence,
    ));
    let mut output = String::new();
    write_expr(&mut output, value, parent_precedence);
    output
}

fn write_expr(output: &mut impl std::fmt::Write, value: &Expr, parent_precedence: u8) {
    write_expr_measured(output, value, parent_precedence, None);
}

fn rendered_expr_lengths(value: &Expr, parent_precedence: u8) -> HashMap<(usize, u8), usize> {
    struct Sink;
    impl std::fmt::Write for Sink {
        fn write_str(&mut self, _: &str) -> std::fmt::Result {
            Ok(())
        }
    }
    let mut lengths = HashMap::new();
    write_expr_measured(&mut Sink, value, parent_precedence, Some(&mut lengths));
    lengths
}

fn write_expr_measured(
    output: &mut impl std::fmt::Write,
    value: &Expr,
    parent_precedence: u8,
    mut lengths: Option<&mut HashMap<(usize, u8), usize>>,
) {
    struct Positioned<'a, W> {
        inner: &'a mut W,
        bytes: usize,
    }
    impl<W: std::fmt::Write> std::fmt::Write for Positioned<'_, W> {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            self.inner.write_str(value)?;
            self.bytes = self.bytes.saturating_add(value.len());
            Ok(())
        }
    }

    let mut output = Positioned {
        inner: output,
        bytes: 0,
    };
    use ExprFormatFrame as Frame;
    let mut frames = FormatFrameStack::new(
        Frame::Expr(value, parent_precedence),
        ScratchStackKind::Expression,
    );
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Expr(value, parent_precedence) => {
                if lengths.is_some() {
                    frames.push(Frame::MeasureEnd(
                        value as *const Expr as usize,
                        parent_precedence,
                        output.bytes,
                    ));
                }
                match &value.kind {
                    ExprKind::Int(number) => write!(output, "{number}").unwrap(),
                    ExprKind::Int32(value) => {
                        // The explicit suffix keeps the declared width stable
                        // across canonical round trips.
                        output.write_str(&format!("{value}i32")).unwrap();
                    }
                    ExprKind::Uint8(value) => {
                        // The explicit suffix keeps the declared width stable
                        // across canonical round trips.
                        write!(output, "{value}u8").unwrap();
                    }
                    ExprKind::Usize(value) => {
                        write!(output, "{value}usize").unwrap();
                    }
                    ExprKind::ArrayU8(values) => {
                        output.write_char('[').unwrap();
                        for (index, value) in values.iter().enumerate() {
                            if index != 0 {
                                output.write_str(", ").unwrap();
                            }
                            write!(output, "{value}u8").unwrap();
                        }
                        output.write_char(']').unwrap();
                    }
                    ExprKind::RepeatArrayU8 { value, count } => {
                        write!(output, "[{value}u8; {count}]").unwrap();
                    }
                    ExprKind::Char(value) => {
                        output.write_str(&canonical_char(*value)).unwrap();
                    }
                    ExprKind::Float32(bits) => {
                        // The explicit suffix keeps the declared precision stable
                        // across canonical round trips.
                        output.write_str(&canonical_f32_bits(*bits)).unwrap();
                        output.write_str("f32").unwrap();
                    }
                    ExprKind::Float64(bits) => {
                        output.write_str(&canonical_f64_bits(*bits)).unwrap()
                    }
                    ExprKind::Bool(value) => write!(output, "{value}").unwrap(),
                    ExprKind::String(value) => {
                        output.write_char('"').unwrap();
                        write_string_escaped(&mut output, value);
                        output.write_char('"').unwrap();
                    }
                    ExprKind::Var(name) => output.write_str(name).unwrap(),
                    ExprKind::Call {
                        name,
                        type_arguments,
                        args,
                    } => {
                        output.write_str(name).unwrap();
                        if !type_arguments.is_empty() {
                            output.write_char('<').unwrap();
                            for (index, argument) in type_arguments.iter().enumerate() {
                                if index != 0 {
                                    output.write_str(", ").unwrap();
                                }
                                write_type(&mut output, argument);
                            }
                            output.write_char('>').unwrap();
                        }
                        output.write_char('(').unwrap();
                        frames.push(Frame::Close(')'));
                        frames.push(Frame::CallArgs(args, 0));
                    }
                    ExprKind::Unary { op, value } => {
                        output
                            .write_str(match op {
                                UnaryOp::Neg => "-",
                                UnaryOp::Not => "!",
                            })
                            .unwrap();
                        frames.push(Frame::Expr(value, 7));
                    }
                    ExprKind::Binary { op, left, right } => {
                        let precedence = op.precedence();
                        let delimited = precedence < parent_precedence;
                        if delimited {
                            output.write_char('(').unwrap();
                        }
                        frames.push(Frame::BinaryRight(right, *op, delimited));
                        frames.push(Frame::Expr(left, precedence));
                    }
                    ExprKind::Block { statements, tail } => {
                        output.write_str("{ ").unwrap();
                        frames.push(Frame::Block(statements, tail, 0));
                    }
                    ExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        output.write_str("if ").unwrap();
                        let delimited = contains_record_construction(condition);
                        if delimited {
                            output.write_char('(').unwrap();
                        }
                        frames.push(Frame::IfThen(then_branch, else_branch));
                        if delimited {
                            frames.push(Frame::Close(')'));
                        }
                        frames.push(Frame::Expr(condition, 0));
                    }
                    ExprKind::ConstructRecord {
                        type_name,
                        type_arguments,
                        fields,
                        ..
                    } => {
                        output.write_str(type_name).unwrap();
                        write_type_arguments(&mut output, type_arguments);
                        if fields.is_empty() {
                            output.write_str(" {}").unwrap();
                        } else {
                            output.write_str(" { ").unwrap();
                            frames.push(Frame::Fields(fields, 0, " }"));
                        }
                    }
                    ExprKind::ConstructVariant {
                        type_name,
                        type_arguments,
                        case_name,
                        fields,
                        ..
                    } => {
                        output.write_str(type_name).unwrap();
                        write_type_arguments(&mut output, type_arguments);
                        write!(output, "::{case_name}").unwrap();
                        if fields.is_empty() {
                            output.write_str(" {}").unwrap();
                        } else {
                            output.write_str(" { ").unwrap();
                            frames.push(Frame::Fields(fields, 0, " }"));
                        }
                    }
                    ExprKind::Match {
                        mode,
                        scrutinee,
                        arms,
                    } => {
                        output.write_str("match ").unwrap();
                        output.write_str(mode.source_prefix()).unwrap();
                        let delimited = contains_record_construction(scrutinee);
                        if delimited {
                            output.write_char('(').unwrap();
                        }
                        frames.push(Frame::MatchArms(arms, 0));
                        if delimited {
                            frames.push(Frame::Close(')'));
                        }
                        frames.push(Frame::Expr(scrutinee, 0));
                    }
                    ExprKind::Try { operand } => {
                        let delimited = matches!(
                            operand.kind,
                            ExprKind::Binary { .. } | ExprKind::If { .. } | ExprKind::Block { .. }
                        );
                        if delimited {
                            output.write_char('(').unwrap();
                        }
                        frames.push(Frame::TryEnd(delimited));
                        frames.push(Frame::Expr(operand, if delimited { 0 } else { 8 }));
                    }
                    ExprKind::UpdateRecord { base, fields } => {
                        let delimited = matches!(
                            base.kind,
                            ExprKind::Binary { .. } | ExprKind::If { .. } | ExprKind::Block { .. }
                        );
                        if delimited {
                            output.write_char('(').unwrap();
                        }
                        frames.push(Frame::PostfixFields(fields));
                        if delimited {
                            frames.push(Frame::Close(')'));
                        }
                        frames.push(Frame::Expr(base, if delimited { 0 } else { 8 }));
                    }
                    ExprKind::Project { base, field, .. } => {
                        let delimited = matches!(
                            base.kind,
                            ExprKind::Binary { .. } | ExprKind::If { .. } | ExprKind::Block { .. }
                        );
                        if delimited {
                            output.write_char('(').unwrap();
                        }
                        frames.push(Frame::ProjectField(field));
                        if delimited {
                            frames.push(Frame::Close(')'));
                        }
                        frames.push(Frame::Expr(base, if delimited { 0 } else { 8 }));
                    }
                    ExprKind::MethodCall {
                        receiver,
                        method,
                        type_arguments,
                        args,
                        ..
                    } => {
                        let delimited = matches!(
                            receiver.kind,
                            ExprKind::Binary { .. } | ExprKind::If { .. } | ExprKind::Block { .. }
                        );
                        if delimited {
                            output.write_char('(').unwrap();
                        }
                        frames.push(Frame::MethodCallSuffix(
                            method,
                            type_arguments,
                            args,
                            delimited,
                        ));
                        frames.push(Frame::Expr(receiver, if delimited { 0 } else { 8 }));
                    }
                    ExprKind::SuperMethod { method, args, .. } => {
                        output.write_str("super.").unwrap();
                        output.write_str(method).unwrap();
                        output.write_char('(').unwrap();
                        if args.is_empty() {
                            output.write_char(')').unwrap();
                        } else {
                            frames.push(Frame::Close(')'));
                            frames.push(Frame::CallArgs(args, 0));
                        }
                    }
                }
            }
            Frame::MeasureEnd(expression, precedence, start) => {
                lengths
                    .as_deref_mut()
                    .expect("measurement frames require a length table")
                    .insert((expression, precedence), output.bytes.saturating_sub(start));
            }
            Frame::CallArgs(args, index) => {
                if let Some(argument) = args.get(index) {
                    if index != 0 {
                        output.write_str(", ").unwrap();
                    }
                    frames.push(Frame::CallArgs(args, index + 1));
                    frames.push(Frame::Expr(argument, 0));
                }
            }
            Frame::BinaryRight(right, op, delimited) => {
                write!(output, " {} ", op.text()).unwrap();
                if delimited {
                    frames.push(Frame::Close(')'));
                }
                frames.push(Frame::Expr(right, op.precedence() + 1));
            }
            Frame::Block(statements, tail, index) => {
                if let Some(statement) = statements.get(index) {
                    match statement {
                        Statement::Let {
                            name,
                            mutable,
                            declared,
                            value,
                            ..
                        } => {
                            if *mutable {
                                write!(output, "let mut {name}").unwrap();
                            } else {
                                write!(output, "let {name}").unwrap();
                            }
                            if let Some(ty) = declared {
                                write!(output, ": ").unwrap();
                                write_type(&mut output, ty);
                            }
                            write!(output, " = ").unwrap();
                            frames.push(Frame::BlockNext(statements, tail, index + 1));
                            frames.push(Frame::Expr(value, 0));
                        }
                        Statement::Assign {
                            name, field, value, ..
                        } => {
                            match field {
                                Some(field) => {
                                    write!(output, "{name}.{} = ", field.name).unwrap();
                                }
                                None => write!(output, "{name} = ").unwrap(),
                            }
                            frames.push(Frame::BlockNext(statements, tail, index + 1));
                            frames.push(Frame::Expr(value, 0));
                        }
                        Statement::Unsafe { audit, body, .. } => {
                            write!(output, "@audit(\"").unwrap();
                            write_escaped(&mut output, audit);
                            write!(output, "\") unsafe ").unwrap();
                            // Unsafe boundary statements are not
                            // semicolon-terminated by the grammar, so the
                            // following separator is a bare space.
                            frames.push(Frame::BlockNextAfterUnsafe(statements, tail, index + 1));
                            // The body is an ordinary block and renders with
                            // the exact same inline block shape.
                            frames.push(Frame::Expr(body, 0));
                        }
                        Statement::While {
                            condition, body, ..
                        } => {
                            write!(output, "while ").unwrap();
                            // While statements are not semicolon-terminated;
                            // like unsafe boundaries they are followed by one
                            // bare space before the next statement. Frames run
                            // in reverse push order, so the trailing separator
                            // is pushed first and the condition last.
                            frames.push(Frame::BlockNextAfterWhile(statements, tail, index + 1));
                            frames.push(Frame::WhileBody(body));
                            frames.push(Frame::Expr(condition, 0));
                        }
                    }
                } else {
                    frames.push(Frame::Close('}'));
                    frames.push(Frame::Expr(tail, 0));
                }
            }
            Frame::BlockNext(statements, tail, index) => {
                output.write_str("; ").unwrap();
                frames.push(Frame::Block(statements, tail, index));
            }
            Frame::BlockNextAfterUnsafe(statements, tail, index) => {
                output.write_char(' ').unwrap();
                frames.push(Frame::Block(statements, tail, index));
            }
            Frame::WhileBody(body) => {
                output.write_char(' ').unwrap();
                frames.push(Frame::Expr(body, 0));
            }
            Frame::BlockNextAfterWhile(statements, tail, index) => {
                output.write_char(' ').unwrap();
                frames.push(Frame::Block(statements, tail, index));
            }
            Frame::IfThen(then_branch, else_branch) => {
                output.write_char(' ').unwrap();
                frames.push(Frame::IfElse(else_branch));
                frames.push(Frame::Expr(then_branch, 0));
            }
            Frame::IfElse(else_branch) => {
                output.write_str(" else ").unwrap();
                frames.push(Frame::Expr(else_branch, 0));
            }
            Frame::Fields(fields, index, suffix) => {
                if let Some(field) = fields.get(index) {
                    if index != 0 {
                        output.write_str(", ").unwrap();
                    }
                    write!(output, "{}: ", field.name).unwrap();
                    frames.push(Frame::Fields(fields, index + 1, suffix));
                    frames.push(Frame::Expr(&field.value, 0));
                } else {
                    output.write_str(suffix).unwrap();
                }
            }
            Frame::MatchArms(arms, index) => {
                if let Some(arm) = arms.get(index) {
                    if index == 0 {
                        output.write_str(" { ").unwrap();
                    } else {
                        output.write_str(", ").unwrap();
                    }
                    write_match_pattern(&mut output, &arm.pattern);
                    // Frames run LIFO: the continuation frame goes in first
                    // so the guard/value render before the next arm.
                    frames.push(Frame::MatchArms(arms, index + 1));
                    match &arm.guard {
                        // Refutable Match v1: `pattern if guard => value`.
                        Some(guard) => {
                            output.write_str(" if ").unwrap();
                            frames.push(Frame::MatchArmValue(&arm.value));
                            frames.push(Frame::Expr(guard.as_ref(), 0));
                        }
                        None => {
                            output.write_str(" => ").unwrap();
                            frames.push(Frame::Expr(&arm.value, 0));
                        }
                    }
                } else if index == 0 {
                    output.write_str(" {  }").unwrap();
                } else {
                    output.write_str(", }").unwrap();
                }
            }
            Frame::MatchArmValue(value) => {
                output.write_str(" => ").unwrap();
                frames.push(Frame::Expr(value, 0));
            }
            Frame::TryEnd(delimited) => {
                if delimited {
                    output.write_char(')').unwrap();
                }
                output.write_char('?').unwrap();
            }
            Frame::PostfixFields(fields) => {
                if fields.is_empty() {
                    output.write_str(" with {}").unwrap();
                } else {
                    output.write_str(" with { ").unwrap();
                    frames.push(Frame::Fields(fields, 0, " }"));
                }
            }
            Frame::ProjectField(field) => write!(output, ".{field}").unwrap(),
            Frame::MethodCallSuffix(method, type_arguments, args, delimited) => {
                if delimited {
                    output.write_char(')').unwrap();
                }
                output.write_char('.').unwrap();
                output.write_str(method).unwrap();
                if !type_arguments.is_empty() {
                    output.write_str("::").unwrap();
                    output.write_char('<').unwrap();
                    for (index, argument) in type_arguments.iter().enumerate() {
                        if index != 0 {
                            output.write_str(", ").unwrap();
                        }
                        write_type(&mut output, argument);
                    }
                    output.write_char('>').unwrap();
                }
                output.write_char('(').unwrap();
                frames.push(Frame::Close(')'));
                frames.push(Frame::CallArgs(args, 0));
            }
            Frame::Close('}') => output.write_str(" }").unwrap(),
            Frame::Close(character) => output.write_char(character).unwrap(),
        }
    }
}

fn write_record_literal_delimited_expr(output: &mut impl std::fmt::Write, value: &Expr) {
    let delimited = contains_record_construction(value);
    if delimited {
        output.write_char('(').unwrap();
    }
    write_expr(output, value, 0);
    if delimited {
        output.write_char(')').unwrap();
    }
}

fn write_type_parameters(
    output: &mut impl std::fmt::Write,
    parameters: &[crate::ast::TypeParameterDeclaration],
) {
    if parameters.is_empty() {
        return;
    }
    output.write_char('<').unwrap();
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            output.write_str(", ").unwrap();
        }
        output.write_str(&parameter.name).unwrap();
    }
    output.write_char('>').unwrap();
}

fn write_type_arguments(output: &mut impl std::fmt::Write, arguments: &[crate::ast::Type]) {
    if arguments.is_empty() {
        return;
    }
    output.write_char('<').unwrap();
    for (index, argument) in arguments.iter().enumerate() {
        if index != 0 {
            output.write_str(", ").unwrap();
        }
        write_type(output, argument);
    }
    output.write_char('>').unwrap();
}

fn write_type(output: &mut impl std::fmt::Write, ty: &crate::ast::Type) {
    use TypeFormatFrame as Frame;
    let mut frames = FormatFrameStack::new(Frame::Type(ty), ScratchStackKind::Type);
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Type(crate::ast::Type::I64) => output.write_str("i64").unwrap(),
            Frame::Type(crate::ast::Type::I32) => output.write_str("i32").unwrap(),
            Frame::Type(crate::ast::Type::Char) => output.write_str("char").unwrap(),
            Frame::Type(crate::ast::Type::U8) => output.write_str("u8").unwrap(),
            Frame::Type(crate::ast::Type::Usize) => output.write_str("usize").unwrap(),
            Frame::Type(crate::ast::Type::ArrayU8(length)) => {
                write!(output, "[u8; {length}]").unwrap();
            }
            Frame::Type(crate::ast::Type::F32) => output.write_str("f32").unwrap(),
            Frame::Type(crate::ast::Type::F64) => output.write_str("f64").unwrap(),
            Frame::Type(crate::ast::Type::Bool) => output.write_str("bool").unwrap(),
            Frame::Type(crate::ast::Type::String) => output.write_str("string").unwrap(),
            Frame::Type(crate::ast::Type::Bytes) => output.write_str("Bytes").unwrap(),
            Frame::Type(crate::ast::Type::Str) => output.write_str("str").unwrap(),
            Frame::Type(crate::ast::Type::SliceU8) => output.write_str("Slice<u8>").unwrap(),
            Frame::Type(crate::ast::Type::Named { name, arguments }) => {
                output.write_str(name).unwrap();
                if !arguments.is_empty() {
                    output.write_char('<').unwrap();
                    frames.push(Frame::Arguments(arguments, 0));
                }
            }
            Frame::Arguments(arguments, index) => {
                if let Some(argument) = arguments.get(index) {
                    if index != 0 {
                        output.write_str(", ").unwrap();
                    }
                    frames.push(Frame::Arguments(arguments, index + 1));
                    frames.push(Frame::Type(argument));
                } else {
                    output.write_char('>').unwrap();
                }
            }
        }
    }
}

fn write_match_pattern(output: &mut impl std::fmt::Write, pattern: &MatchPattern) {
    match pattern {
        MatchPattern::Wildcard { .. } => output.write_char('_').unwrap(),
        MatchPattern::Literal { value, .. } => {
            write_pattern_literal(output, *value);
        }
        MatchPattern::Binding { name, .. } => output.write_str(name).unwrap(),
        MatchPattern::Or { alternatives, .. } => {
            for (index, alternative) in alternatives.iter().enumerate() {
                if index != 0 {
                    output.write_str(" | ").unwrap();
                }
                write_match_pattern(output, alternative);
            }
        }
        MatchPattern::Variant {
            type_name,
            case_name,
            fields,
            ..
        } => {
            write!(output, "{type_name}::{case_name}").unwrap();
            if fields.is_empty() {
                output.write_str(" {}").unwrap();
            } else {
                output.write_str(" { ").unwrap();
                for (index, field) in fields.iter().enumerate() {
                    if index != 0 {
                        output.write_str(", ").unwrap();
                    }
                    if field.name == field.binding {
                        output.write_str(&field.name).unwrap();
                    } else {
                        write!(output, "{}: {}", field.name, field.binding).unwrap();
                    }
                }
                output.write_str(" }").unwrap();
            }
        }
        MatchPattern::Record {
            type_name, fields, ..
        } => write_record_match_pattern(output, type_name, fields),
    }
}

fn write_pattern_literal(output: &mut impl std::fmt::Write, value: crate::ast::PatternLiteral) {
    match value {
        crate::ast::PatternLiteral::Int(value) => write!(output, "{value}").unwrap(),
        // The explicit suffix keeps the declared width stable across
        // canonical round trips, exactly like expression literals.
        crate::ast::PatternLiteral::Int32(value) => write!(output, "{value}i32").unwrap(),
        crate::ast::PatternLiteral::Uint8(value) => write!(output, "{value}u8").unwrap(),
        crate::ast::PatternLiteral::Usize(value) => write!(output, "{value}usize").unwrap(),
        crate::ast::PatternLiteral::Char(value) => {
            output.write_str(&canonical_char(value)).unwrap()
        }
        crate::ast::PatternLiteral::Bool(value) => write!(output, "{value}").unwrap(),
    }
}

fn write_record_match_pattern(
    output: &mut impl std::fmt::Write,
    type_name: &str,
    fields: &[crate::ast::RecordMatchPatternField],
) {
    use PatternFormatFrame as Frame;
    let mut frames =
        FormatFrameStack::new(Frame::Enter(type_name, fields), ScratchStackKind::Pattern);
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(type_name, fields) => {
                output.write_str(type_name).unwrap();
                if fields.is_empty() {
                    output.write_str(" {}").unwrap();
                } else {
                    output.write_str(" { ").unwrap();
                    frames.push(Frame::Fields(fields, 0));
                }
            }
            Frame::Fields(fields, index) => {
                let Some(field) = fields.get(index) else {
                    output.write_str(" }").unwrap();
                    continue;
                };
                if index != 0 {
                    output.write_str(", ").unwrap();
                }
                frames.push(Frame::Fields(fields, index + 1));
                match &field.pattern {
                    crate::ast::RecordMatchFieldPattern::Binding { name, .. }
                        if name == &field.name =>
                    {
                        output.write_str(&field.name).unwrap();
                    }
                    crate::ast::RecordMatchFieldPattern::Binding { name, .. } => {
                        write!(output, "{}: {name}", field.name).unwrap();
                    }
                    crate::ast::RecordMatchFieldPattern::Wildcard { .. } => {
                        write!(output, "{}: _", field.name).unwrap();
                    }
                    crate::ast::RecordMatchFieldPattern::Record {
                        type_name, fields, ..
                    } => {
                        write!(output, "{}: ", field.name).unwrap();
                        frames.push(Frame::Enter(type_name, fields));
                    }
                }
            }
        }
    }
}

fn contains_record_construction(value: &Expr) -> bool {
    use ContainsRecordFrame as Frame;
    fn child(value: &Expr, index: usize) -> Option<&Expr> {
        match &value.kind {
            ExprKind::Call { args, .. } => args.get(index),
            ExprKind::Unary { value, .. }
            | ExprKind::Try { operand: value }
            | ExprKind::Project { base: value, .. } => (index == 0).then_some(value),
            ExprKind::UpdateRecord { base, fields } => {
                if index == 0 {
                    Some(base)
                } else {
                    fields.get(index - 1).map(|field| &field.value)
                }
            }
            ExprKind::Binary { left, right, .. } => {
                [left.as_ref(), right.as_ref()].get(index).copied()
            }
            ExprKind::Block { statements, tail } => {
                let mut offset = 0;
                for statement in statements {
                    let count = statement.child_count();
                    if index < offset + count {
                        return statement.child(index - offset);
                    }
                    offset += count;
                }
                (index == offset).then_some(tail)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => [
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ]
            .get(index)
            .copied(),
            ExprKind::Match {
                scrutinee, arms, ..
            } => {
                if index == 0 {
                    Some(scrutinee)
                } else {
                    let mut cursor = index - 1;
                    for arm in arms {
                        if let Some(guard) = arm.guard.as_deref() {
                            if cursor == 0 {
                                return Some(guard);
                            }
                            cursor -= 1;
                        }
                        if cursor == 0 {
                            return Some(&arm.value);
                        }
                        cursor -= 1;
                    }
                    None
                }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                if index == 0 {
                    Some(receiver)
                } else {
                    args.get(index - 1)
                }
            }
            ExprKind::SuperMethod { args, .. } => args.get(index),
            ExprKind::ConstructRecord { .. }
            | ExprKind::ConstructVariant { .. }
            | ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Char(_)
            | ExprKind::Uint8(_)
            | ExprKind::Usize(_)
            | ExprKind::ArrayU8(_)
            | ExprKind::RepeatArrayU8 { .. }
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Var(_) => None,
        }
    }
    let mut frames = FormatFrameStack::new(Frame::Enter(value), ScratchStackKind::ContainsRecord);
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(value) => {
                if matches!(
                    value.kind,
                    ExprKind::ConstructRecord { .. } | ExprKind::ConstructVariant { .. }
                ) {
                    return true;
                }
                frames.push(Frame::Children(value, 0));
            }
            Frame::Children(value, index) => {
                if let Some(child) = child(value, index) {
                    frames.push(Frame::Children(value, index + 1));
                    frames.push(Frame::Enter(child));
                }
            }
        }
    }
    false
}

fn write_function_body(
    output: &mut impl std::fmt::Write,
    body: &Expr,
    placement: &comments::Placement,
) {
    write_indented_function_body(output, body, 1, placement);
}

fn write_indented_function_body(
    output: &mut impl std::fmt::Write,
    body: &Expr,
    depth: usize,
    placement: &comments::Placement,
) {
    let indent = "    ".repeat(depth);
    writeln!(output, "{{").unwrap();
    if let ExprKind::Block { statements, tail } = &body.kind {
        write_block_items(output, statements, tail, depth, placement);
        placement.closing(output, body.span.end.saturating_sub(1), depth);
    } else {
        output.write_str(&indent).unwrap();
        write_expr(output, body, 0);
        writeln!(output).unwrap();
    }
    let closer = "    ".repeat(depth.saturating_sub(1));
    writeln!(output, "{closer}}}").unwrap();
}

fn write_indent(output: &mut impl std::fmt::Write, depth: usize) {
    for _ in 0..depth {
        output.write_str("    ").unwrap();
    }
}

/// Renders the statements and tail of a block body at `depth`, with the
/// comments that lead or trail each of them.
fn write_block_items(
    output: &mut impl std::fmt::Write,
    statements: &[Statement],
    tail: &Expr,
    depth: usize,
    placement: &comments::Placement,
) {
    for statement in statements {
        let start = statement_start(statement);
        placement.leading(output, start, depth);
        write_block_statement(output, statement, depth, placement);
        placement.trailing(output, start, depth);
    }
    placement.leading(output, tail.span.start, depth);
    write_indent(output, depth);
    write_expr(output, tail, 0);
    writeln!(output).unwrap();
    placement.trailing(output, tail.span.start, depth);
}

fn statement_start(statement: &Statement) -> usize {
    match statement {
        Statement::Let { span, .. }
        | Statement::Assign { span, .. }
        | Statement::Unsafe { span, .. }
        | Statement::While { span, .. } => span.start,
    }
}

/// Renders one statement of a multi-line block body. Unsafe boundary
/// statements open their own indented braces; their ordinary block bodies
/// recurse through this same helper.
fn write_block_statement(
    output: &mut impl std::fmt::Write,
    statement: &Statement,
    depth: usize,
    placement: &comments::Placement,
) {
    match statement {
        Statement::Let {
            name,
            mutable,
            declared,
            value,
            ..
        } => {
            write_indent(output, depth);
            if *mutable {
                write!(output, "let mut {name}").unwrap();
            } else {
                write!(output, "let {name}").unwrap();
            }
            if let Some(ty) = declared {
                write!(output, ": ").unwrap();
                write_type(output, ty);
            }
            write!(output, " = ").unwrap();
            write_expr(output, value, 0);
            writeln!(output, ";").unwrap();
        }
        Statement::Assign {
            name, field, value, ..
        } => {
            write_indent(output, depth);
            match field {
                Some(field) => write!(output, "{name}.{} = ", field.name).unwrap(),
                None => write!(output, "{name} = ").unwrap(),
            }
            write_expr(output, value, 0);
            writeln!(output, ";").unwrap();
        }
        Statement::Unsafe { audit, body, .. } => {
            write_indent(output, depth);
            write!(output, "@audit(\"").unwrap();
            write_escaped(output, audit);
            writeln!(output, "\") unsafe {{").unwrap();
            let ExprKind::Block { statements, tail } = &body.kind else {
                unreachable!("unsafe bodies always parse as blocks");
            };
            write_block_items(output, statements, tail, depth + 1, placement);
            placement.closing(output, body.span.end.saturating_sub(1), depth + 1);
            write_indent(output, depth);
            writeln!(output, "}}").unwrap();
        }
        Statement::While {
            condition, body, ..
        } => {
            write_indent(output, depth);
            write!(output, "while ").unwrap();
            write_expr(output, condition, 0);
            writeln!(output, " {{").unwrap();
            let ExprKind::Block { statements, tail } = &body.kind else {
                unreachable!("while bodies always parse as blocks");
            };
            write_block_items(output, statements, tail, depth + 1, placement);
            placement.closing(output, body.span.end.saturating_sub(1), depth + 1);
            write_indent(output, depth);
            writeln!(output, "}}").unwrap();
        }
    }
}

fn write_escaped(output: &mut impl std::fmt::Write, value: &str) {
    for value in value.chars() {
        match value {
            '\\' => output.write_str("\\\\").unwrap(),
            '"' => output.write_str("\\\"").unwrap(),
            value => output.write_char(value).unwrap(),
        }
    }
}

fn write_joined(output: &mut impl std::fmt::Write, values: &[String], separator: &str) {
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.write_str(separator).unwrap();
        }
        output.write_str(value).unwrap();
    }
}

#[cfg(test)]
mod iterative_formatter_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn expression_blocks_and_empty_match_keep_exact_separators() {
        let block = crate::parse(
            "module t; fn main()->i64 { { let x = 1; let y = 2; x + y } }",
            Path::new("format-block.spx"),
        )
        .unwrap();
        let ExprKind::Block { tail, .. } = &block.functions[0].body.kind else {
            unreachable!()
        };
        assert_eq!(expr(tail, 0), "{ let x = 1; let y = 2; x + y }");

        let empty = crate::parse(
            "module t; fn main(value:i64)->i64 { match value { } }",
            Path::new("format-empty-match.spx"),
        )
        .unwrap();
        let ExprKind::Block { tail, .. } = &empty.functions[0].body.kind else {
            unreachable!()
        };
        assert_eq!(expr(tail, 0), "match value {  }");
    }

    #[test]
    fn measured_render_records_each_subtree_once() {
        let mut sum = String::from("value");
        for _ in 1..64 {
            sum.push_str(" + value");
        }
        let source = format!("module t; fn main(value: i64) -> i64 {{ {sum} }}");
        let program = crate::parse(&source, Path::new("format-measured.spx")).unwrap();
        let ExprKind::Block { tail, .. } = &program.functions[0].body.kind else {
            unreachable!()
        };
        let lengths = rendered_expr_lengths(tail, 0);
        let mut nodes = 0usize;
        let mut stack = vec![tail.as_ref()];
        while let Some(expression) = stack.pop() {
            nodes += 1;
            let mut index = 0;
            while let Some(child) = expression.child(index) {
                stack.push(child);
                index += 1;
            }
        }
        assert_eq!(lengths.len(), nodes);
        assert_eq!(
            lengths[&(tail.as_ref() as *const Expr as usize, 0)],
            sum.len()
        );
    }

    #[test]
    fn unsafe_statement_in_inline_block_stays_parseable() {
        // The grammar terminates an unsafe boundary statement at its block;
        // the enclosing inline block's tail expression follows directly.
        let source = r#"
module t;
permit { unsafe }
fn main(value:i64)->i64 {
    { @audit("checked boundary") unsafe { value } value + 1 }
}
"#;
        let program = crate::parse(source, Path::new("format-unsafe.spx")).unwrap();
        let canonical = crate::format::canonical(&program);
        // The canonical text must re-parse: the unsafe statement is not
        // semicolon-terminated by the grammar.
        let reparsed = crate::parse(&canonical, Path::new("format-unsafe-2.spx"))
            .unwrap_or_else(|error| panic!("canonical text must re-parse: {error}\n{canonical}"));
        assert_eq!(
            canonical,
            crate::format::canonical(&reparsed),
            "canonical form must be idempotent"
        );
        assert!(
            canonical.contains("@audit(\"checked boundary\") unsafe { value } value + 1"),
            "{canonical}"
        );
    }
}
