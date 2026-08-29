#include <signal.h>

int main(void) {
    if (signal(SIGTERM, SIG_IGN) == SIG_ERR) {
        return 2;
    }
    for (;;) {
    }
}
