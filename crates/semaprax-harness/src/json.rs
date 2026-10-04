//! Strict bounded JSON (HP-01): duplicate keys, depth/size limits, number and
//! UTF-8 checks, canonical rendering and domain-separated digests.
//!
//! The parser is hand-written because `serde` is not a direct dependency and a
//! duplicate-key refusal cannot be expressed through `serde_json::from_slice`.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

/// Hard bounds applied while parsing untrusted JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonLimits {
    pub max_bytes: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
}

impl JsonLimits {
    /// Limits for one protocol frame of at most `max_bytes`.
    pub fn frame(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            max_depth: 64,
            max_nodes: 65_536,
        }
    }
}

fn err(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Parse a frame: like [`parse_strict`] but any raw LF/CR byte is refused
/// (`SPX-HPA008`); frames are single-line.
pub fn parse_frame(bytes: &[u8], limits: &JsonLimits) -> HarnessResult<Value> {
    if bytes.len() > limits.max_bytes {
        return Err(err(
            "SPX-HPA002",
            format!(
                "frame of {} bytes exceeds limit {}",
                bytes.len(),
                limits.max_bytes
            ),
        ));
    }
    if bytes.iter().any(|b| *b == b'\n' || *b == b'\r') {
        return Err(err("SPX-HPA008", "frame contains a raw line break"));
    }
    parse_strict(bytes, limits)
}

/// Strict parse. Stable refusals: HPA001 invalid UTF-8, HPA002 oversize,
/// HPA003 depth, HPA004 node count, HPA005 duplicate key, HPA006 invalid
/// number, HPA007 trailing data, HPA009 syntax.
pub fn parse_strict(bytes: &[u8], limits: &JsonLimits) -> HarnessResult<Value> {
    if bytes.len() > limits.max_bytes {
        return Err(err(
            "SPX-HPA002",
            format!(
                "document of {} bytes exceeds limit {}",
                bytes.len(),
                limits.max_bytes
            ),
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|e| {
        err(
            "SPX-HPA001",
            format!("invalid UTF-8 at byte {}", e.valid_up_to()),
        )
    })?;
    let mut p = Parser {
        s: text.as_bytes(),
        text,
        i: 0,
        nodes: 0,
        limits,
    };
    p.ws();
    let v = p.value(1)?;
    p.ws();
    if p.i != p.s.len() {
        return Err(err("SPX-HPA007", format!("trailing data at byte {}", p.i)));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    text: &'a str,
    i: usize,
    nodes: usize,
    limits: &'a JsonLimits,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while matches!(self.s.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn syntax(&self, what: &str) -> HarnessDiagnostic {
        err(
            "SPX-HPA009",
            format!("JSON syntax error at byte {}: {what}", self.i),
        )
    }

    fn node(&mut self) -> HarnessResult<()> {
        self.nodes += 1;
        if self.nodes > self.limits.max_nodes {
            return Err(err(
                "SPX-HPA004",
                format!("more than {} JSON nodes", self.limits.max_nodes),
            ));
        }
        Ok(())
    }

    fn lit(&mut self, word: &str, v: Value) -> HarnessResult<Value> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(self.syntax("unknown literal"))
        }
    }

    fn value(&mut self, depth: usize) -> HarnessResult<Value> {
        if depth > self.limits.max_depth {
            return Err(err(
                "SPX-HPA003",
                format!("nesting deeper than {}", self.limits.max_depth),
            ));
        }
        self.node()?;
        match self.s.get(self.i) {
            None => Err(self.syntax("unexpected end")),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') => self.lit("true", Value::Bool(true)),
            Some(b'f') => self.lit("false", Value::Bool(false)),
            Some(b'n') => self.lit("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.syntax("unexpected character")),
        }
    }

    fn array(&mut self, depth: usize) -> HarnessResult<Value> {
        self.i += 1;
        let mut out = Vec::new();
        self.ws();
        if self.s.get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(Value::Array(out));
        }
        loop {
            self.ws();
            out.push(self.value(depth + 1)?);
            self.ws();
            match self.s.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Array(out));
                }
                _ => return Err(self.syntax("expected `,` or `]`")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> HarnessResult<Value> {
        self.i += 1;
        let mut map = Map::new();
        self.ws();
        if self.s.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.ws();
            if self.s.get(self.i) != Some(&b'"') {
                return Err(self.syntax("expected object key"));
            }
            self.node()?;
            let key = self.string()?;
            self.ws();
            if self.s.get(self.i) != Some(&b':') {
                return Err(self.syntax("expected `:`"));
            }
            self.i += 1;
            self.ws();
            let v = self.value(depth + 1)?;
            if map.contains_key(&key) {
                return Err(err("SPX-HPA005", format!("duplicate object key `{key}`")));
            }
            map.insert(key, v);
            self.ws();
            match self.s.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Object(map));
                }
                _ => return Err(self.syntax("expected `,` or `}`")),
            }
        }
    }

    fn hex4(&mut self) -> HarnessResult<u32> {
        let h = self
            .text
            .get(self.i..self.i + 4)
            .ok_or_else(|| self.syntax("short \\u escape"))?;
        let v = u32::from_str_radix(h, 16).map_err(|_| self.syntax("bad \\u escape"))?;
        if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(self.syntax("bad \\u escape"));
        }
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> HarnessResult<String> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let b = *self
                .s
                .get(self.i)
                .ok_or_else(|| self.syntax("unterminated string"))?;
            match b {
                b'"' => {
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.i += 1;
                    let e = *self
                        .s
                        .get(self.i)
                        .ok_or_else(|| self.syntax("unterminated escape"))?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..0xDC00).contains(&hi) {
                                if !self.s[self.i..].starts_with(b"\\u") {
                                    return Err(self.syntax("lone surrogate"));
                                }
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.syntax("bad surrogate pair"));
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                hi
                            };
                            out.push(
                                char::from_u32(cp).ok_or_else(|| self.syntax("lone surrogate"))?,
                            );
                        }
                        _ => return Err(self.syntax("bad escape")),
                    }
                }
                0..=0x1f => return Err(self.syntax("raw control character in string")),
                _ => {
                    let ch = self.text[self.i..].chars().next().expect("valid utf-8");
                    out.push(ch);
                    self.i += ch.len_utf8();
                }
            }
        }
    }

    fn number(&mut self) -> HarnessResult<Value> {
        let start = self.i;
        if self.s[self.i] == b'-' {
            self.i += 1;
        }
        let int_start = self.i;
        while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        let int_len = self.i - int_start;
        if int_len == 0 || (int_len > 1 && self.s[int_start] == b'0') {
            return Err(err(
                "SPX-HPA006",
                format!("malformed number at byte {start}"),
            ));
        }
        let mut float = false;
        if self.s.get(self.i) == Some(&b'.') {
            float = true;
            self.i += 1;
            let f = self.i;
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
            if self.i == f {
                return Err(err(
                    "SPX-HPA006",
                    format!("malformed number at byte {start}"),
                ));
            }
        }
        if matches!(self.s.get(self.i), Some(b'e' | b'E')) {
            float = true;
            self.i += 1;
            if matches!(self.s.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            let f = self.i;
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
            if self.i == f {
                return Err(err(
                    "SPX-HPA006",
                    format!("malformed number at byte {start}"),
                ));
            }
        }
        let lit = &self.text[start..self.i];
        let bad = || {
            err(
                "SPX-HPA006",
                format!("number `{lit}` is non-finite or outside the lossless range"),
            )
        };
        if float {
            let f: f64 = lit.parse().map_err(|_| bad())?;
            return Number::from_f64(f).map(Value::Number).ok_or_else(bad);
        }
        if let Ok(i) = lit.parse::<i64>() {
            return Ok(Value::Number(i.into()));
        }
        lit.parse::<u64>()
            .map(|u| Value::Number(u.into()))
            .map_err(|_| bad())
    }
}

