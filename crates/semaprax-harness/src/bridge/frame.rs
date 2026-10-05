//! Bounded LF-delimited frame reader shared by the bridge stdio and skills MCP
//! servers (MA-06). The byte cap is enforced on each buffered chunk *before*
//! it is copied, so a hostile client can neither grow the buffer past the cap
//! (plus the reader's own fixed buffer) nor need a newline for the refusal.
//! Same `fill_buf`/`consume` shape as the adapter-stdout reader in
//! `host::process`.
//!
//! Framing matches `BufRead::lines`: LF terminates a frame, one trailing CR is
//! dropped, an unterminated tail at EOF is a frame, and an empty stream is EOF.
//! The result must be valid UTF-8; strict JSON validation stays with the caller.

use std::io::{self, BufRead};

#[derive(Debug)]
pub enum FrameError {
    /// Transport read failure (`ErrorKind::Interrupted` is retried).
    Io(io::Error),
    /// The frame crossed the cap; nothing beyond the cap was buffered.
    TooLarge { cap: usize },
    /// The frame was not valid UTF-8.
    Utf8,
}

impl FrameError {
    /// The session-ending I/O error; the original read error is preserved.
    pub fn into_io(self) -> io::Error {
        match self {
            Self::Io(e) => e,
            Self::TooLarge { cap } => io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame exceeds the {cap}-byte limit (SPX-HPA002); session closed"),
            ),
            Self::Utf8 => io::Error::new(
                io::ErrorKind::InvalidData,
                "stream did not contain valid UTF-8",
            ),
        }
    }
}

