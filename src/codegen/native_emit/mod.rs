use super::*;

pub(super) fn emit_hir_c_with_labels(
    program: &ResolvedProgram,
    contract_labels: &HashMap<ExpressionId, String>,
    output_profile: NativeOutputProfile,
    selected_command: Option<&DeclarationId>,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    if program.types.iter().any(|declaration| {
        matches!(
            declaration.kind,
            ResolvedTypeDeclarationKind::Resource { .. }
        )
    }) {
        return Err(resource_lowering_gate());
    }
    let resource_abi = native_resource::build_resource_abi(program)?;
    let record_layouts = AggregateLayoutCache::build(program, AggregateTarget::Native64)?;
    let variant_layouts = VariantLayoutCache::build(program, VariantTarget::Native64)?;
    let functions = function_index(program)?;
    debug_assert!(resource_abi.resources.is_empty());
    let mut output = crate::bounded_output::CappedString::new();
    if output_profile == NativeOutputProfile::UsefulDataCommand {
        emit_native_prelude_without_public_failure(&mut output, &resource_abi, program);
    } else {
        emit_native_prelude(&mut output, &resource_abi, program);
    }
    if output_profile.supports_stdout_transcript() {
        native_host_output::emit_runtime(&mut output);
    }
    emit_fixed_byte_array_declarations(&mut output, program)?;
    emit_aggregate_declarations(
        &mut output,
        program,
        &resource_abi,
        &record_layouts,
        &variant_layouts,
    )?;
    emit_function_prototypes(&mut output, program, &functions, &resource_abi)?;

    let emission = NativeEmissionContext {
        program,
        resource_abi: &resource_abi,
        functions: &functions,
        contract_labels,
        record_layouts: &record_layouts,
        variant_layouts: &variant_layouts,
        output_profile,
    };
    for function in &program.functions {
        emit_function(
            &mut output,
            function,
            &FunctionExecutionId::Monomorphic(function.id.clone()),
            &emission,
        )?;
    }
    for instance in &program.function_instances {
        emit_function(
            &mut output,
            &instance.function,
            &FunctionExecutionId::Generic(instance.id.clone()),
            &emission,
        )?;
    }

    if output_profile == NativeOutputProfile::UsefulDataCommand {
        let command = selected_command
            .ok_or_else(|| backend_error("native command selection is unavailable"))?;
        let symbol = &functions
            .get(&FunctionExecutionId::Monomorphic(command.clone()))
            .ok_or_else(|| backend_error("selected native command is not indexed"))?
            .symbol;
        native_command::emit_runner(&mut output, symbol);
        native_command::emit_process_adapter(&mut output);
    } else {
        let main = program
            .functions
            .iter()
            .find(|function| function.id == program.entrypoint)
            .ok_or_else(|| backend_error("resolved native entry point is not indexed"))?;
        if !main.params.is_empty() || main.return_type != ResolvedType::I64 {
            return Err(backend_error(
                "resolved native entry point must have type `fn main() -> i64`",
            ));
        }
        let symbol = &functions
            .get(&FunctionExecutionId::Monomorphic(main.id.clone()))
            .ok_or_else(|| backend_error("native entry point is not indexed"))?
            .symbol;
        if output_profile == NativeOutputProfile::StdoutTranscript {
            native_host_output::emit_root_wrapper(&mut output, symbol);
            return Ok(output.into_string());
        }
        write!(
            output,
        "#ifndef SPX_NO_ENTRY_WRAPPER\n\
         int main(void) {{\n\
             struct spx_status_entry spx_status_entries[UINT32_C(1)];\n\
             struct spx_context spx_ctx = {{0}};\n\
             if (!spx_context_init(&spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(1), NULL, NULL, NULL)) {{\n\
                 fputs(\"SEMAPRAX native runtime invariant failure: context initialization\\n\", stderr);\n\
                 return 72;\n\
             }}\n\
             int64_t result;\n\
             spx_status_token status = {symbol}(&spx_ctx, &result);\n\
             if (status != SPX_STATUS_SUCCESS) return spx_public_failure(&spx_ctx, status);\n\
             printf(\"%lld\\n\", (long long)result);\n\
             return 0;\n\
         }}\n\
         #endif\n"
        )
        .expect("writing to a string cannot fail");
    }
    Ok(output.into_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeOutputProfile {
    Legacy,
    StdoutTranscript,
    UsefulDataCommand,
}

impl NativeOutputProfile {
    const fn supports_stdout_transcript(self) -> bool {
        matches!(self, Self::StdoutTranscript | Self::UsefulDataCommand)
    }
}

/// Emit one length-indexed, alignment-one C type for each reachable nonempty
/// fixed byte array. `[u8; 0]` is erased from physical C storage altogether;
/// its expressions use a compiler-only scalar sentinel that is never stored,
/// addressed, copied, or exposed through an ABI.
fn emit_fixed_byte_array_declarations(
    output: &mut impl COutput,
    program: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    let mut lengths = BTreeSet::new();
    let mut include_type = |ty: &ResolvedType| {
        if let ResolvedType::ArrayU8(length) = ty {
            lengths.insert(*length);
        }
    };
    for declaration in &program.types {
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Record { fields, .. }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                for field in fields {
                    include_type(&field.ty);
                }
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    include_type(&field.ty);
                }
            }
            ResolvedTypeDeclarationKind::Resource { .. } => {}
        }
    }
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        include_type(&function.return_type);
        for parameter in &function.params {
            include_type(&parameter.ty);
        }
        let mut pending = vec![&function.body];
        pending.extend(function.requires.iter().chain(&function.ensures));
        while let Some(expression) = pending.pop() {
            include_type(&expression.ty);
            pending.extend(resolved_expr_children(expression));
        }
    }
    for length in lengths {
        if u64::from(length) > crate::byte_data_capacity::MAX_ARRAY_BYTES {
            return Err(backend_error(format!(
                "fixed byte array length `{length}` exceeds the authenticated native bound"
            )));
        }
        if length == 0 {
            continue;
        }
        writeln!(
            output,
            "struct spx_array_u8_{length} {{ uint8_t spx_bytes[{length}]; }};"
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "_Static_assert(sizeof(struct spx_array_u8_{length}) == UINT32_C({length}), \"SEMAPRAX fixed byte array size\");"
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "_Static_assert(_Alignof(struct spx_array_u8_{length}) == UINT32_C(1), \"SEMAPRAX fixed byte array alignment\");"
        )
        .expect("writing to a string cannot fail");
    }
    output.push('\n');
    Ok(())
}

pub(super) fn emit_native_prelude(
    output: &mut impl COutput,
    resource_abi: &native_resource::NativeResourceAbi,
    program: &ResolvedProgram,
) {
    emit_native_prelude_inner(output, resource_abi, program, false);
}

fn emit_native_prelude_without_public_failure(
    output: &mut impl COutput,
    resource_abi: &native_resource::NativeResourceAbi,
    program: &ResolvedProgram,
) {
    emit_native_prelude_inner(output, resource_abi, program, true);
}

fn emit_native_prelude_inner(
    output: &mut impl COutput,
    resource_abi: &native_resource::NativeResourceAbi,
    program: &ResolvedProgram,
    omit_public_failure: bool,
) {
    if program_uses_borrowed_str(program) || program_uses_byte_data(program) {
        native_runtime::emit_status_runtime_with_borrowed_str(output);
    } else {
        native_runtime::emit_status_runtime(output);
    }
    output.push_str(&resource_abi.declarations);
    output.push_str("#include <stdio.h>\n\n");
    if omit_public_failure {
        let marker = "static __attribute__((unused)) int spx_public_failure(";
        let prefix = NATIVE_SCALAR_RUNTIME_C
            .split_once(marker)
            .map(|(prefix, _)| prefix)
            .expect("native public-failure marker must remain exact");
        output.push_str(prefix);
    } else {
        output.push_str(NATIVE_SCALAR_RUNTIME_C);
    }
    if program_uses_u8_arithmetic(program) {
        // Checked u8 helpers stay out of programs that cannot reach them, so
        // existing projections keep their exact committed bytes.
        output.push_str(NATIVE_U8_RUNTIME_C);
    }
    if program_uses_usize_arithmetic(program) {
        // Portable usize is semantic u64 on every target. Keep its helpers
        // reachability-gated so programs without usize preserve exact bytes.
        output.push_str(NATIVE_USIZE_RUNTIME_C);
    }
    if program_uses_strings(program) {
        output.push_str(NATIVE_STRING_RUNTIME_C);
    }
    if program_uses_string_ops(program) {
        // String operation helpers stay out of programs that cannot reach
        // them, so existing projections keep their exact committed bytes.
        output.push_str(NATIVE_STRING_OPS_RUNTIME_C);
    }
    if program_uses_string_ops_v2(program) {
        // Breadth-v2 string operation helpers gate as their own group so
        // first-wave programs keep their exact committed bytes.
        output.push_str(NATIVE_STRING_OPS_V2_RUNTIME_C);
    }
    if program_uses_borrowed_str(program) {
        // Borrowed text is a distinct length-aware carrier. Keep it behind a
        // reachability gate so every pre-text native projection is byte exact.
        output.push_str(NATIVE_BORROWED_STR_RUNTIME_C);
    }
    if program_uses_byte_data(program) {
        native_byte_data::emit_runtime(output);
    }
}

