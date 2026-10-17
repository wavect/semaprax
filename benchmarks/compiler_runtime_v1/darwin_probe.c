#if !defined(__APPLE__)
#error "This fixed probe requires Darwin kernel semantics"
#endif
/* Fixed native OS probe. No model input, compiler invocation or grant creation. */
#include <sys/types.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <arpa/inet.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <spawn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
extern char **environ;
static void hex(const char *s) {
    putchar('"');
    for (const unsigned char *p = (const unsigned char *)s; *p; ++p) printf("%02x", *p);
    putchar('"');
}
static int report(const char *name, int result, int saved) {
    printf("{\"schema\":\"semaprax.darwin-probe.v1\",\"case\":\"%s\",\"pid\":%d,\"uid\":%u,\"result\":%d,\"errno\":%d}\n",
           name, getpid(), (unsigned)geteuid(), result, saved);
    return fflush(stdout) ? 125 : 0;
}
int main(int argc, char **argv) {
    if (argc < 2 || argc > 3 || strlen(argv[1]) > 32) return 126;
    const char *name = argv[1];
    if (!strcmp(name, "environment")) {
        if (argc != 2) return 126;
        printf("{\"schema\":\"semaprax.darwin-probe.v1\",\"case\":\"environment\",\"pid\":%d,\"uid\":%u,\"entries_hex\":[", getpid(), (unsigned)geteuid());
        unsigned count = 0;
        for (char **p = environ; *p; ++p) {
            if (++count > 64 || strlen(*p) > 16384) return 125;
            if (count > 1) putchar(',');
            /* Full values only for declared seven nonsecret environment keys. */
            static const char *allowed[] = {"PATH=", "HOME=", "TMPDIR=", "CODEX_HOME=", "LANG=", "LC_ALL=", "TZ="};
            int known = 0;
            for (unsigned i = 0; i < 7; ++i) if (!strncmp(*p, allowed[i], strlen(allowed[i]))) known = 1;
            if (known) hex(*p);
            else {
                char key[256]; const char *equals = strchr(*p, '=');
                if (!equals || equals - *p >= 255) return 125;
                memcpy(key, *p, (size_t)(equals - *p)); key[equals - *p] = 0;
                hex(key); /* Unknown values are never retained as possible secrets. */
            }
        }
        puts("]}"); return fflush(stdout) ? 125 : 0;
    }
    if (!strcmp(name, "descriptors")) {
        if (argc != 2) return 126;
        DIR *d = opendir("/dev/fd"); if (!d) return report(name, -1, errno);
        int own = dirfd(d), extras = 0; struct dirent *entry;
        while ((entry = readdir(d))) {
            if (!strcmp(entry->d_name, ".") || !strcmp(entry->d_name, "..")) continue;
            char *end = NULL; long fd = strtol(entry->d_name, &end, 10);
            if (!end || *end || fd < 0 || fd > 2147483647L) { closedir(d); return 125; }
            if (fd > 2 && fd != own && fcntl((int)fd, F_GETFD) != -1) ++extras;
        }
        closedir(d);
        for (int fd = 0; fd < 3; ++fd) {
            struct stat info;
            if (fstat(fd, &info)) { ++extras; continue; }
            if (fd > 0 && !S_ISFIFO(info.st_mode)) ++extras;
            if (fd == 0) {
                if (S_ISCHR(info.st_mode)) {
                    struct stat null_info;
                    if (stat("/dev/null", &null_info) || info.st_rdev != null_info.st_rdev || isatty(0)) ++extras;
                } else if (S_ISFIFO(info.st_mode)) {
                    int flags = fcntl(0, F_GETFL); char byte;
                    if (flags < 0 || fcntl(0, F_SETFL, flags | O_NONBLOCK) || read(0, &byte, 1) != 0) ++extras;
                } else ++extras;
            }
        }
        return report(name, extras, 0);
    }
    if (!strcmp(name, "fork")) {
        if (argc != 2) return 126;
        pid_t pid = fork(); int saved = errno;
        if (pid == 0) _exit(0);
        if (pid > 0) { int status; if (waitpid(pid, &status, 0) != pid) return 125; }
        return report(name, pid < 0 ? -1 : 0, pid < 0 ? saved : 0);
    }
    if (!strcmp(name, "foreign-exec")) {
        if (argc != 3 || argv[2][0] != '/') return 126;
        pid_t pid = -1; char *args[] = {argv[2], NULL};
        int result = posix_spawn(&pid, argv[2], NULL, NULL, args, environ);
        if (!result) { int status; if (waitpid(pid, &status, 0) != pid) return 125; }
        return report(name, result ? -1 : 0, result);
    }
    if (!strcmp(name, "network")) {
        if (argc != 2) return 126;
        int fd = socket(AF_INET, SOCK_STREAM, 0);
        if (fd < 0) return report(name, -1, errno);
        struct sockaddr_in address; memset(&address, 0, sizeof address);
        address.sin_len = (unsigned char)sizeof address; address.sin_family = AF_INET; address.sin_port = htons(1);
        address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        int result = connect(fd, (struct sockaddr *)&address, sizeof address), saved = errno;
        close(fd); return report(name, result, result < 0 ? saved : 0);
    }
    if (!strcmp(name, "unix-socket")) {
        if (argc != 3 || argv[2][0] != '/') return 126;
        struct sockaddr_un address; memset(&address, 0, sizeof address);
        if (strlen(argv[2]) >= sizeof address.sun_path) return 126;
        address.sun_len = (unsigned char)sizeof address; address.sun_family = AF_UNIX; strcpy(address.sun_path, argv[2]);
        int fd = socket(AF_UNIX, SOCK_STREAM, 0);
        if (fd < 0) return report(name, -1, errno);
        int result = connect(fd, (struct sockaddr *)&address, sizeof address), saved = errno;
        close(fd); return report(name, result, result < 0 ? saved : 0);
    }
    if (!strcmp(name, "read") || !strcmp(name, "write") || !strcmp(name, "write-open")) {
        if (argc != 3 || argv[2][0] != '/') return 126;
        int writing = strcmp(name, "read") != 0;
        int fd = open(argv[2], (writing ? O_WRONLY : O_RDONLY) | O_NOFOLLOW);
        if (fd < 0) return report(name, -1, errno);
        char byte; ssize_t result = !strcmp(name, "write-open") ? 0 :
            (writing ? write(fd, "X", 1) : read(fd, &byte, 1));
        int saved = errno; close(fd);
        return report(name, result < 0 ? -1 : 0, result < 0 ? saved : 0);
    }
    if (!strcmp(name, "signal")) {
        if (argc != 2) return 126;
        if (report(name, 0, 0)) return 125;
        raise(SIGTERM); return 125;
    }
    if (!strcmp(name, "timeout")) {
        if (argc != 2) return 126;
        if (report(name, 0, 0)) return 125;
        /* Even a broken runtime cannot leave this probe hanging indefinitely. */
        sleep(2); return 0;
    }
    return 126;
}
