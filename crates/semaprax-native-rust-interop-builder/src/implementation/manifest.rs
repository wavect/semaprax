//! Canonical manifest rendering and the independent exact and semantic
//! manifest replays.

use super::*;

/// Canonical manifest row order is a wire contract.  The platform object is
/// always the third row; no caller may infer or sort this order dynamically.
pub(super) const fn canonical_manifest_file_names() -> [&'static str; 6] {
    [
        "descriptor.json",
        "module.c",
        if cfg!(windows) {
            "module.obj"
        } else {
            "module.o"
        },
        "semaprax_native_rust_interop.h",
        "semaprax_native_rust_interop.rs",
        "semaprax_native_rust_interop_ffi.rs",
    ]
}

fn write_raw_digest_json(output: &mut impl std::fmt::Write, bytes: &[u8]) -> std::fmt::Result {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.write_str("\"sha256:")?;
    for byte in Sha256::digest(bytes) {
        let pair = [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0x0f)]];
        output.write_str(std::str::from_utf8(&pair).map_err(|_| std::fmt::Error)?)?;
    }
    output.write_char('"')
}

pub(super) fn write_usize_decimal(
    output: &mut impl std::fmt::Write,
    mut value: usize,
) -> std::fmt::Result {
    let mut bytes = [0_u8; 20];
    let mut start = bytes.len();
    loop {
        start -= 1;
        bytes[start] = b'0' + u8::try_from(value % 10).map_err(|_| std::fmt::Error)?;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    output.write_str(std::str::from_utf8(&bytes[start..]).map_err(|_| std::fmt::Error)?)
}

fn write_manifest_file_row(
    output: &mut impl std::fmt::Write,
    path: &str,
    bytes: &[u8],
) -> std::fmt::Result {
    output.write_str("{\"path\":")?;
    write_json_string(output, path)?;
    output.write_str(",\"sha256\":")?;
    write_raw_digest_json(output, bytes)?;
    output.write_str(",\"bytes\":")?;
    write_usize_decimal(output, bytes.len())?;
    output.write_char('}')
}

pub(super) fn write_manifest(
    output: &mut impl std::fmt::Write,
    prepared: &PreparedNativeRustInterop,
    files: &[(&str, &[u8])],
    clang_path: &str,
    clang_version: &str,
    rustc: &RustcVersion,
    target: &str,
) -> std::fmt::Result {
    output.write_str("{\"schema\":")?;
    write_json_string(output, prepared.bundle_schema())?;
    if let Some(digest) = prepared.project_subject_digest() {
        output.write_str(",\"project_subject_digest\":")?;
        write_json_string(output, digest)?;
    }
    output.write_str(",\"descriptor\":{\"schema\":")?;
    write_json_string(output, prepared.descriptor_schema())?;
    output.write_str(",\"digest\":")?;
    write_json_string(output, &prepared.descriptor_digest)?;
    output.write_str(",\"bytes\":")?;
    write_usize_decimal(output, prepared.descriptor.len())?;
    output.write_str("},\"files\":[")?;
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        write_manifest_file_row(output, path, bytes)?;
    }
    output.write_str("],\"toolchain\":{\"rustc_release\":")?;
    write_json_string(output, rustc.release())?;
    output.write_str(",\"rustc_commit_hash\":")?;
    write_json_string(output, rustc.commit_hash())?;
    output.write_str(",\"host\":")?;
    write_json_string(output, rustc.host())?;
    output.write_str(",\"llvm_version\":")?;
    write_json_string(output, rustc.llvm_version())?;
    output.write_str(",\"clang_path\":")?;
    write_json_string(output, clang_path)?;
    output.write_str(",\"clang_version\":")?;
    write_json_string(output, clang_version)?;
    output.write_str(",\"target\":")?;
    write_json_string(output, target)?;
    output.write_str("},\"limits\":")?;
    write_limits_json(output)?;
    output.write_str(",\"nonclaims\":[")?;
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        write_json_string(output, nonclaim)?;
    }
    output.write_str("]}\n")
}

