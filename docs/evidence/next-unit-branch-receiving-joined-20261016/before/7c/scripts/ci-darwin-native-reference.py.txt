#!/usr/bin/env python3
"""Expose native tool failures without changing the held builder's diagnostics.

This is an ordinary CI reference process, not a substitute for held executable,
directory, process-settlement, or SDK publication admission. Children receive
the empty environment used by the builder's Darwin tools.
"""

import os
from pathlib import Path
import subprocess
import sys
import tempfile


def run(arguments, directory):
    print("native reference:", " ".join(map(str, arguments)), flush=True)
    result = subprocess.run(
        arguments,
        cwd=directory,
        env={},
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
        timeout=120,
    )
    text = result.stdout.decode("utf-8", errors="replace")
    if text:
        print(text[:65_536], end="" if text.endswith("\n") else "\n", flush=True)
    if result.returncode:
        raise SystemExit(f"native reference failed with status {result.returncode}")
    return text


def main():
    if sys.platform != "darwin":
        raise SystemExit("the Darwin native reference requires macOS")
    tools = [Path(os.environ[name]) for name in ("RUSTC", "CLANG")]
    for tool in tools:
        if not tool.is_absolute() or not tool.is_file():
            raise SystemExit(f"native reference requires an absolute tool file: {tool}")
    rustc, clang = tools
    with tempfile.TemporaryDirectory(
        prefix="semaprax-darwin-native-reference-", dir=os.environ["RUNNER_TEMP"]
    ) as temporary:
        directory = Path(temporary)
        version = run([rustc, "-vV"], directory)
        hosts = [line.removeprefix("host: ") for line in version.splitlines() if line.startswith("host: ")]
        if len(hosts) != 1 or hosts[0] not in ("aarch64-apple-darwin", "x86_64-apple-darwin"):
            raise SystemExit("native reference requires exactly one admitted Darwin host")
        target = hosts[0]
        (directory / "bridge.rs").write_text(
            '#[no_mangle]\npub extern "C" fn spx_native_reference() -> i32 {\n'
            '    std::panic::catch_unwind(|| std::hint::black_box(42)).unwrap_or(-1)\n}\n'
        )
        (directory / "main.c").write_text(
            "extern int spx_native_reference(void);\n"
            "int main(void) { return spx_native_reference() == 42 ? 0 : 1; }\n"
        )
        (directory / "module.c").write_text("int spx_native_reference_module(void) { return 0; }\n")
        # Match the prepared Rust staticlib invocation, including its warning
        # policy and unwind runtime. No inherited PATH, SDKROOT or DYLD values.
        run([
            rustc, "--edition=2021", "-Dwarnings", "--crate-type", "staticlib",
            "-C", "panic=unwind", "--target", target, "bridge.rs", "-o", "libbridge.a",
        ], directory)
        for source in ("main", "module"):
            run([
                clang, "-target", target, "-std=c11", "-Wall", "-Wextra", "-Werror",
                "-c", source + ".c", "-o", source + ".o",
            ], directory)
        # Keep the current prepared link spelling and SDK explicit so a missing
        # SDK, unsupported flag or unresolved native symbol is visible in CI.
        run([
            clang, "-target", target, "-Wl,-no_warn_duplicate_libraries",
            "main.o", "module.o", "libbridge.a", "-o", "reference",
            "-isysroot", "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk",
        ], directory)
        run([directory / "reference"], directory)
        print("PASS ordinary Darwin Rust/C tool link and execution reference", flush=True)


if __name__ == "__main__":
    main()