/// Read the next frame, `Ok(None)` at clean EOF. A frame of exactly `cap`
/// bytes is accepted; `cap + 1` is refused.
pub fn read_frame<R: BufRead>(r: &mut R, cap: usize) -> Result<Option<String>, FrameError> {
    // One byte of slack so a `cap`-byte frame ending in CR is not refused
    // before its CR is dropped; the exact check follows below.
    let hard = cap + 1;
    let mut line: Vec<u8> = Vec::new();
    let mut any = false;
    loop {
        let buf = match r.fill_buf() {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(FrameError::Io(e)),
        };
        if buf.is_empty() {
            if !any {
                return Ok(None);
            }
            break;
        }
        any = true;
        let (take, found) = match buf.iter().position(|b| *b == b'\n') {
            Some(i) => (i, true),
            None => (buf.len(), false),
        };
        if line.len() + take > hard {
            return Err(FrameError::TooLarge { cap });
        }
        line.extend_from_slice(&buf[..take]);
        r.consume(take + usize::from(found));
        if found {
            break;
        }
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    if line.len() > cap {
        return Err(FrameError::TooLarge { cap });
    }
    String::from_utf8(line)
        .map(Some)
        .map_err(|_| FrameError::Utf8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor, Read};

    /// Serves fixed chunks, then EOF, counting bytes handed out.
    struct Chunks {
        chunks: Vec<Vec<u8>>,
        at: usize,
        off: usize,
        served: std::rc::Rc<std::cell::Cell<usize>>,
    }
    impl Chunks {
        fn new(chunks: Vec<Vec<u8>>) -> (Self, std::rc::Rc<std::cell::Cell<usize>>) {
            let served = std::rc::Rc::new(std::cell::Cell::new(0));
            (
                Self {
                    chunks,
                    at: 0,
                    off: 0,
                    served: served.clone(),
                },
                served,
            )
        }
    }
    impl Read for Chunks {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            let Some(c) = self.chunks.get(self.at) else {
                return Ok(0);
            };
            let n = (c.len() - self.off).min(out.len());
            out[..n].copy_from_slice(&c[self.off..self.off + n]);
            self.off += n;
            if self.off == c.len() {
                self.at += 1;
                self.off = 0;
            }
            self.served.set(self.served.get() + n);
            Ok(n)
        }
    }

    const CAP: usize = 64;

    fn frames(chunks: Vec<Vec<u8>>) -> Vec<Result<Option<String>, String>> {
        let (c, _) = Chunks::new(chunks);
        let mut r = BufReader::with_capacity(16, c);
        let mut v = vec![];
        loop {
            match read_frame(&mut r, CAP) {
                Ok(None) => {
                    v.push(Ok(None));
                    return v;
                }
                Ok(Some(s)) => v.push(Ok(Some(s))),
                Err(e) => {
                    v.push(Err(format!("{e:?}")));
                    return v;
                }
            }
        }
    }

    #[test]
    fn exactly_at_limit_is_accepted_and_one_more_is_refused() {
        let ok = "a".repeat(CAP);
        let r = frames(vec![format!("{ok}\n").into_bytes()]);
        assert_eq!(r[0], Ok(Some(ok.clone())));
        let r = frames(vec![format!("{ok}b\n").into_bytes()]);
        assert!(r[0].as_ref().unwrap_err().contains("TooLarge"), "{r:?}");
        // CRLF: the cap applies to the frame, not its CR.
        let r = frames(vec![format!("{ok}\r\n").into_bytes()]);
        assert_eq!(r[0], Ok(Some(ok)));
        let r = frames(vec![format!("{}\r\n", "a".repeat(CAP + 1)).into_bytes()]);
        assert!(r[0].is_err());
    }

    #[test]
    fn cap_crossing_split_across_chunks_is_refused() {
        let r = frames(vec![vec![b'a'; 40], vec![b'a'; 40], b"\n".to_vec()]);
        assert!(r[0].as_ref().unwrap_err().contains("TooLarge"), "{r:?}");
        // Exactly the cap split across chunks stays valid.
        let r = frames(vec![vec![b'a'; 40], vec![b'a'; 24], b"\n".to_vec()]);
        assert_eq!(r[0], Ok(Some("a".repeat(CAP))));
    }

    #[test]
    fn unterminated_large_tail_stops_reading_without_a_newline() {
        let (c, served) = Chunks::new(vec![vec![b'x'; 1 << 20]]);
        let mut r = BufReader::with_capacity(16, c);
        let e = read_frame(&mut r, CAP).unwrap_err();
        assert!(matches!(e, FrameError::TooLarge { .. }));
        // Cap plus a small fixed transport buffer, not the whole megabyte.
        assert!(served.get() <= CAP + 1 + 2 * 16, "served {}", served.get());
    }

    #[test]
    fn invalid_utf8_and_read_errors_are_distinct() {
        let r = frames(vec![vec![0xff, 0xfe, b'\n']]);
        assert!(r[0].as_ref().unwrap_err().contains("Utf8"), "{r:?}");
        assert_eq!(
            FrameError::Utf8.into_io().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn clean_eof_unterminated_tail_and_coalesced_frames() {
        assert_eq!(frames(vec![]), vec![Ok(None)]);
        let r = frames(vec![b"{\"a\":1}\n\n{\"b\":2}\r\n{\"c\":3}".to_vec()]);
        assert_eq!(
            r,
            vec![
                Ok(Some("{\"a\":1}".into())),
                Ok(Some(String::new())),
                Ok(Some("{\"b\":2}".into())),
                Ok(Some("{\"c\":3}".into())),
                Ok(None)
            ]
        );
        let mut c = Cursor::new(b"x\n".to_vec());
        assert_eq!(read_frame(&mut c, CAP).unwrap().as_deref(), Some("x"));
        assert!(read_frame(&mut c, CAP).unwrap().is_none());
    }

    #[test]
    fn read_error_is_preserved_and_interrupts_retried() {
        struct Bad(u8);
        impl Read for Bad {
            fn read(&mut self, o: &mut [u8]) -> io::Result<usize> {
                self.0 += 1;
                match self.0 {
                    1 => Err(io::ErrorKind::Interrupted.into()),
                    2 => {
                        o[0] = b'z';
                        Ok(1)
                    }
                    _ => Err(io::Error::new(io::ErrorKind::BrokenPipe, "boom")),
                }
            }
        }
        let mut r = BufReader::new(Bad(0));
        match read_frame(&mut r, CAP) {
            Err(FrameError::Io(e)) => assert_eq!(e.kind(), io::ErrorKind::BrokenPipe),
            o => panic!("{o:?}"),
        }
    }
}
