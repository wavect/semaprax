// Exact captures from both captured hosts for the same public workspace fixtures.
pub(super) const LINUX_GNU_ACTIVE: bool = cfg!(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu"
));
pub(super) const MACOS_ACTIVE: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

struct Wire {
    name: &'static str,
    current: &'static str,
    previous: &'static str,
    sha256: &'static str,
}

const LINUX_GNU_WIRES: [Wire; 9] = [
    Wire {
        name: "public-graph.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/public-graph.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261104/public-graph-portable.json"),
        sha256: "sha256:689e22a18a4556d35330d9fdd10b819ddfcb8074bcb41fdc057b5ce74a4550bf",
    },
    Wire {
        name: "context.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/context.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261104/context-portable.json"),
        sha256: "sha256:fba76f0243e495c33050409b4c7e33d3dee18f1d5284c66ba36b449a8eaee157",
    },
    Wire {
        name: "forward.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/forward.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261104/forward-portable.json"),
        sha256: "sha256:ebe8d4ecf501bd361523544f8409d702544abdd57ff4460e664e6368f480b251",
    },
    Wire {
        name: "capability-context.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/capability-context.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261104/capability-context-portable.json"),
        sha256: "sha256:a254e9f8383fe478c4bd72999aa3a7bd83c16b5be9017bfd5f986d0531456044",
    },
    Wire {
        name: "type-context.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/type-context.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261104/type-context-portable.json"),
        sha256: "sha256:d4bcc926250785f1d3f7211e27b8b6b3768f7e1abc47f1257174725d148e7c81",
    },
    Wire {
        name: "impact.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/impact.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261104/impact-portable.json"),
        sha256: "sha256:a8fa888ef8125120924946437ca50b9002d14fa524ee4e9d581f63929eab5bc3",
    },
    Wire {
        name: "custom-impact.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/custom-impact.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261104/custom-impact-portable.json"),
        sha256: "sha256:0ab652f7cc2bb011cdbd958a6a57c3f01dba197b8467a55dd760474163f3c359",
    },
    Wire {
        name: "review.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/review.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261104/review-portable.json"),
        sha256: "sha256:4654366a67298bc8288309c43f293f9002a56cc24cc64783009ee4a91d9ffd23",
    },
    Wire {
        name: "capability-review.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/capability-review.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261104/capability-review-portable.json"),
        sha256: "sha256:bafbd2ad2f1942ce50b6fe4d8fe9d2ccb684cd6f9e0be928a9099653b4ecc9b5",
    },
];

const MACOS_WIRES: [Wire; 9] = [
    Wire {
        name: "public-graph.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/current/public-graph.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/previous/public-graph.json"),
        sha256: "sha256:1bc0382b70fd145e2bf05240ee5ee201430d53e68a6a5e26de25057d9b72a05f",
    },
    Wire {
        name: "context.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/current/context.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/previous/context.json"),
        sha256: "sha256:c7d6d45013bc02ee7db8b123b65a4260caa93801fa2a98a896555a37e88ff5e9",
    },
    Wire {
        name: "forward.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/current/forward.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/previous/forward.json"),
        sha256: "sha256:5eb298d01197db793563f0e96f9ec24024571b2096c43b56909b66a2461f2cd8",
    },
    Wire {
        name: "capability-context.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/current/capability-context.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/previous/capability-context.json"),
        sha256: "sha256:b870299429deab49444dc1afe4796faffc9523ba8e31c55696cde88234c8a670",
    },
    Wire {
        name: "type-context.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/current/type-context.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/previous/type-context.json"),
        sha256: "sha256:64575d3d1beca94ebdd95876410e4669636f9553f8ec6514851778c643928370",
    },
    Wire {
        name: "impact.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/current/impact.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/previous/impact.json"),
        sha256: "sha256:fe0ec2000302b47a8f5632055c8ed7113e6d3fdc172948765210be11386514af",
    },
    Wire {
        name: "custom-impact.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/current/custom-impact.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/previous/custom-impact.json"),
        sha256: "sha256:1d12998cff2cdce05f217200907e13a40b5ca6ae1a2e296302a5b33370ad4d06",
    },
    Wire {
        name: "review.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/current/review.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/previous/review.json"),
        sha256: "sha256:9f63b701e47d6bd9ab6c6497daabee0bc3d540bfcc5bd5fbe5347b4759fff1c0",
    },
    Wire {
        name: "capability-review.json",
        current: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/current/capability-review.json"),
        previous: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/macos/previous/capability-review.json"),
        sha256: "sha256:bc80e47e8a16ee8efd9b034686f246c930c13cba5b3e9b6bad3be4a2c60c4c72",
    },
];

pub(super) struct WireBank {
    wires: &'static [Wire; 9],
    proof: &'static str,
    schema: &'static str,
    pub(super) builder_bytes: usize,
    previous_builder_bytes: u64,
}

pub(super) const LINUX_GNU: WireBank = WireBank {
    wires: &LINUX_GNU_WIRES,
    proof: include_str!("../../../../docs/evidence/public-wire-accounting-20261117/proof.json"),
    schema: "semaprax.github-workspace-graph-wire-capture.v1",
    builder_bytes: 768_232,
    previous_builder_bytes: 732_542,
};

