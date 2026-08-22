// Shared byte cursor only: descriptor/artifact replay and manifest replay use
// one fail-closed implementation without sharing their semantic reconstruction.
use sha2::{Digest as _, Sha256};

pub(super) struct ExactReplay<'a> {
    source: &'a [u8],
    position: usize,
    failed: bool,
}

impl<'a> ExactReplay<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            position: 0,
            failed: false,
        }
    }

    pub(super) fn text(&mut self, expected: &str) {
        let end = self.position.checked_add(expected.len());
        let mismatch = end.is_none_or(|end| end > self.source.len())
            || end.is_some_and(|end| &self.source[self.position..end] != expected.as_bytes());
        if self.failed || mismatch {
            self.failed = true;
            return;
        }
        self.position = end.unwrap_or(self.position);
    }

    pub(super) fn json(&mut self, value: &str) {
        self.text("\"");
        for character in value.chars() {
            match character {
                '\"' => self.text("\\\""),
                '\\' => self.text("\\\\"),
                '\u{08}' => self.text("\\b"),
                '\t' => self.text("\\t"),
                '\n' => self.text("\\n"),
                '\u{0c}' => self.text("\\f"),
                '\r' => self.text("\\r"),
                character if character <= '\u{1f}' => {
                    let code = u32::from(character);
                    let hex = b"0123456789abcdef";
                    let escaped = [
                        b'\\',
                        b'u',
                        hex[((code >> 12) & 0xf) as usize],
                        hex[((code >> 8) & 0xf) as usize],
                        hex[((code >> 4) & 0xf) as usize],
                        hex[(code & 0xf) as usize],
                    ];
                    self.text(std::str::from_utf8(&escaped).unwrap_or(""));
                }
                character => {
                    let mut encoded = [0_u8; 4];
                    self.text(character.encode_utf8(&mut encoded));
                }
            }
        }
        self.text("\"");
    }

    pub(super) fn number(&mut self, value: impl std::fmt::Display) {
        let rendered = value.to_string();
        #[cfg(test)]
        super::note_post_hir_replay_capacity(rendered.capacity());
        self.text(&rendered);
    }

    pub(super) fn usize_noalloc(&mut self, mut value: usize) {
        let mut bytes = [0_u8; 20];
        let mut start = bytes.len();
        loop {
            start -= 1;
            bytes[start] = b'0' + u8::try_from(value % 10).unwrap_or(0);
            value /= 10;
            if value == 0 {
                break;
            }
        }
        self.text(std::str::from_utf8(&bytes[start..]).unwrap_or(""));
    }

    pub(super) fn raw_digest_json_noalloc(&mut self, bytes: &[u8]) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.text("\"sha256:");
        for byte in Sha256::digest(bytes) {
            let pair = [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0x0f)]];
            self.text(std::str::from_utf8(&pair).unwrap_or(""));
        }
        self.text("\"");
    }

    pub(super) fn finish(self) -> bool {
        !self.failed && self.position == self.source.len()
    }
}
