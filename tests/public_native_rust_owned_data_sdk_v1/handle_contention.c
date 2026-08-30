static bool inject_interleaving = false;
static uint64_t interleaving_count = UINT64_C(0);

static uint64_t spx_test_serial_snapshot(void) {
    uint64_t snapshot = atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed);
    if (inject_interleaving) {
        inject_interleaving = false;
        uint64_t expected = snapshot;
        if (!atomic_compare_exchange_strong_explicit(&spx_owned_data_next_serial_v1,
                &expected, snapshot + UINT64_C(1), memory_order_relaxed, memory_order_relaxed)) abort();
        ++interleaving_count;
    }
    return snapshot;
}

static int rejected_call_preserves_output(spx_context_v1 *context) {
    uint8_t input = UINT8_C(61);
    uint32_t tag = UINT32_C(77);
    uint64_t handle = UINT64_C(0);
    int64_t error = INT64_C(88);
    uint32_t next = context->next_slot;
    uint32_t live = context->live_slots;
    if (spx_owned_data_call_spx_frame_dot_payload_v1(context, &input, UINT64_C(1),
            &tag, &handle, &error) != SPX_OWNED_DATA_ADAPTER_FAILURE) return 1;
    if (tag != UINT32_C(77) || handle != UINT64_C(0) || error != INT64_C(88)
        || context->next_slot != next || context->live_slots != live) return 2;
    return 0;
}

int main(void) {
    uint64_t size = spx_owned_data_context_size_v1();
    spx_context_v1 *context = malloc((size_t)size);
    if (context == NULL || spx_owned_data_context_init_v1(context, size) != 0) return 10;
    uint8_t input = UINT8_C(19);
    spx_slice_u8_v1 view = { .ptr = &input, .len = UINT64_C(1) };
    spx_bytes_v1 bytes = spx_bytes_copy(view);
    uint64_t handle = UINT64_C(0);
    uint64_t before = atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed);
    inject_interleaving = true;
    if (spx_owned_data_attach_v1(context, &bytes, &handle) != SPX_OWNED_DATA_ADAPTER_FAILURE) return 11;
    if (handle != UINT64_C(0) || context->next_slot != UINT32_C(0)
        || context->live_slots != UINT32_C(0) || interleaving_count != UINT64_C(1)) return 12;
    if (atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed) != before + UINT64_C(1)) return 13;
    // The failed reservation did not move or destroy its caller-owned bytes.
    spx_bytes_require_valid(bytes);
    if (bytes.len != UINT64_C(1) || bytes.ptr[0] != input) return 14;
    spx_bytes_drop(&bytes);

    // Exercise the same lost-CAS path through the real exported call. An
    // accidental retry would succeed and publish, failing this assertion.
    inject_interleaving = true;
    if (rejected_call_preserves_output(context) || interleaving_count != UINT64_C(2)) return 15;
    if (spx_owned_data_context_drop_v1(context) != 0
        || spx_owned_data_context_init_v1(context, size) != 0) return 16;

    // Exact issuance boundary; only this private test TU can position it.
    atomic_store_explicit(&spx_owned_data_next_serial_v1, SPX_OWNED_DATA_MAX_SERIAL_V1, memory_order_relaxed);
    uint32_t tag = UINT32_MAX;
    int64_t error = INT64_MAX;
    handle = UINT64_C(0);
    if (spx_owned_data_call_spx_frame_dot_payload_v1(context, &input, UINT64_C(1),
            &tag, &handle, &error) != SPX_OWNED_DATA_SUCCESS) return 17;
    if (tag != UINT32_C(0) || error != INT64_C(0)
        || (handle >> UINT32_C(13)) != SPX_OWNED_DATA_MAX_SERIAL_V1) return 18;
    uint64_t exhausted = SPX_OWNED_DATA_MAX_SERIAL_V1 + UINT64_C(1);
    if (atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed) != exhausted) return 19;
    if (rejected_call_preserves_output(context)) return 20;
    uint64_t length = UINT64_MAX;
    uint8_t copied = UINT8_C(0);
    if (spx_owned_bytes_len_v1(context, handle, &length) != 0 || length != UINT64_C(1)
        || spx_owned_bytes_copy_v1(context, handle, &copied, length) != 0 || copied != input
        || spx_owned_bytes_drop_v1(context, handle) != 0) return 21;
    if (spx_owned_data_context_drop_v1(context) != 0
        || spx_owned_data_context_init_v1(context, size) != 0) return 22;
    if (rejected_call_preserves_output(context)
        || atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed) != exhausted) return 23;
    if (spx_owned_data_context_drop_v1(context) != 0) return 24;
    free(context);
    return 0;
}
