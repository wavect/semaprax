//! Canonicalize the one `LC_UUID` in a private arm64 macOS executable.

use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const MACH_HEADER_64_BYTES: usize = 32;
const MH_MAGIC_64: u32 = 0xfeed_facf;
const CPU_TYPE_ARM64: u32 = 0x0100_000c;
const MH_EXECUTE: u32 = 2;
const LC_UUID: u32 = 0x1b;
const UUID_COMMAND_BYTES: usize = 24;
const MAX_INPUT_BYTES: usize = 128 * 1024 * 1024;
const MAX_LOAD_COMMANDS: usize = 4096;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if arguments.len() != 2
        || arguments[0] == arguments[1]
        || !arguments.iter().all(|path| path.is_absolute())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected distinct absolute INPUT and create-new OUTPUT paths",
        )
        .into());
    }
    canonicalize_file(&arguments[0], &arguments[1])?;
    Ok(())
}

fn canonicalize_file(input: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let metadata = fs::symlink_metadata(input)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Mach-O input must be a regular non-symbolic-link file",
        )
        .into());
    }
    if metadata.len() > MAX_INPUT_BYTES as u64 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Mach-O input is too large").into());
    }
    let mut bytes = Vec::new();
    fs::File::open(input)?
        .take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Mach-O input is too large").into());
    }
    let canonical = canonicalize(&bytes).map_err(|message| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("cannot canonicalize Mach-O UUID: {message}"),
        )
    })?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&canonical)?;
    file.sync_all()?;
    fs::set_permissions(output, metadata.permissions())?;
    Ok(())
}

fn canonicalize(input: &[u8]) -> Result<Vec<u8>, &'static str> {
    if input.len() < MACH_HEADER_64_BYTES {
        return Err("truncated Mach-O header");
    }
    if read_u32(input, 0)? != MH_MAGIC_64
        || read_u32(input, 4)? != CPU_TYPE_ARM64
        || read_u32(input, 12)? != MH_EXECUTE
    {
        return Err("input is not a little-endian arm64 Mach-O executable");
    }
    let command_count = usize::try_from(read_u32(input, 16)?).map_err(|_| "command overflow")?;
    if command_count == 0 || command_count > MAX_LOAD_COMMANDS {
        return Err("load-command count is outside the private bound");
    }
    let command_bytes = usize::try_from(read_u32(input, 20)?).map_err(|_| "size overflow")?;
    let command_end = MACH_HEADER_64_BYTES
        .checked_add(command_bytes)
        .filter(|end| *end <= input.len())
        .ok_or("load-command table is out of bounds")?;
    let mut offset = MACH_HEADER_64_BYTES;
    let mut uuid_offset = None;
    for _ in 0..command_count {
        let command = read_u32(input, offset)?;
        let size = usize::try_from(read_u32(input, offset + 4)?).map_err(|_| "size overflow")?;
        if size < 8 || !size.is_multiple_of(8) {
            return Err("load command has a noncanonical size");
        }
        let next = offset
            .checked_add(size)
            .filter(|next| *next <= command_end)
            .ok_or("load command is out of bounds")?;
        if command == LC_UUID
            && (size != UUID_COMMAND_BYTES || uuid_offset.replace(offset + 8).is_some())
        {
            return Err("expected exactly one canonical LC_UUID command");
        }
        offset = next;
    }
    if offset != command_end {
        return Err("load-command byte count is not exact");
    }
    let uuid_offset = uuid_offset.ok_or("LC_UUID command is absent")?;
    let uuid_end = uuid_offset + 16;
    if input[uuid_offset..uuid_end].iter().all(|byte| *byte == 0) {
        return Err("LC_UUID is structurally zero");
    }
    let mut output = input.to_vec();
    output[uuid_offset..uuid_end].fill(0);
    let digest = Sha256::digest(&output);
    output[uuid_offset..uuid_end].copy_from_slice(&digest[..16]);
    Ok(output)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, &'static str> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or("truncated 32-bit field")?;
    Ok(u32::from_le_bytes(
        value.try_into().map_err(|_| "invalid 32-bit field")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0_u8; 80];
        bytes[0..4].copy_from_slice(&MH_MAGIC_64.to_le_bytes());
        bytes[4..8].copy_from_slice(&CPU_TYPE_ARM64.to_le_bytes());
        bytes[12..16].copy_from_slice(&MH_EXECUTE.to_le_bytes());
        bytes[16..20].copy_from_slice(&1_u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&(UUID_COMMAND_BYTES as u32).to_le_bytes());
        bytes[32..36].copy_from_slice(&LC_UUID.to_le_bytes());
        bytes[36..40].copy_from_slice(&(UUID_COMMAND_BYTES as u32).to_le_bytes());
        bytes[40..56].copy_from_slice(&[7; 16]);
        bytes[56..].copy_from_slice(&[11; 24]);
        bytes
    }

    #[test]
    fn uuid_is_content_bound_and_prior_uuid_independent() {
        let first = canonicalize(&fixture()).unwrap();
        let mut changed_uuid = fixture();
        changed_uuid[40..56].fill(9);
        assert_eq!(canonicalize(&changed_uuid).unwrap(), first);
        assert_ne!(&first[40..56], &[0; 16]);

        let mut zeroed = first.clone();
        zeroed[40..56].fill(0);
        assert_eq!(&first[40..56], &Sha256::digest(zeroed)[..16]);
    }

    #[test]
    fn malformed_or_confused_images_fail_closed() {
        let valid = fixture();
        for hostile in [
            valid[..55].to_vec(),
            {
                let mut value = valid.clone();
                value[0] = 0;
                value
            },
            {
                let mut value = valid.clone();
                value[4..8].copy_from_slice(&0x0100_0007_u32.to_le_bytes());
                value
            },
            {
                let mut value = valid.clone();
                value[36..40].copy_from_slice(&20_u32.to_le_bytes());
                value
            },
            {
                let mut value = valid.clone();
                value[16..20].copy_from_slice(&2_u32.to_le_bytes());
                value[20..24].copy_from_slice(&36_u32.to_le_bytes());
                value[56..60].copy_from_slice(&2_u32.to_le_bytes());
                value[60..64].copy_from_slice(&12_u32.to_le_bytes());
                value
            },
            {
                let mut value = valid.clone();
                value[40..56].fill(0);
                value
            },
        ] {
            assert!(canonicalize(&hostile).is_err());
        }
    }

    #[test]
    fn oversized_file_is_rejected_before_reading_its_contents() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let input = std::env::temp_dir().join(format!(
            "semaprax-private-desktop-uuid-{}-{nonce}.macho",
            std::process::id()
        ));
        let output = input.with_extension("canonical");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&input)
            .unwrap();
        file.set_len((MAX_INPUT_BYTES + 1) as u64).unwrap();
        drop(file);

        let error = canonicalize_file(&input, &output).unwrap_err();
        assert!(error.to_string().contains("Mach-O input is too large"));
        assert!(!output.exists());
        fs::remove_file(input).unwrap();
    }
}
