//! Address encodings used by recipient admission: base58, bech32 checksum
//! verification, and base-conversion.

pub(super) const BASE58_ALPHABET: &[u8] =
    b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

pub(super) fn encode_base58(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    let zeros = bytes.iter().take_while(|byte| **byte == 0).count();
    let mut digits = Vec::new();
    for byte in bytes.iter().skip(zeros) {
        let mut carry = u32::from(*byte);
        for digit in &mut digits {
            let value = u32::from(*digit) * 256 + carry;
            *digit = (value % 58) as u8;
            carry = value / 58;
        }
        while carry != 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let mut encoded = String::with_capacity(zeros + digits.len());
    encoded.extend(std::iter::repeat_n('1', zeros));
    for digit in digits.iter().rev() {
        encoded.push(BASE58_ALPHABET[usize::from(*digit)] as char);
    }
    encoded
}

pub(super) fn decode_base58_32(value: &str) -> Option<[u8; 32]> {
    if value.is_empty() {
        return None;
    }
    let mut output = [0u8; 32];
    for byte in value.bytes() {
        let digit = BASE58_ALPHABET
            .iter()
            .position(|candidate| *candidate == byte)? as u32;
        let mut carry = digit;
        for slot in output.iter_mut().rev() {
            let expanded = u32::from(*slot) * 58 + carry;
            *slot = expanded as u8;
            carry = expanded >> 8;
        }
        if carry != 0 {
            return None;
        }
    }
    (encode_base58(&output) == value).then_some(output)
}
pub(super) fn decode_regtest_p2wpkh(value: &str) -> Option<Vec<u8>> {
    if value.to_ascii_lowercase() != value || !value.starts_with("bcrt1q") {
        return None;
    }
    let position = value.rfind('1')?;
    let hrp = &value[..position];
    if hrp != "bcrt" {
        return None;
    }
    let charset = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    let data: Vec<u8> = value[position + 1..]
        .bytes()
        .map(|b| charset.iter().position(|v| *v == b).map(|n| n as u8))
        .collect::<Option<_>>()?;
    if data.len() < 7 || !bech32_verify(hrp, &data) {
        return None;
    }
    let payload = &data[..data.len() - 6];
    if payload.first() != Some(&0) {
        return None;
    }
    let program = convert_bits(&payload[1..], 5, 8, false)?;
    if program.len() != 20 {
        return None;
    }
    let mut script = vec![0x00, 0x14];
    script.extend(program);
    Some(script)
}
pub(super) fn bech32_verify(hrp: &str, data: &[u8]) -> bool {
    let mut values = Vec::new();
    for b in hrp.bytes() {
        values.push(b >> 5);
    }
    values.push(0);
    for b in hrp.bytes() {
        values.push(b & 31);
    }
    values.extend_from_slice(data);
    let mut chk = 1u32;
    for v in values {
        let top = chk >> 25;
        chk = ((chk & 0x1ffffff) << 5) ^ u32::from(v);
        for (index, g) in [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
            .iter()
            .enumerate()
        {
            if ((top >> index) & 1) != 0 {
                chk ^= *g;
            }
        }
    }
    chk == 1
}
pub(super) fn convert_bits(data: &[u8], from: u32, to: u32, pad: bool) -> Option<Vec<u8>> {
    let mut acc = 0u32;
    let mut bits = 0u32;
    let maxv = (1u32 << to) - 1;
    let mut out = Vec::new();
    for value in data {
        if (u32::from(*value) >> from) != 0 {
            return None;
        }
        acc = (acc << from) | u32::from(*value);
        bits += from;
        while bits >= to {
            bits -= to;
            out.push(((acc >> bits) & maxv) as u8);
        }
    }
    if pad {
        if bits > 0 {
            out.push(((acc << (to - bits)) & maxv) as u8);
        }
    } else if bits >= from || ((acc << (to - bits)) & maxv) != 0 {
        return None;
    }
    Some(out)
}
