//! A closed, bounded JSON value codec for the reference service.
//!
//! The reference-service host crate has no JSON dependency, so this module
//! owns the exact shapes it serves: objects with unique string keys, UTF-8
//! strings, signed 64-bit integers, booleans, null, and arrays. Floats,
//! duplicate keys, trailing bytes, and over-deep nesting are refused, never
//! coerced. Rendering is canonical: object keys sort byte-wise, strings use
//! minimal escapes, and no whitespace is emitted.

/// A closed JSON value. Objects keep validator-chosen order; [`render`]
/// always emits keys in byte-wise sorted order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

/// Stable refusal categories for a malformed or out-of-bounds document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonRefusal {
    TooLarge,
    Malformed,
    TooDeep,
    TooManyValues,
    UnsupportedNumber,
    DuplicateKey,
}

const MAX_DEPTH: usize = 8;
const MAX_VALUES: usize = 4_096;
const MAX_STRING_BYTES: usize = 8_192;

/// Parse exactly one JSON document. `max_bytes` bounds the input; structure
/// is bounded independently by depth, value count, and string length.
pub fn parse(bytes: &[u8], max_bytes: usize) -> Result<JsonValue, JsonRefusal> {
    if bytes.len() > max_bytes {
        return Err(JsonRefusal::TooLarge);
    }
    let mut parser = Parser {
        bytes,
        position: 0,
        values: 0,
    };
    let value = parser.value(0)?;
    parser.whitespace();
    if parser.position != bytes.len() {
        return Err(JsonRefusal::Malformed);
    }
    Ok(value)
}

/// Render one value in canonical form: sorted keys, minimal escapes, no
/// whitespace. Rendering never fails; unrepresentable content cannot exist
/// because only parsed or validator-built values reach it.
pub fn render(value: &JsonValue) -> String {
    let mut out = String::new();
    render_into(value, &mut out);
    out
}

impl JsonValue {
    /// Look up one object member. Non-objects yield `None`.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            Self::Object(members) => members
                .iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// Require a closed object with exactly `keys` (any order, no extras).
    pub fn closed(&self, keys: &[&str]) -> Option<&[(String, JsonValue)]> {
        match self {
            Self::Object(members) if members.len() == keys.len() => {
                for key in keys {
                    if !members.iter().any(|(candidate, _)| candidate == key) {
                        return None;
                    }
                }
                Some(members)
            }
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(values) => Some(values),
            _ => None,
        }
    }
}

fn render_into(value: &JsonValue, out: &mut String) {
    match value {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(true) => out.push_str("true"),
        JsonValue::Bool(false) => out.push_str("false"),
        JsonValue::Int(number) => out.push_str(&number.to_string()),
        JsonValue::Str(text) => render_string(text, out),
        JsonValue::Array(values) => {
            out.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                render_into(value, out);
            }
            out.push(']');
        }
        JsonValue::Object(members) => {
            let mut sorted: Vec<&(String, JsonValue)> = members.iter().collect();
            sorted.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
            out.push('{');
            for (index, (key, value)) in sorted.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                render_string(key, out);
                out.push(':');
                render_into(value, out);
            }
            out.push('}');
        }
    }
}

