#define COBJMACROS
#define UNICODE
#define _UNICODE
#define WIN32_LEAN_AND_MEAN

#include <windows.h>
#include <bcrypt.h>
#include <oleacc.h>
#include <oleauto.h>
#include <shellapi.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <wchar.h>

#pragma comment(lib, "user32.lib")
#pragma comment(lib, "oleacc.lib")
#pragma comment(lib, "ole32.lib")
#pragma comment(lib, "oleaut32.lib")
#pragma comment(lib, "shell32.lib")
#pragma comment(lib, "uuid.lib")
#pragma comment(lib, "bcrypt.lib")
#pragma comment(linker, "/SUBSYSTEM:WINDOWS")

enum {
    SEMAPRAX_BUTTON_ID = 1001,
    SEMAPRAX_TIMER_ID = 2001,
    SEMAPRAX_TIMER_DELAY_MS = 100,
    SEMAPRAX_ENGINE_TIMEOUT_MS = 30000,
    SEMAPRAX_PATH_CAPACITY = 32768
};

static const wchar_t kWindowClassName[] = L"SemapraxNativeUiFixtureWindow";
static const wchar_t kWindowTitle[] = L"SEMAPRAX native UI fixture";
static const wchar_t kButtonAccessibleName[] = L"Run verified SEMAPRAX action";
static const wchar_t kEngineFileName[] = L"SemapraxPrivateEngine.exe";
static const wchar_t kEngineManifestFileName[] =
    L"SemapraxPrivateEngine.sha256";
static const char kEngineManifestPrefix[] =
    "semaprax.private-desktop-engine-sha256.v1 ";
static const char kExpectedEngineOutput[] =
    "SEMAPRAX_DESKTOP_V3_OK platform=windows calls=2 owner=0 "
    "payloads=41,43 replay=exact\n";
static const char kSuccessRecord[] =
    "SEMAPRAX_DESKTOP_UI_V1_OK platform=windows "
    "lifecycle=create,window,shown,control,close,terminate "
    "accessibility=button-name engine=calls-2-replay-exact\n";

typedef enum SemapraxUiStage {
    SEMAPRAX_STAGE_INITIAL = 0,
    SEMAPRAX_STAGE_WINDOW_CREATED,
    SEMAPRAX_STAGE_READY,
    SEMAPRAX_STAGE_CLICKING,
    SEMAPRAX_STAGE_ENGINE_VERIFIED,
    SEMAPRAX_STAGE_DESTROYING,
    SEMAPRAX_STAGE_DESTROYED
} SemapraxUiStage;

typedef struct SemapraxUiState {
    HINSTANCE instance;
    HWND window;
    HWND button;
    SemapraxUiStage stage;
    ULONGLONG timer_started_at;
    unsigned int create_messages;
    unsigned int show_messages;
    unsigned int timer_messages;
    unsigned int click_messages;
    BOOL timer_active;
    BOOL accessible_name_verified;
    BOOL engine_output_verified;
    BOOL destroy_requested;
    BOOL destroy_message_seen;
    BOOL nonclient_destroy_seen;
    BOOL quit_message_seen;
    int failure;
} SemapraxUiState;

static void semaprax_close_handle(HANDLE *handle) {
    if (handle != NULL && *handle != NULL && *handle != INVALID_HANDLE_VALUE) {
        CloseHandle(*handle);
        *handle = NULL;
    }
}

static void semaprax_fail_window(SemapraxUiState *state, HWND window, int failure) {
    if (state == NULL) {
        return;
    }
    if (state->failure == 0) {
        state->failure = failure;
    }
    if (state->timer_active) {
        KillTimer(window, SEMAPRAX_TIMER_ID);
        state->timer_active = FALSE;
    }
    if (IsWindow(window)) {
        state->destroy_requested = TRUE;
        state->stage = SEMAPRAX_STAGE_DESTROYING;
        DestroyWindow(window);
    }
}

static BOOL semaprax_get_sibling_path(const wchar_t *file_name,
                                      wchar_t *destination,
                                      size_t capacity) {
    DWORD length;
    wchar_t *separator;
    size_t directory_length;
    size_t file_name_length;

    if (file_name == NULL || file_name[0] == L'\0' || destination == NULL ||
        capacity < 4 || capacity > UINT32_MAX) {
        return FALSE;
    }
    file_name_length = wcslen(file_name);
    length = GetModuleFileNameW(NULL, destination, (DWORD)capacity);
    if (length == 0 || length >= capacity) {
        return FALSE;
    }
    separator = wcsrchr(destination, L'\\');
    if (separator == NULL) {
        return FALSE;
    }
    directory_length = (size_t)(separator - destination) + 1;
    if (directory_length + file_name_length >= capacity) {
        return FALSE;
    }
    CopyMemory(destination + directory_length, file_name,
               (file_name_length + 1) * sizeof(wchar_t));
    return TRUE;
}

