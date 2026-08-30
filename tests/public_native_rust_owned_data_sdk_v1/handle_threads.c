#if defined(_WIN32)
#include <windows.h>
#else
#include <pthread.h>
#endif

enum { TEST_THREADS = 8, TEST_CALLS = 64 };
struct thread_result {
    uint64_t handles[TEST_CALLS];
    uint32_t succeeded;
    uint32_t rejected;
    int failed;
};
static _Atomic(uint32_t) test_ready = UINT32_C(0);
static _Atomic(bool) test_start = false;

static void exercise_context(struct thread_result *result) {
    uint64_t size = spx_owned_data_context_size_v1();
    spx_context_v1 *context = malloc((size_t)size);
    bool initialized = context != NULL && spx_owned_data_context_init_v1(context, size) == 0;
    atomic_fetch_add_explicit(&test_ready, UINT32_C(1), memory_order_release);
    while (!atomic_load_explicit(&test_start, memory_order_acquire)) { }
    if (!initialized) { result->failed = 1; free(context); return; }
    for (uint32_t call = 0; call < TEST_CALLS; ++call) {
        uint8_t input = (uint8_t)call;
        uint32_t tag = UINT32_C(77);
        uint64_t handle = UINT64_C(0);
        int64_t error = INT64_C(88);
        spx_owned_data_status_v1 status = spx_owned_data_call_spx_frame_dot_payload_v1(
            context, &input, UINT64_C(1), &tag, &handle, &error);
        if (status == SPX_OWNED_DATA_ADAPTER_FAILURE) {
            if (tag != UINT32_C(77) || handle != UINT64_C(0) || error != INT64_C(88)
                || context->live_slots != UINT32_C(0)) { result->failed = 2; break; }
            ++result->rejected;
            continue; // A subsequent call is a new invocation, not a retry.
        }
        if (status != SPX_OWNED_DATA_SUCCESS || tag != UINT32_C(0)
            || handle == UINT64_C(0) || error != INT64_C(0)) { result->failed = 3; break; }
        uint64_t length = UINT64_MAX;
        uint8_t copied = UINT8_C(0);
        if (spx_owned_bytes_len_v1(context, handle, &length) != 0 || length != UINT64_C(1)
            || spx_owned_bytes_copy_v1(context, handle, &copied, length) != 0 || copied != input
            || spx_owned_bytes_drop_v1(context, handle) != 0) { result->failed = 4; break; }
        result->handles[result->succeeded++] = handle;
    }
    if (spx_owned_data_context_drop_v1(context) != 0) result->failed = 5;
    free(context);
}

#if defined(_WIN32)
static DWORD WINAPI worker(LPVOID input) { exercise_context(input); return 0; }
#else
static void *worker(void *input) { exercise_context(input); return NULL; }
#endif

int main(void) {
    struct thread_result results[TEST_THREADS] = {0};
#if defined(_WIN32)
    HANDLE threads[TEST_THREADS];
#else
    pthread_t threads[TEST_THREADS];
#endif
    for (uint32_t index = 0; index < TEST_THREADS; ++index) {
#if defined(_WIN32)
        threads[index] = CreateThread(NULL, 0, worker, &results[index], 0, NULL);
        if (threads[index] == NULL) return 10;
#else
        if (pthread_create(&threads[index], NULL, worker, &results[index]) != 0) return 10;
#endif
    }
    while (atomic_load_explicit(&test_ready, memory_order_acquire) != TEST_THREADS) { }
    atomic_store_explicit(&test_start, true, memory_order_release);
    for (uint32_t index = 0; index < TEST_THREADS; ++index) {
#if defined(_WIN32)
        if (WaitForSingleObject(threads[index], INFINITE) != WAIT_OBJECT_0
            || !CloseHandle(threads[index])) return 11;
#else
        if (pthread_join(threads[index], NULL) != 0) return 11;
#endif
    }
    uint32_t total = UINT32_C(0);
    for (uint32_t index = 0; index < TEST_THREADS; ++index) {
        if (results[index].failed || results[index].succeeded + results[index].rejected != TEST_CALLS) return 12;
        total += results[index].succeeded;
        for (uint32_t item = 0; item < results[index].succeeded; ++item) {
            uint64_t handle = results[index].handles[item];
            for (uint32_t other = 0; other <= index; ++other) {
                uint32_t limit = other == index ? item : results[other].succeeded;
                for (uint32_t prior = 0; prior < limit; ++prior)
                    if (handle == results[other].handles[prior]) return 13;
            }
        }
    }
    return total != UINT32_C(0) ? 0 : 14;
}
