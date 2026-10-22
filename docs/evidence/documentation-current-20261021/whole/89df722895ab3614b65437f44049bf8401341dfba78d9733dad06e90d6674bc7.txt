# Preserve the selected SDK in cleared macOS native test environments

CI selects the Command Line Tools Clang and its SDK explicitly. Native C fixture commands then clear their environment, and the test-only environment binder previously restored only Windows INCLUDE/LIB. Consequently Apple Clang lost SDKROOT and could not find string.h.

The cfg(test) binder now forwards exactly the already selected SDKROOT on macOS. Windows forwarding and every production toolchain function remain unchanged. A complete module inverse replaces only this one test helper with its full predecessor, preserving every other source byte.

A guarded local component runs the actual old and new helper bodies with the same real Command Line Tools Clang, the same target and a C source including string.h. The old helper reproduces the missing-header failure; the new helper compiles successfully. The new cleared child environment contains exactly one entry, SDKROOT, and no inherited HOME or PATH. Compilation and the complete child process tree remain below the unchanged 200 MiB guard; all files are on the external SSD. This is fixture evidence only; the complete native builder and current-head CI suites still require execution.

Fresh source-role and custody checks preserve all historical source views, roles, owner rows and hostile controls. No completed historical publisher is rerun.
