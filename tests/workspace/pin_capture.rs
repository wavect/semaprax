//! Bounded diagnostic receipts for complete fixture wires. No pin or verdict authority.
use std::fmt::Write as _;
use std::io::Write as _;

const MAX_CAPTURE_BYTES: usize = 64 * 1024;

fn sha(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

fn frame(label: &str, bytes: &[u8], digest: &str) -> Option<String> {
    if bytes.len() > MAX_CAPTURE_BYTES
        || !matches!(bytes.first(), Some(b'{' | b'['))
        || label.len() > 64
        || label.is_empty()
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return None;
    }
    let mut line = String::with_capacity(bytes.len() * 2 + 256);
    write!(
        line,
        "PUBLIC_WIRE_RECEIPT_V1 label={label} bytes={} digest={digest} hex=",
        bytes.len()
    )
    .unwrap();
    for byte in bytes {
        write!(line, "{byte:02x}").unwrap();
    }
    line.push('\n');
    Some(line)
}

fn enabled() -> bool {
    std::env::var_os("GITHUB_ACTIONS").is_some_and(|value| value == "true")
}

pub(super) fn observe(label: &str, bytes: &[u8], digest: String) -> String {
    if enabled() {
        if let Some(line) = frame(label, bytes, &digest) {
            // Diagnostic IO failure cannot replace a test verdict or its hash.
            let _ = std::io::stderr().lock().write_all(line.as_bytes());
        }
    }
    digest
}

pub(super) fn record(label: &str, bytes: &[u8]) {
    if enabled() && bytes.len() <= MAX_CAPTURE_BYTES {
        let _ = observe(label, bytes, sha(bytes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_wire_receipt_round_trips_every_byte_and_authenticates_the_full_payload() {
        let bytes = b"{\"schema\":\"receipt.fixture\",\"raw\":\"\x00\xff\n\r\"}";
        let digest = sha(bytes);
        let line = frame("fixture.json", bytes, &digest).unwrap();
        let encoded = line.split_once(" hex=").unwrap().1.trim_end();
        let decoded = (0..encoded.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&encoded[at..at + 2], 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(decoded, bytes);
        assert_eq!(sha(&decoded), digest);
        assert_eq!(line.lines().count(), 1);
        assert!(line.contains(&format!(" bytes={} ", bytes.len())));
        let mut changed = decoded;
        changed[1] ^= 1;
        assert_ne!(sha(&changed), digest);
    }

    #[test]
    fn receipt_bounds_and_label_rejection_do_not_change_the_original_hash() {
        let digest = sha(b"abc");
        assert_eq!(
            digest,
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(observe("plain", b"abc", digest.clone()), digest);
        assert!(frame("plain", b"abc", &digest).is_none());
        assert!(frame("empty", b"", &sha(b"")).is_none());
        let mut bytes = vec![b' '; MAX_CAPTURE_BYTES];
        bytes[0] = b'{';
        assert!(frame("exact", &bytes, &sha(&bytes)).is_some());
        bytes.push(b' ');
        assert!(frame("over", &bytes, &sha(&bytes)).is_none());
        for label in ["", "line\nbreak", "with space"] {
            assert!(frame(label, b"{}", &sha(b"{}")).is_none());
        }
        assert!(frame(&"a".repeat(65), b"{}", &sha(b"{}")).is_none());
    }
}