#[cfg(test)]
pub(super) fn render_manifest(
    prepared: &PreparedNativeRustInterop,
    files: &[(&str, &[u8])],
    clang_path: &str,
    clang_version: &str,
    rustc: &RustcVersion,
    target: &str,
) -> String {
    let mut count = CountingSink {
        bytes: 0,
        maximum: MAX_MANIFEST_BYTES,
        overflowed: false,
    };
    write_manifest(
        &mut count,
        prepared,
        files,
        clang_path,
        clang_version,
        rustc,
        target,
    )
    .expect("manifest count cannot fail");
    assert!(!count.overflowed);
    let mut output = String::with_capacity(count.bytes);
    write_manifest(
        &mut output,
        prepared,
        files,
        clang_path,
        clang_version,
        rustc,
        target,
    )
    .expect("String writing cannot fail");
    assert_eq!(output.capacity(), count.bytes);
    output
}

pub(super) fn replay_manifest_bytes_exact(
    source: &str,
    prepared: &PreparedNativeRustInterop,
    files: &[(&str, &[u8])],
    clang_path: &str,
    clang_version: &str,
    rustc: &RustcVersion,
    target: &str,
) -> bool {
    let mut exact = ExactReplay::new(source);
    exact.text("{\"schema\":");
    exact.json(prepared.bundle_schema());
    if let Some(digest) = prepared.project_subject_digest() {
        exact.text(",\"project_subject_digest\":");
        exact.json(digest);
    }
    exact.text(",\"descriptor\":{\"schema\":");
    exact.json(prepared.descriptor_schema());
    exact.text(",\"digest\":");
    exact.json(&prepared.descriptor_digest);
    exact.text(",\"bytes\":");
    exact.usize_noalloc(prepared.descriptor.len());
    exact.text("},\"files\":[");
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            exact.text(",");
        }
        exact.text("{\"path\":");
        exact.json(path);
        exact.text(",\"sha256\":");
        exact.raw_digest_json_noalloc(bytes);
        exact.text(",\"bytes\":");
        exact.usize_noalloc(bytes.len());
        exact.text("}");
    }
    exact.text("],\"toolchain\":{\"rustc_release\":");
    exact.json(rustc.release());
    exact.text(",\"rustc_commit_hash\":");
    exact.json(rustc.commit_hash());
    exact.text(",\"host\":");
    exact.json(rustc.host());
    exact.text(",\"llvm_version\":");
    exact.json(rustc.llvm_version());
    exact.text(",\"clang_path\":");
    exact.json(clang_path);
    exact.text(",\"clang_version\":");
    exact.json(clang_version);
    exact.text(",\"target\":");
    exact.json(target);
    exact.text("},\"limits\":");
    replay_limits_exact(&mut exact);
    exact.text(",\"nonclaims\":[");
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            exact.text(",");
        }
        exact.json(nonclaim);
    }
    exact.text("]}\n");
    exact.finish()
}

/// Independently consumes the fixed manifest JSON grammar without a DOM or
/// decoded-string allocation.  The exact replay above binds canonical bytes;
/// this cursor separately validates decoded values and the complete member,
/// type, cardinality, depth, and trailing-byte shape.
pub(super) struct ManifestCursor<'a> {
    source: &'a str,
    offset: usize,
    work: usize,
    maximum_work: usize,
}

impl<'a> ManifestCursor<'a> {
    pub(super) fn new(source: &'a str) -> Result<Self, PhaseBLocalError> {
        Ok(Self {
            source,
            offset: 0,
            work: 0,
            maximum_work: source
                .len()
                .checked_mul(2)
                .ok_or(PhaseBLocalError::Replay)?,
        })
    }