fn render_string(text: &str, out: &mut String) {
    out.push('"');
    for scalar in text.chars() {
        match scalar {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{9}' => out.push_str("\\t"),
            '\u{A}' => out.push_str("\\n"),
            '\u{C}' => out.push_str("\\f"),
            '\u{D}' => out.push_str("\\r"),
            '\u{0}'..='\u{1F}' => {
                out.push_str(&format!("\\u{:04x}", scalar as u32));
            }
            _ => out.push(scalar),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    bytes: &'a [u8],
    position: usize,
    values: usize,
}

impl Parser<'_> {
    fn value(&mut self, depth: usize) -> Result<JsonValue, JsonRefusal> {
        if depth > MAX_DEPTH {
            return Err(JsonRefusal::TooDeep);
        }
        self.count()?;
        self.whitespace();
        let byte = self.peek().ok_or(JsonRefusal::Malformed)?;
        match byte {
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => Ok(JsonValue::Str(self.string()?)),
            b't' => self.literal("true", JsonValue::Bool(true)),
            b'f' => self.literal("false", JsonValue::Bool(false)),
            b'n' => self.literal("null", JsonValue::Null),
            b'-' | b'0'..=b'9' => Ok(JsonValue::Int(self.integer()?)),
            _ => Err(JsonRefusal::Malformed),
        }
    }

    fn count(&mut self) -> Result<(), JsonRefusal> {
        self.values = self.values.saturating_add(1);
        if self.values > MAX_VALUES {
            return Err(JsonRefusal::TooManyValues);
        }
        Ok(())
    }

    fn object(&mut self, depth: usize) -> Result<JsonValue, JsonRefusal> {
        self.expect(b'{')?;
        let mut members = Vec::new();
        self.whitespace();
        if self.consume(b'}') {
            return Ok(JsonValue::Object(members));
        }
        loop {
            self.whitespace();
            if self.peek() != Some(b'"') {
                return Err(JsonRefusal::Malformed);
            }
            let key = self.string()?;
            self.whitespace();
            self.expect(b':')?;
            let value = self.value(depth + 1)?;
            if members.iter().any(|(candidate, _)| *candidate == key) {
                return Err(JsonRefusal::DuplicateKey);
            }
            members.push((key, value));
            self.whitespace();
            if self.consume(b',') {
                continue;
            }
            self.expect(b'}')?;
            return Ok(JsonValue::Object(members));
        }
    }

    fn array(&mut self, depth: usize) -> Result<JsonValue, JsonRefusal> {
        self.expect(b'[')?;
        let mut values = Vec::new();
        self.whitespace();
        if self.consume(b']') {
            return Ok(JsonValue::Array(values));
        }
        loop {
            values.push(self.value(depth + 1)?);
            self.whitespace();
            if self.consume(b',') {
                continue;
            }
            self.expect(b']')?;
            return Ok(JsonValue::Array(values));
        }
    }

    fn string(&mut self) -> Result<String, JsonRefusal> {
        self.expect(b'"')?;
        let mut bytes = Vec::new();
        loop {
            let byte = self.next().ok_or(JsonRefusal::Malformed)?;
            match byte {
                b'"' => break,
                b'\\' => {
                    let escape = self.next().ok_or(JsonRefusal::Malformed)?;
                    match escape {
                        b'"' => bytes.push(b'"'),
                        b'\\' => bytes.push(b'\\'),
                        b'/' => bytes.push(b'/'),
                        b'b' => bytes.push(0x08),
                        b'f' => bytes.push(0x0C),
                        b'n' => bytes.push(0x0A),
                        b'r' => bytes.push(0x0D),
                        b't' => bytes.push(0x09),
                        b'u' => {
                            let first = self.hex4()?;
                            let scalar = if (0xD800..0xDC00).contains(&first) {
                                if self.next() != Some(b'\\') || self.next() != Some(b'u') {
                                    return Err(JsonRefusal::Malformed);
                                }
                                let second = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&second) {
                                    return Err(JsonRefusal::Malformed);
                                }
                                0x1_0000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                            } else if (0xDC00..0xE000).contains(&first) {
                                return Err(JsonRefusal::Malformed);
                            } else {
                                first
                            };
                            let scalar = char::from_u32(scalar).ok_or(JsonRefusal::Malformed)?;
                            let mut encoded = [0_u8; 4];
                            bytes.extend_from_slice(scalar.encode_utf8(&mut encoded).as_bytes());
                        }
                        _ => return Err(JsonRefusal::Malformed),
                    }
                }
                0x00..=0x1F => return Err(JsonRefusal::Malformed),
                _ => bytes.push(byte),
            }
            if bytes.len() > MAX_STRING_BYTES {
                return Err(JsonRefusal::TooLarge);
            }
        }
        String::from_utf8(bytes).map_err(|_| JsonRefusal::Malformed)
    }

    fn hex4(&mut self) -> Result<u32, JsonRefusal> {
        let mut value = 0_u32;
        for _ in 0..4 {
            let byte = self.next().ok_or(JsonRefusal::Malformed)?;
            let digit = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                b'A'..=b'F' => byte - b'A' + 10,
                _ => return Err(JsonRefusal::Malformed),
            };
            value = value * 16 + u32::from(digit);
        }
        Ok(value)
    }

    fn integer(&mut self) -> Result<i64, JsonRefusal> {
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.position += 1;
        }
        let digits = self.position;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.position += 1;
        }
        if self.position == digits {
            return Err(JsonRefusal::Malformed);
        }
        // Integers only: a fraction, exponent, or leading zero is refused
        // rather than coerced or skipped.
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return Err(JsonRefusal::UnsupportedNumber);
        }
        let text = std::str::from_utf8(&self.bytes[start..self.position])
            .map_err(|_| JsonRefusal::Malformed)?;
        let unsigned = text.strip_prefix('-').unwrap_or(text);
        if unsigned.len() > 1 && unsigned.starts_with('0') {
            return Err(JsonRefusal::Malformed);
        }
        text.parse::<i64>().map_err(|_| JsonRefusal::Malformed)
    }

    fn literal(&mut self, word: &str, value: JsonValue) -> Result<JsonValue, JsonRefusal> {
        if self.bytes[self.position..].starts_with(word.as_bytes()) {
            self.position += word.len();
            Ok(value)
        } else {
            Err(JsonRefusal::Malformed)
        }
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.position += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.position += 1;
        Some(byte)
    }

    fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), JsonRefusal> {
        if self.consume(byte) {
            Ok(())
        } else {
            Err(JsonRefusal::Malformed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_render_sorts_keys_and_round_trips() {
        let value = parse(br#"{"b":2,"a":[1,true,null],"s":"x\ny"}"#, 1024).unwrap();
        let canonical = render(&value);
        assert_eq!(canonical, r#"{"a":[1,true,null],"b":2,"s":"x\ny"}"#);
        // Parsing preserves document order while rendering is canonical, so
        // stability is asserted on bytes, not member order.
        let again = parse(canonical.as_bytes(), 1024).unwrap();
        assert_eq!(render(&again), canonical);
        assert_eq!(again.get("a").unwrap(), value.get("a").unwrap());
    }

    #[test]
    fn hostile_shapes_refuse() {
        assert_eq!(parse(b"[1,]", 64), Err(JsonRefusal::Malformed));
        assert_eq!(
            parse(b"{\"a\":1} trailing", 64),
            Err(JsonRefusal::Malformed)
        );
        assert_eq!(
            parse(b"{\"a\":1,\"a\":2}", 64),
            Err(JsonRefusal::DuplicateKey)
        );
        assert_eq!(parse(b"1.5", 64), Err(JsonRefusal::UnsupportedNumber));
        assert_eq!(parse(b"1e3", 64), Err(JsonRefusal::UnsupportedNumber));
        assert_eq!(parse(b"01", 64), Err(JsonRefusal::Malformed));
        assert_eq!(parse(b"\"\\ud800\"", 64), Err(JsonRefusal::Malformed));
        assert_eq!(
            parse(b"\"\\ud800\\ud800\"", 64),
            Err(JsonRefusal::Malformed)
        );
        assert_eq!(parse(b"\"ok\"", 2), Err(JsonRefusal::TooLarge));
        let deep = "[".repeat(16) + &"]".repeat(16);
        assert_eq!(parse(deep.as_bytes(), 1024), Err(JsonRefusal::TooDeep));
        let mut wide = String::from("[0");
        for _ in 0..5_000 {
            wide.push_str(",0");
        }
        wide.push(']');
        assert_eq!(
            parse(wide.as_bytes(), wide.len()),
            Err(JsonRefusal::TooManyValues)
        );
    }

    #[test]
    fn surrogate_pair_and_escapes_decode() {
        let value = parse(b"\"A\\ud834\\udd1e\\/\\\"\\\\\"", 64).unwrap();
        assert_eq!(value.as_str(), Some("A\u{1D11E}/\"\\"));
    }

    #[test]
    fn multibyte_text_renders_verbatim_and_round_trips() {
        let value = JsonValue::Str("héllo wörld ✓".to_owned());
        let rendered = render(&value);
        assert_eq!(rendered, "\"héllo wörld ✓\"");
        assert_eq!(parse(rendered.as_bytes(), 64).unwrap(), value);
    }

    #[test]
    fn closed_objects_reject_unknown_or_missing_members() {
        let value = parse(br#"{"a":1,"b":2}"#, 64).unwrap();
        assert!(value.closed(&["a", "b"]).is_some());
        assert!(value.closed(&["a"]).is_none());
        assert!(value.closed(&["a", "b", "c"]).is_none());
        assert!(value.closed(&["a", "zzz"]).is_none());
        assert_eq!(parse(b"[1]", 64).unwrap().closed(&["a"]), None);
    }
}
