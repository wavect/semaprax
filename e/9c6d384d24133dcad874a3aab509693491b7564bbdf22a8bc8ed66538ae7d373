// Exact raw-wire known answers for the measured native builder layouts.
// Semantic facts, full bytes, counters, CLI parity and locking remain checked.
const MACOS_AARCH64_WIRES: [&str; 9] = [
    "sha256:5c6ceca181c351fde97209fec5c1ae7993f18879069d899008641eb5e2eca61c",
    "sha256:c6559c0255638f9957306aacbc1dfbc389b9b692bd95d0c018d7878e2fc99ad6",
    "sha256:4fd64af5eb09e06182e372bce9e4127a7c8f2578313af46a8b8e8d860567b177",
    "sha256:2b1627acf545d8213a1612798f572f308d73c5f36493e271898f4a440060cb11",
    "sha256:9ce18c6600f7c5213d329a309e51bc9241d5a5042791d1c9553a46c9ea38aa1e",
    "sha256:e5a21cfa07ec2cba63112a7fcaeccc542d5ad281692f1ba872895a719d71640c",
    "sha256:1f7ffbeb9d917fb7dd7db1406967b4896f6b050938ff93526e47116749a16233",
    "sha256:79ed872322e14e6395b71e32e0329887a48df5a8c9f302938889b73054018942",
    "sha256:58902e60f1d669acdfee80b1b0f7cfcad721c37c4d10de6fea0b9e7a1d147df2",
];
macro_rules! wire_pin {
    ($slot:literal, $portable:literal) => {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            MACOS_AARCH64_WIRES[$slot]
        } else {
            $portable
        }
    };
}
macro_rules! graph_builder_bytes {
    () => {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            928_685
        } else {
            928_541
        }
    };
}

macro_rules! wire_pins {
    ($portable:expr) => {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            [
                MACOS_AARCH64_WIRES[1],
                MACOS_AARCH64_WIRES[2],
                MACOS_AARCH64_WIRES[3],
                MACOS_AARCH64_WIRES[4],
                MACOS_AARCH64_WIRES[5],
                MACOS_AARCH64_WIRES[6],
                MACOS_AARCH64_WIRES[7],
                MACOS_AARCH64_WIRES[8],
            ]
        } else {
            $portable
        }
    };
}
