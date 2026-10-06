//! One boundary matrix over both streaming sinks (REF-05).
//!
//! The provider and tool sinks share their sticky-rejection, cancellation,
//! deadline, policy-epoch and byte-cap order; only the provider counts chunks.

use super::*;

const LIMIT: u64 = 8;
const DEADLINE: u64 = 100;
const EPOCH: u64 = 7;

#[derive(Clone, Copy, Debug)]
enum Kind {
    Provider,
    Tool,
}

struct Fixture {
    probe: FakeProbe,
    cancellation: AgentCancellation,
    sink: Sink,
}

enum Sink {
    Provider(ProviderSink),
    Tool(ToolResultSink),
}

impl Fixture {
    fn new(kind: Kind) -> Self {
        Self::with_chunk_limit(kind, 64)
    }

    fn with_chunk_limit(kind: Kind, chunk_limit: u64) -> Self {
        let probe = FakeProbe {
            cancelled: Rc::new(Cell::new(false)),
            elapsed: Rc::new(Cell::new(0)),
            epoch: Rc::new(Cell::new(EPOCH)),
        };
        let cancellation = AgentCancellation::new();
        let sink = match kind {
            Kind::Provider => {
                let mut limits = parse_profile(&fixture_profile()).unwrap().limits;
                limits.max_provider_response_bytes = LIMIT;
                limits.max_stream_chunks = chunk_limit;
                limits.max_elapsed_ms = DEADLINE;
                Sink::Provider(ProviderSink::new(
                    limits,
                    u64::MAX,
                    Box::new(probe.clone()),
                    EPOCH,
                    cancellation.clone(),
                ))
            }
            Kind::Tool => Sink::Tool(ToolResultSink::new(
                LIMIT,
                Box::new(probe.clone()),
                EPOCH,
                DEADLINE,
                cancellation.clone(),
            )),
        };
        Self {
            probe,
            cancellation,
            sink,
        }
    }

    fn push(&mut self, chunk: &[u8]) -> bool {
        match &mut self.sink {
            Sink::Provider(sink) => sink.push(chunk),
            Sink::Tool(sink) => sink.push(chunk),
        }
    }

    fn bytes(&self) -> &[u8] {
        match &self.sink {
            Sink::Provider(sink) => &sink.bounded.bytes,
            Sink::Tool(sink) => &sink.bounded.bytes,
        }
    }

    fn boundary(&self) -> Option<AgentRunStatus> {
        match &self.sink {
            Sink::Provider(sink) => sink.bounded.boundary,
            Sink::Tool(sink) => sink.bounded.boundary,
        }
    }

    fn rejection(&self) -> Option<SinkRejection> {
        match &self.sink {
            Sink::Provider(sink) => sink.bounded.rejection,
            Sink::Tool(sink) => sink.bounded.rejection,
        }
    }

    fn chunks(&self) -> Option<u64> {
        match &self.sink {
            Sink::Provider(sink) => Some(sink.chunks),
            Sink::Tool(_) => None,
        }
    }

    /// Asserts the selected failure stays put and no later push appends bytes,
    /// even after every boundary condition has been restored.
    fn assert_sticky(&mut self, kept: &[u8]) {
        let boundary = self.boundary();
        let rejection = self.rejection();
        self.probe.elapsed.set(0);
        self.probe.epoch.set(EPOCH);
        for chunk in [&b""[..], b"z", b"zzzzzzzzz"] {
            assert!(!self.push(chunk));
        }
        assert_eq!(self.bytes(), kept);
        assert_eq!(self.boundary(), boundary);
        assert!(self.rejection() == rejection);
    }
}

const KINDS: [Kind; 2] = [Kind::Provider, Kind::Tool];

#[test]
fn both_sinks_close_on_cancellation_deadline_equality_and_epoch_change() {
    for kind in KINDS {
        let mut sink = Fixture::new(kind);
        assert!(sink.push(b"ab"));
        sink.cancellation.cancel();
        assert!(!sink.push(b"c"), "{kind:?}");
        assert_eq!(sink.boundary(), Some(AgentRunStatus::Cancelled));
        assert!(sink.rejection().is_none());
        sink.assert_sticky(b"ab");

        let mut sink = Fixture::new(kind);
        sink.probe.elapsed.set(DEADLINE - 1);
        assert!(sink.push(b"ab"), "{kind:?}");
        sink.probe.elapsed.set(DEADLINE);
        assert!(!sink.push(b"c"), "{kind:?}");
        assert_eq!(sink.boundary(), Some(AgentRunStatus::DeadlineExceeded));
        sink.assert_sticky(b"ab");

        let mut sink = Fixture::new(kind);
        sink.probe.epoch.set(EPOCH + 1);
        assert!(!sink.push(b""), "{kind:?}");
        assert_eq!(sink.boundary(), Some(AgentRunStatus::PolicyRejected));
        assert_eq!(sink.chunks().unwrap_or(0), 0);
        sink.assert_sticky(b"");
    }
}

