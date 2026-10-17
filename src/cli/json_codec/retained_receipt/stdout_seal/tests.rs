use super::*;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};

fn arguments() -> Vec<String> {
    [
        "missing-project",
        "--source",
        "src/schema.spx",
        "--type",
        "app.row",
        "--output",
        "new.json",
    ]
    .map(str::to_owned)
    .to_vec()
}
fn scratch(label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "codec-seal-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn expected_digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

#[test]
fn nonce_grammar_is_closed_bounded_and_preserves_all_ordinary_arguments() {
    for nonce in ["a", "-", "-single", "A_b-09", &"z".repeat(64)] {
        let mut args = arguments();
        args.extend([
            "--retained-receipt".to_owned(),
            FLAG.to_owned(),
            nonce.to_owned(),
        ]);
        let (ordinary, selected) = select(&args).unwrap();
        assert_eq!(selected, nonce);
        let mut expected = arguments();
        expected.push("--retained-receipt".to_owned());
        assert_eq!(ordinary, expected);
    }
    for nonce in ["", "a b", "a\nb", "é", "--other", "x/y", &"x".repeat(65)] {
        let mut args = arguments();
        args.extend([
            FLAG.to_owned(),
            nonce.to_owned(),
            "--retained-receipt".to_owned(),
        ]);
        assert_eq!(select(&args), Err(2), "nonce={nonce:?}");
    }
    let mut missing = arguments();
    assert_eq!(select(&missing), Err(2));
    missing.push(FLAG.to_owned());
    assert_eq!(select(&missing), Err(2));
    missing.extend(["n".to_owned(), FLAG.to_owned(), "n".to_owned()]);
    assert_eq!(select(&missing), Err(2));
}

#[test]
fn sealed_forms_require_exact_retained_route_before_project_authority() {
    for extra in [
        vec![FLAG, "n"],
        vec![FLAG, "n", "--plan", "p.json"],
        vec![FLAG, "n", "--explain"],
        vec![FLAG, "n", "--retained-receipt", "--plan", "p.json"],
        vec![FLAG, "n", "--retained-receipt", "--explain"],
        vec![FLAG, "n", "--retained-receipt", "--retained-receipt"],
        vec![FLAG, "n", "--retained-receipt", "--unknown", "value"],
    ] {
        let mut args = arguments();
        args.extend(extra.into_iter().map(str::to_owned));
        assert_eq!(
            super::super::super::dispatch(
                super::super::super::super::help::CommandId::JsonCodec,
                &args,
                |_| panic!("malformed sealed form reached Project authority")
            ),
            Err(2)
        );
    }
}

#[test]
fn seal_binds_exact_complete_buffer_including_lf_and_follows_publication() {
    let root = scratch("binding");
    let output = root.join("receipt.json");
    let bytes = b"{\"source\":\"original + generated\"}\n";
    struct Sink<'a> {
        output: &'a Path,
        expected: &'a [u8],
        bytes: Vec<u8>,
        flushes: usize,
    }
    impl Write for Sink<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            assert_eq!(fs::read(self.output).unwrap(), self.expected);
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }
    let mut sink = Sink {
        output: &output,
        expected: bytes,
        bytes: Vec::new(),
        flushes: 0,
    };
    publish_sealed(&output, bytes, "runner-1", &mut sink).unwrap();
    assert_eq!(sink.flushes, 1);
    assert!(sink.bytes.len() <= MAX_SEAL_BYTES);
    assert_eq!(sink.bytes.last(), Some(&b'\n'));
    assert_eq!(sink.bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    let seal: serde_json::Value = serde_json::from_slice(&sink.bytes).unwrap();
    assert_eq!(
        seal,
        serde_json::json!({
            "schema": "semaprax.json-codec-publication-seal.v1", "nonce": "runner-1",
            "receipt_bytes": bytes.len(), "receipt_sha256": expected_digest(bytes)
        })
    );
    assert_ne!(
        seal["receipt_sha256"],
        expected_digest(&bytes[..bytes.len() - 1])
    );
    let mut changed = bytes.to_vec();
    changed[2] ^= 1;
    assert_ne!(seal["receipt_sha256"], expected_digest(&changed));
    assert_eq!(fs::read(&output).unwrap(), bytes);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn publication_refusal_preserves_destination_and_never_writes_a_seal() {
    let root = scratch("refusal");
    let output = root.join("receipt.json");
    fs::write(&output, b"foreign exact bytes").unwrap();
    let expected = publish_new(&output, b"new\n").unwrap_err();
    let mut stdout = Vec::new();
    assert_eq!(
        publish_sealed(&output, b"new\n", "n", &mut stdout),
        Err(expected)
    );
    assert!(stdout.is_empty());
    assert_eq!(fs::read(&output).unwrap(), b"foreign exact bytes");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    let missing = root.join("absent/receipt.json");
    let expected = publish_new(&missing, b"new\n").unwrap_err();
    assert_eq!(
        publish_sealed(&missing, b"new\n", "n", &mut stdout),
        Err(expected)
    );
    assert!(stdout.is_empty());
    assert!(!missing.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn partial_stdout_failure_reports_publication_without_rollback() {
    let root = scratch("write-failure");
    let output = root.join("receipt.json");
    struct Broken {
        prefix: Vec<u8>,
    }
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.prefix.is_empty() {
                self.prefix.extend_from_slice(&bytes[..7]);
                Ok(7)
            } else {
                Err(io::Error::other("seal sink refused"))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("write failure must precede flush")
        }
    }
    let mut stdout = Broken { prefix: Vec::new() };
    assert_eq!(
        publish_sealed(&output, b"exact\n", "n", &mut stdout),
        Err(error(
            "codec receipt was published, but cannot write stdout seal: seal sink refused"
        ))
    );
    assert_eq!(fs::read(&output).unwrap(), b"exact\n");
    assert_eq!(stdout.prefix.len(), 7);
    assert!(serde_json::from_slice::<serde_json::Value>(&stdout.prefix).is_err());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn flush_failure_is_failure_even_after_a_complete_stdout_record() {
    let root = scratch("flush-failure");
    let output = root.join("receipt.json");
    struct Broken {
        bytes: Vec<u8>,
    }
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("flush refused"))
        }
    }
    let mut stdout = Broken { bytes: Vec::new() };
    assert_eq!(
        publish_sealed(&output, b"exact\n", "n", &mut stdout),
        Err(error(
            "codec receipt was published, but cannot write stdout seal: flush refused"
        ))
    );
    assert_eq!(fs::read(&output).unwrap(), b"exact\n");
    assert!(serde_json::from_slice::<serde_json::Value>(&stdout.bytes).is_ok());
    fs::remove_dir_all(root).unwrap();
}
