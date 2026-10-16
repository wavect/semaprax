use super::*;
#[cfg(test)]
pub(in crate::source_verify) fn check_generic_function_cycles<'p>(
    program: &'p dyn ProgramSource,
    graph: &HashMap<String, Vec<String>>,
    generics: &HashSet<&'p str>,
    out: &mut Vec<Diagnostic>,
) {
    let cells = crate::source_verify::capacity::source_capacity_functions(program);
    let globals = &cells[..program.functions().len()];
    let functions = globals.iter().map(|(_, f)| (f.name(), f)).collect();
    let types = TypeTable::new(program);
    check_generic_function_cycles_with_types(
        program, &cells, graph, generics, &types, &functions, out,
    );
}
