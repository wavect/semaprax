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
            MACOS_AARCH64_CURRENT_WIRES[$slot]
        } else {
            $portable
        }
    };
}
macro_rules! graph_builder_bytes {
    () => {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            1_363_484
        } else {
            928_541
        }
    };
}

macro_rules! wire_pins {
    ($portable:expr) => {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            [
                MACOS_AARCH64_CURRENT_WIRES[1],
                MACOS_AARCH64_CURRENT_WIRES[2],
                MACOS_AARCH64_CURRENT_WIRES[3],
                MACOS_AARCH64_CURRENT_WIRES[4],
                MACOS_AARCH64_CURRENT_WIRES[5],
                MACOS_AARCH64_CURRENT_WIRES[6],
                MACOS_AARCH64_CURRENT_WIRES[7],
                MACOS_AARCH64_CURRENT_WIRES[8],
            ]
        } else {
            $portable
        }
    };
}

// Current native KATs are actual full source-held producer bytes, not a forecast.
// Every old native KAT above remains checked through the exact inverse below.
const MACOS_AARCH64_CURRENT_WIRES: [&str; 9] = [
    "sha256:a58b503c66ddc46833c8356be1b65842140216e72d17fc8c7cccbbdfeaa9aef2",
    "sha256:d290ceabe9e43ccbb65c69cb2f65cd04b9667f9484cefda43e3dd51f38e757ec",
    "sha256:85241d40aa58be7dab4ac8fbae1b8d3a0dd5193c0a7c0dc00e73da6f2b884d8a",
    "sha256:a48adbd17e3b63f30fa895966b90930942f1d7bbe43a042a33af99f121c5969d",
    "sha256:af03d1b9083b54c2132dd3429120d422dcc864a51876191b2b997d759975bc96",
    "sha256:c366b7b0b4810c93d894c6660338caedd4fcbfb788c67ebc14827af7d25270ed",
    "sha256:4d0112bcbf18d41bff592bce790a57e8597b537bc5ddb52cb5f05766b9e22822",
    "sha256:166bfe98fc8ae9e19b9f0498345dd2e04f603050a479521bb7163f425fd89c5b",
    "sha256:f4f69af1f915e209393c3ae5aecaaaf0b2552a9efba8f6b417d9b5613b965898",
];

