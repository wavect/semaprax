//! Host-only physical fixtures; compiler-directed settlement is covered separately.

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn private_host_accounting_and_hostile_carriers() {
    // Exercise the final token without billions of invocations or a production
    // mutation hook. Only the initial counter and allocation observation differ.
    let mut token_arena = include_str!("arena.js").to_owned();
    for (original, replacement) in [
        ("function createArena(", "function tokenArena("),
        ("let nextToken=1,", "let nextToken=0x7fffffff,"),
        (
            "const bytes=new Bytes(length);",
            "tokenPayloadAllocations++;const bytes=new Bytes(length);",
        ),
    ] {
        assert_eq!(token_arena.matches(original).count(), 1);
        token_arena = token_arena.replacen(original, replacement, 1);
    }
    let source = [
        "import assert from 'node:assert/strict';\n",
        include_str!("input.js"),
        include_str!("arena.js"),
        &token_arena,
        include_str!("tests/arena.mjs"),
    ]
    .join("\n");
    let mut child = Command::new("node")
        .arg("--input-type=module")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the standalone Wasm host gate requires provisioned Node.js");
    child
        .stdin
        .take()
        .expect("piped Node stdin")
        .write_all(source.as_bytes())
        .expect("send bounded host fixture");
    let output = child.wait_with_output().expect("wait for host fixture");
    assert!(
        output.status.success(),
        "host fixture failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
