//! Shared ordinary function writer; embedded Agent bodies are not a second projection.
use super::super::*;

pub(in super::super) fn write_function(
    function: &crate::ast::Function,
    output: &mut impl std::fmt::Write,
    placement: &comments::Placement,
    depth: usize,
) {
    placement.leading(output, function.span.start, depth);
    write_indent(output, depth);
    if function.explicit_id {
        write!(output, "@id(\"").unwrap();
        write_escaped(output, &function.stable_id);
        writeln!(output, "\")").unwrap();
        write_indent(output, depth);
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
        write_indent(output, depth + 1);
        write!(output, "uses {{ ").unwrap();
        write_joined(output, &function.effects, ", ");
        writeln!(output, " }}").unwrap();
    }
    if let Some(yields) = &function.yields {
        write_indent(output, depth + 1);
        write!(output, "yields ").unwrap();
        write_type(output, &yields.request_type);
        write!(output, " -> ").unwrap();
        write_type(output, &yields.response_type);
        writeln!(output).unwrap();
    }
    if let Some(follows) = &function.follows {
        write_indent(output, depth + 1);
        write!(output, "follows session protocol \"").unwrap();
        write_escaped(output, &follows.protocol_id);
        writeln!(output, "\"").unwrap();
    }
    for contract in &function.requires {
        write_indent(output, depth + 1);
        write!(output, "requires ").unwrap();
        write_record_literal_delimited_expr(output, contract);
        writeln!(output).unwrap();
    }
    for contract in &function.ensures {
        write_indent(output, depth + 1);
        write!(output, "ensures ").unwrap();
        write_record_literal_delimited_expr(output, contract);
        writeln!(output).unwrap();
    }
    write_indent(output, depth);
    if depth == 0 {
        write_function_body(output, &function.body, placement);
    } else {
        write_indented_function_body(output, &function.body, depth + 1, placement);
    }

    placement.trailing(output, function.span.start, depth);
}
