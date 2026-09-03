//! Closed, allocation-free admission for packaged Linux doctor images.
//!
//! This authenticates only the bounded ELF metadata stated below. In
//! particular, absence of `PT_INTERP` does not authenticate kernel binfmt
//! policy, loadability, dynamic tags, or the provenance of the input bytes.

const ELF_HEADER_BYTES: usize = 64;
const PROGRAM_HEADER_BYTES: usize = 56;
const MAX_PROGRAM_HEADERS: usize = 128;
const PT_INTERP: u32 = 3;

/// Native architectures admitted by the Linux doctor provisioner contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedArchitecture {
    X86_64,
    Aarch64,
}

impl ExpectedArchitecture {
    const fn machine(self) -> u16 {
        match self {
            Self::X86_64 => 62,
            Self::Aarch64 => 183,
        }
    }
}

/// Authenticated ELF object kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElfObjectKind {
    Executable,
    PositionIndependentExecutable,
}

/// Facts authenticated by [`verify_static_elf64`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticElf64Facts {
    pub architecture: ExpectedArchitecture,
    pub object_kind: ElfObjectKind,
    pub machine: u16,
    pub program_header_count: u16,
    pub elf64: bool,
    pub little_endian: bool,
    pub has_program_interpreter: bool,
}

/// Closed failure classes for packaged-image admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticElf64Error {
    Invalid,
    ArchitectureMismatch,
    ProgramInterpreter,
}

/// Authenticates the minimum static native ELF64 envelope required for a held
/// Linux doctor image.
///
/// `ET_DYN` is admitted because the existing offline-bundle contract permits a
/// statically linked PIE. Both object kinds still require no `PT_INTERP`.
pub fn verify_static_elf64(
    bytes: &[u8],
    expected_architecture: ExpectedArchitecture,
) -> Result<StaticElf64Facts, StaticElf64Error> {
    let header = range(bytes, 0, ELF_HEADER_BYTES)?;
    if &header[..7] != b"\x7fELF\x02\x01\x01"
        || u32_at(header, 20)? != 1
        || u16_at(header, 52)? != ELF_HEADER_BYTES as u16
        || u16_at(header, 54)? != PROGRAM_HEADER_BYTES as u16
    {
        return Err(StaticElf64Error::Invalid);
    }

    let object_kind = match u16_at(header, 16)? {
        2 => ElfObjectKind::Executable,
        3 => ElfObjectKind::PositionIndependentExecutable,
        _ => return Err(StaticElf64Error::Invalid),
    };
    let machine = u16_at(header, 18)?;
    if machine != expected_architecture.machine() {
        return Err(StaticElf64Error::ArchitectureMismatch);
    }

    let count_u16 = u16_at(header, 56)?;
    let count = usize::from(count_u16);
    if !(1..=MAX_PROGRAM_HEADERS).contains(&count) {
        return Err(StaticElf64Error::Invalid);
    }
    let offset = usize::try_from(u64_at(header, 32)?).map_err(|_| StaticElf64Error::Invalid)?;
    // A program-header table overlapping the ELF header is ambiguous input,
    // even if every indexed read would remain in bounds.
    if offset < ELF_HEADER_BYTES {
        return Err(StaticElf64Error::Invalid);
    }
    let table_bytes = count
        .checked_mul(PROGRAM_HEADER_BYTES)
        .ok_or(StaticElf64Error::Invalid)?;
    let table = range(bytes, offset, table_bytes)?;
    for entry in table.as_chunks::<PROGRAM_HEADER_BYTES>().0 {
        if u32_at(entry, 0)? == PT_INTERP {
            return Err(StaticElf64Error::ProgramInterpreter);
        }
    }

    Ok(StaticElf64Facts {
        architecture: expected_architecture,
        object_kind,
        machine,
        program_header_count: count_u16,
        elf64: true,
        little_endian: true,
        has_program_interpreter: false,
    })
}

fn range(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8], StaticElf64Error> {
    let end = offset
        .checked_add(length)
        .ok_or(StaticElf64Error::Invalid)?;
    bytes.get(offset..end).ok_or(StaticElf64Error::Invalid)
}