pub(super) const MACOS: WireBank = WireBank {
    wires: &MACOS_WIRES,
    proof: include_str!(
        "../../../../docs/evidence/public-wire-accounting-20261117/macos/proof.json"
    ),
    schema: "semaprax.local-workspace-graph-wire-capture.v1",
    builder_bytes: 768_376,
    previous_builder_bytes: 1_466_420,
};

impl WireBank {
    pub(super) fn current_pin(&self, index: usize, previous_pin: &str) -> &'static str {
        let wire = &self.wires[index];
        assert_eq!(
            super::document_digest(wire.previous.as_bytes()),
            previous_pin
        );
        wire.sha256
    }

    pub(super) fn assert_history(&self, name: &str, bytes: &[u8]) -> &'static str {
        let (index, wire) = self
            .wires
            .iter()
            .enumerate()
            .find(|(_, wire)| wire.name == name)
            .expect("closed set of nine captured workspace fixtures");
        let proof: serde_json::Value = serde_json::from_str(self.proof).unwrap();
        assert_eq!(proof["schema"], self.schema);
        let rows = proof["rows"].as_array().unwrap();
        assert_eq!(rows.len(), self.wires.len());
        let row = &rows[index];
        assert_eq!(row["name"], name);
        assert_eq!(
            usize::try_from(row["bytes"].as_u64().unwrap()).unwrap(),
            bytes.len()
        );
        assert_eq!(row["current_sha256"], wire.sha256);
        assert_eq!(super::document_digest(bytes), wire.sha256);
        assert!(bytes.len() <= 64 * 1024);
        assert!(
            bytes == wire.current.as_bytes(),
            "complete current host wire for {name}"
        );

        let restored = replay_edits(
            std::str::from_utf8(bytes).unwrap().to_owned(),
            row["inverse"].as_array().unwrap(),
        );
        assert_eq!(
            restored, wire.previous,
            "complete historical host wire for {name}"
        );
        assert_eq!(
            super::document_digest(restored.as_bytes()),
            row["previous_sha256"].as_str().unwrap()
        );
        wire.previous
    }
}

fn replay_edits(mut wire: String, edits: &[serde_json::Value]) -> String {
    let mut end = wire.len();
    assert!(!edits.is_empty() && edits.len() <= 512);
    for edit in edits.iter().rev() {
        let start = usize::try_from(edit[0].as_u64().unwrap()).unwrap();
        let stop = usize::try_from(edit[1].as_u64().unwrap()).unwrap();
        let after = edit[2].as_str().unwrap();
        let before = edit[3].as_str().unwrap();
        assert!(start <= stop && stop <= end);
        assert_eq!(wire.get(start..stop), Some(after));
        wire.replace_range(start..stop, before);
        assert!(wire.len() <= 64 * 1024);
        end = start;
    }
    wire
}

// The original byte inverse ends before the independently verified November
// accounting update. Execute that complete transition as a separate step.
pub(super) fn join_macos_accounting(index: usize, name: &str, previous: String) -> String {
    const JOIN: &str = include_str!(
        "../../../../docs/evidence/public-wire-accounting-20261117/macos/historical-accounting-join.json"
    );
    const MEASURED: [&str; 9] = [
        include_str!("../../../../docs/evidence/public-wire-accounting-20261104/public-graph-macos.json"),
        include_str!("../../../../docs/evidence/public-wire-accounting-20261104/context-macos.json"),
        include_str!("../../../../docs/evidence/public-wire-accounting-20261104/forward-macos.json"),
        include_str!("../../../../docs/evidence/public-wire-accounting-20261104/capability-context-macos.json"),
        include_str!("../../../../docs/evidence/public-wire-accounting-20261104/type-context-macos.json"),
        include_str!("../../../../docs/evidence/public-wire-accounting-20261104/impact-macos.json"),
        include_str!("../../../../docs/evidence/public-wire-accounting-20261104/custom-impact-macos.json"),
        include_str!("../../../../docs/evidence/public-wire-accounting-20261104/review-macos.json"),
        include_str!("../../../../docs/evidence/public-wire-accounting-20261104/capability-review-macos.json"),
    ];
    let proof: serde_json::Value = serde_json::from_str(JOIN).unwrap();
    let rows = proof.as_array().unwrap();
    assert_eq!(rows.len(), MEASURED.len());
    let row = &rows[index];
    assert_eq!(row["name"], name);
    assert_eq!(
        super::document_digest(previous.as_bytes()),
        row["restored_sha256"].as_str().unwrap()
    );
    let measured = replay_edits(previous, row["inverse"].as_array().unwrap());
    assert_eq!(measured, MEASURED[index]);
    assert_eq!(
        super::document_digest(measured.as_bytes()),
        row["target_sha256"].as_str().unwrap()
    );
    measured
}

#[test]
fn all_captured_host_wires_restore_their_independent_predecessors() {
    for bank in [LINUX_GNU, MACOS] {
        for wire in bank.wires {
            assert_eq!(
                bank.assert_history(wire.name, wire.current.as_bytes()),
                wire.previous
            );
        }
        let graph: serde_json::Value = serde_json::from_str(bank.wires[0].current).unwrap();
        assert_eq!(
            graph["budget"]["used_builder_bytes"].as_u64().unwrap(),
            bank.builder_bytes as u64
        );
        let previous: serde_json::Value = serde_json::from_str(bank.wires[0].previous).unwrap();
        assert_eq!(
            previous["budget"]["used_builder_bytes"].as_u64().unwrap(),
            bank.previous_builder_bytes
        );
    }
}
