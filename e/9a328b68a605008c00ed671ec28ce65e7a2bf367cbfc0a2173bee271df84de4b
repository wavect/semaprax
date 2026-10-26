// Exact raw-wire known answers for the measured native builder layouts.
// Semantic facts, full bytes, counters, CLI parity and locking remain checked.
const MACOS_AARCH64_WIRES: [&str; 9] = [
    "sha256:5668e33966eb0be3206fcce50ba7906d07144e3d71c65e013d932b03f41a295a",
    "sha256:dfc1c18b829853e16ae7c745afcbcc202d747d6629e0cbca23137fd755b95237",
    "sha256:0ca011c9e9546641172dade95822b80e6e84772dbc4625e5b079cd4367562173",
    "sha256:ed8d9b1610dec8aa5e6386e138e38c77138a90d4dbfc17dce6f7bbdedadfefcd",
    "sha256:9d6066fc32d6104a4699a95ba480eac9127ef75445ad8b472268c4428f9c0846",
    "sha256:8c1b2c177d6d726ed24b7149eb436faeec6c51b69d6535c3408d7902976bace5",
    "sha256:41d5a091c994ded86dda5b7db3a679811b43ef2107c78665ac4ff88a36a1a21a",
    "sha256:42b932f36d79379974c3cf1a03fdd5ceadf17aeceb324d6a9843e1053c9f2cec",
    "sha256:95298906f37671fc02d4a9a2454b0426c3b70212283192fb687990cde53e923f",
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
            926_453
        } else {
            926_309
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
