//! Closed test-only instruction recipes, not a general assembler or compiler.
//! Literal executables have no interpreter, library, allocation or retry loop.
use super::executable_image;

#[derive(Clone, Copy)]
pub(super) enum Arg {
    Constant(u64),
    Data(usize),
    Stack(usize),
    SavedFd,
}

#[derive(Clone, Copy)]
pub(super) enum Syscall {
    Read,
    Write,
    Close,
    Openat,
    Clone,
    Capget,
    Capset,
    Prctl,
}

impl Syscall {
    fn number(self) -> u64 {
        let (x86, arm) = match self {
            Self::Read => (0, 63),
            Self::Write => (1, 64),
            Self::Close => (3, 57),
            Self::Openat => (257, 56),
            Self::Clone => (56, 220),
            Self::Capget => (125, 90),
            Self::Capset => (126, 91),
            Self::Prctl => (157, 167),
        };
        if cfg!(target_arch = "x86_64") {
            x86
        } else {
            arm
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Expected {
    Return(i64),
    OpenedFd,
}

enum Step {
    Call(Syscall, [Arg; 6], Expected),
    StackBytes(Vec<u8>),
}

pub(super) struct Program {
    steps: Vec<Step>,
    data: Vec<u8>,
}

impl Program {
    pub(super) fn new() -> Self {
        Self {
            steps: Vec::new(),
            data: Vec::new(),
        }
    }

    pub(super) fn data(&mut self, bytes: &[u8]) -> Arg {
        let offset = self.data.len();
        self.data.extend_from_slice(bytes);
        assert!(self.data.len() <= 8192);
        Arg::Data(offset)
    }

    pub(super) fn call(&mut self, syscall: Syscall, args: &[Arg], expected: Expected) {
        assert!(args.len() <= 6);
        let mut registers = [Arg::Constant(0); 6];
        registers[..args.len()].copy_from_slice(args);
        for argument in registers {
            if let Arg::Stack(offset) = argument {
                assert!(offset < 64);
            }
        }
        self.steps.push(Step::Call(syscall, registers, expected));
        assert!(self.steps.len() <= 64);
    }

    pub(super) fn stack_bytes(&mut self, expected: &[u8]) {
        assert!(expected.len() <= 64);
        self.steps.push(Step::StackBytes(expected.to_vec()));
        assert!(self.steps.len() <= 64);
    }

    pub(super) fn finish(mut self, marker: &[u8]) -> Vec<u8> {
        let text = self.data(marker);
        self.call(
            Syscall::Write,
            &[Arg::Constant(1), text, Arg::Constant(marker.len() as u64)],
            Expected::Return(marker.len() as i64),
        );
        executable_image(&encode(&self.steps, self.data.len()), &self.data)
    }
}

#[cfg(target_arch = "x86_64")]
fn encode(steps: &[Step], data_len: usize) -> Vec<u8> {
    let mut code = vec![0x48, 0x83, 0xec, 64]; // sub rsp,64
    let mut addresses = Vec::new();
    let mut failures = Vec::new();
    for step in steps {
        match step {
            Step::Call(number, args, expected) => {
                for (argument, register) in args.iter().zip([7, 6, 2, 10, 8, 9]) {
                    match argument {
                        Arg::Constant(value) => mov(&mut code, register, *value),
                        Arg::Data(offset) => {
                            assert!(*offset < data_len);
                            mov(&mut code, register, 0);
                            addresses.push((code.len() - 8, *offset));
                        }
                        Arg::Stack(offset) => {
                            code.extend_from_slice(&[0x48, 0x89, 0xe0, 0x48, 0x05]); // mov rax,rsp; add rax,disp32
                            code.extend_from_slice(&(*offset as u32).to_le_bytes());
                            from_rax(&mut code, register);
                        }
                        Arg::SavedFd => {
                            code.extend_from_slice(&[0x4c, 0x89, 0xe0]); // mov rax,r12
                            from_rax(&mut code, register);
                        }
                    }
                }
                mov(&mut code, 0, number.number());
                code.extend_from_slice(&[0x0f, 0x05]);
                match expected {
                    Expected::Return(value) => {
                        mov(&mut code, 11, *value as u64);
                        code.extend_from_slice(&[0x4c, 0x39, 0xd8]); // cmp rax,r11
                        branch(&mut code, 0x85, &mut failures); // jne failure
                    }
                    Expected::OpenedFd => {
                        code.extend_from_slice(&[0x48, 0x85, 0xc0]); // test rax,rax
                        branch(&mut code, 0x88, &mut failures); // js failure
                        code.extend_from_slice(&[0x49, 0x89, 0xc4]); // mov r12,rax
                    }
                }
            }
            Step::StackBytes(bytes) => {
                for (offset, byte) in bytes.iter().enumerate() {
                    code.extend_from_slice(&[0x80, 0xbc, 0x24]); // cmp byte [rsp+disp32],imm8
                    code.extend_from_slice(&(offset as u32).to_le_bytes());
                    code.push(*byte);
                    branch(&mut code, 0x85, &mut failures);
                }
            }
        }
    }
    exit(&mut code, 0);
    let failure = code.len();
    exit(&mut code, 7);
    for offset in failures {
        let displacement = i32::try_from(failure - offset - 4).unwrap();
        code[offset..offset + 4].copy_from_slice(&displacement.to_le_bytes());
    }
    for (offset, data) in addresses {
        let address = 0x0040_0000_u64 + 120 + code.len() as u64 + data as u64;
        code[offset..offset + 8].copy_from_slice(&address.to_le_bytes());
    }
    code
}

#[cfg(target_arch = "x86_64")]
fn mov(code: &mut Vec<u8>, register: u8, value: u64) {
    code.extend_from_slice(&[
        if register >= 8 { 0x49 } else { 0x48 },
        0xb8 | (register & 7),
    ]);
    code.extend_from_slice(&value.to_le_bytes());
}

#[cfg(target_arch = "x86_64")]
fn from_rax(code: &mut Vec<u8>, register: u8) {
    code.extend_from_slice(&[
        if register >= 8 { 0x49 } else { 0x48 },
        0x89,
        0xc0 | (register & 7),
    ]);
}

#[cfg(target_arch = "x86_64")]
fn branch(code: &mut Vec<u8>, condition: u8, failures: &mut Vec<usize>) {
    code.extend_from_slice(&[0x0f, condition]);
    failures.push(code.len());
    code.extend_from_slice(&0i32.to_le_bytes());
}

#[cfg(target_arch = "x86_64")]
fn exit(code: &mut Vec<u8>, status: u64) {
    mov(code, 7, status);
    mov(code, 0, 60);
    code.extend_from_slice(&[0x0f, 0x05]);
}

#[cfg(target_arch = "aarch64")]
fn encode(steps: &[Step], data_len: usize) -> Vec<u8> {
    let mut words = vec![0xd101_03ff]; // sub sp,sp,#64
    let mut addresses = Vec::new();
    let mut failures = Vec::new();
    for step in steps {
        match step {
            Step::Call(number, args, expected) => {
                for (register, argument) in args.iter().enumerate() {
                    let register = register as u32;
                    match argument {
                        Arg::Constant(value) => words.extend(mov(register, *value)),
                        Arg::Data(offset) => {
                            assert!(*offset < data_len);
                            addresses.push((words.len(), register, *offset));
                            words.extend(mov(register, 0));
                        }
                        Arg::Stack(offset) => {
                            words.push(0x9100_03e0 | ((*offset as u32) << 10) | register)
                        } // add xn,sp,#offset
                        Arg::SavedFd => words.push(0xaa13_03e0 | register), // mov xn,x19
                    }
                }
                words.extend(mov(8, number.number()));
                words.push(0xd400_0001); // svc #0
                match expected {
                    Expected::Return(value) => {
                        words.extend(mov(9, *value as u64));
                        words.push(0xeb09_001f); // cmp x0,x9
                        failures.push(words.len());
                        words.push(0x5400_0001); // b.ne failure
                    }
                    Expected::OpenedFd => {
                        words.push(0xf100_001f); // cmp x0,#0
                        failures.push(words.len());
                        words.push(0x5400_0004); // b.mi failure
                        words.push(0xaa00_03f3); // mov x19,x0
                    }
                }
            }
            Step::StackBytes(bytes) => {
                for (offset, byte) in bytes.iter().enumerate() {
                    words.push(0x3940_03e9 | ((offset as u32) << 10)); // ldrb w9,[sp,#offset]
                    words.push(0x7100_013f | (u32::from(*byte) << 10)); // cmp w9,#byte
                    failures.push(words.len());
                    words.push(0x5400_0001);
                }
            }
        }
    }
    exit(&mut words, 0);
    let failure = words.len();
    exit(&mut words, 7);
    for index in failures {
        let offset = u32::try_from(failure - index).unwrap();
        assert!(offset < 1 << 18);
        words[index] |= offset << 5;
    }
    for (index, register, offset) in addresses {
        let address = 0x0040_0000_u64 + 120 + words.len() as u64 * 4 + offset as u64;
        words[index..index + 4].copy_from_slice(&mov(register, address));
    }
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

#[cfg(target_arch = "aarch64")]
fn mov(register: u32, value: u64) -> [u32; 4] {
    [
        0xd280_0000 | (((value & 0xffff) as u32) << 5) | register,
        0xf2a0_0000 | ((((value >> 16) & 0xffff) as u32) << 5) | register,
        0xf2c0_0000 | ((((value >> 32) & 0xffff) as u32) << 5) | register,
        0xf2e0_0000 | ((((value >> 48) & 0xffff) as u32) << 5) | register,
    ]
}

#[cfg(target_arch = "aarch64")]
fn exit(words: &mut Vec<u32>, status: u64) {
    words.extend(mov(0, status));
    words.extend(mov(8, 93));
    words.push(0xd400_0001);
}
