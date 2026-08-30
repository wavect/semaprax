//! Independent, bounded data oracle; never derives expectations from SPX.
pub(super) const CORPUS: &[u8] = include_bytes!("adversarial.json");

pub(super) fn frames() -> Vec<(String, Vec<u8>, bool, i64)> {
    let value: serde_json::Value = serde_json::from_slice(CORPUS).unwrap();
    assert_eq!(value["schema"], "semaprax.frame-payload-corpus.v1");
    assert_eq!(value["maximum_frame_bytes"], 65536);
    let rows = value["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 72);
    let mut expected = Vec::new();
    for bit in 0..32 {
        let mut frame = b"SPX1".to_vec();
        frame.extend_from_slice(&(1u32 << bit).to_be_bytes());
        expected.push((format!("length-bit-{bit}"), frame, false, 3));
    }
    for bit in 0..16 {
        let length = 1usize << bit;
        let mut frame = b"SPX1".to_vec();
        frame.extend_from_slice(&(length as u32).to_be_bytes());
        frame.extend((0..length).map(|index| index as u8));
        expected.push((format!("valid-bit-{bit}"), frame, true, 0));
    }
    for (label, prefix) in [("magic", *b"SPX1\0\0\0\0"), ("bad", *b"\0PX1\0\0\0\0")] {
        for length in 0..8 {
            expected.push((
                format!("short-{label}-{length}"),
                prefix[..length].to_vec(),
                false,
                2,
            ));
        }
    }
    for index in 0..4 {
        let mut frame = b"SPX1".to_vec();
        frame[index] = 0;
        frame.extend_from_slice(&[255; 4]);
        expected.push((format!("bad-magic-{index}"), frame, false, 1));
    }
    for (label, length) in [
        ("ffffffff", u32::MAX),
        ("7fffffff", 0x7fff_ffff),
        ("80000001", 0x8000_0001),
        ("0000ffff", 65535),
    ] {
        let mut frame = b"SPX1".to_vec();
        frame.extend_from_slice(&length.to_be_bytes());
        expected.push((format!("oversized-{label}"), frame, false, 3));
    }
    let actual: Vec<_> = rows
        .iter()
        .map(|row| {
            let frame = if row["kind"] == "hex" {
                super::decode_hex(row["frame_hex"].as_str().unwrap())
            } else {
                assert_eq!(row["kind"], "generated-index-mod-256");
                let length = row["payload_length"].as_u64().unwrap();
                assert!(length <= 32768);
                let mut frame = b"SPX1".to_vec();
                frame.extend_from_slice(&(length as u32).to_be_bytes());
                frame.extend((0..length).map(|index| index as u8));
                frame
            };
            assert!(frame.len() <= 65536);
            (
                row["name"].as_str().unwrap().to_owned(),
                frame,
                row["valid"].as_bool().unwrap(),
                row["error"].as_i64().unwrap_or(0),
            )
        })
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(actual.iter().filter(|row| row.2).count(), 16);
    assert_eq!(
        actual
            .iter()
            .filter(|row| row.2)
            .map(|row| row.1.len() - 8)
            .sum::<usize>(),
        65535
    );
    actual
}

#[test]
fn supplemental_literal_inventory_matches_independent_format_oracle() {
    assert_eq!(frames().len(), 72);
}
