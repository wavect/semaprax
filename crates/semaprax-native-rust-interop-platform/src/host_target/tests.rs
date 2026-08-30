use super::*;

const CURRENT: Option<NativeHostTarget> = current_native_host_target();
const CONST_TRIPLE: &str = NativeHostTarget::X86_64LinuxGnu.triple();

fn admitted() -> [(HostFacts, NativeHostTarget, &'static str); 6] {
    let linux = HostFacts {
        architecture: Architecture::X86_64,
        system: System::Linux,
        vendor: Vendor::Unknown,
        environment: Environment::Gnu,
        empty_abi: true,
        pointer_64: true,
        little_endian: true,
    };
    let darwin = HostFacts {
        system: System::Darwin,
        vendor: Vendor::Apple,
        environment: Environment::Empty,
        ..linux
    };
    let windows = HostFacts {
        system: System::Windows,
        vendor: Vendor::Pc,
        environment: Environment::Msvc,
        ..linux
    };
    [
        (
            linux,
            NativeHostTarget::X86_64LinuxGnu,
            "x86_64-unknown-linux-gnu",
        ),
        (
            HostFacts {
                architecture: Architecture::Aarch64,
                ..linux
            },
            NativeHostTarget::Aarch64LinuxGnu,
            "aarch64-unknown-linux-gnu",
        ),
        (
            darwin,
            NativeHostTarget::X86_64Darwin,
            "x86_64-apple-darwin",
        ),
        (
            HostFacts {
                architecture: Architecture::Aarch64,
                ..darwin
            },
            NativeHostTarget::Aarch64Darwin,
            "aarch64-apple-darwin",
        ),
        (
            windows,
            NativeHostTarget::X86_64WindowsMsvc,
            "x86_64-pc-windows-msvc",
        ),
        (
            HostFacts {
                architecture: Architecture::Aarch64,
                ..windows
            },
            NativeHostTarget::Aarch64WindowsMsvc,
            "aarch64-pc-windows-msvc",
        ),
    ]
}

#[test]
fn six_private_candidates_keep_exact_triples_and_const_entry_points() {
    assert_eq!(CURRENT, current_native_host_target());
    assert_eq!(CONST_TRIPLE, "x86_64-unknown-linux-gnu");
    for (facts, expected, triple) in admitted() {
        assert_eq!(classify(facts), Some(expected));
        assert_eq!(expected.triple(), triple);
    }
}

#[test]
fn reject_other_architecture_system_vendor_abi_width_and_endian() {
    for (facts, _, _) in admitted() {
        for rejected in [
            HostFacts {
                architecture: Architecture::Other,
                ..facts
            },
            HostFacts {
                system: System::Other,
                ..facts
            },
            HostFacts {
                vendor: Vendor::Other,
                ..facts
            },
            HostFacts {
                environment: Environment::Other,
                ..facts
            },
            HostFacts {
                empty_abi: false,
                ..facts
            },
            HostFacts {
                pointer_64: false,
                ..facts
            },
            HostFacts {
                little_endian: false,
                ..facts
            },
        ] {
            assert_eq!(classify(rejected), None);
        }
    }
}

#[test]
fn recognized_but_mismatched_vendor_and_environment_are_rejected() {
    for (facts, expected, _) in admitted() {
        for vendor in [Vendor::Unknown, Vendor::Apple, Vendor::Pc] {
            for environment in [Environment::Gnu, Environment::Msvc, Environment::Empty] {
                let selected = classify(HostFacts {
                    vendor,
                    environment,
                    ..facts
                });
                let matches = match facts.system {
                    System::Linux => {
                        matches!((vendor, environment), (Vendor::Unknown, Environment::Gnu))
                    }
                    System::Darwin => {
                        matches!((vendor, environment), (Vendor::Apple, Environment::Empty))
                    }
                    System::Windows => {
                        matches!((vendor, environment), (Vendor::Pc, Environment::Msvc))
                    }
                    System::Other => unreachable!(),
                };
                assert_eq!(selected, if matches { Some(expected) } else { None });
            }
        }
    }
}

#[test]
fn musl_gnu_windows_x32_and_big_endian_do_not_alias_supported_hosts() {
    let rows = admitted();
    let linux = rows[0].0;
    let aarch64_linux = rows[1].0;
    let windows = rows[4].0;
    for rejected in [
        HostFacts {
            environment: Environment::Other,
            ..linux
        }, // musl
        HostFacts {
            environment: Environment::Gnu,
            ..windows
        }, // Windows GNU
        HostFacts {
            environment: Environment::Gnu,
            empty_abi: false,
            ..windows
        }, // gnullvm
        HostFacts {
            pointer_64: false,
            empty_abi: false,
            ..linux
        }, // gnux32
        HostFacts {
            little_endian: false,
            ..aarch64_linux
        }, // aarch64_be
    ] {
        assert_eq!(classify(rejected), None);
    }
}
