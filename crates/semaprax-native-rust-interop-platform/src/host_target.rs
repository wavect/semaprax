//! Compile-time native-host identity, not a toolchain or cross-compilation API.
//! Public package policies may exclude the private AArch64-MSVC candidate.

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHostTarget {
    X86_64LinuxGnu,
    Aarch64LinuxGnu,
    X86_64Darwin,
    Aarch64Darwin,
    X86_64WindowsMsvc,
    Aarch64WindowsMsvc,
}

impl NativeHostTarget {
    pub const fn triple(self) -> &'static str {
        match self {
            Self::X86_64LinuxGnu => "x86_64-unknown-linux-gnu",
            Self::Aarch64LinuxGnu => "aarch64-unknown-linux-gnu",
            Self::X86_64Darwin => "x86_64-apple-darwin",
            Self::Aarch64Darwin => "aarch64-apple-darwin",
            Self::X86_64WindowsMsvc => "x86_64-pc-windows-msvc",
            Self::Aarch64WindowsMsvc => "aarch64-pc-windows-msvc",
        }
    }
}

#[derive(Clone, Copy)]
enum Architecture {
    X86_64,
    Aarch64,
    Other,
}
#[derive(Clone, Copy)]
enum System {
    Linux,
    Darwin,
    Windows,
    Other,
}
#[derive(Clone, Copy)]
enum Vendor {
    Unknown,
    Apple,
    Pc,
    Other,
}
#[derive(Clone, Copy)]
enum Environment {
    Gnu,
    Msvc,
    Empty,
    Other,
}

#[derive(Clone, Copy)]
struct HostFacts {
    architecture: Architecture,
    system: System,
    vendor: Vendor,
    environment: Environment,
    empty_abi: bool,
    pointer_64: bool,
    little_endian: bool,
}

const fn classify(facts: HostFacts) -> Option<NativeHostTarget> {
    if !facts.empty_abi || !facts.pointer_64 || !facts.little_endian {
        return None;
    }
    match (
        facts.architecture,
        facts.system,
        facts.vendor,
        facts.environment,
    ) {
        (Architecture::X86_64, System::Linux, Vendor::Unknown, Environment::Gnu) => {
            Some(NativeHostTarget::X86_64LinuxGnu)
        }
        (Architecture::Aarch64, System::Linux, Vendor::Unknown, Environment::Gnu) => {
            Some(NativeHostTarget::Aarch64LinuxGnu)
        }
        (Architecture::X86_64, System::Darwin, Vendor::Apple, Environment::Empty) => {
            Some(NativeHostTarget::X86_64Darwin)
        }
        (Architecture::Aarch64, System::Darwin, Vendor::Apple, Environment::Empty) => {
            Some(NativeHostTarget::Aarch64Darwin)
        }
        (Architecture::X86_64, System::Windows, Vendor::Pc, Environment::Msvc) => {
            Some(NativeHostTarget::X86_64WindowsMsvc)
        }
        (Architecture::Aarch64, System::Windows, Vendor::Pc, Environment::Msvc) => {
            Some(NativeHostTarget::Aarch64WindowsMsvc)
        }
        _ => None,
    }
}

/// Return only an admitted compile-time host identity. No environment variable,
/// target string supplied by a caller, or executable probe affects this result.
#[doc(hidden)]
pub const fn current_native_host_target() -> Option<NativeHostTarget> {
    classify(HostFacts {
        architecture: if cfg!(target_arch = "x86_64") {
            Architecture::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Architecture::Aarch64
        } else {
            Architecture::Other
        },
        system: if cfg!(target_os = "linux") {
            System::Linux
        } else if cfg!(target_os = "macos") {
            System::Darwin
        } else if cfg!(target_os = "windows") {
            System::Windows
        } else {
            System::Other
        },
        vendor: if cfg!(target_vendor = "unknown") {
            Vendor::Unknown
        } else if cfg!(target_vendor = "apple") {
            Vendor::Apple
        } else if cfg!(target_vendor = "pc") {
            Vendor::Pc
        } else {
            Vendor::Other
        },
        environment: if cfg!(target_env = "gnu") {
            Environment::Gnu
        } else if cfg!(target_env = "msvc") {
            Environment::Msvc
        } else if cfg!(target_env = "") {
            Environment::Empty
        } else {
            Environment::Other
        },
        empty_abi: cfg!(target_abi = ""),
        pointer_64: cfg!(target_pointer_width = "64"),
        little_endian: cfg!(target_endian = "little"),
    })
}

#[cfg(test)]
mod tests;
