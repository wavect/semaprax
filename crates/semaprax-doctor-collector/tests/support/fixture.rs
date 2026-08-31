//! Literal wire/ELF fixtures: not installed-tool compatibility or provenance.
use sha2::{Digest, Sha256};

pub const SELECTOR: &str = "collector-fixture";
pub const VERSION: &[u8] = b"clang version 1.0.0\n";

pub fn architecture() -> u8 {
    if cfg!(target_arch = "x86_64") {
        1
    } else {
        2
    }
}

pub fn bundle() -> Vec<u8> {
    native_bundle(VERSION)
}

pub fn native_bundle(version: &[u8]) -> Vec<u8> {
    bundle_files(&[("bin/clang", executable(version, Ending::Exit(0)))], 1)
}

pub fn all_bundle(node_ending: Ending) -> Vec<u8> {
    bundle_files(
        &[
            ("bin/clang", executable(VERSION, Ending::Exit(0))),
            ("bin/node", executable(b"v22.0.0\n", node_ending)),
            ("bin/rustc", executable(b"rustc 1.88.0\n", Ending::Exit(0))),
        ],
        7,
    )
}

fn bundle_files(files: &[(&str, Vec<u8>)], roles: u8) -> Vec<u8> {
    let mut bytes = b"SPXDOC1\0".to_vec();
    bytes.extend_from_slice(&[architecture(), roles]);
    bytes.extend_from_slice(&(SELECTOR.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&(files.len() as u32).to_le_bytes());
    for ordinal in 0..3 {
        let index = if roles & (1 << ordinal) != 0 {
            ordinal
        } else {
            u32::MAX
        };
        bytes.extend_from_slice(&index.to_le_bytes());
    }
    bytes.extend_from_slice(SELECTOR.as_bytes());
    for (path, elf) in files {
        bytes.extend_from_slice(&(path.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&[1, 0]);
        bytes.extend_from_slice(&(elf.len() as u64).to_le_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.extend_from_slice(elf);
    }
    bytes
}

pub fn request(bundle: &[u8]) -> Vec<u8> {
    request_target(bundle, 1)
}

pub fn request_target(bundle: &[u8], target: u8) -> Vec<u8> {
    let mut bytes = b"SPXDWK1\0".to_vec();
    bytes.extend_from_slice(&[1, architecture(), target, [4, 1, 2, 7][target as usize]]);
    bytes.extend_from_slice(&[0x37; 32]);
    bytes.extend_from_slice(&(bundle.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&Sha256::digest(bundle));
    bytes.push(u8::try_from(SELECTOR.len()).unwrap());
    bytes.extend_from_slice(SELECTOR.as_bytes());
    bytes
}

pub fn reply(request: &[u8]) -> Vec<u8> {
    let mut bytes = b"SPXDWR1\0".to_vec();
    bytes.extend_from_slice(&Sha256::digest(request));
    bytes.extend_from_slice(&request[12..44]);
    bytes.extend_from_slice(&request[8..12]);
    bytes.extend_from_slice(&[1, 1, 0]); // row count, Clang role, success.
    bytes.extend_from_slice(&(VERSION.len() as u32).to_le_bytes());
    bytes.extend_from_slice(VERSION);
    bytes
}

#[derive(Clone, Copy)]
pub enum Ending {
    Exit(u8),
    Spin,
    CloseAndSpin,
}

/// Closed recipes only. One exact write, then exit or spin. A short write exits
/// 7. Reply surrogates remain below PIPE_BUF; the large actual-tool fixture
/// calibrates its complete bounded write before testing report sink failure.
pub fn executable(payload: &[u8], ending: Ending) -> Vec<u8> {
    assert!(payload.len() <= 65_535);
    let code = machine_code(payload.len(), ending);
    image(&code, payload)
}

pub(super) fn image(code: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut elf = vec![0; 120];
    elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    elf[16..18].copy_from_slice(&2u16.to_le_bytes());
    elf[18..20].copy_from_slice(&(if architecture() == 1 { 62u16 } else { 183u16 }).to_le_bytes());
    elf[20..24].copy_from_slice(&1u32.to_le_bytes());
    elf[24..32].copy_from_slice(&(0x0040_0000u64 + 120).to_le_bytes());
    elf[32..40].copy_from_slice(&64u64.to_le_bytes());
    elf[52..54].copy_from_slice(&64u16.to_le_bytes());
    elf[54..56].copy_from_slice(&56u16.to_le_bytes());
    elf[56..58].copy_from_slice(&1u16.to_le_bytes());
    elf[64..68].copy_from_slice(&1u32.to_le_bytes());
    elf[68..72].copy_from_slice(&5u32.to_le_bytes());
    elf[80..88].copy_from_slice(&0x0040_0000u64.to_le_bytes());
    let length = (120 + code.len() + payload.len()) as u64;
    elf[96..104].copy_from_slice(&length.to_le_bytes());
    elf[104..112].copy_from_slice(&length.to_le_bytes());
    elf[112..120].copy_from_slice(&4096u64.to_le_bytes());
    elf.extend_from_slice(code);
    elf.extend_from_slice(payload);
    elf
}

#[cfg(target_arch = "x86_64")]
fn machine_code(length: usize, ending: Ending) -> Vec<u8> {
    let mut code = vec![0xb8, 1, 0, 0, 0, 0xbf, 1, 0, 0, 0, 0x48, 0x8d, 0x35];
    let address = code.len();
    code.extend_from_slice(&0i32.to_le_bytes());
    code.push(0xba);
    code.extend_from_slice(&(length as u32).to_le_bytes());
    code.extend_from_slice(&[0x0f, 0x05, 0x48, 0x3d]); // syscall; cmp rax,length
    code.extend_from_slice(&(length as u32).to_le_bytes());
    code.extend_from_slice(&[0x75, 0]); // jne failure
    let mut rejections = vec![code.len() - 1];
    match ending {
        Ending::Exit(status) => {
            code.extend_from_slice(&[0xbf, status, 0, 0, 0, 0xb8, 60, 0, 0, 0, 0x0f, 0x05]);
        }
        Ending::Spin | Ending::CloseAndSpin => {
            if matches!(ending, Ending::CloseAndSpin) {
                for fd in [1, 2] {
                    code.extend_from_slice(&[0xbf, fd, 0, 0, 0, 0xb8, 3, 0, 0, 0, 0x0f, 0x05]);
                    // test rax,rax; jne failure: spin only after close == 0.
                    code.extend_from_slice(&[0x48, 0x85, 0xc0, 0x75, 0]);
                    rejections.push(code.len() - 1);
                }
            }
            code.extend_from_slice(&[0xeb, 0xfe]);
        }
    }
    let failure = code.len();
    code.extend_from_slice(&[0xbf, 7, 0, 0, 0, 0xb8, 60, 0, 0, 0, 0x0f, 0x05]);
    let displacement = i32::try_from(code.len() - address - 4).unwrap();
    code[address..address + 4].copy_from_slice(&displacement.to_le_bytes());
    for rejection in rejections {
        code[rejection] = u8::try_from(failure - rejection - 1).unwrap();
    }
    code
}

#[cfg(target_arch = "aarch64")]
fn machine_code(length: usize, ending: Ending) -> Vec<u8> {
    let mov = |register: u32, value: u32| 0xd280_0000 | (value << 5) | register;
    let mut words = vec![
        mov(0, 1),
        0,
        mov(2, length as u32),
        mov(8, 64),
        0xd400_0001,
        0xeb02_001f,
        0,
    ]; // adr x1,payload; write; cmp x0,x2; b.ne failure
    let mut rejections = vec![6];
    match ending {
        Ending::Exit(status) => words.extend([mov(0, status.into()), mov(8, 93), 0xd400_0001]),
        Ending::Spin | Ending::CloseAndSpin => {
            if matches!(ending, Ending::CloseAndSpin) {
                for fd in [1, 2] {
                    words.extend([mov(0, fd), mov(8, 57), 0xd400_0001]);
                    // cmp x0,#0; b.ne failure: spin only after close == 0.
                    words.extend([0xf100_001f, 0]);
                    rejections.push(words.len() - 1);
                }
            }
            words.push(0x1400_0000);
        }
    }
    let failure = words.len();
    words.extend([mov(0, 7), mov(8, 93), 0xd400_0001]);
    let displacement = u32::try_from(words.len() * 4 - 4).unwrap();
    words[1] = 0x1000_0001 | ((displacement & 3) << 29) | ((displacement >> 2) << 5);
    for rejection in rejections {
        words[rejection] = 0x5400_0001 | (u32::try_from(failure - rejection).unwrap() << 5);
    }
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}
