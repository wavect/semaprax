//! Test-only provider observation; pipe write boundaries are not evidence.

use super::*;

pub(super) fn build_observed_provider(root: &std::path::Path) -> std::path::PathBuf {
    let (c, subject, hir_graph, sources) =
        project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            codegen::emit_hir_c_with_stdin_stream_records(
                revision.public_api_program(),
                "consumer.command",
            )
            .map(|c| {
                (
                    c,
                    revision.project_revision().to_owned(),
                    revision.semantic_graph_digest().to_owned(),
                    revision
                        .sources()
                        .iter()
                        .map(|source| {
                            format!(
                                "{}={:x}",
                                source.path(),
                                Sha256::digest(source.source().as_bytes())
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(","),
                )
            })
            .map_err(|error| vec![error])
        })
        .unwrap();
    let read = "size_t count = fread(buffer, sizeof(uint8_t), (size_t)capacity, context->stream);";
    assert_eq!(
        c.matches(read).count(),
        1,
        "instrument only the process provider read"
    );
    // This observes the actual returned provider length without changing its
    // transport, capacity, returned bytes, EOF or status. The unmodified binary
    // separately owns acceptance/publication evidence in the parent fixture.
    let observed = c.replace(
        read,
        &format!("{read}\n    fprintf(stderr, \"provider-chunk:%zu\\n\", count);"),
    );
    eprintln!("provider-boundary-audit project={subject} sources_sha256={sources} hir_graph={hir_graph} original_c_sha256={:x} observed_c_sha256={:x}", Sha256::digest(c.as_bytes()), Sha256::digest(observed.as_bytes()));
    let source = root.join("observed-provider.c");
    let binary = root.join("observed-provider");
    std::fs::write(&source, observed).unwrap();
    let output = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O0"])
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

pub(super) fn execute_observed(binary: &std::path::Path, input: &[u8]) -> std::process::Output {
    let mut output = execute(binary, input);
    let mut chunks = Vec::new();
    let mut diagnostic = Vec::new();
    for line in output.stderr.split_inclusive(|byte| *byte == b'\n') {
        if let Some(value) = line.strip_prefix(b"provider-chunk:") {
            chunks.push(
                std::str::from_utf8(value)
                    .unwrap()
                    .trim_end()
                    .parse::<usize>()
                    .unwrap(),
            );
        } else {
            diagnostic.extend_from_slice(line);
        }
    }
    let mut expected = vec![4096; input.len() / 4096];
    if input.len() % 4096 != 0 {
        expected.push(input.len() % 4096);
    }
    expected.push(0); // Explicit EOF read, not an inferred writer boundary.
    assert_eq!(chunks, expected, "observed provider chunk inventory");
    output.stderr = diagnostic;
    output
}

pub(super) fn lexical_splits(binary: &std::path::Path) {
    // Strict whole-document structural errors precede lexical faults. Once
    // structure succeeds, the earlier UTF-8/escape fault wins; truncated literal
    // errors identify token start. Expected offsets are independent raw bytes.
    let cases: &[(&[u8], usize)] = &[
        (b"\"\xc0\\q\"", 1),
        (b"\"\\q\xc0\"", 1),
        (b"tru", 0),
        (b"fals", 0),
        (b"nll", 0),
        (b"1e", 2),
        (b"-", 1),
        (b"\"abc", 4),
        (b"\"abc\\", 4),
        (b"\"\\uD800\"", 1),
        (b"\"\\uDC00\"", 1),
        (b"{\"x\":\"\xc0\\q\"", 10),
    ];
    for &(token, at) in cases {
        for split in 0..=token.len() {
            let prefix = 4096 - split;
            let mut input = vec![b' '; prefix];
            input.extend_from_slice(token);
            let output = execute_observed(binary, &input);
            assert_eq!(output.status.code(), Some(2), "{token:?}, split {split}");
            assert!(output.stdout.is_empty());
            assert_eq!(
                output.stderr,
                format!("stream:1:{}\n", prefix + at).as_bytes(),
                "{token:?}, split {split}"
            );
        }
    }
    // Valid multi-byte scalars and surrogate pairs cross the same observed
    // provider boundary, then fail the explicit ASCII identifier schema policy.
    for token in [
        b"{\"servers\":[\"\xc3\xa9\"],\"patients\":[]}".as_slice(),
        b"{\"servers\":[\"\\uD83D\\uDE00\"],\"patients\":[]}",
    ] {
        for split in 0..=token.len() {
            let mut input = vec![b' '; 4096 - split];
            input.extend_from_slice(token);
            let output = execute_observed(binary, &input);
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert_eq!(output.stderr, b"schema:6:12\n"); // Normalized input domain.
        }
    }
}