fn assert_measured_host_history(name: &str, bytes: &[u8]) {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return;
    }
    let slot = match name {
        "public-graph.json" => 0,
        "context.json" => 1,
        "forward.json" => 2,
        "capability-context.json" => 3,
        "type-context.json" => 4,
        "impact.json" => 5,
        "custom-impact.json" => 6,
        "review.json" => 7,
        "capability-review.json" => 8,
        _ => return,
    };
    assert_eq!(
        document_digest(bytes),
        MACOS_AARCH64_CURRENT_WIRES[slot],
        "complete actual native wire before resource-only historical inverse"
    );
    let (length, edits): (usize, &[(usize, usize, &str, &str)]) = match slot {
        0 => (6597, &[
            (237, 301, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e"),
            (5421, 5479, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
        ]),
        1 => (4514, &[
            (249, 405, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:522c59337b46bc89a548fa5aaca806f556e4ecae345ba7b84f456258daf069ce", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:9e90941e42779d79013db1d86bb607ed98ba571fda3678e842bc6a694acb531a"),
            (1716, 1774, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (2013, 2014, "4", "3"),
        ]),
        2 => (4515, &[
            (249, 405, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:f156d5c1abfc019d7e7580a2892e6f3f063052234d79f2ded90c2a8168bcf330", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:63e85876698c1602d213ce25b031bc7dede6c6a89bda89b3100bf3e78eb2ba2c"),
            (1717, 1775, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (2014, 2015, "5", "4"),
        ]),
        3 => (4338, &[
            (249, 405, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:0110e30ae0bb2e7b1d93f448280e1a5cd359f9de954829612056cd583ea1df8e", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:f1e7ceecbf9be910aadbe90dd3fa6cd333a75114bc6e0754303d5e2d6eb8f498"),
            (1688, 1746, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (1985, 1986, "8", "7"),
        ]),
        4 => (4012, &[
            (249, 405, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:d79614fa7e4506e8e1d3a378dec03fa36e216b077cab59da590383917bbc563e", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:446bc479f5b78f18962c0e48484cfce8432817b695676902a0e4de512b173b0a"),
            (1725, 1783, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (2022, 2023, "2", "1"),
        ]),
        5 => (4160, &[
            (248, 404, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:cc8b11a5bcbfc653eb08246d5300f52456c7661a545ed1c3cc4583e0694df6a8", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:bd021de53b386e00a9e8a75414fa27d1b59bcca5e9a6a4e28fe7739373accfcf"),
            (1729, 1787, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (2025, 2027, "60", "59"),
        ]),
        6 => (4569, &[
            (248, 404, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:8edb3e581809678195524e6059a9fffd1a5172de2ff7fc2e2f0b72d0b9a67c9b", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:141aec2dfa84de4cd77f551fd7e5359ac8cd1a859a51abd596cfb40864df06b6"),
            (1688, 1746, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (1985, 1986, "9", "8"),
        ]),
        7 => (12709, &[
            (248, 404, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:cb50c0d8556287ee286f5f6b03bd9e3c5f19988e514801a30e9e4b292d18265c", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:dce9f2efe50d337989addc094a09abac86cba37c0916d02cd39dc268bd9c149e"),
            (877, 1033, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:f9cd53a82aceb01badd285f8b62a645d39175a4266f1025958dc96ef1762b384", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:26e114921f57ff46352f8937e084624e981c01f7b2a05ad4440d6d2104e13a29"),
            (2358, 2416, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (2655, 2656, "1", "0"),
            (4877, 5033, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:c1af1ce18ac8f67d6be8cd15622c4e7edfcf51ee54a7b069cdd13f2e768fab6a", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:2aaf0c0d48f54a503d4ff1d58b18073ebf62f68b98e936c8ed5e380c98ec67c6"),
            (6361, 6419, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (6658, 6659, "3", "2"),
            (11462, 11520, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (11760, 11812, "1,\"used_impact_bytes\":4163,\"used_output_bytes\":12709", "0,\"used_impact_bytes\":4162,\"used_output_bytes\":12706"),
        ]),
        8 => (13626, &[
            (248, 404, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:8250659bd1b53b70c3f0e95449c006dd5f8340c5933ca31793b3795d0822289e", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:44d9db74e09e69f70b3dcda0f3e5d5087fdd65c8c0c90726d6d4b48733db02f9"),
            (839, 995, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:d548dff33fbcd26b12a2db65eff66b9c53fe21eca95ba3ea3959f8cc373bbdce", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:5282be6dcead21b2a38b2ced9219de43a6e3b99857be1185cbeb33c5e96aed1b"),
            (2282, 2340, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (2579, 2580, "2", "1"),
            (5190, 5346, "bda229fe56adadfb61e2a2890e55ff7c189db8d47637d94a0ef26521864750c4\",\"artifact_digest\":\"sha256:248e24965500a67679206b6ac17fb27b0ef521baa00177aa56f81f33cf21c977", "9731766560ddc61b4ab6ea54ee772fbacb596a8ceef7c9b511622266b6e9ff7e\",\"artifact_digest\":\"sha256:83145f8ee357b075c38c64606e51bebdfbb9dca63f00319beb13955e0939bac5"),
            (6636, 6694, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (6933, 6934, "5", "4"),
            (12378, 12436, "1363484,\"used_manifest_bytes\":584,\"used_output_bytes\":6597", "928685,\"used_manifest_bytes\":584,\"used_output_bytes\":6596"),
            (12677, 12729, "2,\"used_impact_bytes\":4575,\"used_output_bytes\":13626", "1,\"used_impact_bytes\":4574,\"used_output_bytes\":13623"),
        ]),
        _ => unreachable!(),
    };
    assert_eq!(bytes.len(), length);
    let mut restored = std::str::from_utf8(bytes).unwrap().to_owned();
    let mut next = restored.len();
    for &(start, end, after, before) in edits.iter().rev() {
        assert!(start <= end && end <= next);
        assert_eq!(restored.get(start..end), Some(after));
        restored.replace_range(start..end, before);
        next = start;
    }
    assert_eq!(
        document_digest(restored.as_bytes()),
        MACOS_AARCH64_WIRES[slot],
        "entire historical native KAT, all semantics and unknown fields preserved"
    );
}
