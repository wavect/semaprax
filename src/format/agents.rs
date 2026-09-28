use std::fmt::Write;

use crate::ast::Program;
#[path = "agents/embedded.rs"]
mod embedded;
pub(super) use embedded::write_function;

pub(super) fn write_agents(
    program: &Program,
    placement: &super::comments::Placement,
    output: &mut impl Write,
) {
    for agent in &program.agents {
        writeln!(output).unwrap();
        placement.leading(output, agent.span.start, 0);
        write!(output, "@id(\"").unwrap();
        super::write_escaped(output, &agent.stable_id);
        writeln!(output, "\")").unwrap();
        writeln!(output, "agent {} {{", agent.name).unwrap();
        writeln!(output, "    types {{").unwrap();
        for role in &agent.types {
            placement.leading(output, role.span.start, 2);
            write!(output, "        @id(\"").unwrap();
            super::write_escaped(output, &role.stable_id);
            writeln!(output, "\")").unwrap();
            writeln!(output, "        type {};", role.role.source_name()).unwrap();
            placement.trailing(output, role.span.start, 2);
        }
        writeln!(output, "    }}").unwrap();
        writeln!(output, "    operations {{").unwrap();
        for operation in &agent.operations {
            if let Some(function) = agent.embedded_function(operation, &program.functions) {
                write_function(function, output, placement, 2);
                continue;
            }
            placement.leading(output, operation.span.start, 2);
            write!(output, "        @id(\"").unwrap();
            super::write_escaped(output, &operation.stable_id);
            writeln!(output, "\")").unwrap();
            writeln!(
                output,
                "        {}fn {};",
                operation.kind.source_prefix(),
                operation.role.source_name()
            )
            .unwrap();
            placement.trailing(output, operation.span.start, 2);
        }
        writeln!(output, "    }}").unwrap();
        if let Some(binding) = &agent.model_wait {
            writeln!(output, "    model_wait_v1 {{").unwrap();
            writeln!(
                output,
                "        propose = {};",
                super::canonical_string(&binding.helper_id)
            )
            .unwrap();
            writeln!(output, "    }}").unwrap();
        }
        writeln!(output, "    runtime_v1 {{").unwrap();
        writeln!(
            output,
            "        canonical_json {};",
            super::canonical_string(&agent.runtime_v1_json)
        )
        .unwrap();
        writeln!(output, "    }}").unwrap();
        placement.closing(output, agent.span.end.saturating_sub(1), 0);
        writeln!(output, "}}").unwrap();
        placement.trailing(output, agent.span.start, 0);
    }
}

pub(super) fn is_embedded(program: &Program, index: usize) -> bool {
    program.agents.iter().any(|agent| {
        agent.operations.iter().any(|operation| {
            operation.embedded_function_index == Some(index)
                && agent
                    .embedded_function(operation, &program.functions)
                    .is_some()
        })
    })
}