fn program_uses_byte_data(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        if matches!(
            function.return_type,
            ResolvedType::SliceU8 | ResolvedType::Bytes | ResolvedType::ArrayU8(_)
        ) || function.params.iter().any(|param| {
            matches!(
                param.ty,
                ResolvedType::SliceU8 | ResolvedType::Bytes | ResolvedType::ArrayU8(_)
            )
        }) {
            return true;
        }
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if matches!(
            expression.ty,
            ResolvedType::SliceU8 | ResolvedType::Bytes | ResolvedType::ArrayU8(_)
        ) {
            return true;
        }
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::byte_ops::by_id(callee.as_str()).is_some() {
                return true;
            }
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved signature, body, or contract admits an owned string
/// value that lowers through the string runtime helpers.
fn program_uses_strings(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        if matches!(function.return_type, ResolvedType::String)
            || function
                .params
                .iter()
                .any(|param| matches!(param.ty, ResolvedType::String))
        {
            return true;
        }
        pending.push(&function.body);
        for contract in function.requires.iter().chain(&function.ensures) {
            pending.push(contract);
        }
    }
    while let Some(expression) = pending.pop() {
        if matches!(expression.ty, ResolvedType::String)
            || matches!(expression.kind, ResolvedExprKind::String(_))
        {
            return true;
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved function body or contract calls a compiler-owned
/// string operation intrinsic.
fn program_uses_string_ops(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        for contract in function.requires.iter().chain(&function.ensures) {
            pending.push(contract);
        }
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str()).is_some() {
                return true;
            }
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved function body or contract calls a breadth-v2
/// compiler-owned string operation intrinsic.
fn program_uses_string_ops_v2(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        for contract in function.requires.iter().chain(&function.ensures) {
            pending.push(contract);
        }
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str())
                .is_some_and(crate::string_ops::StringOp::is_breadth_v2)
            {
                return true;
            }
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved function body or contract calls a compiler-owned
/// borrowed-text operation intrinsic.
fn program_uses_borrowed_str(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        if matches!(function.return_type, ResolvedType::Str)
            || function
                .params
                .iter()
                .any(|param| matches!(param.ty, ResolvedType::Str))
        {
            return true;
        }
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if matches!(expression.ty, ResolvedType::Str) {
            return true;
        }
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::str_ops::by_id(callee.as_str()).is_some() {
                return true;
            }
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved function body or contract contains checked u8
/// arithmetic that lowers through the u8 runtime helpers.
fn program_uses_u8_arithmetic(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        for contract in function.requires.iter().chain(&function.ensures) {
            pending.push(contract);
        }
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Binary { op, left, right } = &expression.kind {
            if matches!(left.ty, ResolvedType::U8)
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                )
            {
                return true;
            }
            pending.push(left);
            pending.push(right);
            continue;
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

fn program_uses_usize_arithmetic(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        pending.extend(function.requires.iter());
        pending.extend(function.ensures.iter());
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Binary { op, left, right } = &expression.kind {
            if matches!(left.ty, ResolvedType::Usize)
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                )
            {
                return true;
            }
            pending.push(left);
            pending.push(right);
            continue;
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Every direct resolved child of an expression.
fn resolved_expr_children<'a>(
    expression: &'a ResolvedExpr,
) -> Box<dyn Iterator<Item = &'a ResolvedExpr> + 'a> {
    match &expression.kind {
        ResolvedExprKind::Binary { left, right, .. } => {
            Box::new([left.as_ref(), right.as_ref()].into_iter())
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => Box::new(std::iter::once(value.as_ref())),
        ResolvedExprKind::Block { statements, tail } => Box::new(
            statements
                .iter()
                .flat_map(|statement| {
                    (0..statement.child_count()).filter_map(move |index| statement.child(index))
                })
                .chain(std::iter::once(tail.as_ref())),
        ),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => Box::new(
            [
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ]
            .into_iter(),
        ),
        ResolvedExprKind::Call { args, .. } => Box::new(args.iter()),
        ResolvedExprKind::NativeRustImportCall(call) => Box::new(call.args.iter()),
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            Box::new(fields.iter().map(|field| &field.value))
        }
        ResolvedExprKind::Match { scrutinee, arms } => Box::new(
            std::iter::once(scrutinee.as_ref()).chain(
                arms.iter()
                    .filter_map(|arm| arm.guard.as_deref())
                    .chain(arms.iter().map(|arm| &arm.value)),
            ),
        ),
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            Box::new(std::iter::once(base.as_ref()).chain(fields.iter().map(|field| &field.value)))
        }
        ResolvedExprKind::BorrowPlace { .. } => Box::new(std::iter::empty()),
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_) => Box::new(std::iter::empty()),
    }
}

fn emit_aggregate_declarations(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    record_layouts: &AggregateLayoutCache,
    variant_layouts: &VariantLayoutCache,
) -> Result<(), Diagnostic> {
    let records = record_layouts.layouts().collect::<Vec<_>>();
    let variants = variant_layouts.layouts().collect::<Vec<_>>();
    if records.is_empty() && variants.is_empty() {
        return Ok(());
    }

    output.push_str("#include <stddef.h>\n\n");
    for record in &records {
        writeln!(output, "struct {};", c_record_symbol(&record.instance))
            .expect("writing to a string cannot fail");
    }
    for variant in &variants {
        writeln!(output, "struct {};", c_variant_symbol(&variant.instance))
            .expect("writing to a string cannot fail");
    }
    output.push('\n');

    let mut visiting = BTreeSet::new();
    let mut emitted = BTreeSet::new();
    for record in &records {
        emit_record_declaration(
            output,
            program,
            resource_abi,
            record_layouts,
            &record.instance,
            &mut visiting,
            &mut emitted,
        )?;
    }
    for variant in &variants {
        emit_variant_declaration(output, program, resource_abi, variant)?;
    }
    Ok(())
}

fn emit_record_declaration(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    layouts: &AggregateLayoutCache,
    instance: &ResolvedType,
    visiting: &mut BTreeSet<ResolvedType>,
    emitted: &mut BTreeSet<ResolvedType>,
) -> Result<(), Diagnostic> {
    if emitted.contains(instance) {
        return Ok(());
    }
    if !visiting.insert(instance.clone()) {
        return Err(backend_error(format!(
            "native aggregate instance `{}` is recursively embedded",
            instance.identity_key()
        )));
    }
    let layout = layouts.layout(instance)?.clone();
    layout.validate(program)?;
    for field in &layout.fields {
        if record_declaration_id(program, &field.ty)?.is_some() {
            emit_record_declaration(
                output,
                program,
                resource_abi,
                layouts,
                &field.ty,
                visiting,
                emitted,
            )?;
        }
    }

    let symbol = c_record_symbol(instance);
    writeln!(output, "struct {symbol} {{").expect("writing to a string cannot fail");
    if layout.size == 0 {
        // ISO C11 has no empty objects or empty structs. This byte is an ABI
        // carrier only: the authenticated SEMAPRAX layout remains size zero,
        // and every zero-sized semantic field stays erased below.
        output.push_str("    uint8_t spx_zero_sized_record_carrier;\n");
    } else {
        for field in layout.fields.iter().filter(|field| field.size != 0) {
            writeln!(
                output,
                "    {} {};",
                c_value_type(program, resource_abi, &field.ty)?,
                c_field_symbol(&field.field)
            )
            .expect("writing to a string cannot fail");
        }
    }
    output.push_str("};\n");
    if layout.size == 0 {
        writeln!(
            output,
            "_Static_assert(sizeof(struct {symbol}) == UINT32_C(1), \"SEMAPRAX zero-sized native aggregate carrier size\");"
        )
        .expect("writing to a string cannot fail");
    } else {
        writeln!(
            output,
            "_Static_assert(sizeof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native aggregate size\");",
            layout.size
        )
        .expect("writing to a string cannot fail");
    }
    writeln!(
        output,
        "_Static_assert(_Alignof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native aggregate alignment\");",
        layout.align
    )
    .expect("writing to a string cannot fail");
    for field in layout.fields.iter().filter(|field| field.size != 0) {
        writeln!(
            output,
            "_Static_assert(offsetof(struct {symbol}, {}) == UINT32_C({}), \"SEMAPRAX native aggregate field offset\");",
            c_field_symbol(&field.field),
            field.offset
        )
        .expect("writing to a string cannot fail");
    }
    output.push('\n');
    visiting.remove(instance);
    emitted.insert(instance.clone());
    Ok(())
}

fn emit_variant_declaration(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    layout: &VariantLayout,
) -> Result<(), Diagnostic> {
    layout.validate(program)?;
    let symbol = c_variant_symbol(&layout.instance);
    writeln!(output, "struct {symbol} {{").expect("writing to a string cannot fail");
    output.push_str("    uint32_t spx_tag;\n    union {\n");
    for case in &layout.cases {
        output.push_str("        struct {\n");
        if case.fields.is_empty() {
            output.push_str("            uint8_t spx_empty_variant_payload;\n");
        } else {
            for field in case.fields.iter().filter(|field| field.size != 0) {
                writeln!(
                    output,
                    "            {} {};",
                    c_value_type(program, resource_abi, &field.ty)?,
                    c_field_symbol(&field.field)
                )
                .expect("writing to a string cannot fail");
            }
        }
        writeln!(output, "        }} {};", c_case_symbol(&case.case))
            .expect("writing to a string cannot fail");
    }
    output.push_str("    } spx_payload;\n};\n");
    writeln!(
        output,
        "_Static_assert(sizeof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native variant size\");",
        layout.size
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "_Static_assert(_Alignof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native variant alignment\");",
        layout.align
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "_Static_assert(offsetof(struct {symbol}, spx_tag) == UINT32_C(0), \"SEMAPRAX native variant tag offset\");"
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "_Static_assert(offsetof(struct {symbol}, spx_payload) == UINT32_C({}), \"SEMAPRAX native variant payload offset\");",
        layout.payload_offset
    )
    .expect("writing to a string cannot fail");
    for case in &layout.cases {
        let case_symbol = c_case_symbol(&case.case);
        for field in case.fields.iter().filter(|field| field.size != 0) {
            let absolute_offset = layout
                .payload_offset
                .checked_add(field.offset)
                .ok_or_else(|| backend_error("native variant field offset overflows u32"))?;
            writeln!(
                output,
                "_Static_assert(offsetof(struct {symbol}, spx_payload.{case_symbol}.{}) == UINT32_C({}), \"SEMAPRAX native variant field offset\");",
                c_field_symbol(&field.field),
                absolute_offset
            )
            .expect("writing to a string cannot fail");
        }
    }
    output.push('\n');
    Ok(())
}

fn c_value_type(
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    ty: &ResolvedType,
) -> Result<String, Diagnostic> {
    if matches!(ty, ResolvedType::ArrayU8(0)) {
        // ISO C11 has no zero-sized value type. Ordinary internal calls use
        // one byte as a non-semantic ABI carrier while all actual array
        // storage and element access remain erased.
        Ok("uint8_t".to_owned())
    } else if let ResolvedType::ArrayU8(length) = ty {
        Ok(format!("struct spx_array_u8_{length}"))
    } else if record_declaration_id(program, ty)?.is_some() {
        Ok(format!("struct {}", c_record_symbol(ty)))
    } else if variant_declaration_id(program, ty)?.is_some() {
        Ok(format!("struct {}", c_variant_symbol(ty)))
    } else {
        resource_abi.c_type(program, ty).map(str::to_owned)
    }
}

fn is_aggregate_type(program: &ResolvedProgram, ty: &ResolvedType) -> Result<bool, Diagnostic> {
    Ok(matches!(ty, ResolvedType::ArrayU8(length) if *length != 0)
        || record_declaration_id(program, ty)?.is_some()
        || variant_declaration_id(program, ty)?.is_some())
}

fn record_declaration_id<'a>(
    program: &ResolvedProgram,
    ty: &'a ResolvedType,
) -> Result<Option<&'a DeclarationId>, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| backend_error(format!("unknown native type `{declaration}`")))?;
    if !matches!(
        item.kind,
        ResolvedTypeDeclarationKind::Record { .. } | ResolvedTypeDeclarationKind::Class { .. }
    ) {
        return Ok(None);
    }
    if arguments.len() != item.type_parameters.len()
        || arguments
            .iter()
            .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
    {
        return Err(backend_error(format!(
            "native record representation requires exact concrete i64/bool arguments for `{}`",
            ty.identity_key()
        )));
    }
    Ok(Some(declaration))
}

fn variant_declaration_id<'a>(
    program: &ResolvedProgram,
    ty: &'a ResolvedType,
) -> Result<Option<&'a DeclarationId>, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| backend_error(format!("unknown native type `{declaration}`")))?;
    if !matches!(item.kind, ResolvedTypeDeclarationKind::Variant { .. }) {
        return Ok(None);
    }
    if arguments.len() != item.type_parameters.len()
        || arguments.iter().any(|argument| {
            !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                && !(declaration.as_str() == crate::prelude::OPTION_ID
                    && *argument == ResolvedType::U8)
        })
    {
        return Err(backend_error(format!(
            "native variant representation requires admitted exact concrete arguments for `{}`",
            ty.identity_key()
        )));
    }
    Ok(Some(declaration))
}

pub(super) fn c_record_symbol(ty: &ResolvedType) -> String {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        unreachable!("record C symbols require nominal types");
    };
    let mut symbol = stable_c_symbol("spx_record_", declaration);
    if !arguments.is_empty() {
        let mut digest = Sha256::new();
        digest.update(b"semaprax.native-record-instance.v1\0");
        digest.update(ty.identity_key().as_bytes());
        symbol.push_str("_inst_");
        for byte in digest.finalize() {
            write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
        }
    }
    symbol
}

fn c_variant_symbol(ty: &ResolvedType) -> String {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        unreachable!("variant C symbols require nominal types");
    };
    let mut symbol = stable_c_symbol("spx_variant_", declaration);
    if !arguments.is_empty() {
        let mut digest = Sha256::new();
        digest.update(b"semaprax.native-variant-instance.v1\0");
        digest.update(ty.identity_key().as_bytes());
        symbol.push_str("_inst_");
        for byte in digest.finalize() {
            write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
        }
    }
    symbol
}

fn c_case_symbol(id: &DeclarationId) -> String {
    stable_c_symbol("spx_case_", id)
}

fn c_field_symbol(id: &DeclarationId) -> String {
    stable_c_symbol("spx_field_", id)
}

fn stable_c_symbol(prefix: &str, id: &DeclarationId) -> String {
    let mut symbol = crate::bounded_output::CappedString::new();
    symbol.push_str(prefix);
    for byte in id.as_str().bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.into_string()
}

pub(super) fn emit_function_prototypes(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    functions: &HashMap<FunctionExecutionId, CFunction>,
    resource_abi: &native_resource::NativeResourceAbi,
) -> Result<(), Diagnostic> {
    for (function, execution) in program
        .functions
        .iter()
        .map(|function| {
            (
                function,
                FunctionExecutionId::Monomorphic(function.id.clone()),
            )
        })
        .chain(program.function_instances.iter().map(|instance| {
            (
                &instance.function,
                FunctionExecutionId::Generic(instance.id.clone()),
            )
        }))
    {
        let metadata = functions
            .get(&execution)
            .ok_or_else(|| backend_error(format!("function `{}` is not indexed", function.id)))?;
        write!(
            output,
            "static __attribute__((unused)) spx_status_token {}(struct spx_context *spx_ctx",
            metadata.symbol,
        )
        .expect("writing to a string cannot fail");
        for param in &function.params {
            let ty = c_value_type(program, resource_abi, &param.ty)?;
            if is_aggregate_type(program, &param.ty)? {
                write!(output, ", const {ty} *").expect("writing to a string cannot fail");
            } else {
                write!(output, ", {ty}").expect("writing to a string cannot fail");
            }
        }
        writeln!(
            output,
            ", {} *spx_result_out);",
            c_value_type(program, resource_abi, &function.return_type)?
        )
        .expect("writing to a string cannot fail");
    }
    output.push('\n');
    Ok(())
}

#[cfg(test)]
pub(super) fn preflight_resource_lowering(
    program: &ResolvedProgram,
    functions: &HashMap<FunctionExecutionId, CFunction>,
    resource_abi: &native_resource::NativeResourceAbi,
    contract_labels: &HashMap<ExpressionId, String>,
) -> Result<(), Diagnostic> {
    let mut first_failure = None;
    for function in &program.functions {
        match native_cleanup::classify(program, function) {
            Ok(cleanup) => {
                match native_value::plan(program, function, &cleanup, resource_abi, contract_labels)
                {
                    Ok(mut values) => {
                        match crate::semantic_trace::build_semantic_event_dictionary(
                            program,
                            &function.id,
                        ) {
                            Ok(dictionary) => {
                                values.cleanup_bindings.semantic_events = Some(dictionary);
                            }
                            Err(diagnostic) => {
                                first_failure.get_or_insert(diagnostic);
                                continue;
                            }
                        }
                        match native_host_contract::derive_from_admitted(
                            program,
                            &function.id,
                            resource_abi,
                            &cleanup,
                            &values,
                        ) {
                            Ok(host_template) => {
                                match native_adapter_abi::derive(&host_template) {
                                    Ok(descriptor) => {
                                        let _descriptor_header =
                                            native_adapter_abi::emit_header(&descriptor);
                                        if let Err(diagnostic) = native_adapter_abi::emit_source(
                                            &descriptor,
                                            "semaprax_adapter_descriptor.h",
                                        ) {
                                            first_failure.get_or_insert(diagnostic);
                                        }
                                    }
                                    Err(diagnostic) => {
                                        first_failure.get_or_insert(diagnostic);
                                    }
                                }
                                let _declarations = native_value::emit_declarations(&values);
                                match native_cleanup_emit::emit_with_block_prologues(
                                    &cleanup,
                                    &values.cleanup_bindings,
                                    |block, output| {
                                        output.push_str(&native_value::emit_block_prologue(
                                            &values, block,
                                        ));
                                        Ok(())
                                    },
                                ) {
                                    Ok(_cleanup_body) => {}
                                    Err(diagnostic) => {
                                        first_failure.get_or_insert(diagnostic);
                                    }
                                }
                            }
                            Err(diagnostic) => {
                                first_failure.get_or_insert(diagnostic);
                            }
                        }
                    }
                    Err(diagnostic) => {
                        first_failure.get_or_insert(diagnostic);
                    }
                }
            }
            Err(diagnostic) => {
                first_failure.get_or_insert(diagnostic);
            }
        }
        if let Err(diagnostic) = native_trace::required_event_capacity(program, function) {
            first_failure.get_or_insert(diagnostic);
        }
    }

    // Exercise the same ABI declaration/type/prototype order that the eventual
    // resource emitter will use, then discard it. The loop above separately
    // constructs the gated value/cleanup bodies; the exact conformance harness
    // composes those inside strict C functions. No resource artifact may escape
    // until a public host ownership boundary is defined and proven.
    let mut staged_output = crate::bounded_output::CappedString::new();
    emit_native_prelude(&mut staged_output, resource_abi, program);
    if let Err(diagnostic) =
        emit_function_prototypes(&mut staged_output, program, functions, resource_abi)
    {
        first_failure.get_or_insert(diagnostic);
    }
    first_failure.map_or(Ok(()), Err)
}

const NATIVE_STRING_RUNTIME_C: &str = r#"#include <stdlib.h>
#include <string.h>

static __attribute__((unused)) char *spx_string_from_literal(
    const char *spx_data, uint64_t spx_len
) {
    char *spx_copy = (char *)malloc((size_t)spx_len + 1u);
    if (spx_copy == NULL) spx_runtime_invariant_failure("string allocation failed");
    memcpy(spx_copy, spx_data, (size_t)spx_len);
    spx_copy[spx_len] = '\0';
    return spx_copy;
}

static __attribute__((unused)) char *spx_string_clone(const char *spx_source) {
    return spx_string_from_literal(spx_source, (uint64_t)strlen(spx_source));
}

static __attribute__((unused)) bool spx_string_eq(const char *a, const char *b) {
    return strcmp(a, b) == 0;
}

static __attribute__((unused)) void spx_string_drop(char *spx_value) {
    free(spx_value);
}
"#;

const NATIVE_STRING_OPS_RUNTIME_C: &str = r#"static __attribute__((unused)) int64_t spx_string_len(const char *spx_value) {
    return (int64_t)strlen(spx_value);
}

static __attribute__((unused)) bool spx_string_is_empty(const char *spx_value) {
    return spx_value[0] == '\0';
}