    fn bytes(&self) -> &'a [u8] {
        self.source.as_bytes()
    }

    fn advance(&mut self, bytes: usize) -> Result<(), PhaseBLocalError> {
        self.offset = self
            .offset
            .checked_add(bytes)
            .ok_or(PhaseBLocalError::Replay)?;
        self.work = self
            .work
            .checked_add(bytes)
            .ok_or(PhaseBLocalError::Replay)?;
        if self.offset > self.source.len() || self.work > self.maximum_work {
            return Err(PhaseBLocalError::Replay);
        }
        Ok(())
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), PhaseBLocalError> {
        let end = self
            .offset
            .checked_add(expected.len())
            .ok_or(PhaseBLocalError::Replay)?;
        if self.bytes().get(self.offset..end) != Some(expected) {
            return Err(PhaseBLocalError::Replay);
        }
        self.advance(expected.len())
    }

    fn hex_quad(&mut self) -> Result<u16, PhaseBLocalError> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let byte = *self
                .bytes()
                .get(self.offset)
                .ok_or(PhaseBLocalError::Replay)?;
            self.advance(1)?;
            let digit = match byte {
                b'0'..=b'9' => u16::from(byte - b'0'),
                b'a'..=b'f' => u16::from(byte - b'a') + 10,
                b'A'..=b'F' => u16::from(byte - b'A') + 10,
                _ => return Err(PhaseBLocalError::Replay),
            };
            value = value
                .checked_mul(16)
                .and_then(|value| value.checked_add(digit))
                .ok_or(PhaseBLocalError::Replay)?;
        }
        Ok(value)
    }

    fn json_character(&mut self) -> Result<char, PhaseBLocalError> {
        let byte = *self
            .bytes()
            .get(self.offset)
            .ok_or(PhaseBLocalError::Replay)?;
        if byte == b'"' || byte < 0x20 {
            return Err(PhaseBLocalError::Replay);
        }
        if byte != b'\\' {
            let character = self
                .source
                .get(self.offset..)
                .ok_or(PhaseBLocalError::Replay)?
                .chars()
                .next()
                .ok_or(PhaseBLocalError::Replay)?;
            self.advance(character.len_utf8())?;
            return Ok(character);
        }
        self.advance(1)?;
        let escape = *self
            .bytes()
            .get(self.offset)
            .ok_or(PhaseBLocalError::Replay)?;
        self.advance(1)?;
        match escape {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\u{08}'),
            b'f' => Ok('\u{0c}'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => {
                let first = self.hex_quad()?;
                let scalar = if (0xd800..=0xdbff).contains(&first) {
                    self.expect(b"\\u")?;
                    let second = self.hex_quad()?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(PhaseBLocalError::Replay);
                    }
                    0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
                } else if (0xdc00..=0xdfff).contains(&first) {
                    return Err(PhaseBLocalError::Replay);
                } else {
                    u32::from(first)
                };
                char::from_u32(scalar).ok_or(PhaseBLocalError::Replay)
            }
            _ => Err(PhaseBLocalError::Replay),
        }
    }

    pub(super) fn string_eq(&mut self, expected: &str) -> Result<(), PhaseBLocalError> {
        self.expect(b"\"")?;
        for expected in expected.chars() {
            if self.json_character()? != expected {
                return Err(PhaseBLocalError::Replay);
            }
        }
        self.expect(b"\"")
    }

    pub(super) fn usize_eq(&mut self, expected: usize) -> Result<(), PhaseBLocalError> {
        let first = *self
            .bytes()
            .get(self.offset)
            .ok_or(PhaseBLocalError::Replay)?;
        if !first.is_ascii_digit() {
            return Err(PhaseBLocalError::Replay);
        }
        let mut value = 0_usize;
        let mut digits = 0_usize;
        while let Some(byte @ b'0'..=b'9') = self.bytes().get(self.offset).copied() {
            if digits == 1 && first == b'0' {
                return Err(PhaseBLocalError::Replay);
            }
            value = value
                .checked_mul(10)
                .and_then(|value| value.checked_add(usize::from(byte - b'0')))
                .ok_or(PhaseBLocalError::Replay)?;
            self.advance(1)?;
            digits += 1;
        }
        if value == expected {
            Ok(())
        } else {
            Err(PhaseBLocalError::Replay)
        }
    }

    fn raw_digest_eq(&mut self, bytes: &[u8]) -> Result<(), PhaseBLocalError> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.expect(b"\"sha256:")?;
        for byte in Sha256::digest(bytes) {
            self.expect(&[HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0x0f)]])?;
        }
        self.expect(b"\"")
    }

    pub(super) fn finish(self) -> Result<usize, PhaseBLocalError> {
        if self.offset == self.source.len() && self.work <= self.maximum_work {
            Ok(self.work)
        } else {
            Err(PhaseBLocalError::Replay)
        }
    }
}