static int semaprax_lower_hex_value(unsigned char value) {
    if (value >= (unsigned char)'0' && value <= (unsigned char)'9') {
        return (int)(value - (unsigned char)'0');
    }
    if (value >= (unsigned char)'a' && value <= (unsigned char)'f') {
        return (int)(value - (unsigned char)'a') + 10;
    }
    return -1;
}

static BOOL semaprax_load_expected_engine_digest(
    const wchar_t *manifest_path, unsigned char expected_digest[32]) {
    enum {
        SEMAPRAX_SHA256_HEX_LENGTH = 64,
        SEMAPRAX_ENGINE_MANIFEST_LENGTH =
            (sizeof(kEngineManifestPrefix) - 1) + SEMAPRAX_SHA256_HEX_LENGTH + 1
    };
    HANDLE manifest = INVALID_HANDLE_VALUE;
    LARGE_INTEGER size;
    unsigned char bytes[SEMAPRAX_ENGINE_MANIFEST_LENGTH];
    DWORD bytes_read = 0;
    size_t index;
    BOOL success = FALSE;

    manifest = CreateFileW(manifest_path, GENERIC_READ, FILE_SHARE_READ, NULL,
                           OPEN_EXISTING,
                           FILE_ATTRIBUTE_NORMAL | FILE_FLAG_SEQUENTIAL_SCAN,
                           NULL);
    if (manifest == INVALID_HANDLE_VALUE ||
        !GetFileSizeEx(manifest, &size) ||
        size.QuadPart != SEMAPRAX_ENGINE_MANIFEST_LENGTH ||
        !ReadFile(manifest, bytes, sizeof(bytes), &bytes_read, NULL) ||
        bytes_read != sizeof(bytes) ||
        memcmp(bytes, kEngineManifestPrefix,
               sizeof(kEngineManifestPrefix) - 1) != 0 ||
        bytes[sizeof(bytes) - 1] != (unsigned char)'\n') {
        goto cleanup;
    }
    for (index = 0; index < sizeof(expected_digest[0]) * 32; ++index) {
        int high = semaprax_lower_hex_value(
            bytes[sizeof(kEngineManifestPrefix) - 1 + (index * 2)]);
        int low = semaprax_lower_hex_value(
            bytes[sizeof(kEngineManifestPrefix) + (index * 2)]);
        if (high < 0 || low < 0) {
            goto cleanup;
        }
        expected_digest[index] = (unsigned char)((high << 4) | low);
    }
    success = TRUE;

cleanup:
    semaprax_close_handle(&manifest);
    return success;
}

