/* Applied only before the sealed source functions. Public runtime ABI above
   remains ordinary; no public or imported pointer may cross these bindings. */
static __attribute__((unused)) spx_shb_bytes spx_partition_zeroed(struct spx_context *spx_ctx, uint64_t count) {
    spx_shb_bytes value = {0};
    spx_shb_zeroed(spx_shb_held(spx_ctx), count, &value);
    return value;
}
#define spx_bytes_v1 spx_shb_bytes
static __attribute__((unused)) spx_shb_bytes spx_partition_copy(
    struct spx_context *spx_ctx, spx_slice_u8_v1 view
) {
    spx_shb_bytes value = {0};
    spx_shb_copy(spx_shb_held(spx_ctx), view, &value);
    return value;
}
#define spx_vec_v1 spx_shb_vec
#define spx_bytes_move(value) spx_shb_move(spx_shb_held(spx_ctx), (value))
#define spx_bytes_drop(value) spx_shb_drop(spx_shb_held(spx_ctx), (value))
#define spx_vec_move(context, value) spx_shb_vec_move(spx_shb_held(context), (value))
#define spx_vec_drop(context, value) spx_shb_vec_drop(spx_shb_held(context), (value))
#define spx_vec_len(context, value, tag) ((void)(tag), spx_shb_vec_len(spx_shb_held(context), (value)))
#define spx_vec_capacity(context, value, tag) ((void)(tag), spx_shb_vec_capacity(spx_shb_held(context), (value)))
#define spx_leaf_check(context, value, descriptor) ((void)(descriptor), spx_shb_vec_valid(spx_shb_held(context), (value)))
#define spx_bytes_as_slice(value) spx_shb_view(spx_shb_held(spx_ctx), (value))