/// Canonical rendering: sorted keys, no whitespace.
pub fn canonical(value: &Value) -> String {
    let mut out = String::new();
    render(value, &mut out);
    out
}

fn render(v: &Value, out: &mut String) {
    match v {
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                render(x, out);
            }
            out.push(']');
        }
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(k.clone()).to_string());
                out.push(':');
                render(&m[k], out);
            }
            out.push('}');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// `sha256:<64 hex>` over `domain` + NUL + canonical JSON.
pub fn digest(domain: &str, value: &Value) -> String {
    sha256_labeled(domain, canonical(value).as_bytes())
}

/// Same domain separation over raw bytes.
pub fn sha256_labeled(domain: &str, bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(domain.as_bytes());
    h.update([0u8]);
    h.update(bytes);
    format!("sha256:{}", hex(&h.finalize()))
}

/// Plain `sha256:<hex>` of bytes.
pub fn sha256_plain(bytes: &[u8]) -> String {
    format!("sha256:{}", hex(&Sha256::digest(bytes)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lim() -> JsonLimits {
        JsonLimits::frame(1024)
    }

    #[test]
    fn rejects_and_accepts() {
        assert!(parse_strict(br#"{"a":[1,2.5,"x\u00e9\ud83d\ude00"]}"#, &lim()).is_ok());
        assert_eq!(
            parse_strict(br#"{"a":1,"a":2}"#, &lim()).unwrap_err().code,
            "SPX-HPA005"
        );
        assert_eq!(
            parse_strict(b"1e999", &lim()).unwrap_err().code,
            "SPX-HPA006"
        );
        assert_eq!(
            parse_strict(b"18446744073709551616", &lim())
                .unwrap_err()
                .code,
            "SPX-HPA006"
        );
        assert_eq!(
            parse_strict(b"[1] x", &lim()).unwrap_err().code,
            "SPX-HPA007"
        );
        assert_eq!(
            parse_strict(&[b'"', 0xff, b'"'], &lim()).unwrap_err().code,
            "SPX-HPA001"
        );
        assert_eq!(
            parse_frame(b"[1,\n2]", &lim()).unwrap_err().code,
            "SPX-HPA008"
        );
        let deep = "[".repeat(200);
        assert_eq!(
            parse_strict(deep.as_bytes(), &lim()).unwrap_err().code,
            "SPX-HPA003"
        );
        assert_eq!(
            canonical(&parse_strict(br#"{"b":1,"a":[true,null]}"#, &lim()).unwrap()),
            r#"{"a":[true,null],"b":1}"#
        );
    }
}
