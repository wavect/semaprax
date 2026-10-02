/* Explicitly provisioned aarch64 Linux authority launcher. No implicit build.
 * The outer VM admits the image/loader; this process narrows filesystem and
 * syscall authority before entering Node. Missing kernel support is fatal. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/audit.h>
#include <linux/filter.h>
#include <linux/landlock.h>
#include <linux/sched.h>
#include <linux/seccomp.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>
#ifndef LANDLOCK_ACCESS_FS_TRUNCATE
#define LANDLOCK_ACCESS_FS_TRUNCATE (1ULL << 14)
#endif

static void die(const char *s) { perror(s); exit(125); }
static void rule(int rules, const char *path, uint64_t rights) {
    int fd = open(path, O_PATH | O_CLOEXEC);
    if (fd < 0) die(path);
    struct landlock_path_beneath_attr r = {.allowed_access = rights, .parent_fd = fd};
    if (syscall(SYS_landlock_add_rule, rules, LANDLOCK_RULE_PATH_BENEATH, &r, 0)) die("landlock rule");
    close(fd);
}
static void confine(void) {
    if (syscall(SYS_landlock_create_ruleset, NULL, 0, LANDLOCK_CREATE_RULESET_VERSION) < 3) die("landlock ABI 3 required");
    const uint64_t read = LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR;
    /* Handle all ABI-3 filesystem rights; no symlink, socket, device or FIFO
     * creation is granted, and cross-hierarchy rename/link remains denied. */
    const uint64_t all = (1ULL << 15) - 1;
    struct landlock_ruleset_attr attr = {.handled_access_fs = all};
    int rules = syscall(SYS_landlock_create_ruleset, &attr, sizeof(attr), 0);
    if (rules < 0) die("landlock create");
    rule(rules, "/runtime", read | LANDLOCK_ACCESS_FS_EXECUTE);
    rule(rules, "/usr/lib", read | LANDLOCK_ACCESS_FS_EXECUTE);
    rule(rules, "/phase", read | LANDLOCK_ACCESS_FS_WRITE_FILE | LANDLOCK_ACCESS_FS_REMOVE_DIR |
         LANDLOCK_ACCESS_FS_REMOVE_FILE | LANDLOCK_ACCESS_FS_MAKE_DIR |
         LANDLOCK_ACCESS_FS_MAKE_REG | LANDLOCK_ACCESS_FS_TRUNCATE);
    rule(rules, "/dev/null", LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_WRITE_FILE);
    if (prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)) die("no new privileges");
    if (syscall(SYS_landlock_restrict_self, rules, 0)) die("landlock restrict");
    close(rules);
#define DENY(n) BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, (n), 0, 1), BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EPERM)
    struct sock_filter filter[] = {
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, arch)),
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, AUDIT_ARCH_AARCH64, 1, 0),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, nr)),
        DENY(SYS_socket), DENY(SYS_socketpair), DENY(SYS_unshare), DENY(SYS_setns),
        DENY(SYS_mount), DENY(SYS_ptrace), DENY(SYS_bpf),
        DENY(SYS_process_vm_readv), DENY(SYS_process_vm_writev),
        /* glibc falls back to clone; only threads in this process may start. */
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_clone3, 0, 1),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | ENOSYS),
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_clone, 0, 4),
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, args[0])),
        BPF_JUMP(BPF_JMP | BPF_JSET | BPF_K, CLONE_THREAD, 1, 0),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EPERM),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)
    };
    struct sock_fprog program = {.len = sizeof(filter) / sizeof(filter[0]), .filter = filter};
    if (prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &program)) die("seccomp");
}
static int probe(void) {
    int fd = open("/phase/allowed", O_CREAT | O_RDWR | O_TRUNC, 0600);
    if (fd < 0) return 2;
    if (write(fd, "ok", 2) != 2 || lseek(fd, 0, SEEK_SET) != 0) return 3;
    char b[2]; if (read(fd, b, 2) != 2 || memcmp(b, "ok", 2)) return 4;
    close(fd);
    errno = 0; fd = open("/denied/canary", O_RDONLY);
    if (fd != -1 || errno != EACCES) return 5;
    errno = 0; fd = open("/denied/canary", O_WRONLY);
    if (fd != -1 || errno != EACCES) return 6;
    errno = 0; pid_t child = fork();
    if (child == 0) _exit(10);
    if (child != -1 || errno != EPERM) { if (child > 0) waitpid(child, NULL, 0); return 7; }
    errno = 0; fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd != -1 || errno != EPERM) return 8;
    puts("{\"phase_read_write\":true,\"sibling_read\":\"EACCES\",\"sibling_write\":\"EACCES\",\"fork\":\"EPERM\",\"socket\":\"EPERM\"}");
    return 0;
}
int main(int argc, char **argv) {
    if (argc < 2 || getuid() == 0 || getgid() == 0) return 124;
    if (!strcmp(argv[1], "--probe") && argc == 2) {
        int fd = open("/denied/canary", O_RDWR);
        if (fd < 0) return 120;
        close(fd);
        fd = socket(AF_INET, SOCK_STREAM, 0);
        if (fd < 0) return 121;
        close(fd);
        pid_t child = fork();
        if (child < 0) return 122;
        if (child == 0) _exit(0);
        int status;
        if (waitpid(child, &status, 0) != child || status != 0) return 123;
    }
    confine();
    if (!strcmp(argv[1], "--probe") && argc == 2) return probe();
    if (strcmp(argv[1], "/runtime/node")) return 124;
    execv(argv[1], argv + 1);
    die("exact node exec");
}
