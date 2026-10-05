//! Raw-byte bounded NDJSON framing for the project stdio transport.

use std::io::{self, BufRead, Write};

use super::codec;

pub(crate) const DEFAULT_MAX_REQUEST_BYTES: usize = 64 * 1024;
pub(crate) const DEFAULT_MAX_RESPONSE_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_REQUEST_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StdioLimits {
    request_bytes: usize,
    response_bytes: usize,
}

impl StdioLimits {
    pub(crate) fn new(request_bytes: usize, response_bytes: usize) -> Result<Self, String> {
        if request_bytes == 0 || request_bytes > MAX_REQUEST_BYTES {
            return Err(format!(
                "stdio request byte limit must be within 1..={MAX_REQUEST_BYTES}"
            ));
        }
        if response_bytes < codec::response_overflow_error().len() + 1
            || response_bytes > MAX_RESPONSE_BYTES
        {
            return Err(format!(
                "stdio response byte limit must be within {}..={MAX_RESPONSE_BYTES}",
                codec::response_overflow_error().len() + 1
            ));
        }
        Ok(Self {
            request_bytes,
            response_bytes,
        })
    }

    pub(crate) const fn request_bytes(self) -> usize {
        self.request_bytes
    }

    pub(crate) const fn response_bytes(self) -> usize {
        self.response_bytes
    }
}

