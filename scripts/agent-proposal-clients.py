#!/usr/bin/env python3
"""Execute the provisioned Proposal clients on the current host, fail closed."""

import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    root = Path(__file__).resolve().parent.parent
    environment = os.environ.copy()
    environment["SEMAPRAX_REQUIRE_AGENT_PROPOSAL_CLIENTS"] = "1"
    environment["SEMAPRAX_TEST_PYTHON"] = str(Path(sys.executable).resolve())
    for variable, tool in [("SEMAPRAX_TEST_NODE", "node"), ("SEMAPRAX_TEST_CARGO", "cargo")]:
        executable = shutil.which(tool)
        if executable is None:
            raise SystemExit(f"required provisioned tool is absent: {tool}")
        # Preserve tool-manager proxy paths such as ~/.cargo/bin/cargo. Resolving
        # that symlink points at the rustup binary itself, so subsequent Cargo
        # arguments would be parsed by rustup instead of cargo.
        environment[variable] = str(Path(executable).absolute())
    tsc = root / "platform-tests/wasm-scalar-browser-v1/node_modules/typescript/bin/tsc"
    if not tsc.is_file():
        raise SystemExit("provision locked TypeScript before running this gate")
    environment["SEMAPRAX_TEST_TSC_JS"] = str(tsc)
    return subprocess.run(
        [environment["SEMAPRAX_TEST_CARGO"], "test", "--locked", "--offline", "-p",
         "semaprax", "--test", "agent_runtime_v1",
         "agent_proposal_client_execution::generated_proposal_clients_compile_execute_and_round_trip",
         "--", "--exact", "--nocapture"],
        cwd=root, env=environment, check=False,
    ).returncode


if __name__ == "__main__":
    sys.exit(main())