static BOOL semaprax_verify_engine_digest(const wchar_t *engine_path,
                                          HANDLE *locked_engine) {
    wchar_t manifest_path[SEMAPRAX_PATH_CAPACITY];
    unsigned char expected_digest[32];
    unsigned char actual_digest[32];
    unsigned char buffer[32768];
    BCRYPT_ALG_HANDLE algorithm = NULL;
    BCRYPT_HASH_HANDLE hash = NULL;
    HANDLE engine = INVALID_HANDLE_VALUE;
    PUCHAR hash_object = NULL;
    DWORD hash_object_length = 0;
    DWORD hash_length = 0;
    DWORD property_length = 0;
    DWORD bytes_read = 0;
    unsigned int difference = 0;
    size_t index;
    BOOL success = FALSE;

    if (engine_path == NULL || locked_engine == NULL ||
        *locked_engine != NULL ||
        !semaprax_get_sibling_path(kEngineManifestFileName, manifest_path,
                                   ARRAYSIZE(manifest_path)) ||
        !semaprax_load_expected_engine_digest(manifest_path,
                                              expected_digest)) {
        return FALSE;
    }
    engine = CreateFileW(engine_path, GENERIC_READ, FILE_SHARE_READ, NULL,
                         OPEN_EXISTING,
                         FILE_ATTRIBUTE_NORMAL | FILE_FLAG_SEQUENTIAL_SCAN,
                         NULL);
    if (engine == INVALID_HANDLE_VALUE ||
        !BCRYPT_SUCCESS(BCryptOpenAlgorithmProvider(
            &algorithm, BCRYPT_SHA256_ALGORITHM, NULL, 0)) ||
        !BCRYPT_SUCCESS(BCryptGetProperty(
            algorithm, BCRYPT_OBJECT_LENGTH,
            (PUCHAR)&hash_object_length, sizeof(hash_object_length),
            &property_length, 0)) ||
        property_length != sizeof(hash_object_length) ||
        hash_object_length == 0 ||
        !BCRYPT_SUCCESS(BCryptGetProperty(
            algorithm, BCRYPT_HASH_LENGTH, (PUCHAR)&hash_length,
            sizeof(hash_length), &property_length, 0)) ||
        property_length != sizeof(hash_length) ||
        hash_length != sizeof(actual_digest)) {
        goto cleanup;
    }
    hash_object = (PUCHAR)HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY,
                                   hash_object_length);
    if (hash_object == NULL ||
        !BCRYPT_SUCCESS(BCryptCreateHash(
            algorithm, &hash, hash_object, hash_object_length, NULL, 0, 0))) {
        goto cleanup;
    }
    for (;;) {
        if (!ReadFile(engine, buffer, sizeof(buffer), &bytes_read, NULL)) {
            goto cleanup;
        }
        if (bytes_read == 0) {
            break;
        }
        if (!BCRYPT_SUCCESS(
                BCryptHashData(hash, buffer, bytes_read, 0))) {
            goto cleanup;
        }
    }
    if (!BCRYPT_SUCCESS(BCryptFinishHash(
            hash, actual_digest, sizeof(actual_digest), 0))) {
        goto cleanup;
    }
    for (index = 0; index < sizeof(actual_digest); ++index) {
        difference |= (unsigned int)(actual_digest[index] ^
                                     expected_digest[index]);
    }
    if (difference != 0) {
        goto cleanup;
    }
    *locked_engine = engine;
    engine = INVALID_HANDLE_VALUE;
    success = TRUE;

cleanup:
    if (hash != NULL) {
        BCryptDestroyHash(hash);
    }
    if (hash_object != NULL) {
        SecureZeroMemory(hash_object, hash_object_length);
        HeapFree(GetProcessHeap(), 0, hash_object);
    }
    if (algorithm != NULL) {
        BCryptCloseAlgorithmProvider(algorithm, 0);
    }
    semaprax_close_handle(&engine);
    SecureZeroMemory(expected_digest, sizeof(expected_digest));
    SecureZeroMemory(actual_digest, sizeof(actual_digest));
    return success;
}

