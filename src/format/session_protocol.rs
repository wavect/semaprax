//! Canonical projection of declared session protocols (issue #297). The
//! parser admits exactly one clause order, so this writer is total: states,
//! initial, terminals in source order, transitions in source order. Like a
//! static `protocol`, the declaration is one comment-placement leaf.

use std::fmt::Write;

use crate::ast::{SessionProtocolDeclaration, SessionProtocolName, SessionProtocolNext};

pub(super) fn write_session_protocols(
    protocols: &[SessionProtocolDeclaration],
    placement: &super::comments::Placement,
    output: &mut impl Write,
) {
    for protocol in protocols {
        writeln!(output).unwrap();
        placement.leading(output, protocol.span.start, 0);
        if protocol.explicit_id {
            write!(output, "@id(\"").unwrap();
            super::write_escaped(output, &protocol.stable_id);
            writeln!(output, "\")").unwrap();
        }
        write!(output, "session protocol \"").unwrap();
        super::write_escaped(output, &protocol.name);
        writeln!(output, "\" {{").unwrap();
        write!(output, "    states ").unwrap();
        write_name_set(output, &protocol.states);
        writeln!(output).unwrap();
        writeln!(output, "    initial {};", protocol.initial.name).unwrap();
        for terminal in &protocol.terminals {
            write!(output, "    terminal {} cleanup ", terminal.state.name).unwrap();
            write_name_set(output, &terminal.cleanup);
            writeln!(output).unwrap();
        }
        for transition in &protocol.transitions {
            write!(
                output,
                "    on {} {}: {} {}",
                transition.from.name,
                transition.label.name,
                transition.kind.keyword(),
                transition.payload.name
            )
            .unwrap();
            if let Some(capability) = &transition.capability {
                write!(output, " requires capability {}", capability.name).unwrap();
            }
            if transition.consumes_resource {
                write!(output, " consumes resource").unwrap();
            }
            if let Some(via) = &transition.via {
                write!(output, " via \"").unwrap();
                super::write_escaped(output, &via.name);
                write!(output, "\"").unwrap();
            }
            match &transition.next {
                SessionProtocolNext::Then(state) => {
                    writeln!(output, " -> {};", state.name).unwrap();
                }
                SessionProtocolNext::Choice(branches) => {
                    write!(output, " -> choice {{ ").unwrap();
                    for (index, (label, state)) in branches.iter().enumerate() {
                        if index > 0 {
                            write!(output, ", ").unwrap();
                        }
                        write!(output, "{}: {}", label.name, state.name).unwrap();
                    }
                    writeln!(output, " }};").unwrap();
                }
            }
        }
        writeln!(output, "}}").unwrap();
        placement.trailing(output, protocol.span.start, 0);
    }
}

fn write_name_set(output: &mut impl Write, names: &[SessionProtocolName]) {
    if names.is_empty() {
        write!(output, "{{}}").unwrap();
        return;
    }
    write!(output, "{{ ").unwrap();
    for (index, name) in names.iter().enumerate() {
        if index > 0 {
            write!(output, ", ").unwrap();
        }
        write!(output, "{}", name.name).unwrap();
    }
    write!(output, " }}").unwrap();
}
