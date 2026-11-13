/* Source-only IterStep layout over the selected shared owners. */
typedef struct {
    uint32_t spx_tag;
    uint32_t reserved;
    union {
        struct { spx_shb_bytes ITER_ITEM; spx_shb_iter ITER_REST; } ITER_CASE;
    } spx_payload;
} spx_partition_iter_step;

#if UINTPTR_MAX == UINT64_MAX
_Static_assert(sizeof(spx_partition_iter_step) == 88, "native64 shared Bytes IterStep size");
_Static_assert(_Alignof(spx_partition_iter_step) == 8, "native64 shared Bytes IterStep alignment");
_Static_assert(offsetof(spx_partition_iter_step, spx_tag) == 0, "native64 shared Bytes IterStep tag");
_Static_assert(offsetof(spx_partition_iter_step, spx_payload.ITER_CASE.ITER_ITEM) == 8,
    "native64 shared Bytes IterStep item offset");
_Static_assert(offsetof(spx_partition_iter_step, spx_payload.ITER_CASE.ITER_REST) == 32,
    "native64 shared Bytes IterStep rest offset");
#endif

static __attribute__((unused)) spx_shb_iter spx_partition_iter_move(
    struct spx_context *context, spx_shb_iter *source
) {
    (void)spx_shb_iter_valid(spx_shb_held(context), source);
    spx_shb_iter moved = *source;
    *source = (spx_shb_iter){0};
    return moved;
}

static __attribute__((unused)) uint32_t spx_partition_iter_next(
    struct spx_context *context, spx_shb_iter *source, spx_partition_iter_step *out
) {
    if (out == NULL) spx_runtime_invariant_failure("partition iterator result is absent");
    spx_partition_iter_step staged = {0};
    spx_shb_iter_next(spx_shb_held(context), source, &staged.spx_tag,
        &staged.spx_payload.ITER_CASE.ITER_ITEM, &staged.spx_payload.ITER_CASE.ITER_REST);
    *out = staged;
    return SPX_STATUS_SUCCESS;
}

#define spx_iter_v1 spx_shb_iter
#define spx_iter_bytes_step_v2 spx_partition_iter_step
#define spx_iter_bytes_from_vec(context, source) spx_shb_iter_from_vec(spx_shb_held(context), (source))
#define spx_iter_bytes_next spx_partition_iter_next
#define spx_iter_move spx_partition_iter_move
#define spx_iter_drop(context, value) spx_shb_iter_drop(spx_shb_held(context), (value))