static __attribute__((unused)) char *spx_string_concat(
    char *spx_left, char *spx_right
) {
    uint64_t spx_left_len = (uint64_t)strlen(spx_left);
    uint64_t spx_right_len = (uint64_t)strlen(spx_right);
    char *spx_joined = (char *)malloc((size_t)(spx_left_len + spx_right_len) + 1u);
    if (spx_joined == NULL) spx_runtime_invariant_failure("string allocation failed");
    memcpy(spx_joined, spx_left, (size_t)spx_left_len);
    memcpy(spx_joined + spx_left_len, spx_right, (size_t)spx_right_len);
    spx_joined[spx_left_len + spx_right_len] = '\0';
    return spx_joined;
}
"#;

const NATIVE_STRING_OPS_V2_RUNTIME_C: &str = r#"static __attribute__((unused)) bool spx_string_starts_with(
    const char *spx_value, const char *spx_prefix
) {
    return strncmp(spx_value, spx_prefix, strlen(spx_prefix)) == 0;
}

static __attribute__((unused)) bool spx_string_contains(
    const char *spx_value, const char *spx_needle
) {
    return strstr(spx_value, spx_needle) != NULL;
}

static __attribute__((unused)) int64_t spx_string_len_chars(const char *spx_value) {
    int64_t spx_count = 0;
    for (const unsigned char *spx_cursor = (const unsigned char *)spx_value;
         *spx_cursor != UINT8_C(0);
         ++spx_cursor) {
        if ((*spx_cursor & UINT8_C(0xC0)) != UINT8_C(0x80)) ++spx_count;
    }
    return spx_count;
}

static __attribute__((unused)) char *spx_string_from_char(uint32_t spx_scalar) {
    char spx_encoded[4];
    uint64_t spx_length;
    if (spx_scalar < UINT32_C(0x80)) {
        spx_encoded[0] = (char)(uint8_t)spx_scalar;
        spx_length = UINT64_C(1);
    } else if (spx_scalar < UINT32_C(0x800)) {
        spx_encoded[0] = (char)(uint8_t)(UINT8_C(0xC0) | (uint8_t)(spx_scalar >> 6));
        spx_encoded[1] = (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)(spx_scalar & UINT32_C(0x3F)));
        spx_length = UINT64_C(2);
    } else if (spx_scalar < UINT32_C(0x10000)) {
        spx_encoded[0] = (char)(uint8_t)(UINT8_C(0xE0) | (uint8_t)(spx_scalar >> 12));
        spx_encoded[1] =
            (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)((spx_scalar >> 6) & UINT32_C(0x3F)));
        spx_encoded[2] = (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)(spx_scalar & UINT32_C(0x3F)));
        spx_length = UINT64_C(3);
    } else {
        spx_encoded[0] = (char)(uint8_t)(UINT8_C(0xF0) | (uint8_t)(spx_scalar >> 18));
        spx_encoded[1] =
            (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)((spx_scalar >> 12) & UINT32_C(0x3F)));
        spx_encoded[2] =
            (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)((spx_scalar >> 6) & UINT32_C(0x3F)));
        spx_encoded[3] = (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)(spx_scalar & UINT32_C(0x3F)));
        spx_length = UINT64_C(4);
    }
    return spx_string_from_literal(spx_encoded, spx_length);
}
"#;

const NATIVE_BORROWED_STR_RUNTIME_C: &str = r#"#include <limits.h>
#include <stddef.h>
#include <string.h>

typedef struct {
    const uint8_t *data;
    uint64_t len;
} spx_str_v1;

#define SPX_BORROWED_STR_MAX_BYTES UINT64_C(65536)

static __attribute__((unused)) void spx_str_require_valid(spx_str_v1 value) {
    if (value.len != UINT64_C(0) && value.data == NULL) {
        spx_runtime_invariant_failure("borrowed str has null data with nonzero length");
    }
    if (value.len > SPX_BORROWED_STR_MAX_BYTES
        || value.len > (uint64_t)SIZE_MAX
        || value.len > (uint64_t)INT64_MAX) {
        spx_runtime_invariant_failure("borrowed str length exceeds native profile");
    }
    uint64_t offset = UINT64_C(0);
    while (offset < value.len) {
        const uint8_t first = value.data[offset];
        uint64_t width = UINT64_C(0);
        if (first <= UINT8_C(0x7f)) {
            width = UINT64_C(1);
        } else if (first >= UINT8_C(0xc2) && first <= UINT8_C(0xdf)) {
            width = UINT64_C(2);
        } else if (first >= UINT8_C(0xe0) && first <= UINT8_C(0xef)) {
            width = UINT64_C(3);
        } else if (first >= UINT8_C(0xf0) && first <= UINT8_C(0xf4)) {
            width = UINT64_C(4);
        } else {
            spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
        }
        if (width > value.len - offset) {
            spx_runtime_invariant_failure("borrowed str has truncated UTF-8");
        }
        if (width >= UINT64_C(2)) {
            const uint8_t second = value.data[offset + UINT64_C(1)];
            if ((second & UINT8_C(0xc0)) != UINT8_C(0x80)
                || (first == UINT8_C(0xe0) && second < UINT8_C(0xa0))
                || (first == UINT8_C(0xed) && second > UINT8_C(0x9f))
                || (first == UINT8_C(0xf0) && second < UINT8_C(0x90))
                || (first == UINT8_C(0xf4) && second > UINT8_C(0x8f))) {
                spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
            }
        }
        for (uint64_t tail = UINT64_C(2); tail < width; ++tail) {
            if ((value.data[offset + tail] & UINT8_C(0xc0)) != UINT8_C(0x80)) {
                spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
            }
        }
        offset += width;
    }
}

static __attribute__((unused)) int64_t spx_str_len_bytes(spx_str_v1 value) {
    spx_str_require_valid(value);
    return (int64_t)value.len;
}

static __attribute__((unused)) bool spx_str_is_empty(spx_str_v1 value) {
    spx_str_require_valid(value);
    return value.len == UINT64_C(0);
}

static __attribute__((unused)) bool spx_str_starts_with(
    spx_str_v1 value, spx_str_v1 prefix
) {
    spx_str_require_valid(value);
    spx_str_require_valid(prefix);
    if (prefix.len == UINT64_C(0)) return true;
    if (prefix.len > value.len) return false;
    return memcmp(value.data, prefix.data, (size_t)prefix.len) == 0;
}

static __attribute__((unused)) bool spx_str_contains(
    spx_str_v1 value, spx_str_v1 needle
) {
    spx_str_require_valid(value);
    spx_str_require_valid(needle);
    if (needle.len == UINT64_C(0)) return true;
    if (needle.len > value.len) return false;

    /* Fixed-capacity KMP: every byte advances or shortens the matched prefix. */
    uint16_t prefix[SPX_BORROWED_STR_MAX_BYTES];
    prefix[0] = UINT16_C(0);
    uint64_t matched = UINT64_C(0);
    for (uint64_t index = UINT64_C(1); index < needle.len; ++index) {
        while (matched != UINT64_C(0) && needle.data[matched] != needle.data[index]) {
            matched = (uint64_t)prefix[matched - UINT64_C(1)];
        }
        if (needle.data[matched] == needle.data[index]) ++matched;
        prefix[index] = (uint16_t)matched;
    }

    matched = UINT64_C(0);
    for (uint64_t index = UINT64_C(0); index < value.len; ++index) {
        while (matched != UINT64_C(0) && needle.data[matched] != value.data[index]) {
            matched = (uint64_t)prefix[matched - UINT64_C(1)];
        }
        if (needle.data[matched] == value.data[index]) {
            ++matched;
            if (matched == needle.len) return true;
        }
    }
    return false;
}
"#;

