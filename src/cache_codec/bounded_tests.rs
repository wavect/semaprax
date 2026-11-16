// Encoded byte bounds and immutable legacy wire checks.
use super::*;

#[test]
fn boxed_sequence_keeps_vector_wire_depth_nodes_and_allocation_refusal() {
    let values = vec![7u16, 2u16, 7u16];
    let wire = encode(&values).unwrap();
    let payload = values.len() * std::mem::size_of::<u16>();
    let decoder = || Decoder {
        bytes: &wire,
        offset: 0,
        nodes: 0,
        depth: 0,
        compact_loan_rows: false,
        allocation: MAX_ALLOCATION - payload,
    };
    let mut vector = decoder();
    let decoded_vector = Vec::<u16>::decode(&mut vector).unwrap();
    let mut boxed = decoder();
    let decoded_box = Box::<[u16]>::decode(&mut boxed).unwrap();
    assert_eq!(decoded_box.as_ref(), decoded_vector.as_slice());
    assert_eq!(encode(&decoded_box).unwrap(), wire);
    assert_eq!(boxed.offset, vector.offset);
    assert_eq!(boxed.nodes, vector.nodes);
    assert_eq!(boxed.depth, vector.depth);
    assert_eq!(boxed.allocation, MAX_ALLOCATION);
    let mut refused = decoder();
    refused.allocation += 1;
    assert_eq!(
        Box::<[u16]>::decode(&mut refused).unwrap_err()[0].code,
        "SPX-G305"
    );
    assert_eq!(refused.allocation, MAX_ALLOCATION - payload + 1);
    assert_eq!(
        refused.offset, 4,
        "refuse before allocating or decoding items"
    );
}

#[test]
fn bounded_encoding_keeps_legacy_wire_and_exact_utf8_boundary() {
    let value = String::from("é");
    let expected = vec![2, 0, 0, 0, 0xc3, 0xa9];
    assert_eq!(encode(&value).unwrap(), expected);
    assert_eq!(encode_bounded(&value, expected.len()).unwrap(), expected);
    assert_eq!(encode_bounded(&value, MAX_BYTES).unwrap(), expected);
    assert_eq!(decode::<String>(&expected).unwrap(), value);
    let errors = encode_bounded(&value, expected.len() - 1).unwrap_err();
    assert_eq!(errors[0].code, "SPX-G305");
}

#[test]
fn bounded_encoding_handles_zero_small_and_excessive_limits() {
    assert_eq!(encode_bounded(&7u8, 1).unwrap(), vec![7]);
    assert_eq!(encode_bounded(&7u8, 0).unwrap_err()[0].code, "SPX-G305");
    assert_eq!(
        encode_bounded(&7u8, MAX_BYTES + 1).unwrap_err()[0].code,
        "SPX-G305"
    );
    let mut empty = Encoder {
        bytes: Vec::new(),
        compact_loan_rows: false,
        node_limit_reached: false,
        byte_limit: 0,
        nodes: 0,
        depth: 0,
    };
    empty.raw(&[]).unwrap();
    assert!(empty.bytes.is_empty());
    assert_eq!(empty.raw(&[1]).unwrap_err()[0].code, "SPX-G305");
    assert!(empty.bytes.is_empty());
}

#[test]
fn bounded_raw_refusal_preserves_prior_bytes_at_growth_boundary() {
    let mut out = Encoder {
        bytes: Vec::new(),
        compact_loan_rows: false,
        node_limit_reached: false,
        byte_limit: 257,
        nodes: 0,
        depth: 0,
    };
    out.raw(&[17; 256]).unwrap();
    assert_eq!(out.raw(&[18, 19]).unwrap_err()[0].code, "SPX-G305");
    assert_eq!(out.bytes, vec![17; 256]);
    out.raw(&[20]).unwrap();
    assert_eq!(out.bytes.len(), 257);
    assert_eq!(out.bytes[256], 20);
    assert_eq!(out.raw(&[21]).unwrap_err()[0].code, "SPX-G305");
    assert_eq!(out.bytes[256], 20);
}

