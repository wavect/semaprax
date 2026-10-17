//! Correlates exact published bytes; stdout carries no execution authority.
use super::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;

const FLAG: &str = "--receipt-stdout-seal";
const MAX_NONCE_BYTES: usize = 64;
const MAX_SEAL_BYTES: usize = 512;

#[derive(Serialize)]
struct PublicationSeal<'a> {
    schema: &'static str,
    nonce: &'a str,
    receipt_bytes: usize,
    receipt_sha256: String,
}

fn select(args: &[String]) -> Result<(Vec<String>, &str), u8> {
    let fail = || {
        eprintln!(
            "{USAGE} --retained-receipt {FLAG} <nonce: 1..64 ASCII letters/digits/_/-, no leading -- >"
        );
        2
    };
    let mut positions = args
        .iter()
        .enumerate()
        .filter(|(_, arg)| arg.as_str() == FLAG);
    let (index, _) = positions.next().ok_or_else(fail)?;
    if positions.next().is_some() {
        return Err(fail());
    }
    let nonce = args.get(index + 1).ok_or_else(fail)?;
    if nonce.is_empty()
        || nonce.starts_with("--")
        || nonce.len() > MAX_NONCE_BYTES
        || !nonce
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(fail());
    }
    let mut ordinary = args.to_vec();
    ordinary.drain(index..index + 2);
    Ok((ordinary, nonce))
}

pub(super) fn run(
    args: &[String],
    report: impl Fn(&[Diagnostic]) -> u8,
    stdout: &mut impl Write,
) -> Result<(), u8> {
    let (ordinary, nonce) = select(args)?;
    // The unchanged receiver still validates exactly one retained flag and all
    // ordinary selectors before Project authority or any publication.
    super::run_with_publisher(&ordinary, report, |output, bytes| {
        publish_sealed(output, bytes, nonce, stdout)
    })
}

fn publish_sealed(
    output: &Path,
    bytes: &[u8],
    nonce: &str,
    stdout: &mut impl Write,
) -> Result<(), Diagnostic> {
    // Hash the exact original publication buffer, including its LF. Never
    // reopen an agent-writable artifact to decide what the compiler produced.
    let seal = PublicationSeal {
        schema: "semaprax.json-codec-publication-seal.v1",
        nonce,
        receipt_bytes: bytes.len(),
        receipt_sha256: format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
        ),
    };
    let mut encoded =
        serde_json::to_vec(&seal).map_err(|_| error("cannot encode codec stdout seal"))?;
    encoded.push(b'\n');
    if encoded.len() > MAX_SEAL_BYTES {
        return Err(error("codec stdout seal exceeds 512 bytes"));
    }
    publish_new(output, bytes)?;
    // A failed write/flush can leave a prefix on stdout. The published file
    // remains: no rollback, second artifact, or successful seal is asserted.
    stdout
        .write_all(&encoded)
        .and_then(|()| stdout.flush())
        .map_err(|failure| {
            error(format!(
                "codec receipt was published, but cannot write stdout seal: {failure}"
            ))
        })
}

#[cfg(test)]
#[path = "stdout_seal/tests.rs"]
mod tests;