static BOOL semaprax_run_engine_exact(void) {
    SECURITY_ATTRIBUTES security_attributes;
    STARTUPINFOW startup_info;
    PROCESS_INFORMATION process_info;
    wchar_t engine_path[SEMAPRAX_PATH_CAPACITY];
    wchar_t command_line[SEMAPRAX_PATH_CAPACITY];
    HANDLE stdout_read = NULL;
    HANDLE stdout_write = NULL;
    HANDLE locked_engine = NULL;
    HANDLE null_input = INVALID_HANDLE_VALUE;
    HANDLE null_error = INVALID_HANDLE_VALUE;
    const size_t expected_length = sizeof(kExpectedEngineOutput) - 1;
    size_t observed_length = 0;
    BOOL bytes_match = TRUE;
    BOOL process_finished = FALSE;
    BOOL pipe_finished = FALSE;
    BOOL launched = FALSE;
    BOOL success = FALSE;
    ULONGLONG deadline;
    DWORD exit_code = 1;

    ZeroMemory(&security_attributes, sizeof(security_attributes));
    security_attributes.nLength = sizeof(security_attributes);
    security_attributes.bInheritHandle = TRUE;
    ZeroMemory(&startup_info, sizeof(startup_info));
    startup_info.cb = sizeof(startup_info);
    ZeroMemory(&process_info, sizeof(process_info));

    if (!semaprax_get_sibling_path(kEngineFileName, engine_path,
                                   ARRAYSIZE(engine_path)) ||
        !semaprax_verify_engine_digest(engine_path, &locked_engine)) {
        goto cleanup;
    }
    if (wcslen(engine_path) + 3 > ARRAYSIZE(command_line)) {
        goto cleanup;
    }
    command_line[0] = L'"';
    wcscpy_s(command_line + 1, ARRAYSIZE(command_line) - 1, engine_path);
    wcscat_s(command_line, ARRAYSIZE(command_line), L"\"");

    if (!CreatePipe(&stdout_read, &stdout_write, &security_attributes, 0)) {
        goto cleanup;
    }
    if (!SetHandleInformation(stdout_read, HANDLE_FLAG_INHERIT, 0)) {
        goto cleanup;
    }
    null_input = CreateFileW(L"NUL", GENERIC_READ,
                             FILE_SHARE_READ | FILE_SHARE_WRITE,
                             &security_attributes, OPEN_EXISTING, 0, NULL);
    null_error = CreateFileW(L"NUL", GENERIC_WRITE,
                             FILE_SHARE_READ | FILE_SHARE_WRITE,
                             &security_attributes, OPEN_EXISTING, 0, NULL);
    if (null_input == INVALID_HANDLE_VALUE || null_error == INVALID_HANDLE_VALUE) {
        goto cleanup;
    }

    startup_info.dwFlags = STARTF_USESTDHANDLES;
    startup_info.hStdInput = null_input;
    startup_info.hStdOutput = stdout_write;
    startup_info.hStdError = null_error;

    if (!CreateProcessW(engine_path, command_line, NULL, NULL, TRUE,
                        CREATE_NO_WINDOW, NULL, NULL, &startup_info,
                        &process_info)) {
        goto cleanup;
    }
    launched = TRUE;
    semaprax_close_handle(&locked_engine);
    semaprax_close_handle(&process_info.hThread);
    semaprax_close_handle(&stdout_write);
    semaprax_close_handle(&null_input);
    semaprax_close_handle(&null_error);

    deadline = GetTickCount64() + SEMAPRAX_ENGINE_TIMEOUT_MS;
    while (!process_finished || !pipe_finished) {
        DWORD available = 0;
        DWORD wait_result = WaitForSingleObject(process_info.hProcess, 0);

        if (wait_result == WAIT_OBJECT_0) {
            process_finished = TRUE;
        } else if (wait_result != WAIT_TIMEOUT) {
            goto cleanup;
        }

        if (!pipe_finished) {
            if (!PeekNamedPipe(stdout_read, NULL, 0, NULL, &available, NULL)) {
                if (GetLastError() == ERROR_BROKEN_PIPE) {
                    pipe_finished = TRUE;
                } else {
                    goto cleanup;
                }
            } else if (available != 0) {
                unsigned char buffer[256];
                DWORD bytes_read = 0;
                DWORD requested = available < sizeof(buffer)
                                      ? available
                                      : (DWORD)sizeof(buffer);
                DWORD index;

                if (!ReadFile(stdout_read, buffer, requested, &bytes_read, NULL)) {
                    if (GetLastError() == ERROR_BROKEN_PIPE) {
                        pipe_finished = TRUE;
                    } else {
                        goto cleanup;
                    }
                }
                for (index = 0; index < bytes_read; ++index) {
                    if (observed_length >= expected_length ||
                        buffer[index] !=
                            (unsigned char)kExpectedEngineOutput[observed_length]) {
                        bytes_match = FALSE;
                    }
                    if (observed_length != SIZE_MAX) {
                        ++observed_length;
                    } else {
                        bytes_match = FALSE;
                    }
                }
                continue;
            } else if (process_finished) {
                pipe_finished = TRUE;
            }
        }

        if (!process_finished || !pipe_finished) {
            if (GetTickCount64() >= deadline) {
                goto cleanup;
            }
            WaitForSingleObject(process_info.hProcess, 10);
        }
    }

    if (!GetExitCodeProcess(process_info.hProcess, &exit_code)) {
        goto cleanup;
    }
    success = exit_code == 0 && bytes_match &&
              observed_length == expected_length;

cleanup:
    if (launched && !process_finished && process_info.hProcess != NULL) {
        TerminateProcess(process_info.hProcess, 125);
        WaitForSingleObject(process_info.hProcess, 5000);
    }
    semaprax_close_handle(&process_info.hThread);
    semaprax_close_handle(&process_info.hProcess);
    semaprax_close_handle(&stdout_read);
    semaprax_close_handle(&stdout_write);
    semaprax_close_handle(&null_input);
    semaprax_close_handle(&null_error);
    semaprax_close_handle(&locked_engine);
    return success;
}

