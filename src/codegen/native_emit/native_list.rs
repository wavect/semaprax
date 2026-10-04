//! Invocation-scoped persistent cons cells for the source `List<i64>` profile.
//!
//! A cons allocates one node and never mutates its tail. All nodes allocated by
//! one root call remain live until that call returns, so a copied list value is
//! only a pointer copy and a destructured tail cannot dangle.

use super::COutput;
use crate::hir::ResolvedProgram;

pub(super) const RUNTIME_C: &str = r#"
typedef struct spx_list_node *spx_list_v1;
#define SPX_LIST_MAX_LENGTH UINT64_C(8192)

static __attribute__((unused)) spx_status_token spx_list_failure(
    struct spx_context *ctx
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(ctx, "semaprax.runtime.v1", UINT32_C(2),
        SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token)) {
        spx_runtime_invariant_failure("list failure status arena exhaustion");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_list_cons(
    struct spx_context *ctx, int64_t head, spx_list_v1 tail, spx_list_v1 *out
) {
    if (tail != NULL && tail->length >= SPX_LIST_MAX_LENGTH)
        return spx_list_failure(ctx);
    struct spx_list_node *node = (struct spx_list_node *)malloc(sizeof(*node));
    if (node == NULL) return spx_list_failure(ctx);
    node->head = head;
    node->length = tail == NULL ? UINT64_C(1) : tail->length + UINT64_C(1);
    node->tail = tail;
    node->allocated_next = ctx->list_nodes;
    ctx->list_nodes = node;
    *out = node;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) void spx_list_release(struct spx_context *ctx) {
    struct spx_list_node *node = ctx->list_nodes;
    ctx->list_nodes = NULL;
    while (node != NULL) {
        struct spx_list_node *next = node->allocated_next;
        free(node);
        node = next;
    }
}
"#;

pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram) {
    if crate::list_ops::resolved_program_uses_list(program) {
        output.push_str(RUNTIME_C);
    }
}