impl Default for StdioLimits {
    fn default() -> Self {
        Self {
            request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Frame {
    Data(Vec<u8>),
    OversizedTerminal,
    Eof,
}

pub(crate) struct FrameReader<R: BufRead> {
    inner: R,
    max_bytes: usize,
    terminal: bool,
}

impl<R: BufRead> FrameReader<R> {
    pub(crate) fn new(inner: R, limits: StdioLimits) -> Self {
        Self {
            inner,
            max_bytes: limits.request_bytes(),
            terminal: false,
        }
    }

    /// Read exactly one LF-delimited frame. As soon as the retained bytes
    /// would exceed the cap, the reader becomes terminal and reports the
    /// condition once without another blocking read: a rejected session is
    /// never resynchronized, so waiting for its LF or EOF would only let a
    /// client hold the sequential daemon in unbounded rejected-input work.
    pub(crate) fn read_frame(&mut self) -> io::Result<Frame> {
        if self.terminal {
            return Ok(Frame::Eof);
        }
        let mut frame = Vec::new();
        loop {
            let available = self.inner.fill_buf()?;
            if available.is_empty() {
                self.terminal = true;
                return if frame.is_empty() {
                    Ok(Frame::Eof)
                } else {
                    Ok(Frame::Data(frame))
                };
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let payload_len = newline.unwrap_or(available.len());
            if payload_len > self.max_bytes - frame.len() {
                // Only bytes already delivered by this one read are consumed.
                self.inner.consume(payload_len);
                self.terminal = true;
                return Ok(Frame::OversizedTerminal);
            }
            frame.extend_from_slice(&available[..payload_len]);
            let consumed = newline.map_or(payload_len, |position| position + 1);
            self.inner.consume(consumed);
            if newline.is_some() {
                return Ok(Frame::Data(frame));
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn into_inner(self) -> R {
        self.inner
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WriteDisposition {
    Written,
    OverflowErrorWritten,
}

pub(crate) struct FrameWriter<W: Write> {
    inner: W,
    max_bytes: usize,
}

impl<W: Write> FrameWriter<W> {
    pub(crate) fn new(inner: W, limits: StdioLimits) -> Self {
        Self {
            inner,
            max_bytes: limits.response_bytes(),
        }
    }

    /// Write exactly one response plus one LF and flush it immediately. A
    /// response that exceeds the configured cap is replaced, never truncated.
    pub(crate) fn write_response(&mut self, response: &[u8]) -> io::Result<WriteDisposition> {
        if response.contains(&b'\n') || response.contains(&b'\r') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "stdio response contains a raw line break",
            ));
        }
        let framed_bytes = response.len().checked_add(1).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "stdio response size overflow")
        })?;
        let (response, disposition) = if framed_bytes > self.max_bytes {
            (
                codec::response_overflow_error(),
                WriteDisposition::OverflowErrorWritten,
            )
        } else {
            (response, WriteDisposition::Written)
        };
        self.inner.write_all(response)?;
        self.inner.write_all(b"\n")?;
        self.inner.flush()?;
        Ok(disposition)
    }

    #[cfg(test)]
    pub(crate) fn into_inner(self) -> W {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    fn limits(request: usize, response: usize) -> StdioLimits {
        StdioLimits::new(request, response).unwrap()
    }

    #[test]
    fn reader_accepts_exact_limit_and_final_unterminated_frames() {
        let input = Cursor::new(b"1234\n{}\nlast".to_vec());
        let mut reader = FrameReader::new(BufReader::with_capacity(2, input), limits(8, 256));
        assert_eq!(reader.read_frame().unwrap(), Frame::Data(b"1234".to_vec()));
        assert_eq!(reader.read_frame().unwrap(), Frame::Data(b"{}".to_vec()));
        assert_eq!(reader.read_frame().unwrap(), Frame::Data(b"last".to_vec()));
        assert_eq!(reader.read_frame().unwrap(), Frame::Eof);
    }

    /// Serves scripted chunks, then fails every further read. A reader that
    /// asks for more input after the cap is exceeded observes the sentinel.
    struct ScriptedChunks {
        chunks: std::collections::VecDeque<Vec<u8>>,
        offset: usize,
        reads: usize,
    }

    impl ScriptedChunks {
        fn new(chunks: &[&[u8]]) -> Self {
            Self {
                chunks: chunks.iter().map(|chunk| chunk.to_vec()).collect(),
                offset: 0,
                reads: 0,
            }
        }
    }

    impl io::Read for ScriptedChunks {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let available = self.fill_buf()?;
            let count = available.len().min(buffer.len());
            buffer[..count].copy_from_slice(&available[..count]);
            self.consume(count);
            Ok(count)
        }
    }

    impl BufRead for ScriptedChunks {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            if self
                .chunks
                .front()
                .is_some_and(|chunk| self.offset == chunk.len())
            {
                self.chunks.pop_front();
                self.offset = 0;
            }
            self.reads += 1;
            match self.chunks.front() {
                Some(chunk) => Ok(&chunk[self.offset..]),
                None => Err(io::Error::other("sentinel: read past rejected frame")),
            }
        }

        fn consume(&mut self, amount: usize) {
            self.offset += amount;
        }
    }

    #[test]
    fn oversized_frame_is_terminal_without_a_further_read() {
        let mut reader = FrameReader::new(ScriptedChunks::new(&[b"12345"]), limits(4, 256));
        assert_eq!(reader.read_frame().unwrap(), Frame::OversizedTerminal);
        assert_eq!(reader.read_frame().unwrap(), Frame::Eof);
        let inner = reader.into_inner();
        assert_eq!(inner.reads, 1, "rejection waited for LF or EOF");
        assert_eq!(inner.offset, 5, "only the delivered chunk is consumed");
    }

    #[test]
    fn cap_crossed_across_chunks_rejects_on_the_crossing_read() {
        let chunks: &[&[u8]] = &[b"1234", b"5", b"never requested"];
        let mut reader = FrameReader::new(ScriptedChunks::new(chunks), limits(4, 256));
        assert_eq!(reader.read_frame().unwrap(), Frame::OversizedTerminal);
        assert_eq!(reader.read_frame().unwrap(), Frame::Eof);
        let inner = reader.into_inner();
        assert_eq!(inner.reads, 2);
        assert_eq!(inner.chunks.back().unwrap(), b"never requested");
    }

    #[test]
    fn oversized_frame_is_terminal_and_trailing_frames_never_surface() {
        let input = Cursor::new(b"12345\nignored\n".to_vec());
        let mut reader = FrameReader::new(BufReader::with_capacity(2, input), limits(4, 256));
        assert_eq!(reader.read_frame().unwrap(), Frame::OversizedTerminal);
        assert_eq!(reader.read_frame().unwrap(), Frame::Eof);
        // Bounded read: only the cap plus one transport buffer was pulled.
        let inner = reader.into_inner().into_inner();
        assert!(inner.position() <= 4 + 2, "read {}", inner.position());
    }

    #[test]
    fn oversized_unterminated_frame_is_bounded_and_terminal() {
        let input = Cursor::new(vec![b'x'; 4096]);
        let mut reader = FrameReader::new(BufReader::with_capacity(7, input), limits(4, 256));
        assert_eq!(reader.read_frame().unwrap(), Frame::OversizedTerminal);
        assert_eq!(reader.read_frame().unwrap(), Frame::Eof);
        let inner = reader.into_inner().into_inner();
        assert_eq!(inner.position(), 7, "rejection drained past one buffer");
    }

    #[test]
    fn exact_cap_frames_split_across_chunks_stay_admitted() {
        let chunks: &[&[u8]] = &[b"12", b"34", b"\n\n{}", b"\nab", b"cd"];
        let mut reader = FrameReader::new(ScriptedChunks::new(chunks), limits(4, 256));
        assert_eq!(reader.read_frame().unwrap(), Frame::Data(b"1234".to_vec()));
        assert_eq!(reader.read_frame().unwrap(), Frame::Data(Vec::new()));
        assert_eq!(reader.read_frame().unwrap(), Frame::Data(b"{}".to_vec()));
        // The exact-cap final frame waits for its terminator like any frame.
        let error = reader.read_frame().unwrap_err();
        assert!(error.to_string().starts_with("sentinel"));
    }

    #[test]
    fn writer_uses_one_lf_flushes_and_replaces_oversized_responses() {
        let output = Cursor::new(Vec::new());
        let mut writer = FrameWriter::new(output, limits(8, 128));
        assert_eq!(
            writer.write_response(br#"{"ok":1}"#).unwrap(),
            WriteDisposition::Written
        );
        assert_eq!(
            writer.write_response(&[b'x'; 129]).unwrap(),
            WriteDisposition::OverflowErrorWritten
        );
        let bytes = writer.into_inner().into_inner();
        let expected = [
            br#"{"ok":1}"#.as_slice(),
            b"\n",
            codec::response_overflow_error(),
            b"\n",
        ]
        .concat();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn writer_rejects_raw_line_breaks_and_limits_are_closed() {
        let mut writer = FrameWriter::new(Vec::new(), limits(8, 128));
        assert_eq!(
            writer.write_response(b"{}\r\n").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(StdioLimits::new(0, 128).is_err());
        assert!(StdioLimits::new(8, codec::response_overflow_error().len()).is_err());
    }
}