static BOOL semaprax_verify_accessible_button(HWND button) {
    IAccessible *accessible = NULL;
    VARIANT self;
    BSTR name = NULL;
    HRESULT result;
    BOOL matches = FALSE;
    size_t expected_length = ARRAYSIZE(kButtonAccessibleName) - 1;

    VariantInit(&self);
    self.vt = VT_I4;
    self.lVal = CHILDID_SELF;
    result = AccessibleObjectFromWindow(button, OBJID_CLIENT, &IID_IAccessible,
                                        (void **)&accessible);
    if (FAILED(result) || accessible == NULL) {
        goto cleanup;
    }
    result = IAccessible_get_accName(accessible, self, &name);
    if (FAILED(result) || name == NULL) {
        goto cleanup;
    }
    matches = SysStringLen(name) == expected_length &&
              CompareStringOrdinal(name, (int)expected_length,
                                   kButtonAccessibleName,
                                   (int)expected_length, FALSE) == CSTR_EQUAL;

cleanup:
    if (name != NULL) {
        SysFreeString(name);
    }
    if (accessible != NULL) {
        IAccessible_Release(accessible);
    }
    VariantClear(&self);
    return matches;
}

static LRESULT CALLBACK semaprax_window_proc(HWND window, UINT message,
                                              WPARAM w_param, LPARAM l_param) {
    SemapraxUiState *state =
        (SemapraxUiState *)GetWindowLongPtrW(window, GWLP_USERDATA);

    if (message == WM_NCCREATE) {
        CREATESTRUCTW *create = (CREATESTRUCTW *)l_param;
        state = (SemapraxUiState *)create->lpCreateParams;
        if (state == NULL || state->stage != SEMAPRAX_STAGE_INITIAL) {
            return FALSE;
        }
        SetWindowLongPtrW(window, GWLP_USERDATA, (LONG_PTR)state);
        state->window = window;
        return TRUE;
    }

    if (state == NULL) {
        return DefWindowProcW(window, message, w_param, l_param);
    }

    switch (message) {
    case WM_CREATE:
        if (state->create_messages != 0 ||
            state->stage != SEMAPRAX_STAGE_INITIAL) {
            return -1;
        }
        state->create_messages = 1;
        state->button = CreateWindowExW(
            0, L"BUTTON", kButtonAccessibleName,
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON,
            30, 35, 260, 48, window, (HMENU)(INT_PTR)SEMAPRAX_BUTTON_ID,
            state->instance, NULL);
        if (state->button == NULL || GetParent(state->button) != window ||
            GetWindowLongPtrW(state->button, GWLP_ID) != SEMAPRAX_BUTTON_ID) {
            state->failure = 31;
            return -1;
        }
        state->stage = SEMAPRAX_STAGE_WINDOW_CREATED;
        return 0;

    case WM_SHOWWINDOW:
        if (w_param != FALSE) {
            ++state->show_messages;
        }
        break;

    case WM_TIMER:
        if (w_param != SEMAPRAX_TIMER_ID ||
            state->stage != SEMAPRAX_STAGE_READY ||
            !state->timer_active || state->timer_messages != 0 ||
            GetTickCount64() < state->timer_started_at +
                                   SEMAPRAX_TIMER_DELAY_MS) {
            semaprax_fail_window(state, window, 41);
            return 0;
        }
        KillTimer(window, SEMAPRAX_TIMER_ID);
        state->timer_active = FALSE;
        state->timer_messages = 1;
        state->stage = SEMAPRAX_STAGE_CLICKING;
        SendMessageW(state->button, BM_CLICK, 0, 0);
        if (state->click_messages != 1 && IsWindow(window)) {
            semaprax_fail_window(state, window, 42);
        }
        return 0;

    case WM_COMMAND:
        if (LOWORD(w_param) == SEMAPRAX_BUTTON_ID &&
            HIWORD(w_param) == BN_CLICKED &&
            (HWND)l_param == state->button) {
            if (state->stage != SEMAPRAX_STAGE_CLICKING ||
                state->click_messages != 0 || state->timer_messages != 1) {
                semaprax_fail_window(state, window, 51);
                return 0;
            }
            state->click_messages = 1;
            if (!semaprax_run_engine_exact()) {
                semaprax_fail_window(state, window, 52);
                return 0;
            }
            state->engine_output_verified = TRUE;
            state->stage = SEMAPRAX_STAGE_ENGINE_VERIFIED;
            state->destroy_requested = TRUE;
            state->stage = SEMAPRAX_STAGE_DESTROYING;
            if (!DestroyWindow(window)) {
                state->failure = 53;
            }
            return 0;
        }
        semaprax_fail_window(state, window, 54);
        return 0;

    case WM_CLOSE:
        semaprax_fail_window(state, window, 61);
        return 0;

    case WM_DESTROY:
        if (state->destroy_message_seen) {
            state->failure = 62;
        }
        state->destroy_message_seen = TRUE;
        if (state->failure == 0 &&
            (!state->destroy_requested ||
             state->stage != SEMAPRAX_STAGE_DESTROYING ||
             state->create_messages != 1 || state->show_messages == 0 ||
             state->timer_messages != 1 || state->click_messages != 1 ||
             !state->accessible_name_verified ||
             !state->engine_output_verified)) {
            state->failure = 63;
        }
        state->stage = SEMAPRAX_STAGE_DESTROYED;
        PostQuitMessage(state->failure == 0 ? 0 : state->failure);
        return 0;

    case WM_NCDESTROY:
        state->nonclient_destroy_seen = TRUE;
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        state->window = NULL;
        break;

    default:
        break;
    }
    return DefWindowProcW(window, message, w_param, l_param);
}