pub(super) fn replay_manifest_semantic(
    source: &str,
    prepared: &PreparedNativeRustInterop,
    files: &[(&str, &[u8]); 6],
    clang_path: &str,
    clang_version: &str,
    rustc: &RustcVersion,
    target: &str,
) -> Result<usize, PhaseBLocalError> {
    let mut cursor = ManifestCursor::new(source)?;
    cursor.expect(b"{\"schema\":")?;
    cursor.string_eq(prepared.bundle_schema())?;
    if let Some(digest) = prepared.project_subject_digest() {
        cursor.expect(b",\"project_subject_digest\":")?;
        cursor.string_eq(digest)?;
    }
    cursor.expect(b",\"descriptor\":{\"schema\":")?;
    cursor.string_eq(prepared.descriptor_schema())?;
    cursor.expect(b",\"digest\":")?;
    cursor.string_eq(&prepared.descriptor_digest)?;
    cursor.expect(b",\"bytes\":")?;
    cursor.usize_eq(prepared.descriptor.len())?;
    cursor.expect(b"},\"files\":[")?;
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            cursor.expect(b",")?;
        }
        cursor.expect(b"{\"path\":")?;
        cursor.string_eq(path)?;
        cursor.expect(b",\"sha256\":")?;
        cursor.raw_digest_eq(bytes)?;
        cursor.expect(b",\"bytes\":")?;
        cursor.usize_eq(bytes.len())?;
        cursor.expect(b"}")?;
    }
    cursor.expect(b"],\"toolchain\":{\"rustc_release\":")?;
    cursor.string_eq(rustc.release())?;
    cursor.expect(b",\"rustc_commit_hash\":")?;
    cursor.string_eq(rustc.commit_hash())?;
    cursor.expect(b",\"host\":")?;
    cursor.string_eq(rustc.host())?;
    cursor.expect(b",\"llvm_version\":")?;
    cursor.string_eq(rustc.llvm_version())?;
    cursor.expect(b",\"clang_path\":")?;
    cursor.string_eq(clang_path)?;
    cursor.expect(b",\"clang_version\":")?;
    cursor.string_eq(clang_version)?;
    cursor.expect(b",\"target\":")?;
    cursor.string_eq(target)?;
    cursor.expect(b"},\"limits\":{")?;
    for (index, (name, value)) in LIMIT_ROWS.iter().enumerate() {
        if index != 0 {
            cursor.expect(b",")?;
        }
        cursor.string_eq(name)?;
        cursor.expect(b":")?;
        cursor.usize_eq(*value)?;
    }
    cursor.expect(b"},\"nonclaims\":[")?;
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            cursor.expect(b",")?;
        }
        cursor.string_eq(nonclaim)?;
    }
    cursor.expect(b"]}\n")?;
    cursor.finish()
}

pub(super) fn replay_manifest(
    source: &str,
    prepared: &PreparedNativeRustInterop,
    files: &[(&str, &[u8]); 6],
    tools: &ToolchainFacts,
) -> Result<(), PhaseBLocalError> {
    if !replay_manifest_bytes_exact(
        source,
        prepared,
        files,
        platform::tool_path(&tools.clang),
        &tools.clang_version,
        &tools.rustc_version,
        &prepared.target.triple,
    ) {
        return Err(PhaseBLocalError::Replay);
    }
    replay_manifest_semantic(
        source,
        prepared,
        files,
        platform::tool_path(&tools.clang),
        &tools.clang_version,
        &tools.rustc_version,
        &prepared.target.triple,
    )
    .map(|_| ())
}
