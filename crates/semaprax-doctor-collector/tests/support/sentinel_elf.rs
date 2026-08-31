//! Trusted sibling sentinel: ready, exact one-byte challenge, done, exit zero.
//! A blocked challenge read prevents normal completion before the collector.
pub fn executable() -> Vec<u8> {
    super::fixture::image(&code(), b"ready\ndone\n")
}

#[cfg(target_arch = "x86_64")]
fn code() -> Vec<u8> {
    let mut code = vec![0x48, 0x83, 0xec, 8]; // reserve private stack byte.
    let mut addresses = Vec::new();
    let mut rejections = Vec::new();
    write(&mut code, &mut addresses, &mut rejections, 0, 6);
    code.extend_from_slice(&[
        0x31, 0xff, 0x48, 0x89, 0xe6, 0xba, 1, 0, 0, 0, 0xb8, 0, 0, 0, 0, 0x0f, 0x05,
    ]); // read(0,rsp,1).
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 1, 0x75, 0]);
    rejections.push(code.len() - 1);
    code.extend_from_slice(&[0x80, 0x3c, 0x24, 42, 0x75, 0]); // cmp byte[rsp],42.
    rejections.push(code.len() - 1);
    write(&mut code, &mut addresses, &mut rejections, 6, 5);
    code.extend_from_slice(&[0xbf, 0, 0, 0, 0, 0xb8, 60, 0, 0, 0, 0x0f, 0x05]);
    let failure = code.len();
    code.extend_from_slice(&[0xbf, 7, 0, 0, 0, 0xb8, 60, 0, 0, 0, 0x0f, 0x05]);
    for (address, payload_offset) in addresses {
        let displacement = i32::try_from(code.len() + payload_offset - address - 4).unwrap();
        code[address..address + 4].copy_from_slice(&displacement.to_le_bytes());
    }
    for rejection in rejections {
        code[rejection] = u8::try_from(failure - rejection - 1).unwrap();
    }
    code
}

#[cfg(target_arch = "x86_64")]
fn write(
    code: &mut Vec<u8>,
    addresses: &mut Vec<(usize, usize)>,
    rejections: &mut Vec<usize>,
    offset: usize,
    length: u8,
) {
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xbf, 1, 0, 0, 0, 0x48, 0x8d, 0x35]);
    addresses.push((code.len(), offset));
    code.extend_from_slice(&0i32.to_le_bytes());
    code.extend_from_slice(&[
        0xba, length, 0, 0, 0, 0x0f, 0x05, 0x48, 0x83, 0xf8, length, 0x75, 0,
    ]);
    rejections.push(code.len() - 1);
}

#[cfg(target_arch = "aarch64")]
fn code() -> Vec<u8> {
    let mov = |register: u32, value: u32| 0xd280_0000 | (value << 5) | register;
    let mut words = vec![0xd100_43ff]; // sub sp,sp,#16.
    let mut addresses = Vec::new();
    let mut rejections = Vec::new();
    write(&mut words, &mut addresses, &mut rejections, 0, 6);
    words.extend([
        mov(0, 0),
        0x9100_03e1,
        mov(2, 1),
        mov(8, 63),
        0xd400_0001,
        0xf100_041f,
        0,
    ]); // read(0,sp,1); cmp x0,#1; b.ne failure.
    rejections.push(words.len() - 1);
    words.extend([0x3940_03e9, 0x7100_a93f, 0]); // ldrb w9,[sp]; cmp w9,#42.
    rejections.push(words.len() - 1);
    write(&mut words, &mut addresses, &mut rejections, 6, 5);
    words.extend([mov(0, 0), mov(8, 93), 0xd400_0001]);
    let failure = words.len();
    words.extend([mov(0, 7), mov(8, 93), 0xd400_0001]);
    for (address, payload_offset) in addresses {
        let displacement = u32::try_from((words.len() - address) * 4 + payload_offset).unwrap();
        words[address] = 0x1000_0001 | ((displacement & 3) << 29) | ((displacement >> 2) << 5);
    }
    for rejection in rejections {
        words[rejection] = 0x5400_0001 | (u32::try_from(failure - rejection).unwrap() << 5);
    }
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

#[cfg(target_arch = "aarch64")]
fn write(
    words: &mut Vec<u32>,
    addresses: &mut Vec<(usize, usize)>,
    rejections: &mut Vec<usize>,
    offset: usize,
    length: u32,
) {
    let mov = |register: u32, value: u32| 0xd280_0000 | (value << 5) | register;
    words.push(mov(0, 1));
    addresses.push((words.len(), offset));
    words.extend([0, mov(2, length), mov(8, 64), 0xd400_0001, 0xeb02_001f, 0]);
    rejections.push(words.len() - 1);
}
