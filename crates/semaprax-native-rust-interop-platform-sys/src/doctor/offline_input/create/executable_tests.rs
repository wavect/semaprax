//! Executable storage observations, not execution, provenance or loader approval.
use super::create_doctor_offline_executable;
use crate::{DoctorOfflineInputError as Error, DOCTOR_OFFLINE_INPUT_MAX_BYTES};

// Independent literal native ELF framing; never advertised as a runnable image.
fn image(length: usize) -> Vec<u8> {
    assert!(length >= 120);
    let mut bytes = vec![0; length];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
    let machine = if cfg!(target_arch = "aarch64") {
        183u16
    } else {
        62u16
    };
    bytes[18..20].copy_from_slice(&machine.to_le_bytes());
    bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
    bytes[32..40].copy_from_slice(&64u64.to_le_bytes());
    bytes[52..54].copy_from_slice(&64u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&1u16.to_le_bytes());
    bytes[64..68].copy_from_slice(&1u32.to_le_bytes());
    for (index, byte) in bytes[120..].iter_mut().enumerate() {
        *byte = (index % 256) as u8;
    }
    bytes
}

#[test]
fn limits_and_empty_input_precede_platform_and_elf_selection() {
    for (bytes, limit, error) in [
        (&b""[..], 0, Error::Invalid),
        (&b"x"[..], 0, Error::Invalid),
        (&b""[..], DOCTOR_OFFLINE_INPUT_MAX_BYTES + 1, Error::Limit),
        (&b"x"[..], DOCTOR_OFFLINE_INPUT_MAX_BYTES + 1, Error::Limit),
        (&b""[..], 1, Error::Invalid),
        (&b"xy"[..], 1, Error::Limit),
    ] {
        assert_eq!(
            create_doctor_offline_executable(bytes, limit).unwrap_err(),
            error
        );
    }
}

#[cfg(not(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
#[test]
fn unsupported_host_does_not_interpret_or_convert_images() {
    for bytes in [image(120), b"#!/bin/sh\nexit 0\n".to_vec()] {
        assert_eq!(
            create_doctor_offline_executable(&bytes, bytes.len()).unwrap_err(),
            Error::Unsupported
        );
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod native;
