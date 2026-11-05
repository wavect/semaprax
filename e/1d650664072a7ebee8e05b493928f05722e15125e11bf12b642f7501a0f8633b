#![allow(
    clippy::duplicate_mod,
    reason = "The complete fixture intentionally compiles independently against current and predecessor owner types."
)]
//! Closed Main7f8 fixture reader implementation and bounded owning controls.
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Failure {
    Bound,
    Open,
    Metadata,
    NonRegular,
    Length,
    Allocation,
    Read,
    Tail,
    Digest,
    Utf8,
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn admit(length: usize, cap: usize) -> Result<(), Failure> {
    if length > cap {
        return Err(Failure::Bound);
    }
    Ok(())
}
fn acquire(length: usize) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| Failure::Allocation)?;
    bytes.resize(length, 0);
    Ok(bytes)
}
fn checked_read(
    mut reader: impl Read,
    length: usize,
    expected: &str,
    cap: usize,
    buffer: impl FnOnce(usize) -> Result<Vec<u8>, Failure>,
) -> Result<String, Failure> {
    admit(length, cap)?;
    let mut bytes = buffer(length)?;
    if bytes.len() != length {
        return Err(Failure::Allocation);
    }
    reader.read_exact(&mut bytes).map_err(|_| Failure::Read)?;
    let mut tail = [0u8; 1];
    if reader.read(&mut tail).map_err(|_| Failure::Read)? != 0 {
        return Err(Failure::Tail);
    }
    if sha(&bytes) != expected {
        return Err(Failure::Digest);
    }
    String::from_utf8(bytes).map_err(|_| Failure::Utf8)
}
fn checked_file(
    open: impl FnOnce() -> std::io::Result<File>,
    length: usize,
    expected: &str,
    cap: usize,
    buffer: impl FnOnce(usize) -> Result<Vec<u8>, Failure>,
) -> Result<String, Failure> {
    // The selected bound is checked before even opening; actual descriptor kind
    // and exact length are checked before acquiring any content buffer.
    admit(length, cap)?;
    let file = open().map_err(|_| Failure::Open)?;
    let metadata = file.metadata().map_err(|_| Failure::Metadata)?;
    if !metadata.is_file() {
        return Err(Failure::NonRegular);
    }
    if metadata.len() != u64::try_from(length).map_err(|_| Failure::Bound)? {
        return Err(Failure::Length);
    }
    checked_read(file, length, expected, cap, buffer)
}
pub(super) fn read(
    path: &Path,
    length: usize,
    expected: &str,
    cap: usize,
) -> Result<String, Failure> {
    checked_file(|| File::open(path), length, expected, cap, acquire)
}

// Called in the existing complete incoming-provider owner before fixture data,
// outside allocation/work observations. No original fixture is ever modified.
pub(super) fn controls() {
    use std::cell::Cell;
    use std::io::Cursor;
    let bytes = b"{}";
    let expected = sha(bytes);
    assert_eq!(
        checked_read(Cursor::new(bytes), 2, &expected, 2, acquire),
        Ok("{}".to_owned())
    );
    assert_eq!(
        checked_read(Cursor::new(b"{"), 2, &expected, 2, acquire),
        Err(Failure::Read)
    );
    assert_eq!(
        checked_read(Cursor::new(b"{} "), 2, &expected, 2, acquire),
        Err(Failure::Tail)
    );
    assert_eq!(
        checked_read(Cursor::new(b"[]"), 2, &expected, 2, acquire),
        Err(Failure::Digest)
    );
    assert_eq!(
        checked_read(Cursor::new([0xffu8]), 1, &sha(&[0xff]), 2, acquire),
        Err(Failure::Utf8)
    );
    assert_eq!(
        checked_read(Cursor::new([0xffu8]), 1, &expected, 2, acquire),
        Err(Failure::Digest),
        "SHA precedes UTF8"
    );
    let acquired = Cell::new(false);
    assert_eq!(
        checked_read(Cursor::new(bytes), usize::MAX, &expected, 2, |_| {
            acquired.set(true);
            panic!("refused bound must precede buffer acquisition")
        }),
        Err(Failure::Bound)
    );
    assert!(!acquired.get());
    assert_eq!(
        checked_read(Cursor::new(bytes), 2, &expected, 2, |_| Err(
            Failure::Allocation
        )),
        Err(Failure::Allocation)
    );
    struct ReadFailure;
    impl Read for ReadFailure {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::Other))
        }
    }
    assert_eq!(
        checked_read(ReadFailure, 2, &expected, 2, acquire),
        Err(Failure::Read)
    );

    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch = Scratch(std::env::temp_dir().join(format!(
        "semaprax-main7f8-reader-{}-{nonce}",
        std::process::id()
    )));
    std::fs::create_dir(&scratch.0).unwrap();
    let path = scratch.0.join("control.json");
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(read(&path, 2, &expected, 2), Ok("{}".to_owned()));
    assert_eq!(
        read(&scratch.0.join("missing.json"), 2, &expected, 2),
        Err(Failure::Open)
    );
    // Some platforms refuse opening directories before metadata can be read.
    let nonregular = checked_file(
        || File::open(&scratch.0),
        0,
        &sha(b""),
        2,
        |_| panic!("nonregular handle must precede buffer acquisition"),
    );
    assert!(
        nonregular == Err(Failure::NonRegular)
            || (cfg!(windows) && nonregular == Err(Failure::Open))
    );
    let opened = Cell::new(false);
    assert_eq!(
        checked_file(
            || {
                opened.set(true);
                File::open(&path)
            },
            3,
            &expected,
            2,
            |_| { panic!("bound must precede buffer acquisition") }
        ),
        Err(Failure::Bound)
    );
    assert!(!opened.get(), "hardcap refusal precedes opening");
    for length in [1, 3] {
        assert_eq!(
            checked_file(
                || File::open(&path),
                length,
                &expected,
                3,
                |_| { panic!("exact descriptor length must precede buffer acquisition") }
            ),
            Err(Failure::Length)
        );
    }
    std::fs::write(&path, b"[]").unwrap();
    assert_eq!(read(&path, 2, &expected, 2), Err(Failure::Digest));
    std::fs::write(&path, [0xffu8]).unwrap();
    assert_eq!(read(&path, 1, &sha(&[0xff]), 2), Err(Failure::Utf8));
}
