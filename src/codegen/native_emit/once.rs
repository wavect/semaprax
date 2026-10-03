//! Native affine callable carrier. Physical ownership follows NativeBytesPlan.
use super::*;

pub(super) fn declarations(output: &mut impl COutput) {
    output.push_str("typedef spx_status_token (*spx_once_entry)(struct spx_context *, spx_bytes_v1, int64_t *);\n");
    output
        .push_str("typedef struct { spx_once_entry entry; spx_bytes_v1 capture; } spx_once_v1;\n");
    output.push_str("static __attribute__((unused)) spx_once_v1 spx_once_move(spx_once_v1 *source) { if (!source->entry) spx_runtime_invariant_failure(\"dead affine callable\"); spx_once_v1 value = *source; *source = (spx_once_v1){0}; return value; }\n");
    output.push_str("static __attribute__((unused)) void spx_once_drop(spx_once_v1 *value) { if (!value->entry) spx_runtime_invariant_failure(\"dead affine callable drop\"); value->entry = NULL; spx_bytes_drop(&value->capture); }\n");
}

pub(super) fn signature(output: &mut impl COutput, symbol: &str) {
    write!(output, "static spx_status_token {symbol}(struct spx_context *spx_ctx, spx_bytes_v1 spx_capture, int64_t *spx_result_out)").expect("string write");
}

pub(super) fn thunk(
    output: &mut impl COutput,
    expression: &ResolvedExpr,
    emission: &NativeEmissionContext<'_>,
) -> Result<(), Diagnostic> {
    let target = emission
        .functions
        .get(&FunctionExecutionId::Monomorphic(hir::closure::closure_id(
            &expression.id,
        )))
        .ok_or_else(|| backend_error("affine body is absent from checked function index"))?;
    signature(output, &closure::thunk_symbol(&expression.id));
    writeln!(
        output,
        " {{ return {}(spx_ctx, spx_capture, spx_result_out); }}",
        target.symbol
    )
    .expect("string write");
    Ok(())
}

pub(super) fn construct<O: COutput>(
    emitter: &mut CEmitter<'_, O>,
    expression: &ResolvedExpr,
) -> Result<CValue, Diagnostic> {
    hir::closure::once::validate(emitter.program, expression)?;
    let (_, args) = hir::closure::once::call(expression)
        .ok_or_else(|| backend_error("affine constructor boundary missing"))?;
    let capture = emitter.emit_expr(&args[0])?;
    let capture = emitter.stage_bytes_call_argument(
        &expression.id,
        0,
        &args[0],
        hir::OwnershipMode::Own,
        capture,
    )?;
    let plan = emitter
        .bytes_plan
        .ok_or_else(|| backend_error("affine constructor lacks canonical plan"))?;
    let (argument, flag, _) = plan.call_argument(&expression.id, 0)?;
    if capture.code != argument {
        return Err(backend_error("affine capture not staged"));
    }
    let destination = plan
        .value(&crate::cleanup_plan::StorageId::Temporary(
            expression.id.clone(),
        ))?
        .to_owned();
    emitter.line(&format!(
        "{destination}.entry = {};",
        closure::thunk_symbol(&expression.id)
    ));
    emitter.line(&format!(
        "{destination}.capture = spx_bytes_move(&{argument}); {flag} = false;"
    ));
    // This bounded inline construction cannot fail after the authenticated commit.
    let value = CValue {
        code: destination,
        ty: ResolvedType::OnceFunction,
    };
    emitter.apply_owned_plan_at_value(&expression.id, &value)?;
    Ok(CValue {
        code: plan
            .result_at(&expression.id)
            .ok_or_else(|| backend_error("affine construction result missing"))?
            .to_owned(),
        ty: value.ty,
    })
}

pub(super) fn invoke<O: COutput>(
    emitter: &mut CEmitter<'_, O>,
    expression: &ResolvedExpr,
    callable: &ResolvedExpr,
) -> Result<CValue, Diagnostic> {
    hir::function_value::validate_invocation(expression)?;
    let value = emitter.emit_expr(callable)?;
    let value = emitter.stage_bytes_call_argument(
        &expression.id,
        0,
        callable,
        hir::OwnershipMode::Own,
        value,
    )?;
    let plan = emitter
        .bytes_plan
        .ok_or_else(|| backend_error("affine invoke lacks canonical plan"))?;
    let (argument, flag, _) = plan.call_argument(&expression.id, 0)?;
    if value.code != argument {
        return Err(backend_error("affine receiver not staged"));
    }
    let receiver = emitter.temporary(&ResolvedType::OnceFunction)?;
    let result = emitter.call_result_temporary(&ResolvedType::I64)?;
    emitter.line(&format!(
        "{receiver} = spx_once_move(&{argument}); {flag} = false;"
    ));
    emitter.line(&format!(
        "spx_status = {receiver}.entry(spx_ctx, spx_bytes_move(&{receiver}.capture), &{result});"
    ));
    emitter.line(&format!("{receiver}.entry = NULL;"));
    emitter.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
    Ok(CValue {
        code: result,
        ty: ResolvedType::I64,
    })
}