#[test]
fn snapshot_compacts_only_after_the_encoder_reaches_its_node_limit() {
    use std::cell::Cell;

    struct Probe<'a> {
        exhaust_nodes: bool,
        calls: &'a Cell<usize>,
    }
    impl Codec for Probe<'_> {
        fn encode(&self, out: &mut Encoder) -> Result<()> {
            self.calls.set(self.calls.get() + 1);
            if out.compact_loan_rows {
                return 19u8.encode(out);
            }
            if self.exhaust_nodes {
                out.nodes = MAX_NODES;
                return 19u8.encode(out);
            }
            Err(capacity("cache codec exceeds its node limit"))
        }

        fn decode(_: &mut Decoder<'_>) -> Result<Self> {
            unreachable!("encoder-only control")
        }
    }

    let calls = Cell::new(0);
    let compact = encode_snapshot_wire(&Probe {
        exhaust_nodes: true,
        calls: &calls,
    })
    .unwrap();
    assert!(is_compact_snapshot(&compact));
    assert_eq!(decode_snapshot_wire::<u8>(&compact).unwrap(), 19);
    assert_eq!(calls.get(), 2);

    calls.set(0);
    let errors = encode_snapshot_wire(&Probe {
        exhaust_nodes: false,
        calls: &calls,
    })
    .unwrap_err();
    assert_eq!(errors[0].code, "SPX-G305");
    assert_eq!(calls.get(), 1, "diagnostic text confers no retry authority");

    let legacy = encode_snapshot_wire(&19u8).unwrap();
    assert_eq!(legacy, encode(&19u8).unwrap());
    assert!(!is_compact_snapshot(&legacy));
}

#[test]
fn compact_loan_rows_preserve_every_field_and_refuse_invalid_counts() {
    use crate::hir::ExpressionId;
    use crate::loan_plan::{LoanEdge, LoanEndpoint, LoanId, LoanPointPhase, LoanProgramPoint};

    let endpoint = LoanEndpoint {
        point: LoanProgramPoint {
            expression: ExpressionId::from_owned("compact.point".to_owned()),
            phase: LoanPointPhase::After,
        },
        live_before: vec![LoanId(0), LoanId(7)].into_boxed_slice(),
        starts: vec![LoanId(9)].into_boxed_slice(),
        kills: Box::default(),
        live_after: vec![LoanId(0), LoanId(9)].into_boxed_slice(),
    };
    let wire = encode_inner(&endpoint, MAX_BYTES, &[COMPACT_LOAN_SNAPSHOT], true)
        .map_err(EncodeRejection::into_diagnostics)
        .unwrap();
    assert_eq!(
        decode_snapshot_wire::<LoanEndpoint>(&wire).unwrap(),
        endpoint
    );
    assert_eq!(
        decode::<LoanEndpoint>(&encode(&endpoint).unwrap()).unwrap(),
        endpoint
    );

    let edge = LoanEdge {
        from: 17,
        to: 29,
        live: vec![LoanId(3)].into_boxed_slice(),
    };
    let wire = encode_inner(&edge, MAX_BYTES, &[COMPACT_LOAN_SNAPSHOT], true)
        .map_err(EncodeRejection::into_diagnostics)
        .unwrap();
    assert_eq!(decode_snapshot_wire::<LoanEdge>(&wire).unwrap(), edge);
    assert_eq!(decode::<LoanEdge>(&encode(&edge).unwrap()).unwrap(), edge);

    let mut unknown = wire.clone();
    unknown[COMPACT_LOAN_SNAPSHOT.len() + 6] = 1;
    assert_eq!(
        decode_snapshot_wire::<LoanEdge>(&unknown).unwrap_err()[0].code,
        "SPX-G304"
    );

    let mut oversized = wire.clone();
    oversized[COMPACT_LOAN_SNAPSHOT.len() + 5] = 1;
    assert_eq!(
        decode_snapshot_wire::<LoanEdge>(&oversized).unwrap_err()[0].code,
        "SPX-G305"
    );

    assert_eq!(
        decode_snapshot_wire::<LoanEdge>(&wire[..wire.len() - 1]).unwrap_err()[0].code,
        "SPX-G304"
    );
}