const NATIVE_U8_RUNTIME_C: &str = r#"static __attribute__((unused)) spx_status_token spx_rt_u8_add(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    int64_t result = (int64_t)a + (int64_t)b;
    if (result < 0 || result > UINT8_MAX) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = (uint8_t)result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_u8_sub(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    int64_t result = (int64_t)a - (int64_t)b;
    if (result < 0 || result > UINT8_MAX) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = (uint8_t)result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_u8_mul(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    int64_t result = (int64_t)a * (int64_t)b;
    if (result < 0 || result > UINT8_MAX) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = (uint8_t)result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_u8_div(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    *result_out = (uint8_t)((int64_t)a / (int64_t)b);
    return SPX_STATUS_SUCCESS;
}
"#;

const NATIVE_USIZE_RUNTIME_C: &str = r#"static __attribute__((unused)) spx_status_token spx_rt_usize_add(
    struct spx_context *spx_ctx, uint64_t a, uint64_t b, uint64_t *result_out
) {
    if (a > UINT64_MAX - b) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = a + b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_usize_sub(
    struct spx_context *spx_ctx, uint64_t a, uint64_t b, uint64_t *result_out
) {
    if (a < b) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = a - b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_usize_mul(
    struct spx_context *spx_ctx, uint64_t a, uint64_t b, uint64_t *result_out
) {
    if (b != UINT64_C(0) && a > UINT64_MAX / b) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = a * b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_usize_div(
    struct spx_context *spx_ctx, uint64_t a, uint64_t b, uint64_t *result_out
) {
    if (b == UINT64_C(0)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    *result_out = a / b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_usize_rem(
    struct spx_context *spx_ctx, uint64_t a, uint64_t b, uint64_t *result_out
) {
    if (b == UINT64_C(0)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO, "invalid remainder"
        );
    }
    *result_out = a % b;
    return SPX_STATUS_SUCCESS;
}
"#;

struct NativeEmissionContext<'a> {
    program: &'a ResolvedProgram,
    resource_abi: &'a native_resource::NativeResourceAbi,
    functions: &'a HashMap<FunctionExecutionId, CFunction>,
    contract_labels: &'a HashMap<ExpressionId, String>,
    record_layouts: &'a AggregateLayoutCache,
    variant_layouts: &'a VariantLayoutCache,
    output_profile: NativeOutputProfile,
}

fn emit_function(
    output: &mut impl COutput,
    function: &ResolvedFunction,
    execution: &FunctionExecutionId,
    emission: &NativeEmissionContext<'_>,
) -> Result<(), Diagnostic> {
    let program = emission.program;
    let resource_abi = emission.resource_abi;
    let functions = emission.functions;
    let contract_labels = emission.contract_labels;
    let has_try = expression_has_try(&function.body);
    let bytes_plan = native_bytes::NativeBytesPlan::build(function)?;
    let metadata = functions
        .get(execution)
        .ok_or_else(|| backend_error(format!("function `{}` is not indexed", function.id)))?;
    write!(
        output,
        "static __attribute__((unused)) spx_status_token {}(struct spx_context *spx_ctx",
        metadata.symbol
    )
    .expect("writing to a string cannot fail");
    for (index, param) in function.params.iter().enumerate() {
        let ty = c_value_type(program, resource_abi, &param.ty)?;
        if is_aggregate_type(program, &param.ty)? {
            write!(output, ", const {ty} *spx_param_{index}")
                .expect("writing to a string cannot fail");
        } else {
            write!(output, ", {ty} spx_param_{index}").expect("writing to a string cannot fail");
        }
    }
    writeln!(
        output,
        ", {} *spx_result_out) {{",
        c_value_type(program, resource_abi, &function.return_type)?
    )
    .expect("writing to a string cannot fail");

    if let Some(plan) = &bytes_plan {
        output.push_str(&plan.declarations(function));
        for (index, parameter) in function.params.iter().enumerate() {
            if matches!(parameter.ty, ResolvedType::Bytes) {
                output.push_str(&plan.initialize_parameter(
                    &crate::cleanup_plan::StorageId::Value(parameter.id.clone()),
                    &format!("spx_param_{index}"),
                )?);
            }
        }
    }
    let mut variables = HashMap::new();
    for (index, param) in function.params.iter().enumerate() {
        let name = if matches!(param.ty, ResolvedType::Bytes) {
            bytes_plan
                .as_ref()
                .ok_or_else(|| backend_error("owned Bytes parameter has no cleanup plan"))?
                .value(&crate::cleanup_plan::StorageId::Value(param.id.clone()))?
                .to_owned()
        } else if is_aggregate_type(program, &param.ty)? {
            format!("(*spx_param_{index})")
        } else {
            format!("spx_param_{index}")
        };
        variables.insert(
            param.id.clone(),
            CBinding {
                name,
                ty: param.ty.clone(),
            },
        );
    }
    let mut emitter = CEmitter::new(
        output,
        variables,
        &function.return_type,
        emission,
        bytes_plan.as_ref(),
    );
    emitter.line("spx_status_token spx_status = SPX_STATUS_SUCCESS;");
    if has_try {
        emitter.line("bool spx_result_staged = false;");
    }
    emitter.line("(void)spx_ctx;");
    let borrowed_params = function
        .params
        .iter()
        .enumerate()
        .filter(|(_, param)| matches!(param.ty, ResolvedType::Str))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let borrowed_byte_params = function
        .params
        .iter()
        .enumerate()
        .filter(|(_, param)| matches!(param.ty, ResolvedType::SliceU8))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if !borrowed_params.is_empty() || !borrowed_byte_params.is_empty() {
        emitter
            .line("const bool spx_borrowed_str_root = spx_ctx->borrowed_str_depth == UINT32_C(0);");
        emitter.line("if (spx_ctx->borrowed_str_depth == UINT32_MAX) spx_runtime_invariant_failure(\"borrowed str call depth exhausted\");");
        emitter.line("if (spx_borrowed_str_root) {");
        emitter.indent += 1;
        emitter.line("uint64_t spx_borrowed_root_bytes = UINT64_C(0);");
        if !borrowed_params.is_empty() {
            for index in &borrowed_params {
                emitter.line(&format!("spx_str_require_valid(spx_param_{index});"));
                emitter.line(&format!(
                    "if (spx_param_{index}.len > SPX_BORROWED_STR_MAX_BYTES - spx_borrowed_root_bytes) spx_runtime_invariant_failure(\"borrowed invocation exceeds cumulative root byte budget\");"
                ));
                emitter.line(&format!(
                    "spx_borrowed_root_bytes += spx_param_{index}.len;"
                ));
            }
        }
        if !borrowed_byte_params.is_empty() {
            for index in &borrowed_byte_params {
                emitter.line(&format!("spx_borrowed_root_bytes = spx_slice_u8_charge_root(spx_borrowed_root_bytes, spx_param_{index});"));
            }
        }
        emitter.indent -= 1;
        emitter.line("}");
        emitter.line("++spx_ctx->borrowed_str_depth;");
    }
    emitter.line(&format!(
        "{} spx_result = {{0}};",
        c_value_type(program, resource_abi, &function.return_type)?
    ));
    if matches!(function.return_type, ResolvedType::Bytes) {
        emitter.line("(void)spx_result;");
    }
    for index in 0..function.params.len() {
        emitter.line(&format!("(void)spx_param_{index};"));
    }
    for contract in &function.requires {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_type(&condition.ty, &ResolvedType::Bool, "precondition")?;
        emitter.line(&format!("if (!({})) {{", condition.code));
        emitter.indent += 1;
        emitter.line(&format!(
            "spx_status = spx_rt_contract(spx_ctx, SPX_STATUS_CONTRACT_REQUIRES_FALSE, \"requires\", \"{}\", \"{}\");",
            c_string(&function.name),
            c_string(contract_label(contract, contract_labels))
        ));
        emitter.line("goto spx_epilogue;");
        emitter.indent -= 1;
        emitter.line("}");
    }
    emitter.try_target_enabled = true;
    let body = emitter.emit_expr(&function.body)?;
    emitter.try_target_enabled = false;
    emitter.require_type(&body.ty, &function.return_type, "function body")?;
    if !matches!(body.ty, ResolvedType::Bytes) {
        emitter.line(&format!("spx_result = {};", body.code));
    }
    if has_try {
        emitter.line("spx_result_staged = true;");
        emitter.label("spx_postconditions");
    }

    emitter.variables.insert(
        function.result_id.clone(),
        CBinding {
            name: if matches!(function.return_type, ResolvedType::Bytes) {
                bytes_plan
                    .as_ref()
                    .ok_or_else(|| backend_error("owned Bytes result has no cleanup plan"))?
                    .provisional()?
                    .0
                    .to_owned()
            } else {
                "spx_result".to_owned()
            },
            ty: function.return_type.clone(),
        },
    );
    for contract in &function.ensures {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_type(&condition.ty, &ResolvedType::Bool, "postcondition")?;
        emitter.line(&format!("if (!({})) {{", condition.code));
        emitter.indent += 1;
        emitter.line(&format!(
            "spx_status = spx_rt_contract(spx_ctx, SPX_STATUS_CONTRACT_ENSURES_FALSE, \"ensures\", \"{}\", \"{}\");",
            c_string(&function.name),
            c_string(contract_label(contract, contract_labels))
        ));
        emitter.line("goto spx_epilogue;");
        emitter.indent -= 1;
        emitter.line("}");
    }
    emitter.line("goto spx_epilogue;");
    drop(emitter);
    output.push_str("spx_epilogue:\n");
    if !borrowed_params.is_empty() || !borrowed_byte_params.is_empty() {
        output.push_str("    if (spx_ctx->borrowed_str_depth == UINT32_C(0)) spx_runtime_invariant_failure(\"borrowed str call depth underflow\");\n");
        output.push_str("    --spx_ctx->borrowed_str_depth;\n");
    }
    // Callee-owned parameters free their storage on every exit path; a moved
    // Bytes carrier is normalized by `spx_bytes_move`, making this exact-once.
    // the staged result is handed to the caller instead.
    for (index, param) in function.params.iter().enumerate() {
        if matches!(param.ty, ResolvedType::String) {
            output.push_str(&format!("    spx_string_drop(spx_param_{index});\n"));
        }
    }
    if let Some(plan) = &bytes_plan {
        output.push_str(&plan.epilogue());
    }
    if has_try {
        output.push_str("    if (spx_status == SPX_STATUS_SUCCESS && !spx_result_staged) spx_runtime_invariant_failure(\"unstaged function result\");\n");
    }
    output.push_str("    if (spx_status != SPX_STATUS_SUCCESS) return spx_status;\n");
    if matches!(function.return_type, ResolvedType::Bytes) {
        let (value, flag) = bytes_plan
            .as_ref()
            .ok_or_else(|| backend_error("owned Bytes result has no cleanup plan"))?
            .provisional()?;
        output.push_str(&format!(
            "    if (!{flag}) spx_runtime_invariant_failure(\"dead Bytes provisional result\");\n    *spx_result_out = spx_bytes_move(&{value});\n    {flag} = false;\n"
        ));
    } else {
        output.push_str("    *spx_result_out = spx_result;\n");
    }
    output.push_str("    return SPX_STATUS_SUCCESS;\n");
    output.push_str("}\n\n");
    Ok(())
}

fn contract_label<'a>(
    expression: &'a ResolvedExpr,
    labels: &'a HashMap<ExpressionId, String>,
) -> &'a str {
    labels
        .get(&expression.id)
        .map_or_else(|| expression.id.as_str(), String::as_str)
}

fn expression_has_try(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => true,
        ResolvedExprKind::Call { args, .. } => args.iter().any(expression_has_try),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().any(expression_has_try),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => expression_has_try(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            expression_has_try(left) || expression_has_try(right)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| {
                (0..statement.child_count())
                    .any(|index| statement.child(index).is_some_and(expression_has_try))
            }) || expression_has_try(tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_has_try(condition)
                || expression_has_try(then_branch)
                || expression_has_try(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.iter().any(|field| expression_has_try(&field.value))
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            expression_has_try(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| expression_has_try(guard))
                        || expression_has_try(&arm.value)
                })
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            expression_has_try(base) || fields.iter().any(|field| expression_has_try(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::BorrowPlace { .. }
        | ResolvedExprKind::Place(_) => false,
    }
}

pub(super) fn write_and_compile_c(c_source: &str, output: &Path) -> Result<(), Diagnostic> {
    write_and_compile_c_with_mode(c_source, output, false)
}

pub(super) fn write_and_compile_c_with_mode(
    c_source: &str,
    output: &Path,
    native_command: bool,
) -> Result<(), Diagnostic> {
    static BUILD_ID: AtomicU64 = AtomicU64::new(0);
    let build_id = BUILD_ID.fetch_add(1, Ordering::Relaxed);
    let c_path = std::env::temp_dir().join(format!(
        "semaprax-codegen-{}-{build_id}.c",
        std::process::id()
    ));
    std::fs::write(&c_path, c_source).map_err(|error| {
        Diagnostic::io(
            "SPX-I101",
            format!(
                "cannot write temporary C source {}: {error}",
                c_path.display()
            ),
        )
    })?;
    let mut compiler = Command::new("clang");
    compiler.args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"]);
    #[cfg(all(windows, target_env = "gnu"))]
    if native_command {
        compiler.arg("-municode");
    }
    #[cfg(not(all(windows, target_env = "gnu")))]
    let _ = native_command;
    let result = compiler.arg(&c_path).arg("-o").arg(output).output();
    let _ = std::fs::remove_file(&c_path);
    let result = result.map_err(|error| {
        Diagnostic::io(
            "SPX-B101",
            format!("failed to start clang; install a C11 toolchain: {error}"),
        )
    })?;
    if !result.status.success() {
        return Err(Diagnostic::io(
            "SPX-B102",
            format!(
                "native backend failed:\n{}",
                String::from_utf8_lossy(&result.stderr)
            ),
        ));
    }
    Ok(())
}

#[derive(Clone)]
pub(super) struct CFunction {
    symbol: String,
    params: Vec<ResolvedType>,
    return_type: ResolvedType,
}

pub(super) fn function_index(
    program: &ResolvedProgram,
) -> Result<HashMap<FunctionExecutionId, CFunction>, Diagnostic> {
    let mut functions = HashMap::new();
    for function in &program.functions {
        let declaration = program
            .declarations
            .declaration(&function.id)
            .ok_or_else(|| {
                backend_error(format!(
                    "resolved function `{}` has no declaration",
                    function.id
                ))
            })?;
        if declaration.kind != DeclarationKind::Function {
            return Err(backend_error(format!(
                "resolved callable `{}` does not refer to a function declaration",
                function.id
            )));
        }
        let metadata = CFunction {
            symbol: c_function_symbol(&function.id),
            params: function
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            return_type: function.return_type.clone(),
        };
        if functions
            .insert(
                FunctionExecutionId::Monomorphic(function.id.clone()),
                metadata,
            )
            .is_some()
        {
            return Err(backend_error(format!(
                "duplicate resolved function identity `{}`",
                function.id
            )));
        }
    }
    for instance in &program.function_instances {
        let function = &instance.function;
        let execution = FunctionExecutionId::Generic(instance.id.clone());
        let metadata = CFunction {
            symbol: c_function_execution_symbol(&execution),
            params: function
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            return_type: function.return_type.clone(),
        };
        if functions.insert(execution, metadata).is_some() {
            return Err(backend_error(format!(
                "duplicate resolved function instance `{}`",
                instance.id
            )));
        }
    }
    Ok(functions)
}

fn c_function_execution_symbol(id: &FunctionExecutionId) -> String {
    let identity = match id {
        FunctionExecutionId::Monomorphic(declaration) => format!(
            "semaprax.function-execution.v1:monomorphic:{}:{}",
            declaration.as_str().len(),
            declaration
        ),
        FunctionExecutionId::Generic(instance) => format!(
            "semaprax.function-execution.v1:generic:{}:{}",
            instance.as_str().len(),
            instance
        ),
    };
    let mut symbol = crate::bounded_output::CappedString::new();
    symbol.push_str("spx_exec_");
    for byte in identity.bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.into_string()
}

pub(super) fn c_function_symbol(id: &DeclarationId) -> String {
    let mut symbol = crate::bounded_output::CappedString::new();
    symbol.push_str("spx_decl_");
    for byte in id.as_str().bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.into_string()
}

/// Refutable Match v1: exact C spelling of a literal pattern, using the same
/// conventions as the matching expression literals.
fn c_pattern_literal(value: hir::PatternValue) -> String {
    match value {
        hir::PatternValue::Int(value) => c_i64(value),
        hir::PatternValue::Int32(value) => format!("INT32_C({value})"),
        hir::PatternValue::Uint8(value) => format!("UINT8_C({value})"),
        hir::PatternValue::Usize(value) => format!("UINT64_C({value})"),
        hir::PatternValue::Char(value) => format!("UINT32_C(0x{value:x})"),
        hir::PatternValue::Bool(value) => value.to_string(),
    }
}

#[derive(Clone)]
struct CBinding {
    name: String,
    ty: ResolvedType,
}

struct CValue {
    code: String,
    ty: ResolvedType,
}

struct CEmitter<'a, O: COutput> {
    output: &'a mut O,
    program: &'a ResolvedProgram,
    resource_abi: &'a native_resource::NativeResourceAbi,
    variables: HashMap<ValueId, CBinding>,
    functions: &'a HashMap<FunctionExecutionId, CFunction>,
    record_layouts: &'a AggregateLayoutCache,
    variant_layouts: &'a VariantLayoutCache,
    return_type: &'a ResolvedType,
    bytes_plan: Option<&'a native_bytes::NativeBytesPlan>,
    output_profile: NativeOutputProfile,
    try_target_enabled: bool,
    next_local: usize,
    indent: usize,
}

impl<'a, O: COutput> CEmitter<'a, O> {
    fn new(
        output: &'a mut O,
        variables: HashMap<ValueId, CBinding>,
        return_type: &'a ResolvedType,
        emission: &'a NativeEmissionContext<'a>,
        bytes_plan: Option<&'a native_bytes::NativeBytesPlan>,
    ) -> Self {
        Self {
            output,
            program: emission.program,
            resource_abi: emission.resource_abi,
            variables,
            functions: emission.functions,
            record_layouts: emission.record_layouts,
            variant_layouts: emission.variant_layouts,
            return_type,
            bytes_plan,
            output_profile: emission.output_profile,
            try_target_enabled: false,
            next_local: 0,
            indent: 1,
        }
    }

    fn line(&mut self, value: &str) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        writeln!(self.output, "{value}").expect("writing to a string cannot fail");
    }

    fn label(&mut self, value: &str) {
        writeln!(self.output, "{value}: ;").expect("writing to a string cannot fail");
    }

    fn temporary(&mut self, ty: &ResolvedType) -> Result<String, Diagnostic> {
        if matches!(ty, ResolvedType::ArrayU8(0)) {
            return Ok("UINT8_C(0)".to_owned());
        }
        let name = format!("spx_internal_{}", self.next_local);
        self.next_local += 1;
        self.line(&format!(
            "{} {name};",
            c_value_type(self.program, self.resource_abi, ty)?
        ));
        Ok(name)
    }

    fn call_result_temporary(&mut self, ty: &ResolvedType) -> Result<String, Diagnostic> {
        if matches!(ty, ResolvedType::ArrayU8(0)) {
            let name = format!("spx_internal_{}", self.next_local);
            self.next_local += 1;
            self.line(&format!("uint8_t {name} = UINT8_C(0);"));
            Ok(name)
        } else {
            self.temporary(ty)
        }
    }

    fn require_type(
        &self,
        actual: &ResolvedType,
        expected: &ResolvedType,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if actual == expected {
            Ok(())
        } else {
            Err(backend_error(format!(
                "inconsistent HIR type for {context}: expected `{}`, found `{}`",
                expected.identity_key(),
                actual.identity_key()
            )))
        }
    }

    /// Refutable Match v1 native lowering: the scrutinee stages once, then
    /// every arm tests `!matched && (<literal equality>)` with an optional
    /// inner guard branch. `&&` short-circuits so a guard evaluates exactly
    /// once per reached arm whose pattern matched; failing guards leave
    /// `matched` false and fall through to the following arms. The resolver
    /// guarantees one trailing irrefutable guard-free catch-all, but the
    /// defensive no-arm check mirrors exhaustive matches.
    fn emit_scalar_match(
        &mut self,
        expr: &ResolvedExpr,
        scrutinee: &CValue,
        arms: &[hir::ResolvedMatchArm],
    ) -> Result<CValue, Diagnostic> {
        let staged = self.temporary(&scrutinee.ty)?;
        self.line(&format!("{staged} = {};", scrutinee.code));
        let result = if matches!(expr.ty, ResolvedType::Bytes) {
            self.bytes_plan
                .ok_or_else(|| backend_error("owned Bytes match has no cleanup plan"))?
                .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                .to_owned()
        } else {
            self.temporary(&expr.ty)?
        };
        let matched = self.temporary(&ResolvedType::Bool)?;
        self.line(&format!("{matched} = false;"));
        for arm in arms {
            let saved = self.variables.clone();
            if let hir::ResolvedMatchPattern::Binding(binding) = &arm.pattern {
                self.variables.insert(
                    binding.id.clone(),
                    CBinding {
                        name: staged.clone(),
                        ty: binding.ty.clone(),
                    },
                );
            }
            let test = match &arm.pattern {
                hir::ResolvedMatchPattern::Wildcard | hir::ResolvedMatchPattern::Binding(_) => None,
                hir::ResolvedMatchPattern::Literal(value) => {
                    Some(format!("{staged} == {}", c_pattern_literal(*value)))
                }
                hir::ResolvedMatchPattern::Or(alternatives) => Some(
                    alternatives
                        .iter()
                        .map(|alternative| match alternative {
                            hir::ResolvedMatchPattern::Literal(value) => {
                                format!("{staged} == {}", c_pattern_literal(*value))
                            }
                            _ => unreachable!("or-pattern alternatives are literals"),
                        })
                        .collect::<Vec<_>>()
                        .join(" || "),
                ),
                hir::ResolvedMatchPattern::Variant { .. }
                | hir::ResolvedMatchPattern::Record { .. } => {
                    return Err(backend_error(
                        "aggregate pattern has a Copy-scalar match scrutinee",
                    ));
                }
            };
            match &test {
                Some(test) => self.line(&format!("if (!{matched} && ({test})) {{")),
                None => self.line(&format!("if (!{matched}) {{")),
            }
            self.indent += 1;
            if let Some(guard) = &arm.guard {
                // The guard evaluates once here, after the pattern matched
                // and before any part of the arm value; a false guard leaves
                // `matched` untouched and falls through to the next arm.
                let flag = self.emit_expr(guard)?;
                self.require_type(&flag.ty, &ResolvedType::Bool, "match guard")?;
                self.line(&format!("if ({}) {{", flag.code));
                self.indent += 1;
                self.line(&format!("{matched} = true;"));
                let value = self.emit_expr(&arm.value)?;
                self.require_type(&value.ty, &expr.ty, "match arm result")?;
                if matches!(expr.ty, ResolvedType::Bytes) {
                    let transitions = self
                        .bytes_plan
                        .expect("checked above")
                        .apply_at(&arm.value.id)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                } else {
                    self.line(&format!("{result} = {};", value.code));
                }
                self.indent -= 1;
                self.line("}");
            } else {
                self.line(&format!("{matched} = true;"));
                let value = self.emit_expr(&arm.value)?;
                self.require_type(&value.ty, &expr.ty, "match arm result")?;
                if matches!(expr.ty, ResolvedType::Bytes) {
                    let transitions = self
                        .bytes_plan
                        .expect("checked above")
                        .apply_at(&arm.value.id)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                } else {
                    self.line(&format!("{result} = {};", value.code));
                }
            }
            self.variables = saved;
            self.indent -= 1;
            self.line("}");
        }
        self.line(&format!(
            "if (!{matched}) spx_runtime_invariant_failure(\"refutable match selected no arm\");"
        ));
        Ok(CValue {
            code: result,
            ty: expr.ty.clone(),
        })
    }

    fn emit_string_op(
        &mut self,
        op: crate::string_ops::StringOp,
        args: &[ResolvedExpr],
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        // Arguments stage left-to-right; every argument evaluation yields a
        // fresh caller-owned buffer, and consuming operations free their
        // inputs exactly at the operation site like owned string equality.
        let mut arguments = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            let value = self.emit_expr(argument)?;
            self.require_type(
                &value.ty,
                &op.param_types()[index],
                "string operation argument",
            )?;
            arguments.push(value);
        }
        self.require_type(result_type, &op.return_type(), "string operation result")?;
        let temporary = self.temporary(&op.return_type())?;
        match op {
            crate::string_ops::StringOp::Len => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_len({input});"));
                self.line(&format!("spx_string_drop({input});"));
            }
            crate::string_ops::StringOp::IsEmpty => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_is_empty({input});"));
                self.line(&format!("spx_string_drop({input});"));
            }
            crate::string_ops::StringOp::Concat => {
                let left = &arguments[0].code;
                let right = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_concat({left}, {right});"
                ));
                self.line(&format!("spx_string_drop({left});"));
                self.line(&format!("spx_string_drop({right});"));
            }
            crate::string_ops::StringOp::StartsWith => {
                let value = &arguments[0].code;
                let prefix = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_starts_with({value}, {prefix});"
                ));
                self.line(&format!("spx_string_drop({value});"));
                self.line(&format!("spx_string_drop({prefix});"));
            }
            crate::string_ops::StringOp::Contains => {
                let value = &arguments[0].code;
                let needle = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_contains({value}, {needle});"
                ));
                self.line(&format!("spx_string_drop({value});"));
                self.line(&format!("spx_string_drop({needle});"));
            }
            crate::string_ops::StringOp::LenChars => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_len_chars({input});"));
                self.line(&format!("spx_string_drop({input});"));
            }
            crate::string_ops::StringOp::FromChar => {
                let scalar = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_from_char({scalar});"));
            }
        }
        Ok(CValue {
            code: temporary,
            ty: op.return_type(),
        })
    }

    fn emit_str_op(
        &mut self,
        op: crate::str_ops::StrOp,
        args: &[ResolvedExpr],
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        // A borrowed view is copied as a two-word carrier only. Operations do
        // not allocate, clone, retain, consume, or drop either source view.
        let mut arguments = Vec::with_capacity(args.len());
        if args.len() != op.arity() {
            return Err(backend_error(format!(
                "borrowed str operation `{}` has {} arguments; expected {}",
                op.id(),
                args.len(),
                op.arity()
            )));
        }
        for argument in args {
            let value = self.emit_expr(argument)?;
            self.require_type(
                &value.ty,
                &ResolvedType::Str,
                "borrowed str operation argument",
            )?;
            arguments.push(value);
        }
        self.require_type(
            result_type,
            &op.return_type(),
            "borrowed str operation result",
        )?;
        let temporary = self.temporary(&op.return_type())?;
        match op {
            crate::str_ops::StrOp::LenBytes => self.line(&format!(
                "{temporary} = spx_str_len_bytes({});",
                arguments[0].code
            )),
            crate::str_ops::StrOp::IsEmpty => self.line(&format!(
                "{temporary} = spx_str_is_empty({});",
                arguments[0].code
            )),
            crate::str_ops::StrOp::StartsWith => self.line(&format!(
                "{temporary} = spx_str_starts_with({}, {});",
                arguments[0].code, arguments[1].code
            )),
            crate::str_ops::StrOp::Contains => self.line(&format!(
                "{temporary} = spx_str_contains({}, {});",
                arguments[0].code, arguments[1].code
            )),
        }
        Ok(CValue {
            code: temporary,
            ty: op.return_type(),
        })
    }

    fn emit_byte_op(
        &mut self,
        op: crate::byte_ops::ByteOp,
        args: &[ResolvedExpr],
        result_type: &ResolvedType,
        expression: &ExpressionId,
    ) -> Result<CValue, Diagnostic> {
        if args.len() != op.arity() {
            return Err(backend_error(format!(
                "byte operation `{}` has {} arguments; expected {}",
                op.id(),
                args.len(),
                op.arity()
            )));
        }
        let mut arguments = Vec::with_capacity(args.len());
        for (argument, expected) in args.iter().zip(op.param_types()) {
            let value = self.emit_expr(argument)?;
            self.require_type(&value.ty, expected, "byte operation argument")?;
            arguments.push(value);
        }
        let return_type = op.return_type();
        self.require_type(result_type, &return_type, "byte operation result")?;
        let temporary = if matches!(op, crate::byte_ops::ByteOp::Copy) {
            self.bytes_plan
                .as_ref()
                .ok_or_else(|| backend_error("bytes_copy has no canonical cleanup plan"))?
                .value(&crate::cleanup_plan::StorageId::Temporary(
                    expression.clone(),
                ))?
                .to_owned()
        } else {
            self.temporary(&return_type)?
        };
        match op {
            crate::byte_ops::ByteOp::Len => {
                self.line(&format!(
                    "{temporary} = spx_byte_len({});",
                    arguments[0].code
                ));
            }
            crate::byte_ops::ByteOp::Get => {
                let layout = self.variant_layout(&return_type)?;
                let none_id = DeclarationId::new(crate::prelude::OPTION_NONE_ID);
                let some_id = DeclarationId::new(crate::prelude::OPTION_SOME_ID);
                let value_id = DeclarationId::new(crate::prelude::OPTION_SOME_VALUE_ID);
                let none = layout.case(&none_id).ok_or_else(|| {
                    backend_error("Option<u8> layout has no compiler-owned None case")
                })?;
                let some = layout.case(&some_id).ok_or_else(|| {
                    backend_error("Option<u8> layout has no compiler-owned Some case")
                })?;
                let field = some.field(&value_id).ok_or_else(|| {
                    backend_error("Option<u8> layout has no compiler-owned Some payload")
                })?;
                self.require_type(&field.ty, &ResolvedType::U8, "byte_get Some payload")?;
                let slice = &arguments[0].code;
                let index = &arguments[1].code;
                self.line(&format!("spx_slice_u8_require_valid({slice});"));
                self.line(&format!("memset(&{temporary}, 0, sizeof({temporary}));"));
                self.line(&format!("if ({index} < ({slice}).len) {{"));
                self.indent += 1;
                self.line(&format!(
                    "{temporary}.spx_payload.{}.{} = ({slice}).ptr[{index}];",
                    c_case_symbol(&some_id),
                    c_field_symbol(&value_id)
                ));
                self.line(&format!("{temporary}.spx_tag = UINT32_C({});", some.tag));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                self.line(&format!("{temporary}.spx_tag = UINT32_C({});", none.tag));
                self.indent -= 1;
                self.line("}");
            }
            crate::byte_ops::ByteOp::Copy => {
                self.line(&format!(
                    "{temporary} = spx_bytes_copy({});",
                    arguments[0].code
                ));
            }
            crate::byte_ops::ByteOp::BytesAsSlice
            | crate::byte_ops::ByteOp::ArrayAsSlice
            | crate::byte_ops::ByteOp::StrAsBytes => {
                return Err(backend_error(format!(
                    "byte view `{}` reached native lowering without authenticated BorrowPlace HIR",
                    op.id()
                )));
            }
        }
        let mut code = temporary;
        if let Some(plan) = self.bytes_plan {
            let transitions = plan.apply_at(expression)?;
            for line in transitions.lines() {
                self.line(line);
            }
            if matches!(return_type, ResolvedType::Bytes) {
                code = plan
                    .result_at(expression)
                    .ok_or_else(|| backend_error("bytes_copy has no initialized result slot"))?
                    .to_owned();
            }
        }
        Ok(CValue {
            code,
            ty: return_type,
        })
    }

    fn emit_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::Int(value) => {
                self.require_type(&expr.ty, &ResolvedType::I64, "integer literal")?;
                CValue {
                    code: c_i64(*value),
                    ty: ResolvedType::I64,
                }
            }
            ResolvedExprKind::Int32(value) => {
                self.require_type(&expr.ty, &ResolvedType::I32, "i32 literal")?;
                CValue {
                    code: format!("INT32_C({value})"),
                    ty: ResolvedType::I32,
                }
            }
            ResolvedExprKind::Char(value) => {
                self.require_type(&expr.ty, &ResolvedType::Char, "char literal")?;
                CValue {
                    code: format!("UINT32_C(0x{value:x})"),
                    ty: ResolvedType::Char,
                }
            }
            ResolvedExprKind::Uint8(value) => {
                self.require_type(&expr.ty, &ResolvedType::U8, "u8 literal")?;
                CValue {
                    code: format!("UINT8_C({value})"),
                    ty: ResolvedType::U8,
                }
            }
            ResolvedExprKind::Usize(value) => {
                self.require_type(&expr.ty, &ResolvedType::Usize, "usize literal")?;
                CValue {
                    code: format!("UINT64_C({value})"),
                    ty: ResolvedType::Usize,
                }
            }
            ResolvedExprKind::ArrayU8(values) => {
                let expected = ResolvedType::ArrayU8(
                    u32::try_from(values.len())
                        .map_err(|_| backend_error("fixed byte array length exceeds u32"))?,
                );
                self.require_type(&expr.ty, &expected, "fixed byte array literal")?;
                if values.is_empty() {
                    return Ok(CValue {
                        code: "UINT8_C(0)".to_owned(),
                        ty: expr.ty.clone(),
                    });
                } else {
                    let temporary = self.temporary(&expr.ty)?;
                    let bytes = values
                        .iter()
                        .map(|value| format!("UINT8_C({value})"))
                        .collect::<Vec<_>>()
                        .budgeted_join(", ");
                    self.line(&format!(
                        "{temporary} = (struct spx_array_u8_{}) {{ .spx_bytes = {{ {bytes} }} }};",
                        values.len()
                    ));
                    CValue {
                        code: temporary,
                        ty: expr.ty.clone(),
                    }
                }
            }
            ResolvedExprKind::RepeatArrayU8 { value, count } => {
                let expected = ResolvedType::ArrayU8(*count);
                self.require_type(&expr.ty, &expected, "repeated fixed byte array literal")?;
                if *count == 0 {
                    return Ok(CValue {
                        code: "UINT8_C(0)".to_owned(),
                        ty: expr.ty.clone(),
                    });
                } else {
                    let temporary = self.temporary(&expr.ty)?;
                    self.line(&format!(
                        "memset({temporary}.spx_bytes, UINT8_C({value}), UINT32_C({count}));"
                    ));
                    CValue {
                        code: temporary,
                        ty: expr.ty.clone(),
                    }
                }
            }
            ResolvedExprKind::Float32(bits) => {
                self.require_type(&expr.ty, &ResolvedType::F32, "float literal")?;
                CValue {
                    code: format!("{}f", crate::format::canonical_f32_bits(*bits)),
                    ty: ResolvedType::F32,
                }
            }
            ResolvedExprKind::Float64(bits) => {
                self.require_type(&expr.ty, &ResolvedType::F64, "float literal")?;
                CValue {
                    code: crate::format::canonical_f64_bits(*bits),
                    ty: ResolvedType::F64,
                }
            }
            ResolvedExprKind::Bool(value) => {
                self.require_type(&expr.ty, &ResolvedType::Bool, "boolean literal")?;
                CValue {
                    code: value.to_string(),
                    ty: ResolvedType::Bool,
                }
            }
            ResolvedExprKind::String(value) => {
                self.require_type(&expr.ty, &ResolvedType::String, "string literal")?;
                let temporary = self.temporary(&ResolvedType::String)?;
                self.line(&format!(
                    "{temporary} = spx_string_from_literal(\"{}\", UINT64_C({}));",
                    c_string(value),
                    value.len()
                ));
                CValue {
                    code: temporary,
                    ty: ResolvedType::String,
                }
            }
            ResolvedExprKind::Place(place) => {
                let value = self.emit_place(place)?;
                self.require_type(&expr.ty, &value.ty, "place expression")?;
                // Every read of an owned string place yields a fresh buffer so
                // the source place keeps its unique owner.
                if matches!(value.ty, ResolvedType::String) {
                    let temporary = self.temporary(&ResolvedType::String)?;
                    self.line(&format!("{temporary} = spx_string_clone({});", value.code));
                    return Ok(CValue {
                        code: temporary,
                        ty: value.ty,
                    });
                }
                value
            }
            ResolvedExprKind::BorrowPlace { operation, place } => {
                let op = crate::byte_ops::by_id(operation.as_str()).ok_or_else(|| {
                    backend_error(format!(
                        "unknown compiler-owned byte view identity `{operation}`"
                    ))
                })?;
                if !op.is_view() {
                    return Err(backend_error(format!(
                        "non-view byte operation `{operation}` used BorrowPlace HIR"
                    )));
                }
                let source = self.emit_place(place)?;
                let temporary = self.temporary(&ResolvedType::SliceU8)?;
                match op {
                    crate::byte_ops::ByteOp::BytesAsSlice => {
                        self.require_type(
                            &source.ty,
                            &ResolvedType::Bytes,
                            "owned byte borrow source",
                        )?;
                        self.line(&format!(
                            "{temporary} = spx_bytes_as_slice(&({}));",
                            source.code
                        ));
                    }
                    crate::byte_ops::ByteOp::ArrayAsSlice => {
                        let ResolvedType::ArrayU8(length) = source.ty else {
                            return Err(backend_error(
                                "fixed byte array borrow has a non-array source",
                            ));
                        };
                        if length == 0 {
                            self.line(&format!(
                                "{temporary} = (spx_slice_u8_v1) {{ .ptr = NULL, .len = UINT64_C(0) }};"
                            ));
                        } else {
                            self.line(&format!(
                                "{temporary} = (spx_slice_u8_v1) {{ .ptr = ({}).spx_bytes, .len = UINT64_C({length}) }};",
                                source.code
                            ));
                        }
                        self.line(&format!("spx_slice_u8_require_valid({temporary});"));
                    }
                    crate::byte_ops::ByteOp::StrAsBytes => {
                        self.require_type(
                            &source.ty,
                            &ResolvedType::Str,
                            "borrowed UTF-8 byte view source",
                        )?;
                        self.line(&format!("spx_str_require_valid({});", source.code));
                        self.line(&format!(
                            "{temporary} = (spx_slice_u8_v1) {{ .ptr = ({}).len == UINT64_C(0) ? NULL : (const uint8_t *)({}).data, .len = ({}).len }};",
                            source.code, source.code, source.code
                        ));
                        self.line(&format!("spx_slice_u8_require_valid({temporary});"));
                    }
                    crate::byte_ops::ByteOp::Len
                    | crate::byte_ops::ByteOp::Get
                    | crate::byte_ops::ByteOp::Copy => unreachable!(),
                }
                CValue {
                    code: temporary,
                    ty: ResolvedType::SliceU8,
                }
            }
            ResolvedExprKind::Call {
                callee,
                instance,
                args,
                ..
            } => {
                if instance.is_none() {
                    if crate::host_io_ops::by_id(callee.as_str()).is_some() {
                        if !self.output_profile.supports_stdout_transcript() {
                            return Err(backend_error(
                                "host stdout write requires the native stdout-transcript profile",
                            ));
                        }
                        if args.len() != 1 {
                            return Err(backend_error(
                                "host stdout write arity disagrees with resolved HIR",
                            ));
                        }
                        let value = self.emit_expr(&args[0])?;
                        self.require_type(
                            &value.ty,
                            &ResolvedType::SliceU8,
                            "host stdout write argument",
                        )?;
                        self.require_type(
                            &expr.ty,
                            &ResolvedType::Usize,
                            "host stdout write result",
                        )?;
                        let temporary = self.temporary(&ResolvedType::Usize)?;
                        self.line(&format!(
                            "{temporary} = spx_host_stdout_write_v1(spx_ctx, {});",
                            value.code
                        ));
                        return Ok(CValue {
                            code: temporary,
                            ty: ResolvedType::Usize,
                        });
                    }
                    if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                        return self.emit_str_op(op, args, &expr.ty);
                    }
                    if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                        return self.emit_byte_op(op, args, &expr.ty, &expr.id);
                    }
                    if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                        return self.emit_string_op(op, args, &expr.ty);
                    }
                }
                let execution = instance.as_ref().map_or_else(
                    || FunctionExecutionId::Monomorphic(callee.clone()),
                    |instance| FunctionExecutionId::Generic(instance.clone()),
                );
                let target = self.functions.get(&execution).ok_or_else(|| {
                    backend_error(format!("resolved callee `{callee}` is not indexed"))
                })?;
                if args.len() != target.params.len() {
                    return Err(backend_error(format!(
                        "resolved call to `{callee}` has {} arguments; expected {}",
                        args.len(),
                        target.params.len()
                    )));
                }
                let target = target.clone();
                let mut arguments = Vec::with_capacity(args.len());
                for (index, (arg, expected)) in args.iter().zip(&target.params).enumerate() {
                    let argument = self.emit_expr(arg)?;
                    self.require_type(&argument.ty, expected, &format!("call argument {index}"))?;
                    arguments.push(if matches!(expected, ResolvedType::Bytes) {
                        let plan = self.bytes_plan.ok_or_else(|| {
                            backend_error("owned Bytes call has no canonical cleanup plan")
                        })?;
                        let transitions = plan.apply_at(&arg.id)?;
                        for line in transitions.lines() {
                            self.line(line);
                        }
                        let parameter_index = u32::try_from(index)
                            .map_err(|_| backend_error("native call has too many parameters"))?;
                        let (value, _) = plan.call_argument(&expr.id, parameter_index)?;
                        format!("spx_bytes_move(&{value})")
                    } else if is_aggregate_type(self.program, expected)? {
                        format!("&({})", argument.code)
                    } else {
                        argument.code
                    });
                }
                self.require_type(&expr.ty, &target.return_type, "call result")?;
                let temporary = if matches!(target.return_type, ResolvedType::Bytes) {
                    self.bytes_plan
                        .ok_or_else(|| {
                            backend_error("owned Bytes call result has no cleanup plan")
                        })?
                        .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                        .to_owned()
                } else {
                    self.call_result_temporary(&target.return_type)?
                };
                self.line(&format!(
                    "spx_status = {}(spx_ctx{}{}, &{temporary});",
                    target.symbol,
                    if arguments.is_empty() { "" } else { ", " },
                    arguments.budgeted_join(", ")
                ));
                if let Some(plan) = self.bytes_plan {
                    for (index, expected) in target.params.iter().enumerate() {
                        if matches!(expected, ResolvedType::Bytes) {
                            let (_, flag) = plan.call_argument(
                                &expr.id,
                                u32::try_from(index).map_err(|_| {
                                    backend_error("native call has too many parameters")
                                })?,
                            )?;
                            self.line(&format!("{flag} = false;"));
                        }
                    }
                }
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                if let Some(plan) = self.bytes_plan {
                    let transitions = plan.apply_at(&expr.id)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                }
                CValue {
                    code: if matches!(target.return_type, ResolvedType::Bytes) {
                        self.bytes_plan
                            .and_then(|plan| plan.result_at(&expr.id))
                            .ok_or_else(|| {
                                backend_error("owned call has no canonical result transfer")
                            })?
                            .to_owned()
                    } else {
                        temporary
                    },
                    ty: target.return_type,
                }
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                return Err(backend_error(format!(
                    "native Rust import `{}` is unavailable in the ordinary native backend",
                    call.import
                )));
            }
            ResolvedExprKind::Unary { op, value } => {
                let value = self.emit_expr(value)?;
                let (ty, operand_type) = match op {
                    UnaryOp::Neg => match &value.ty {
                        ResolvedType::F32 => (ResolvedType::F32, ResolvedType::F32),
                        ResolvedType::F64 => (ResolvedType::F64, ResolvedType::F64),
                        ResolvedType::I32 => (ResolvedType::I32, ResolvedType::I32),
                        _ => (ResolvedType::I64, ResolvedType::I64),
                    },
                    UnaryOp::Not => (ResolvedType::Bool, ResolvedType::Bool),
                };
                self.require_type(&value.ty, &operand_type, "unary operand")?;
                self.require_type(&expr.ty, &ty, "unary result")?;
                let temporary = self.temporary(&ty)?;
                match op {
                    UnaryOp::Neg if matches!(ty, ResolvedType::F32 | ResolvedType::F64) => {
                        self.line(&format!("{temporary} = (-({}));", value.code));
                    }
                    UnaryOp::Neg if ty == ResolvedType::I32 => {
                        self.line(&format!(
                            "spx_status = spx_rt_neg_i32(spx_ctx, {}, &{temporary});",
                            value.code
                        ));
                        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                    }
                    UnaryOp::Neg => {
                        self.line(&format!(
                            "spx_status = spx_rt_neg(spx_ctx, {}, &{temporary});",
                            value.code
                        ));
                        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                    }
                    UnaryOp::Not => self.line(&format!("{temporary} = (!{});", value.code)),
                }
                CValue {
                    code: temporary,
                    ty,
                }
            }
            ResolvedExprKind::Binary { op, left, right } => {
                return self.emit_binary(*op, left, right, &expr.ty);
            }
            ResolvedExprKind::Block { statements, tail } => {
                let saved = self.variables.clone();
                for statement in statements {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            let value = self.emit_expr(value)?;
                            self.require_type(&value.ty, &binding.ty, "local binding")?;
                            let local = if matches!(binding.ty, ResolvedType::Bytes) {
                                let plan = self.bytes_plan.ok_or_else(|| {
                                    backend_error(
                                        "owned Bytes binding has no canonical cleanup plan",
                                    )
                                })?;
                                let storage =
                                    crate::cleanup_plan::StorageId::Value(binding.id.clone());
                                let expected = plan.value(&storage)?.to_owned();
                                if value.code != expected {
                                    let transitions = plan.transfer_to(&storage)?;
                                    for line in transitions.lines() {
                                        self.line(line);
                                    }
                                }
                                expected
                            } else if matches!(binding.ty, ResolvedType::ArrayU8(0)) {
                                // The expression has already been evaluated;
                                // its zero-sized Copy value has no C storage.
                                "UINT8_C(0)".to_owned()
                            } else {
                                let local = format!("spx_local_{}", self.next_local);
                                self.next_local += 1;
                                self.line(&format!(
                                    "{} {local} = {};",
                                    c_value_type(self.program, self.resource_abi, &binding.ty)?,
                                    value.code
                                ));
                                self.line(&format!("(void){local};"));
                                local
                            };
                            if self
                                .variables
                                .insert(
                                    binding.id.clone(),
                                    CBinding {
                                        name: local,
                                        ty: binding.ty.clone(),
                                    },
                                )
                                .is_some()
                            {
                                return Err(backend_error(format!(
                                    "duplicate resolved local identity `{}`",
                                    binding.id
                                )));
                            }
                        }
                        ResolvedStatement::Assign {
                            binding,
                            field,
                            value: assigned,
                            ..
                        } => {
                            // The assigned value is emitted fully first; the
                            // store is a plain C11 assignment into the local
                            // or, for Field Mutation v1, into its one direct
                            // scalar field.
                            let value = self.emit_expr(assigned)?;
                            match field {
                                Some(field_id) => {
                                    let layout = self.record_layout(&binding.ty)?;
                                    let field =
                                        layout.field(field_id).cloned().ok_or_else(|| {
                                            backend_error(format!(
                                                "native record `{}` has no assignment field `{field_id}`",
                                                layout.record
                                            ))
                                        })?;
                                    self.require_type(&value.ty, &field.ty, "field assignment")?;
                                    if matches!(field.ty, ResolvedType::String) {
                                        return Err(backend_error(
                                            "string field assignment has no admitted native lowering",
                                        ));
                                    }
                                    if matches!(field.ty, ResolvedType::Bytes) {
                                        return Err(backend_error(
                                            "owned Bytes field assignment has no admitted native lowering",
                                        ));
                                    }
                                    let target =
                                        self.variables.get(&binding.id).ok_or_else(|| {
                                            backend_error(format!(
                                                "assignment target `{}` has no native local",
                                                binding.id
                                            ))
                                        })?;
                                    if field.size != 0 {
                                        self.line(&format!(
                                            "{}.{} = {};",
                                            target.name,
                                            c_field_symbol(&field.field),
                                            value.code
                                        ));
                                    }
                                }
                                None => {
                                    self.require_type(&value.ty, &binding.ty, "assignment")?;
                                    if matches!(binding.ty, ResolvedType::String) {
                                        return Err(backend_error(
                                            "string assignment has no admitted native lowering",
                                        ));
                                    }
                                    if matches!(binding.ty, ResolvedType::Bytes) {
                                        return Err(backend_error(
                                            "owned Bytes assignment is outside the immutable data profile",
                                        ));
                                    }
                                    let target =
                                        self.variables.get(&binding.id).ok_or_else(|| {
                                            backend_error(format!(
                                                "assignment target `{}` has no native local",
                                                binding.id
                                            ))
                                        })?;
                                    if !matches!(binding.ty, ResolvedType::ArrayU8(0)) {
                                        self.line(&format!("{} = {};", target.name, value.code));
                                    }
                                }
                            }
                        }
                        ResolvedStatement::Unsafe { body, .. } => {
                            // Backends treat the boundary transparently: emit
                            // exactly the ordinary block body and discard its
                            // scalar Copy result.
                            let value = self.emit_expr(body)?;
                            if matches!(value.ty, ResolvedType::String) {
                                return Err(backend_error(
                                    "discarding an owned string has no admitted native lowering",
                                ));
                            }
                            self.line(&format!("(void)({});", value.code));
                        }
                        ResolvedStatement::While {
                            condition, body, ..
                        } => {
                            // Bounded While-Loops v1 lowers to a native C11
                            // loop. Because checked sub-expressions lower to
                            // statements, the condition re-evaluates at the
                            // top of every iteration and breaks out on false;
                            // checked-arithmetic failures inside the loop jump
                            // to the shared epilogue exactly like
                            // straight-line failures.
                            self.line("for (;;) {");
                            self.indent += 1;
                            let condition = self.emit_expr(condition)?;
                            self.require_type(
                                &condition.ty,
                                &ResolvedType::Bool,
                                "while condition",
                            )?;
                            self.line(&format!("if (!({})) break;", condition.code));
                            let body_value = self.emit_expr(body)?;
                            if matches!(body_value.ty, ResolvedType::String) {
                                return Err(backend_error(
                                    "discarding an owned string has no admitted native lowering",
                                ));
                            }
                            self.line(&format!("(void)({});", body_value.code));
                            self.indent -= 1;
                            self.line("}");
                        }
                    }
                }
                let mut tail = self.emit_expr(tail)?;
                self.require_type(&tail.ty, &expr.ty, "block result")?;
                // Owned string locals introduced in this block free exactly
                // their own buffer when the block exits; outer bindings and
                // the tail value are untouched. The order is sorted so the
                // projection stays byte-deterministic.
                let mut introduced_strings: Vec<String> = self
                    .variables
                    .iter()
                    .filter(|(id, binding)| {
                        matches!(binding.ty, ResolvedType::String) && !saved.contains_key(*id)
                    })
                    .map(|(_, binding)| binding.name.clone())
                    .collect();
                introduced_strings.sort();
                for name in introduced_strings {
                    self.line(&format!("spx_string_drop({name});"));
                }
                if matches!(tail.ty, ResolvedType::Bytes) {
                    let plan = self.bytes_plan.ok_or_else(|| {
                        backend_error("owned Bytes block has no canonical cleanup plan")
                    })?;
                    let transitions = plan.apply_at(&expr.id)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                    tail.code = plan
                        .result_at(&expr.id)
                        .ok_or_else(|| {
                            backend_error("owned Bytes block has no canonical result transfer")
                        })?
                        .to_owned();
                }
                if let Some(plan) = self.bytes_plan {
                    let anchors = statements
                        .iter()
                        .flat_map(|statement| {
                            let mut anchors = Vec::with_capacity(2);
                            if let ResolvedStatement::Let { binding, .. } = statement {
                                if binding.ty == ResolvedType::Bytes {
                                    anchors.push(crate::cleanup_plan::StorageId::Value(
                                        binding.id.clone(),
                                    ));
                                }
                            }
                            let value = match statement {
                                ResolvedStatement::Let { value, .. }
                                | ResolvedStatement::Assign { value, .. } => Some(value),
                                ResolvedStatement::Unsafe { body, .. } => Some(body.as_ref()),
                                ResolvedStatement::While { .. } => None,
                            };
                            if let Some(value) =
                                value.filter(|value| value.ty == ResolvedType::Bytes)
                            {
                                anchors.push(crate::cleanup_plan::StorageId::Temporary(
                                    value.id.clone(),
                                ));
                            }
                            anchors
                        })
                        .collect::<BTreeSet<_>>();
                    let cleanup = plan.scope_exit(&anchors)?;
                    for line in cleanup.lines() {
                        self.line(line);
                    }
                }
                self.variables = saved;
                tail
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.emit_expr(condition)?;
                self.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
                let temporary = if matches!(expr.ty, ResolvedType::Bytes) {
                    self.bytes_plan
                        .ok_or_else(|| backend_error("owned Bytes if has no cleanup plan"))?
                        .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                        .to_owned()
                } else {
                    self.temporary(&expr.ty)?
                };
                self.line(&format!("if ({}) {{", condition.code));
                self.indent += 1;
                let then_value = self.emit_expr(then_branch)?;
                self.require_type(&then_value.ty, &expr.ty, "then branch")?;
                if matches!(expr.ty, ResolvedType::Bytes) {
                    let plan = self.bytes_plan.expect("checked above");
                    let transitions = plan.transfer_from_to(
                        &then_value.code,
                        &crate::cleanup_plan::StorageId::Temporary(expr.id.clone()),
                    )?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                } else if !matches!(expr.ty, ResolvedType::ArrayU8(0)) {
                    self.line(&format!("{temporary} = {};", then_value.code));
                }
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                let else_value = self.emit_expr(else_branch)?;
                self.require_type(&else_value.ty, &expr.ty, "else branch")?;
                if matches!(expr.ty, ResolvedType::Bytes) {
                    let plan = self.bytes_plan.expect("checked above");
                    let transitions = plan.transfer_from_to(
                        &else_value.code,
                        &crate::cleanup_plan::StorageId::Temporary(expr.id.clone()),
                    )?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                } else if !matches!(expr.ty, ResolvedType::ArrayU8(0)) {
                    self.line(&format!("{temporary} = {};", else_value.code));
                }
                self.indent -= 1;
                self.line("}");
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::ConstructRecord { record, fields } => {
                let layout = self.record_layout(&expr.ty)?;
                if layout.record != *record {
                    return Err(backend_error(format!(
                        "native record constructor `{record}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let temporary = self.temporary(&expr.ty)?;
                if layout.size == 0 {
                    self.line(&format!(
                        "{temporary}.spx_zero_sized_record_carrier = UINT8_C(0);"
                    ));
                }
                for initializer in fields {
                    let field = layout.field(&initializer.field).cloned().ok_or_else(|| {
                        backend_error(format!(
                            "native record `{record}` has no field `{}`",
                            initializer.field
                        ))
                    })?;
                    let value = self.emit_expr(&initializer.value)?;
                    self.require_type(&value.ty, &field.ty, "record field initializer")?;
                    if field.size != 0 {
                        self.line(&format!(
                            "{temporary}.{} = {};",
                            c_field_symbol(&field.field),
                            value.code
                        ));
                    }
                }
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                let layout = self.variant_layout(&expr.ty)?;
                if layout.variant != *variant {
                    return Err(backend_error(format!(
                        "native variant constructor `{variant}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let case_layout = layout.case(case).cloned().ok_or_else(|| {
                    backend_error(format!("native variant `{variant}` has no case `{case}`"))
                })?;
                let mut values = Vec::with_capacity(fields.len());
                for initializer in fields {
                    let field =
                        case_layout
                            .field(&initializer.field)
                            .cloned()
                            .ok_or_else(|| {
                                backend_error(format!(
                                    "native variant case `{case}` has no field `{}`",
                                    initializer.field
                                ))
                            })?;
                    let value = self.emit_expr(&initializer.value)?;
                    self.require_type(&value.ty, &field.ty, "variant field initializer")?;
                    values.push((field, value));
                }
                let temporary = self.temporary(&expr.ty)?;
                self.line(&format!("memset(&{temporary}, 0, sizeof({temporary}));"));
                let case_symbol = c_case_symbol(case);
                for (field, value) in values {
                    if field.size != 0 {
                        self.line(&format!(
                            "{temporary}.spx_payload.{case_symbol}.{} = {};",
                            c_field_symbol(&field.field),
                            value.code
                        ));
                    }
                }
                self.line(&format!(
                    "{temporary}.spx_tag = UINT32_C({});",
                    case_layout.tag
                ));
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                if is_aggregate_type(self.program, &expr.ty)? {
                    return Err(backend_error("copy match arms must produce i64 or bool"));
                }
                let scrutinee = self.emit_expr(scrutinee)?;
                // Refutable Match v1: Copy-scalar scrutinees lower to the
                // literal/guard decision chain; aggregates keep the exact
                // pre-feature lowering below.
                if matches!(
                    scrutinee.ty,
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::U8
                        | ResolvedType::Char
                        | ResolvedType::Bool
                ) {
                    return self.emit_scalar_match(expr, &scrutinee, arms);
                }
                if let Some(record) = record_declaration_id(self.program, &scrutinee.ty)?.cloned() {
                    let [arm] = arms.as_slice() else {
                        return Err(backend_error(
                            "irrefutable record match must have exactly one arm",
                        ));
                    };
                    let staged = self.temporary(&scrutinee.ty)?;
                    self.line(&format!("{staged} = {};", scrutinee.code));
                    let saved = self.variables.clone();
                    match &arm.pattern {
                        hir::ResolvedMatchPattern::Wildcard => {}
                        hir::ResolvedMatchPattern::Record {
                            record: pattern_record,
                            instance,
                            fields,
                        } => self.bind_record_match_pattern(
                            &staged,
                            &scrutinee.ty,
                            pattern_record,
                            instance,
                            fields,
                        )?,
                        hir::ResolvedMatchPattern::Variant { .. } => {
                            return Err(backend_error(
                                "variant pattern has a record match scrutinee",
                            ));
                        }
                        hir::ResolvedMatchPattern::Literal(_)
                        | hir::ResolvedMatchPattern::Or(_)
                        | hir::ResolvedMatchPattern::Binding(_) => {
                            return Err(backend_error(
                                "refutable pattern has an aggregate record match scrutinee",
                            ));
                        }
                    }
                    if record_declaration_id(self.program, &scrutinee.ty)? != Some(&record) {
                        return Err(backend_error(
                            "record match scrutinee identity changed during lowering",
                        ));
                    }
                    let value = self.emit_expr(&arm.value)?;
                    self.require_type(&value.ty, &expr.ty, "record match arm result")?;
                    self.variables = saved;
                    return Ok(CValue {
                        code: value.code,
                        ty: expr.ty.clone(),
                    });
                }
                let layout = self.variant_layout(&scrutinee.ty)?;
                let staged = self.temporary(&scrutinee.ty)?;
                self.line(&format!("{staged} = {};", scrutinee.code));
                self.line(&format!(
                    "if ({staged}.spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid variant tag\");",
                    layout.cases.len()
                ));
                let result = if matches!(expr.ty, ResolvedType::Bytes) {
                    self.bytes_plan
                        .ok_or_else(|| backend_error("owned Bytes match has no cleanup plan"))?
                        .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                        .to_owned()
                } else {
                    self.temporary(&expr.ty)?
                };
                let matched = self.temporary(&ResolvedType::Bool)?;
                self.line(&format!("{matched} = false;"));
                for arm in arms {
                    let saved = self.variables.clone();
                    match &arm.pattern {
                        hir::ResolvedMatchPattern::Variant {
                            variant,
                            case,
                            fields,
                        } => {
                            if *variant != layout.variant {
                                return Err(backend_error(format!(
                                    "match arm variant `{variant}` disagrees with `{}`",
                                    layout.variant
                                )));
                            }
                            let case_layout = layout.case(case).cloned().ok_or_else(|| {
                                backend_error(format!("match arm references unknown case `{case}`"))
                            })?;
                            self.line(&format!(
                                "if (!{matched} && {staged}.spx_tag == UINT32_C({})) {{",
                                case_layout.tag
                            ));
                            self.indent += 1;
                            self.line(&format!("{matched} = true;"));
                            let case_symbol = c_case_symbol(case);
                            for pattern_field in fields {
                                let field = case_layout
                                    .field(&pattern_field.field)
                                    .cloned()
                                    .ok_or_else(|| {
                                        backend_error(format!(
                                            "match case `{case}` has no field `{}`",
                                            pattern_field.field
                                        ))
                                    })?;
                                self.require_type(
                                    &pattern_field.binding.ty,
                                    &field.ty,
                                    "match payload binding",
                                )?;
                                self.variables.insert(
                                    pattern_field.binding.id.clone(),
                                    CBinding {
                                        name: format!(
                                            "({staged}).spx_payload.{case_symbol}.{}",
                                            c_field_symbol(&field.field)
                                        ),
                                        ty: field.ty,
                                    },
                                );
                            }
                        }
                        hir::ResolvedMatchPattern::Wildcard => {
                            self.line(&format!("if (!{matched}) {{"));
                            self.indent += 1;
                            self.line(&format!("{matched} = true;"));
                        }
                        hir::ResolvedMatchPattern::Record { .. } => {
                            return Err(backend_error(
                                "record pattern has a variant match scrutinee",
                            ));
                        }
                        hir::ResolvedMatchPattern::Literal(_)
                        | hir::ResolvedMatchPattern::Or(_)
                        | hir::ResolvedMatchPattern::Binding(_) => {
                            return Err(backend_error(
                                "refutable pattern has an aggregate variant match scrutinee",
                            ));
                        }
                    }
                    let value = self.emit_expr(&arm.value)?;
                    self.require_type(&value.ty, &expr.ty, "match arm result")?;
                    if matches!(expr.ty, ResolvedType::Bytes) {
                        let transitions = self
                            .bytes_plan
                            .expect("checked above")
                            .apply_at(&arm.value.id)?;
                        for line in transitions.lines() {
                            self.line(line);
                        }
                    } else {
                        self.line(&format!("{result} = {};", value.code));
                    }
                    self.variables = saved;
                    self.indent -= 1;
                    self.line("}");
                }
                self.line(&format!(
                    "if (!{matched}) spx_runtime_invariant_failure(\"exhaustive variant match selected no arm\");"
                ));
                CValue {
                    code: result,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } => {
                if !self.try_target_enabled {
                    return Err(backend_error(
                        "copy-result propagation is allowed only in a function body",
                    ));
                }
                self.require_type(
                    residual_type,
                    self.return_type,
                    "copy-result residual target",
                )?;
                let operand_layout = self.variant_layout(&operand.ty)?;
                let residual_layout = self.variant_layout(residual_type)?;
                if operand_layout.variant != *result || residual_layout.variant != *result {
                    return Err(backend_error(
                        "copy-result propagation does not reference its resolved Result declaration",
                    ));
                }
                let operand_ok = operand_layout
                    .case(ok_case)
                    .and_then(|case| case.field(ok_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result propagation has no resolved Ok payload")
                    })?;
                let operand_err = operand_layout
                    .case(err_case)
                    .and_then(|case| case.field(err_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result propagation has no resolved Err payload")
                    })?;
                let residual_err = residual_layout
                    .case(err_case)
                    .and_then(|case| case.field(err_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result residual has no resolved Err payload")
                    })?;
                self.require_type(&operand_ok.1.ty, &expr.ty, "copy-result Ok payload")?;
                self.require_type(
                    &operand_err.1.ty,
                    &residual_err.1.ty,
                    "copy-result Err payload",
                )?;

                let operand_value = self.emit_expr(operand)?;
                self.require_type(&operand_value.ty, &operand.ty, "copy-result operand")?;
                let operand_stage = self.temporary(&operand.ty)?;
                self.line(&format!("{operand_stage} = {};", operand_value.code));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid variant tag\");",
                    operand_layout.cases.len()
                ));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag == UINT32_C({})) {{",
                    operand_err.0.tag
                ));
                self.indent += 1;
                self.line("memset(&spx_result, 0, sizeof(spx_result));");
                self.line(&format!(
                    "spx_result.spx_payload.{}.{} = {operand_stage}.spx_payload.{}.{};",
                    c_case_symbol(err_case),
                    c_field_symbol(err_field),
                    c_case_symbol(err_case),
                    c_field_symbol(err_field),
                ));
                self.line(&format!(
                    "spx_result.spx_tag = UINT32_C({});",
                    residual_err.0.tag
                ));
                self.line("spx_result_staged = true;");
                self.line("goto spx_postconditions;");
                self.indent -= 1;
                self.line("}");
                self.line(&format!(
                    "if ({operand_stage}.spx_tag != UINT32_C({})) spx_runtime_invariant_failure(\"invalid Result tag\");",
                    operand_ok.0.tag
                ));
                let output = self.temporary(&expr.ty)?;
                self.line(&format!(
                    "{output} = {operand_stage}.spx_payload.{}.{};",
                    c_case_symbol(ok_case),
                    c_field_symbol(ok_field),
                ));
                CValue {
                    code: output,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } => {
                if !self.try_target_enabled {
                    return Err(backend_error(
                        "copy-Option propagation is allowed only in a function body",
                    ));
                }
                self.require_type(
                    residual_type,
                    self.return_type,
                    "copy-Option residual target",
                )?;
                let operand_layout = self.variant_layout(&operand.ty)?;
                let residual_layout = self.variant_layout(residual_type)?;
                if operand_layout.variant != *option || residual_layout.variant != *option {
                    return Err(backend_error(
                        "copy-Option propagation does not reference its resolved Option declaration",
                    ));
                }
                let operand_some = operand_layout
                    .case(some_case)
                    .and_then(|case| case.field(some_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-Option propagation has no resolved Some payload")
                    })?;
                let operand_none = operand_layout.case(none_case).ok_or_else(|| {
                    backend_error("copy-Option propagation has no resolved None case")
                })?;
                let residual_none = residual_layout.case(none_case).ok_or_else(|| {
                    backend_error("copy-Option residual has no resolved None case")
                })?;
                if !operand_none.fields.is_empty() || !residual_none.fields.is_empty() {
                    return Err(backend_error(
                        "copy-Option None case unexpectedly has a payload",
                    ));
                }
                self.require_type(&operand_some.1.ty, &expr.ty, "copy-Option Some payload")?;

                let operand_value = self.emit_expr(operand)?;
                self.require_type(&operand_value.ty, &operand.ty, "copy-Option operand")?;
                let operand_stage = self.temporary(&operand.ty)?;
                self.line(&format!("{operand_stage} = {};", operand_value.code));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid variant tag\");",
                    operand_layout.cases.len()
                ));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag == UINT32_C({})) {{",
                    operand_none.tag
                ));
                self.indent += 1;
                self.line("memset(&spx_result, 0, sizeof(spx_result));");
                self.line(&format!(
                    "spx_result.spx_tag = UINT32_C({});",
                    residual_none.tag
                ));
                self.line("spx_result_staged = true;");
                self.line("goto spx_postconditions;");
                self.indent -= 1;
                self.line("}");
                self.line(&format!(
                    "if ({operand_stage}.spx_tag != UINT32_C({})) spx_runtime_invariant_failure(\"invalid Option tag\");",
                    operand_some.0.tag
                ));
                let output = self.temporary(&expr.ty)?;
                self.line(&format!(
                    "{output} = {operand_stage}.spx_payload.{}.{};",
                    c_case_symbol(some_case),
                    c_field_symbol(some_field),
                ));
                CValue {
                    code: output,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::Project { base, field } => {
                let base = self.emit_expr(base)?;
                let layout = self.record_layout(&base.ty)?;
                let field = layout.field(field).cloned().ok_or_else(|| {
                    backend_error(format!(
                        "native record `{}` has no projected field `{field}`",
                        layout.record
                    ))
                })?;
                self.require_type(&expr.ty, &field.ty, "record projection")?;
                CValue {
                    code: if field.size == 0 {
                        "UINT8_C(0)".to_owned()
                    } else {
                        format!("({}).{}", base.code, c_field_symbol(&field.field))
                    },
                    ty: field.ty,
                }
            }
            ResolvedExprKind::Upcast { source } => {
                // Class Inheritance v1: the ancestor prefix moves
                // field-by-field from the consumed descendant value; the
                // canonical layouts guarantee identical offsets.
                let source = self.emit_expr(source)?;
                let target_layout = self.record_layout(&expr.ty)?;
                let source_layout = self.record_layout(&source.ty)?;
                if target_layout.record == source_layout.record {
                    return Err(backend_error(format!(
                        "native upcast `{}` requires a descendant source",
                        expr.ty.identity_key()
                    )));
                }
                for field in &target_layout.fields {
                    let source_field =
                        source_layout.field(&field.field).cloned().ok_or_else(|| {
                            backend_error(format!(
                                "native upcast source `{}` has no inherited field `{}`",
                                source_layout.record, field.field
                            ))
                        })?;
                    if (source_field.offset, source_field.size, source_field.align)
                        != (field.offset, field.size, field.align)
                    {
                        return Err(backend_error(format!(
                            "native upcast field `{}` disagrees with the ancestor prefix layout",
                            field.field
                        )));
                    }
                }
                let temporary = self.temporary(&expr.ty)?;
                if target_layout.fields.is_empty() {
                    self.line(&format!(
                        "{temporary}.spx_empty_record_padding = UINT8_C(0);"
                    ));
                }
                for field in &target_layout.fields {
                    if field.size != 0 {
                        self.line(&format!(
                            "{temporary}.{} = ({}).{};",
                            c_field_symbol(&field.field),
                            source.code,
                            c_field_symbol(&field.field)
                        ));
                    }
                }
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                let base = self.emit_expr(base)?;
                self.require_type(&base.ty, &expr.ty, "record update base")?;
                let layout = self.record_layout(&expr.ty)?;
                if layout.record != *record {
                    return Err(backend_error(format!(
                        "native record update `{record}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let temporary = self.temporary(&expr.ty)?;
                self.line(&format!("{temporary} = {};", base.code));
                for replacement in fields {
                    let field = layout.field(&replacement.field).cloned().ok_or_else(|| {
                        backend_error(format!(
                            "native record `{record}` has no update field `{}`",
                            replacement.field
                        ))
                    })?;
                    let value = self.emit_expr(&replacement.value)?;
                    self.require_type(&value.ty, &field.ty, "record update field")?;
                    if field.size != 0 {
                        self.line(&format!(
                            "{temporary}.{} = {};",
                            c_field_symbol(&field.field),
                            value.code
                        ));
                    }
                }
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_place(&self, place: &hir::Place) -> Result<CValue, Diagnostic> {
        let binding = self.variables.get(&place.root).cloned().ok_or_else(|| {
            backend_error(format!("resolved value `{}` is not in scope", place.root))
        })?;
        let mut code = binding.name;
        let mut ty = binding.ty;
        for projection in &place.projections {
            let PlaceProjection::Field(field) = projection else {
                return Err(backend_error(
                    "native variant-field projection is outside executable records v1",
                ));
            };
            let layout = self.record_layout(&ty)?;
            let field = layout.field(field).cloned().ok_or_else(|| {
                backend_error(format!(
                    "native record `{}` has no place field `{field}`",
                    layout.record
                ))
            })?;
            code = if field.size == 0 {
                "UINT8_C(0)".to_owned()
            } else {
                format!("({code}).{}", c_field_symbol(&field.field))
            };
            ty = field.ty;
        }
        Ok(CValue { code, ty })
    }

    fn record_layout(&self, ty: &ResolvedType) -> Result<AggregateLayout, Diagnostic> {
        record_declaration_id(self.program, ty)?.ok_or_else(|| {
            backend_error(format!(
                "native aggregate operation requires a record, found `{}`",
                ty.identity_key()
            ))
        })?;
        let layout = self.record_layouts.layout(ty)?.clone();
        layout.validate(self.program)?;
        Ok(layout)
    }

    fn variant_layout(&self, ty: &ResolvedType) -> Result<VariantLayout, Diagnostic> {
        variant_declaration_id(self.program, ty)?.ok_or_else(|| {
            backend_error(format!(
                "native variant operation requires a variant, found `{}`",
                ty.identity_key()
            ))
        })?;
        let layout = self.variant_layouts.layout(ty)?.clone();
        layout.validate(self.program)?;
        Ok(layout)
    }

    fn bind_record_match_pattern(
        &mut self,
        base: &str,
        expected: &ResolvedType,
        record: &DeclarationId,
        instance: &ResolvedType,
        fields: &[hir::ResolvedRecordMatchPatternField],
    ) -> Result<(), Diagnostic> {
        self.require_type(instance, expected, "record pattern instance")?;
        let layout = self.record_layout(expected)?;
        if layout.record != *record || fields.len() != layout.fields.len() {
            return Err(backend_error(
                "record pattern disagrees with its exact aggregate layout",
            ));
        }
        let mut seen = BTreeSet::new();
        for field in fields {
            let layout_field = layout.field(&field.field).cloned().ok_or_else(|| {
                backend_error(format!(
                    "record pattern `{record}` has unknown field `{}`",
                    field.field
                ))
            })?;
            if !seen.insert(field.field.clone()) {
                return Err(backend_error(format!(
                    "record pattern `{record}` repeats field `{}`",
                    field.field
                )));
            }
            let field_code = if layout_field.size == 0 {
                "UINT8_C(0)".to_owned()
            } else {
                format!("({base}).{}", c_field_symbol(&layout_field.field))
            };
            match &field.pattern {
                hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    self.require_type(&binding.ty, &layout_field.ty, "record pattern binding")?;
                    if binding.ownership != hir::OwnershipMode::Value
                        || self
                            .variables
                            .insert(
                                binding.id.clone(),
                                CBinding {
                                    name: field_code,
                                    ty: layout_field.ty,
                                },
                            )
                            .is_some()
                    {
                        return Err(backend_error(
                            "record pattern binding is not a fresh Copy value",
                        ));
                    }
                }
                hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
                hir::ResolvedRecordMatchFieldPattern::Record {
                    record,
                    instance,
                    fields,
                } => self.bind_record_match_pattern(
                    &field_code,
                    &layout_field.ty,
                    record,
                    instance,
                    fields,
                )?,
            }
        }
        Ok(())
    }

    fn emit_binary(
        &mut self,
        op: BinaryOp,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        let left = self.emit_expr(left)?;
        if matches!(op, BinaryOp::Eq | BinaryOp::Ne) && is_aggregate_type(self.program, &left.ty)? {
            return Err(backend_error(
                "aggregate equality is outside executable copy variants v1",
            ));
        }
        // Owned strings compare by UTF-8 contents; both operand buffers stay
        // owned by this expression and are freed right after the comparison.
        if matches!(op, BinaryOp::Eq | BinaryOp::Ne) && matches!(left.ty, ResolvedType::String) {
            let right = self.emit_expr(right)?;
            self.require_type(&right.ty, &ResolvedType::String, "binary right operand")?;
            self.require_type(result_type, &ResolvedType::Bool, "binary result")?;
            let temporary = self.temporary(&ResolvedType::Bool)?;
            let comparison = if op == BinaryOp::Eq {
                format!("spx_string_eq({}, {})", left.code, right.code)
            } else {
                format!("!spx_string_eq({}, {})", left.code, right.code)
            };
            self.line(&format!("{temporary} = {comparison};"));
            self.line(&format!("spx_string_drop({});", left.code));
            self.line(&format!("spx_string_drop({});", right.code));
            return Ok(CValue {
                code: temporary,
                ty: ResolvedType::Bool,
            });
        }
        if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) && matches!(left.ty, ResolvedType::String) {
            return Err(backend_error(
                "string operands only support equality comparison",
            ));
        }
        let float_operand = matches!(left.ty, ResolvedType::F32 | ResolvedType::F64);
        // Chars compare by Unicode scalar value; C unsigned comparison on
        // uint32_t matches the verified ordering exactly.
        let char_operand = matches!(left.ty, ResolvedType::Char);
        let int32_operand = matches!(left.ty, ResolvedType::I32);
        let narrow_operand = matches!(left.ty, ResolvedType::U8);
        let usize_operand = matches!(left.ty, ResolvedType::Usize);
        let operand_type = match op {
            BinaryOp::And | BinaryOp::Or => ResolvedType::Bool,
            BinaryOp::Eq | BinaryOp::Ne => left.ty.clone(),
            _ if float_operand
                || char_operand
                || int32_operand
                || narrow_operand
                || usize_operand =>
            {
                left.ty.clone()
            }
            _ => ResolvedType::I64,
        };
        self.require_type(&left.ty, &operand_type, "binary left operand")?;
        let expected_result = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                if float_operand || int32_operand =>
            {
                left.ty.clone()
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                if narrow_operand =>
            {
                ResolvedType::U8
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                if usize_operand =>
            {
                ResolvedType::Usize
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                ResolvedType::I64
            }
            _ => ResolvedType::Bool,
        };
        self.require_type(result_type, &expected_result, "binary result")?;
        if float_operand && op == BinaryOp::Rem {
            return Err(backend_error(
                "floating-point remainder has no admitted native lowering",
            ));
        }
        if narrow_operand && op == BinaryOp::Rem {
            return Err(backend_error(
                "u8 remainder has no admitted native lowering",
            ));
        }
        if char_operand
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            )
        {
            return Err(backend_error(
                "char arithmetic has no admitted native lowering",
            ));
        }
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            let temporary = self.temporary(&ResolvedType::Bool)?;
            if op == BinaryOp::And {
                self.line(&format!("if ({}) {{", left.code));
                self.indent += 1;
                let right = self.emit_expr(right)?;
                self.require_type(&right.ty, &ResolvedType::Bool, "lazy right operand")?;
                self.line(&format!("{temporary} = {};", right.code));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                self.line(&format!("{temporary} = false;"));
            } else {
                self.line(&format!("if ({}) {{", left.code));
                self.indent += 1;
                self.line(&format!("{temporary} = true;"));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                let right = self.emit_expr(right)?;
                self.require_type(&right.ty, &ResolvedType::Bool, "lazy right operand")?;
                self.line(&format!("{temporary} = {};", right.code));
            }
            self.indent -= 1;
            self.line("}");
            return Ok(CValue {
                code: temporary,
                ty: ResolvedType::Bool,
            });
        }
        let right = self.emit_expr(right)?;
        self.require_type(&right.ty, &operand_type, "binary right operand")?;
        let temporary = self.temporary(&expected_result)?;
        if float_operand
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
            )
        {
            // IEEE-754 semantics are total: overflow, signed zero, and
            // division by zero follow the hardware rules and never select a
            // failure status.
            self.line(&format!(
                "{temporary} = ({} {} {});",
                left.code,
                op.text(),
                right.code
            ));
        } else if narrow_operand
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
            )
        {
            // Checked u8 arithmetic computes in int64_t and range-checks the
            // 0..=255 result before narrowing to the uint8_t temporary.
            let helper = match op {
                BinaryOp::Add => "spx_rt_u8_add",
                BinaryOp::Sub => "spx_rt_u8_sub",
                BinaryOp::Mul => "spx_rt_u8_mul",
                BinaryOp::Div => "spx_rt_u8_div",
                _ => unreachable!("u8 arithmetic operation was matched above"),
            };
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {}, {}, &{temporary});",
                left.code, right.code
            ));
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        } else if usize_operand
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            )
        {
            let helper = match op {
                BinaryOp::Add => "spx_rt_usize_add",
                BinaryOp::Sub => "spx_rt_usize_sub",
                BinaryOp::Mul => "spx_rt_usize_mul",
                BinaryOp::Div => "spx_rt_usize_div",
                BinaryOp::Rem => "spx_rt_usize_rem",
                _ => unreachable!("usize arithmetic operation was matched above"),
            };
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {}, {}, &{temporary});",
                left.code, right.code
            ));
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        } else if matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
        ) {
            let helper = match op {
                BinaryOp::Add if int32_operand => "spx_rt_add_i32",
                BinaryOp::Sub if int32_operand => "spx_rt_sub_i32",
                BinaryOp::Mul if int32_operand => "spx_rt_mul_i32",
                BinaryOp::Div if int32_operand => "spx_rt_div_i32",
                BinaryOp::Rem if int32_operand => "spx_rt_rem_i32",
                BinaryOp::Add => "spx_rt_add",
                BinaryOp::Sub => "spx_rt_sub",
                BinaryOp::Mul => "spx_rt_mul",
                BinaryOp::Div => "spx_rt_div",
                BinaryOp::Rem => "spx_rt_rem",
                _ => unreachable!("checked arithmetic operation was matched above"),
            };
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {}, {}, &{temporary});",
                left.code, right.code
            ));
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        } else {
            self.line(&format!(
                "{temporary} = ({} {} {});",
                left.code,
                op.text(),
                right.code
            ));
        }
        Ok(CValue {
            code: temporary,
            ty: expected_result,
        })
    }
}

pub(super) fn c_string(value: &str) -> String {
    let mut escaped = crate::bounded_output::CappedString::new();
    for byte in value.as_bytes() {
        match *byte {
            b'\\' => escaped.push_str("\\\\"),
            b'"' => escaped.push_str("\\\""),
            b'\n' => escaped.push_str("\\n"),
            b'\r' => escaped.push_str("\\r"),
            b'\t' => escaped.push_str("\\t"),
            b'?' | 0x00..=0x1f | 0x7f..=0xff => {
                write!(escaped, "\\{byte:03o}").expect("writing to a string cannot fail");
            }
            value => escaped.push(char::from(value)),
        }
    }
    escaped.into_string()
}
