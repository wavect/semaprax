use super::*;
use semaprax::agent_lifecycle::iterative::effects::{NativeTargetHost, TargetStageBackend};
use semaprax::execution_revision::typed::resume_migrated_agent_runtime_v2;
use std::os::unix::ffi::OsStrExt;

#[test]
fn sanitized_held_native_migration_and_recovery() {
    let Some(clang) = std::env::var_os("SEMAPRAX_TEST_NATIVE_STAGE_CLANG")
        .map(std::path::PathBuf::from)
        .into_iter()
        .chain(
            [
                "/usr/bin/clang",
                "/usr/local/bin/clang",
                "/opt/homebrew/bin/clang",
            ]
            .map(std::path::PathBuf::from),
        )
        .find(|path| NativeTargetHost::open(path).is_ok())
    else {
        panic!("sanitized migration gate requires an explicit held clang");
    };
    let clang = clang
        .canonicalize()
        .expect("sanitized migration gate pins the selected clang path");
    let root = std::env::temp_dir().join(format!(
        "semaprax-migration-sanitizer-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let launcher = root.join("held-clang-sanitized");
    let launcher_source = root.join("held-clang-sanitized.c");
    let clang_literal = clang
        .as_os_str()
        .as_bytes()
        .iter()
        .map(|byte| format!("\\{byte:03o}"))
        .collect::<String>();
    std::fs::write(
        &launcher_source,
        format!(
            r#"#include <errno.h>
#include <sys/wait.h>
#include <unistd.h>

static int has_symbol(const char *needle) {{
    int pipefd[2];
    if (pipe(pipefd) != 0) return 0;
    pid_t pid = fork();
    if (pid < 0) return 0;
    if (pid == 0) {{
        if (dup2(pipefd[1], STDOUT_FILENO) < 0) _exit(127);
        close(pipefd[0]);
        close(pipefd[1]);
        execl("/usr/bin/nm", "nm", "-a", "native_executor", (char *)0);
        _exit(127);
    }}
    close(pipefd[1]);
    char output[4096];
    const char *matched = needle;
    int found = 0;
    int complete = 1;
    ssize_t read_count;
    while ((read_count = read(pipefd[0], output, sizeof output)) != 0) {{
        if (read_count < 0) {{
            if (errno == EINTR) continue;
            complete = 0;
            break;
        }}
        for (ssize_t index = 0; index < read_count && !found; ++index) {{
            if (output[index] == *matched) {{
                ++matched;
                found = *matched == '\0';
            }} else {{
                matched = needle;
                if (output[index] == *matched) ++matched;
            }}
        }}
    }}
    close(pipefd[0]);
    int status;
    return complete && waitpid(pid, &status, 0) == pid && WIFEXITED(status)
        && WEXITSTATUS(status) == 0 && found;
}}

int main(int argc, char **argv) {{
    char *clang_argv[argc + 3];
    clang_argv[0] = "{clang_literal}";
    clang_argv[1] = "-fsanitize=address,undefined";
    clang_argv[2] = "-fno-omit-frame-pointer";
    for (int index = 1; index < argc; ++index) clang_argv[index + 2] = argv[index];
    clang_argv[argc + 2] = 0;
    pid_t pid = fork();
    if (pid < 0) return 127;
    if (pid == 0) {{
        execv(clang_argv[0], clang_argv);
        _exit(127);
    }}
    int status;
    if (waitpid(pid, &status, 0) != pid || !WIFEXITED(status)) return 127;
    if (WEXITSTATUS(status) != 0) return WEXITSTATUS(status);
    if (!has_symbol("__asan_")) return 97;
    if (!has_symbol("__ubsan_")) return 98;
    return 0;
}}
"#,
        ),
    )
    .unwrap();
    let launcher_compile = std::process::Command::new(&clang)
        .env_clear()
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg("--ld-path=/usr/bin/ld")
        .arg(&launcher_source)
        .arg("-o")
        .arg(&launcher)
        .output()
        .expect("compile ELF sanitizer launcher with pinned clang");
    assert!(
        launcher_compile.status.success(),
        "compile ELF sanitizer launcher: stdout={} stderr={}",
        String::from_utf8_lossy(&launcher_compile.stdout),
        String::from_utf8_lossy(&launcher_compile.stderr),
    );
    let native =
        NativeTargetHost::open(&launcher).expect("ELF sanitizer launcher is held capability");
    let a = first();
    let b = successor(&a, "State", "StateB", "b", &["marker"], false);
    let (migration, before, after) =
        migrated_with_metered_backend(&a, &b, TargetStageBackend::Native(&native));
    let handoff = migration.handoff_digest().unwrap();
    let mut host = handler();
    let mut store = Store::default();
    let completed = migration
        .run_durable_metered_with_backend(
            &mut host,
            &AgentCancellation::new(),
            &mut store,
            TargetStageBackend::Native(&native),
            10_000,
        )
        .expect("held native target records durable migration receipts");
    assert!(completed.run().observations_complete());
    assert_eq!(host.calls.len(), 3);
    let snapshot: serde_json::Value = serde_json::from_str(&store.document).unwrap();
    let checkpoint: serde_json::Value =
        serde_json::from_str(snapshot["checkpoint"].as_str().unwrap()).unwrap();
    let entries = checkpoint["entries"].as_array().unwrap();
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry["event"]["kind"] == "stage_reservation")
            .count(),
        entries
            .iter()
            .filter(|entry| entry["event"]["kind"] == "semantic_work")
            .count(),
        "every held-native migration stage reservation has an authenticated receipt",
    );
    let retained = store.document.clone();
    let resumed = resume_migrated_agent_runtime_v2(
        bind(&a, b"chain payload"),
        bind(&b, b"destination input ignored"),
        &retained,
        &handoff,
        &before,
        &after,
    )
    .expect("held-native v4 migration handoff recovers");
    let replay = resumed
        .run_durable_metered_with_backend(
            &mut host,
            &AgentCancellation::new(),
            &mut store,
            TargetStageBackend::Native(&native),
            10_000,
        )
        .expect("same held native target replays durable migration receipts");
    assert!(replay.run().observations_complete());
    assert_eq!(replay.run().run().run().dispatched(), 0);
    assert_eq!(host.calls.len(), 3);
    std::fs::remove_dir_all(root).unwrap();
}
