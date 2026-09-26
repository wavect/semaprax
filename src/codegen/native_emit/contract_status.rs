//! Contract-failure status emission for one native function.
use super::*;

pub(super) fn contract_label<'a>(
    expression: &'a ResolvedExpr,
    labels: &'a HashMap<ExpressionId, String>,
) -> &'a str {
    labels
        .get(&expression.id)
        .map_or_else(|| expression.id.as_str(), String::as_str)
}

pub(super) fn emit_contract_status<O: COutput>(
    emitter: &mut CEmitter<'_, O>,
    function: &ResolvedFunction,
    code: &str,
    phase: &str,
    expression: &str,
) {
    let mut format = String::new();
    let mut values = String::new();
    for (index, param) in function.params.iter().enumerate() {
        if index != 0 {
            format.push_str(", ");
        }
        format.push_str(&param.name);
        format.push_str(" = ");
        match &param.ty {
            ResolvedType::I64 => {
                format.push_str("%lld");
                values.push_str(&format!(", (long long)spx_param_{index}"));
            }
            ResolvedType::I32 => {
                format.push_str("%d");
                values.push_str(&format!(", (int)spx_param_{index}"));
            }
            ResolvedType::U8 | ResolvedType::Char => {
                format.push_str("%u");
                values.push_str(&format!(", (unsigned int)spx_param_{index}"));
            }
            ResolvedType::Usize => {
                format.push_str("%llu");
                values.push_str(&format!(", (unsigned long long)spx_param_{index}"));
            }
            ResolvedType::Bool => {
                format.push_str("%s");
                values.push_str(&format!(", spx_param_{index} ? \"true\" : \"false\""));
            }
            ResolvedType::F32 => {
                format.push_str("%.9g");
                values.push_str(&format!(", (double)spx_param_{index}"));
            }
            ResolvedType::F64 => {
                format.push_str("%.17g");
                values.push_str(&format!(", spx_param_{index}"));
            }
            _ => format.push_str("<data>"),
        }
    }
    if format.is_empty() {
        format.push_str("none");
    }
    emitter.line("char spx_contract_arguments[SPX_STATUS_ARGUMENTS_MAX_BYTES];");
    emitter.line(&format!(
        "int spx_contract_arguments_written = snprintf(spx_contract_arguments, sizeof spx_contract_arguments, \"{}\"{});",
        c_string(&format), values
    ));
    emitter.line("if (spx_contract_arguments_written < 0 || (size_t)spx_contract_arguments_written >= sizeof spx_contract_arguments) spx_runtime_invariant_failure(\"contract argument formatting overflow\");");
    emitter.line(&format!(
        "spx_status = spx_rt_contract_with_arguments(spx_ctx, {code}, \"{phase}\", \"{}\", \"{}\", spx_contract_arguments);",
        c_string(function.id.as_str()),
        c_string(expression)
    ));
}
