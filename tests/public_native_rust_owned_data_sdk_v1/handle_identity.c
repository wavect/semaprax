static int issue(spx_context_v1 *context, uint8_t byte, uint64_t *handle) {
    uint32_t tag = UINT32_MAX;
    int64_t error = INT64_MAX;
    spx_owned_data_status_v1 status = spx_owned_data_call_spx_frame_dot_payload_v1(
        context, &byte, UINT64_C(1), &tag, handle, &error);
    return status == SPX_OWNED_DATA_SUCCESS && tag == UINT32_C(0)
        && *handle != UINT64_C(0) && error == INT64_C(0) ? 0 : 1;
}

static int expect_byte(spx_context_v1 *context, uint64_t handle, uint8_t expected) {
    uint64_t length = UINT64_MAX;
    uint8_t byte = UINT8_C(0);
    return spx_owned_bytes_len_v1(context, handle, &length) == SPX_OWNED_DATA_SUCCESS
        && length == UINT64_C(1)
        && spx_owned_bytes_copy_v1(context, handle, &byte, UINT64_C(1)) == SPX_OWNED_DATA_SUCCESS
        && byte == expected ? 0 : 1;
}

static int reject_without_mutation(spx_context_v1 *context, uint64_t handle) {
    uint64_t length = UINT64_MAX;
    uint8_t byte = UINT8_C(91);
    uint32_t live = context->live_slots;
    uint32_t next = context->next_slot;
    if (spx_owned_bytes_len_v1(context, handle, &length) != SPX_OWNED_DATA_INVALID_HANDLE
        || length != UINT64_MAX) return 1;
    if (spx_owned_bytes_copy_v1(context, handle, &byte, UINT64_C(1)) != SPX_OWNED_DATA_INVALID_HANDLE
        || byte != UINT8_C(91)) return 2;
    if (spx_owned_bytes_drop_v1(context, handle) != SPX_OWNED_DATA_INVALID_HANDLE
        || context->live_slots != live || context->next_slot != next) return 3;
    return 0;
}

int main(void) {
    uint64_t size = spx_owned_data_context_size_v1();
    spx_context_v1 *first = malloc((size_t)size);
    spx_context_v1 *second = malloc((size_t)size);
    if (first == NULL || second == NULL) return 10;
    if (spx_owned_data_context_init_v1(first, size) != 0
        || spx_owned_data_context_init_v1(second, size) != 0) return 11;
    uint64_t first_handle = UINT64_C(0), second_handle = UINT64_C(0);
    if (issue(first, UINT8_C(17), &first_handle)
        || issue(second, UINT8_C(29), &second_handle)) return 12;
    if (first_handle == second_handle) return 13;
    if (reject_without_mutation(first, second_handle)
        || reject_without_mutation(second, first_handle)) return 14;
    if (expect_byte(first, first_handle, UINT8_C(17))
        || expect_byte(second, second_handle, UINT8_C(29))) return 15;
    if (spx_owned_bytes_drop_v1(first, first_handle) != 0
        || spx_owned_data_context_drop_v1(first) != 0) return 16;

    // Reuse precisely the same allocation, not merely another fresh context.
    if (spx_owned_data_context_init_v1(first, size) != 0) return 17;
    uint64_t reincarnated = UINT64_C(0);
    if (issue(first, UINT8_C(43), &reincarnated) || reincarnated == first_handle) return 18;
    if (reject_without_mutation(first, first_handle)
        || reject_without_mutation(first, second_handle)
        || reject_without_mutation(second, reincarnated)) return 19;
    if (expect_byte(first, reincarnated, UINT8_C(43))
        || expect_byte(second, second_handle, UINT8_C(29))) return 20;
    if (spx_owned_bytes_drop_v1(first, reincarnated) != 0
        || spx_owned_bytes_drop_v1(second, second_handle) != 0) return 21;
    if (reject_without_mutation(first, reincarnated)) return 22;

    uint64_t handles[4096] = {0};
    for (uint32_t index = 0; index < UINT32_C(4096); ++index) {
        if (issue(first, (uint8_t)index, &handles[index])) return 23;
        if ((handles[index] & UINT64_C(0x1fff)) != (uint64_t)index + UINT64_C(1)) return 24;
        if (index == UINT32_C(4094) && first->live_slots != UINT32_C(4095)) return 25;
    }
    if (first->live_slots != UINT32_C(4096) || first->next_slot != UINT32_C(4096)) return 26;
    uint64_t before_serial = atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed);
    uint32_t tag = UINT32_C(77);
    uint64_t handle = UINT64_C(0);
    int64_t error = INT64_C(88);
    uint8_t input = UINT8_C(9);
    if (spx_owned_data_call_spx_frame_dot_payload_v1(first, &input, UINT64_C(1),
            &tag, &handle, &error) != SPX_OWNED_DATA_ADAPTER_FAILURE) return 27;
    if (tag != UINT32_C(77) || handle != UINT64_C(0) || error != INT64_C(88)
        || first->live_slots != UINT32_C(4096) || first->next_slot != UINT32_C(4096)
        || atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed) != before_serial) return 28;
    uint64_t serial_bits = handles[0] & ~UINT64_C(0x1fff);
    if (reject_without_mutation(first, serial_bits)
        || reject_without_mutation(first, serial_bits | UINT64_C(4097))
        || reject_without_mutation(first, serial_bits | UINT64_C(8191))) return 29;
    for (uint32_t index = 0; index < UINT32_C(4096); ++index) {
        if (expect_byte(first, handles[index], (uint8_t)index)) return 30;
        if (spx_owned_bytes_drop_v1(first, handles[index]) != 0) return 31;
        if (reject_without_mutation(first, handles[index])) return 32;
    }
    if (spx_owned_data_context_drop_v1(first) != 0
        || spx_owned_data_context_drop_v1(second) != 0) return 33;
    free(first);
    free(second);
    return 0;
}
