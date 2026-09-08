//! Inline consuming scalar iterator carriers. They reuse the authenticated
//! bounded-Vec runtime and never allocate an iterator wrapper.

pub(super) fn emit_runtime(output: &mut impl super::COutput) {
    use super::native_emit::{c_case_symbol, c_field_symbol};
    use crate::hir::DeclarationId;
    output.push_str(
        &RUNTIME_C
            .replace(
                "ITER_CASE",
                &c_case_symbol(&DeclarationId::new(crate::iterator_ops::YIELD_ID)),
            )
            .replace(
                "ITER_ITEM",
                &c_field_symbol(&DeclarationId::new(crate::iterator_ops::ITEM_ID)),
            )
            .replace(
                "ITER_REST",
                &c_field_symbol(&DeclarationId::new(crate::iterator_ops::REST_ID)),
            ),
    );
}

pub(super) fn program_uses_iterator(program: &crate::hir::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            let mut found = crate::iterator_ops::element(&function.return_type).is_some()
                || function
                    .params
                    .iter()
                    .any(|p| crate::iterator_ops::element(&p.ty).is_some());
            let mut pending = vec![&function.body];
            pending.extend(&function.requires);
            pending.extend(&function.ensures);
            while let Some(expression) = pending.pop() {
                found |= crate::iterator_ops::element(&expression.ty).is_some();
                crate::hir::push_resolved_expression_children_in_authored_order(
                    expression,
                    &mut pending,
                );
            }
            found
        })
}

pub(super) fn c_type(ty: &crate::hir::ResolvedType) -> Option<&'static str> {
    if crate::iterator_ops::is_iter(ty) {
        Some("spx_iter_v1")
    } else if crate::iterator_ops::is_step(ty) {
        Some("spx_iter_step_v1")
    } else {
        None
    }
}

const RUNTIME_C: &str = r#"typedef struct {
    spx_vec_v1 vec;
    uint64_t cursor;
} spx_iter_v1;

typedef struct {
    uint32_t spx_tag;
    uint32_t reserved;
    union { struct { uint64_t ITER_ITEM; spx_iter_v1 ITER_REST; } ITER_CASE; } spx_payload;
} spx_iter_step_v1;

static __attribute__((unused)) spx_iter_v1 spx_iter_from_vec(struct spx_context *spx_ctx, spx_vec_v1 *source, uint32_t tag) {
    if (source == NULL) spx_runtime_invariant_failure("invalid iterator Vec source");
    (void)spx_vec_require_valid(spx_ctx, source, tag);
    spx_iter_v1 result = { .vec = spx_vec_move(spx_ctx, source), .cursor = UINT64_C(0) }; return result;
}
static __attribute__((unused)) uint32_t spx_iter_next(struct spx_context *spx_ctx, spx_iter_v1 *source, uint32_t tag, spx_iter_step_v1 *out) {
    if (source == NULL || out == NULL) spx_runtime_invariant_failure("invalid iterator source or output");
    (void)spx_vec_require_valid(spx_ctx, &source->vec, tag);
    spx_iter_step_v1 result = {0};
    if (source->cursor == source->vec.len) { spx_vec_drop(spx_ctx, &source->vec); *source = (spx_iter_v1){0}; *out=result; return SPX_STATUS_SUCCESS; }
    uint32_t status=spx_vec_get(spx_ctx, &source->vec, tag, source->cursor, &result.spx_payload.ITER_CASE.ITER_ITEM);
    if (status != SPX_STATUS_SUCCESS) return status;
    result.spx_tag = UINT32_C(1);
    result.spx_payload.ITER_CASE.ITER_REST.vec = spx_vec_move(spx_ctx, &source->vec);
    result.spx_payload.ITER_CASE.ITER_REST.cursor = source->cursor + UINT64_C(1);
    *source = (spx_iter_v1){0}; *out=result; return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_iter_v1 spx_iter_move(struct spx_context *spx_ctx, spx_iter_v1 *source) {
    if (source == NULL) spx_runtime_invariant_failure("invalid iterator move");
    spx_iter_v1 result={.vec=spx_vec_move(spx_ctx,&source->vec),.cursor=source->cursor}; *source=(spx_iter_v1){0}; return result;
}
static __attribute__((unused)) void spx_iter_drop(struct spx_context *spx_ctx, spx_iter_v1 *value) {
    if (value == NULL) spx_runtime_invariant_failure("invalid iterator drop");
    if (value->vec.authority != UINT32_C(0)) spx_vec_drop(spx_ctx, &value->vec); *value = (spx_iter_v1){0};
}
"#;

pub(super) fn item_read(carrier: &str, ty: &crate::hir::ResolvedType) -> String {
    use crate::hir::{DeclarationId, ResolvedType};
    let bits = format!(
        "({carrier}).spx_payload.{}.{}",
        super::native_emit::c_case_symbol(&DeclarationId::new(crate::iterator_ops::YIELD_ID)),
        super::native_emit::c_field_symbol(&DeclarationId::new(crate::iterator_ops::ITEM_ID))
    );
    match ty {
        ResolvedType::I64 => format!("((int64_t){bits})"),
        ResolvedType::I32 => format!("((int32_t){bits})"),
        ResolvedType::U8 => format!("((uint8_t){bits})"),
        ResolvedType::Usize => bits,
        ResolvedType::Char => format!("((uint32_t){bits})"),
        ResolvedType::F32 => format!("spx_vec_bits_f32({bits})"),
        ResolvedType::F64 => format!("spx_vec_bits_f64({bits})"),
        ResolvedType::Bool => format!("((bool){bits})"),
        _ => unreachable!("checked iterator scalar"),
    }
}

pub(super) fn item_bits(code: &str, ty: &crate::hir::ResolvedType) -> String {
    match ty {
        crate::hir::ResolvedType::F32 => format!("spx_vec_f32_bits({code})"),
        crate::hir::ResolvedType::F64 => format!("spx_vec_f64_bits({code})"),
        _ => format!("((uint64_t)({code}))"),
    }
}