#[test]
fn both_sinks_admit_the_exact_byte_cap_and_refuse_cap_plus_one() {
    for kind in KINDS {
        let mut sink = Fixture::new(kind);
        assert!(sink.push(&[b'x'; LIMIT as usize - 1]), "{kind:?}");
        assert!(sink.push(b"y"), "{kind:?}");
        assert_eq!(sink.bytes().len(), LIMIT as usize);
        assert!(!sink.push(b"z"), "{kind:?}");
        assert!(sink.rejection() == Some(SinkRejection::Bytes));
        assert!(sink.boundary().is_none());
        let mut kept = vec![b'x'; LIMIT as usize - 1];
        kept.push(b'y');
        sink.assert_sticky(&kept);

        let mut sink = Fixture::new(kind);
        assert!(!sink.push(&[b'x'; LIMIT as usize + 1]), "{kind:?}");
        assert!(sink.rejection() == Some(SinkRejection::Bytes));
        sink.assert_sticky(b"");
    }
}

#[test]
fn simultaneous_failures_select_cancellation_then_deadline_then_epoch_then_bytes() {
    for kind in KINDS {
        let mut sink = Fixture::new(kind);
        sink.cancellation.cancel();
        sink.probe.elapsed.set(DEADLINE);
        sink.probe.epoch.set(EPOCH + 1);
        assert!(!sink.push(&[0; LIMIT as usize + 1]));
        assert_eq!(sink.boundary(), Some(AgentRunStatus::Cancelled), "{kind:?}");
        assert!(sink.rejection().is_none());

        let mut sink = Fixture::new(kind);
        sink.probe.elapsed.set(DEADLINE + 1);
        sink.probe.epoch.set(EPOCH + 1);
        assert!(!sink.push(&[0; LIMIT as usize + 1]));
        assert_eq!(
            sink.boundary(),
            Some(AgentRunStatus::DeadlineExceeded),
            "{kind:?}"
        );

        let mut sink = Fixture::new(kind);
        sink.probe.epoch.set(EPOCH + 1);
        assert!(!sink.push(&[0; LIMIT as usize + 1]));
        assert_eq!(
            sink.boundary(),
            Some(AgentRunStatus::PolicyRejected),
            "{kind:?}"
        );
        assert!(sink.rejection().is_none());
        assert_eq!(sink.chunks().unwrap_or(0), 0);

        // A byte refusal is sticky against a later boundary.
        let mut sink = Fixture::new(kind);
        assert!(!sink.push(&[0; LIMIT as usize + 1]));
        sink.cancellation.cancel();
        assert!(!sink.push(b""));
        assert!(sink.rejection() == Some(SinkRejection::Bytes));
        assert!(sink.boundary().is_none(), "{kind:?}");
    }
}

#[test]
fn fragmented_multibyte_input_is_buffered_as_bytes() {
    let text = "\u{e9}\u{20ac}";
    for kind in KINDS {
        let mut sink = Fixture::new(kind);
        for byte in text.as_bytes() {
            assert!(sink.push(std::slice::from_ref(byte)), "{kind:?}");
        }
        assert_eq!(sink.bytes(), text.as_bytes());
        assert!(sink.rejection().is_none() && sink.boundary().is_none());
    }
}

#[test]
fn provider_chunks_count_empty_and_rejected_pushes_and_win_over_bytes() {
    let mut sink = Fixture::with_chunk_limit(Kind::Provider, 3);
    assert!(sink.push(b""));
    assert!(sink.push(b"ab"));
    assert!(sink.push(b""));
    assert_eq!(sink.chunks(), Some(3));
    // At the chunk limit, an over-cap chunk is a chunk rejection, not a byte one.
    assert!(!sink.push(&[0; LIMIT as usize + 1]));
    assert!(sink.rejection() == Some(SinkRejection::Chunks));
    assert_eq!(sink.chunks(), Some(4));
    sink.assert_sticky(b"ab");
    assert_eq!(sink.chunks(), Some(4));

    // A byte rejection still consumed its chunk.
    let mut sink = Fixture::with_chunk_limit(Kind::Provider, 3);
    assert!(!sink.push(&[0; LIMIT as usize + 1]));
    assert!(sink.rejection() == Some(SinkRejection::Bytes));
    assert_eq!(sink.chunks(), Some(1));

    // A boundary refusal does not.
    let mut sink = Fixture::with_chunk_limit(Kind::Provider, 3);
    sink.cancellation.cancel();
    assert!(!sink.push(b""));
    assert_eq!(sink.chunks(), Some(0));
}

#[test]
fn tool_sink_has_no_chunk_limit() {
    let mut sink = Fixture::with_chunk_limit(Kind::Tool, 1);
    for _ in 0..=MAX_STREAM_CHUNKS {
        assert!(sink.push(b""));
    }
    for byte in 0..LIMIT as u8 {
        assert!(sink.push(&[byte]));
    }
    assert_eq!(sink.bytes().len(), LIMIT as usize);
    assert!(sink.rejection().is_none());
}
