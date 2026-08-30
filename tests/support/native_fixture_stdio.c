/* Test-only exact-byte stdout transport. Include before allocations.c so CRT
 * headers are outside allocator macros; never part of a production digest. */
#include <stdio.h>
#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>
#endif

static int fixture_binary_stdout(void) {
#if defined(_WIN32)
    return _setmode(_fileno(stdout), _O_BINARY) != -1;
#else
    return 1;
#endif
}