fn word<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], StaticElf64Error> {
    range(bytes, offset, N)?
        .try_into()
        .map_err(|_| StaticElf64Error::Invalid)
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, StaticElf64Error> {
    Ok(u16::from_le_bytes(word(bytes, offset)?))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, StaticElf64Error> {
    Ok(u32::from_le_bytes(word(bytes, offset)?))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, StaticElf64Error> {
    Ok(u64::from_le_bytes(word(bytes, offset)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn image(architecture: ExpectedArchitecture, kind: u16, count: u16) -> Vec<u8> {
        let mut bytes = vec![0; ELF_HEADER_BYTES + usize::from(count) * PROGRAM_HEADER_BYTES];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        put16(&mut bytes, 16, kind);
        put16(&mut bytes, 18, architecture.machine());
        put32(&mut bytes, 20, 1);
        put64(&mut bytes, 32, ELF_HEADER_BYTES as u64);
        put16(&mut bytes, 52, ELF_HEADER_BYTES as u16);
        put16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
        put16(&mut bytes, 56, count);
        bytes
    }

    fn invalid(bytes: &[u8]) {
        assert_eq!(
            verify_static_elf64(bytes, ExpectedArchitecture::X86_64),
            Err(StaticElf64Error::Invalid)
        );
    }

    #[test]
    fn admits_both_native_machines_and_existing_static_pie_contract() {
        for architecture in [ExpectedArchitecture::X86_64, ExpectedArchitecture::Aarch64] {
            for (kind, expected_kind) in [
                (2, ElfObjectKind::Executable),
                (3, ElfObjectKind::PositionIndependentExecutable),
            ] {
                let bytes = image(architecture, kind, 1);
                assert_eq!(
                    verify_static_elf64(&bytes, architecture),
                    Ok(StaticElf64Facts {
                        architecture,
                        object_kind: expected_kind,
                        machine: architecture.machine(),
                        program_header_count: 1,
                        elf64: true,
                        little_endian: true,
                        has_program_interpreter: false,
                    })
                );
            }
        }
    }

    #[test]
    fn rejects_program_interpreter_anywhere_in_the_bounded_table() {
        for index in 0..3 {
            let mut bytes = image(ExpectedArchitecture::X86_64, 2, 3);
            put32(
                &mut bytes,
                ELF_HEADER_BYTES + index * PROGRAM_HEADER_BYTES,
                PT_INTERP,
            );
            assert_eq!(
                verify_static_elf64(&bytes, ExpectedArchitecture::X86_64),
                Err(StaticElf64Error::ProgramInterpreter)
            );
        }
    }

    #[test]
    fn rejects_wrong_class_endian_version_kind_machine_and_header_shapes() {
        for offset in 0..7 {
            let mut bytes = image(ExpectedArchitecture::X86_64, 2, 1);
            bytes[offset] ^= 0xff;
            invalid(&bytes);
        }
        for (offset, values) in [
            (16, &[0, 1, 4, u16::MAX][..]),
            (52, &[0, 63, 65, u16::MAX][..]),
            (54, &[0, 55, 57, u16::MAX][..]),
            (56, &[0, 129, u16::MAX][..]),
        ] {
            for value in values {
                let mut bytes = image(ExpectedArchitecture::X86_64, 2, 1);
                put16(&mut bytes, offset, *value);
                invalid(&bytes);
            }
        }
        for version in [0, 2, u32::MAX] {
            let mut bytes = image(ExpectedArchitecture::X86_64, 2, 1);
            put32(&mut bytes, 20, version);
            invalid(&bytes);
        }
        assert_eq!(
            verify_static_elf64(
                &image(ExpectedArchitecture::Aarch64, 2, 1),
                ExpectedArchitecture::X86_64
            ),
            Err(StaticElf64Error::ArchitectureMismatch)
        );
    }

    #[test]
    fn requires_one_complete_nonoverlapping_bounded_program_header_table() {
        let bytes = image(ExpectedArchitecture::X86_64, 2, 1);
        for length in 0..bytes.len() {
            invalid(&bytes[..length]);
        }
        for offset in [0, 1, 63] {
            let mut malformed = bytes.clone();
            put64(&mut malformed, 32, offset);
            invalid(&malformed);
        }
        for offset in [65, bytes.len() as u64, u64::MAX - 55, u64::MAX] {
            let mut malformed = bytes.clone();
            put64(&mut malformed, 32, offset);
            invalid(&malformed);
        }

        let maximum = image(ExpectedArchitecture::X86_64, 2, 128);
        assert!(verify_static_elf64(&maximum, ExpectedArchitecture::X86_64).is_ok());
        invalid(&maximum[..maximum.len() - 1]);
    }

    #[test]
    fn bytes_outside_the_declared_table_cannot_create_or_hide_an_interpreter() {
        let mut trailing = image(ExpectedArchitecture::X86_64, 2, 1);
        trailing.extend_from_slice(&PT_INTERP.to_le_bytes());
        assert!(verify_static_elf64(&trailing, ExpectedArchitecture::X86_64).is_ok());

        let mut hidden = image(ExpectedArchitecture::X86_64, 2, 1);
        hidden.extend_from_slice(&[0; PROGRAM_HEADER_BYTES]);
        put32(
            &mut hidden,
            ELF_HEADER_BYTES + PROGRAM_HEADER_BYTES,
            PT_INTERP,
        );
        assert!(verify_static_elf64(&hidden, ExpectedArchitecture::X86_64).is_ok());
        put16(&mut hidden, 56, 2);
        assert_eq!(
            verify_static_elf64(&hidden, ExpectedArchitecture::X86_64),
            Err(StaticElf64Error::ProgramInterpreter)
        );
    }
}
