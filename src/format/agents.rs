use std::fmt::Write;

use crate::ast::AgentDeclaration;

pub(super) fn write_agents(
    agents: &[AgentDeclaration],
    placement: &super::comments::Placement,
    output: &mut impl Write,
) {
    for agent in agents {
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
