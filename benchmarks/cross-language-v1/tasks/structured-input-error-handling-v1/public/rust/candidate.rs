pub fn validate(kind: i64, version: i64, payload_len: i64) -> i64 {
    if kind != 7 { 1 } else if version != 1 { 2 } else if !(1..=64).contains(&payload_len) { 3 } else { 0 }
}