static BOOL semaprax_prepare_result_path(wchar_t *destination,
                                         size_t capacity) {
    int argument_count = 0;
    wchar_t **arguments = CommandLineToArgvW(GetCommandLineW(), &argument_count);
    DWORD length;
    BOOL success = FALSE;

    if (arguments == NULL || argument_count != 2 || arguments[1][0] == L'\0') {
        goto cleanup;
    }
    length = GetFullPathNameW(arguments[1], (DWORD)capacity, destination, NULL);
    if (length == 0 || length >= capacity) {
        goto cleanup;
    }
    if (GetFileAttributesW(destination) != INVALID_FILE_ATTRIBUTES) {
        goto cleanup;
    }
    if (GetLastError() != ERROR_FILE_NOT_FOUND &&
        GetLastError() != ERROR_PATH_NOT_FOUND) {
        goto cleanup;
    }
    success = TRUE;

cleanup:
    if (arguments != NULL) {
        LocalFree(arguments);
    }
    return success;
}

static BOOL semaprax_write_success_record(const wchar_t *destination) {
    wchar_t temporary[SEMAPRAX_PATH_CAPACITY];
    size_t destination_length = wcslen(destination);
    HANDLE file = INVALID_HANDLE_VALUE;
    DWORD bytes_written = 0;
    DWORD attempt;
    BOOL success = FALSE;
    BOOL temporary_exists = FALSE;

    if (destination_length + 40 >= ARRAYSIZE(temporary)) {
        return FALSE;
    }
    for (attempt = 0; attempt < 16; ++attempt) {
        int count = _snwprintf_s(
            temporary, ARRAYSIZE(temporary), _TRUNCATE,
            L"%ls.semaprax-tmp-%lu-%lu", destination,
            (unsigned long)GetCurrentProcessId(), (unsigned long)attempt);
        if (count < 0) {
            return FALSE;
        }
        file = CreateFileW(temporary, GENERIC_WRITE, 0, NULL, CREATE_NEW,
                           FILE_ATTRIBUTE_NORMAL, NULL);
        if (file != INVALID_HANDLE_VALUE) {
            temporary_exists = TRUE;
            break;
        }
        if (GetLastError() != ERROR_FILE_EXISTS &&
            GetLastError() != ERROR_ALREADY_EXISTS) {
            return FALSE;
        }
    }
    if (file == INVALID_HANDLE_VALUE) {
        return FALSE;
    }
    if (!WriteFile(file, kSuccessRecord, (DWORD)(sizeof(kSuccessRecord) - 1),
                   &bytes_written, NULL) ||
        bytes_written != sizeof(kSuccessRecord) - 1 ||
        !FlushFileBuffers(file)) {
        goto cleanup;
    }
    if (!CloseHandle(file)) {
        file = INVALID_HANDLE_VALUE;
        goto cleanup;
    }
    file = INVALID_HANDLE_VALUE;
    if (!MoveFileExW(temporary, destination, MOVEFILE_WRITE_THROUGH)) {
        goto cleanup;
    }
    temporary_exists = FALSE;
    success = TRUE;

cleanup:
    if (file != INVALID_HANDLE_VALUE) {
        CloseHandle(file);
    }
    if (temporary_exists) {
        DeleteFileW(temporary);
    }
    return success;
}

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE previous_instance,
                    PWSTR command_line, int show_command) {
    WNDCLASSEXW window_class;
    SemapraxUiState state;
    wchar_t result_path[SEMAPRAX_PATH_CAPACITY];
    HWND window;
    RECT window_rect;
    MSG message;
    HRESULT com_result;
    BOOL com_initialized = FALSE;
    BOOL class_registered = FALSE;
    int return_code = 1;

    (void)previous_instance;
    (void)command_line;
    (void)show_command;
    ZeroMemory(&state, sizeof(state));
    state.instance = instance;
    state.stage = SEMAPRAX_STAGE_INITIAL;

    if (!semaprax_prepare_result_path(result_path, ARRAYSIZE(result_path))) {
        return 2;
    }
    com_result = CoInitializeEx(NULL, COINIT_APARTMENTTHREADED);
    if (FAILED(com_result)) {
        return 3;
    }
    com_initialized = TRUE;

    ZeroMemory(&window_class, sizeof(window_class));
    window_class.cbSize = sizeof(window_class);
    window_class.style = CS_HREDRAW | CS_VREDRAW;
    window_class.lpfnWndProc = semaprax_window_proc;
    window_class.hInstance = instance;
    window_class.hCursor = LoadCursorW(NULL, IDC_ARROW);
    window_class.hbrBackground = (HBRUSH)(COLOR_WINDOW + 1);
    window_class.lpszClassName = kWindowClassName;
    if (window_class.hCursor == NULL || RegisterClassExW(&window_class) == 0) {
        goto cleanup;
    }
    class_registered = TRUE;

    window = CreateWindowExW(
        0, kWindowClassName, kWindowTitle,
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
        CW_USEDEFAULT, CW_USEDEFAULT, 340, 150, NULL, NULL, instance, &state);
    if (window == NULL || state.window != window ||
        state.stage != SEMAPRAX_STAGE_WINDOW_CREATED ||
        state.create_messages != 1 || state.button == NULL) {
        state.failure = 71;
        goto cleanup;
    }

    ShowWindow(window, SW_SHOW);
    if (!UpdateWindow(window) || !IsWindowVisible(window) ||
        !IsWindowVisible(state.button) || GetParent(window) != NULL ||
        !GetWindowRect(window, &window_rect) ||
        window_rect.right <= window_rect.left ||
        window_rect.bottom <= window_rect.top || state.show_messages == 0) {
        semaprax_fail_window(&state, window, 72);
    } else if (!semaprax_verify_accessible_button(state.button)) {
        semaprax_fail_window(&state, window, 73);
    } else {
        state.accessible_name_verified = TRUE;
        state.stage = SEMAPRAX_STAGE_READY;
        state.timer_started_at = GetTickCount64();
        if (SetTimer(window, SEMAPRAX_TIMER_ID, SEMAPRAX_TIMER_DELAY_MS, NULL) !=
            SEMAPRAX_TIMER_ID) {
            semaprax_fail_window(&state, window, 74);
        } else {
            state.timer_active = TRUE;
        }
    }

    while ((return_code = (int)GetMessageW(&message, NULL, 0, 0)) > 0) {
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
    if (return_code == 0) {
        state.quit_message_seen = TRUE;
    } else {
        state.failure = 75;
    }

cleanup:
    if (state.window != NULL && IsWindow(state.window)) {
        semaprax_fail_window(&state, state.window,
                             state.failure == 0 ? 76 : state.failure);
    }
    if (class_registered && !UnregisterClassW(kWindowClassName, instance) &&
        state.failure == 0) {
        state.failure = 77;
    }
    if (com_initialized) {
        CoUninitialize();
    }

    if (state.failure == 0 &&
        state.stage == SEMAPRAX_STAGE_DESTROYED &&
        state.create_messages == 1 && state.show_messages > 0 &&
        state.timer_messages == 1 && state.click_messages == 1 &&
        state.accessible_name_verified && state.engine_output_verified &&
        state.destroy_requested && state.destroy_message_seen &&
        state.nonclient_destroy_seen && state.quit_message_seen &&
        semaprax_write_success_record(result_path)) {
        return 0;
    }
    return state.failure == 0 ? 1 : state.failure;
}
