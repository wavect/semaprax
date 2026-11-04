// Exact raw-wire known answers for the measured native builder layouts.
// Semantic facts, full bytes, counters, CLI parity and locking remain checked.
const MACOS_AARCH64_WIRES: [&str; 9] = [
    "sha256:2ec54e800d9e5f542855404a209238219786d92b62d1e518b303522877a2faba",
    "sha256:d68f77b6d1c2beaf031520b0bfe05493e9d32cb16954f1f9fbfe64aca61cf34e",
    "sha256:77566d318c5388f658949e2494a4ec9dc1da93e021923e1e36e451d6a5b8b64a",
    "sha256:625ab0fba8b0fcf64eae0b8cfc921293377dff6d5c5cb81eec983a7007f37948",
    "sha256:47d87561eacc4cd786c423b339ae4740f77402b82f081b20d4df107a8077640a",
    "sha256:77caf7da96a8033699da60db74e6881ba5443101f1b0771e67a5753f11f4d518",
    "sha256:a70091b7871a4cf52abaedd0a7d721e9957c8995ce5342c585c32fb2beead1a2",
    "sha256:78eefbb8c3b8387945ebf0c4249b47e6de3194c0ad46ffa5a61828c3f1787258",
    "sha256:ecd75b0dd5f58625b6ca16aad524346982082003008ab085b0965463564af8bc",
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
            732_686
        } else {
            732_542
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
