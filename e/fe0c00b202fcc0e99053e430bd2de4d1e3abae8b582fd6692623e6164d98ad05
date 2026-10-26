//! Closed whole-input successors for the original boxed-state source readers.
//! Restore only the omitted compact-result change and later catalog pins; all
//! original positional inverses and complete predecessor assertions still run.
use sha2::{Digest, Sha256};
#[path = "../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
fn authenticate(path: &str, supplied: &str, compiled: &str, length: usize, sha: &str) {
    assert_eq!(supplied, compiled, "complete live boxed reader input");
    assert_eq!(compiled.len(), length);
    assert_eq!(digest(compiled), sha);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(&root.join(path), length, sha, 65536).unwrap(),
        compiled
    );
}
fn replace(input: &mut String, new: &str, old: &str, count: usize) {
    assert_eq!(
        input.matches(new).count(),
        count,
        "complete exact boxed inverse"
    );
    *input = input.replace(new, old);
}
pub(in crate::source_reflows_merge) fn input(source: &str, current: &str) -> String {
    if source == "docs/AGENT-SKILL-BUNDLE-V1.json" {
        return bundle(current);
    }
    let normalized = if source == "src/cleanup_plan/replay/path_join/boxed_states.rs" {
        authenticate(
            source,
            current,
            include_str!("../cleanup_plan/replay/path_join/boxed_states.rs"),
            7277,
            "1cb644157ccf8246f6a81011d0dd099a64f1809cffae15bb3e993c1d64eb93c5",
        );
        let mut before = current.to_owned();
        replace(
            &mut before,
            "use crate::cleanup_plan::replay::physical_budget::ByteFailure;\n",
            "",
            1,
        );
        replace(
            &mut before,
            concat!(
                "            // At most one positive-payment failure reaches conversion. Pay its\n",
                "            // actual/formal compact values and Diagnostic construction/return\n",
                "            // once, before any entry; sticky failure forbids reuse.\n",
                "            + 2 * size_of::<ByteFailure>()\n",
                "            + 2 * size_of::<Diagnostic>()\n"
            ),
            "",
            1,
        );
        replace(
            &mut before,
            "+ size_of::<Result<(), ByteFailure>>()",
            "+ size_of::<Result<(), Diagnostic>>()",
            1,
        );
        replace(
            &mut before,
            "budget.reserve_compact_bytes(",
            "budget.reserve_bytes(",
            4,
        );
        replace(
            &mut before,
            "Some(error.into_diagnostic())",
            "Some(error)",
            4,
        );
        assert_eq!(before.len(), 6796);
        assert_eq!(
            digest(&before),
            "9e736102f9b39a731db6ab6ae3d17280eb04e5fa7df913c86cc503ec1771566f"
        );
        before
    } else {
        current.to_owned()
    };
    super::super::compile_fixture_input(source, &normalized)
}
fn bundle(current: &str) -> String { let next488_physical_current = current; let next488_epoch; let current = if digest(current) == "282c7d4eba939d34b7a6fcf94cc3c45d9dcfbba0f7075668f7f98080962e8d03" { authenticate("docs/AGENT-SKILL-BUNDLE-V1.json", current, include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"), 5544, "282c7d4eba939d34b7a6fcf94cc3c45d9dcfbba0f7075668f7f98080962e8d03"); let mut prior = current.to_owned(); replace(&mut prior, "sha256:8d4ce3e78d3e31640468f338c46bb5a1cf451c203ec4beebb8df6e84df693146", "sha256:a041275d58191e4e29ea94c565e29d604dfd4ab0ca186fa2ab8316f1c9d20de2", 1); replace(&mut prior, "sha256:4f2920b7f825da8420e6e39f4ddf2c394fbd9dd28b6fb4514ab626b49697f012", "sha256:4f3b1979d609cadf9154d7caa1e0859f04acd83bca89eeb010f64b9600d439a9", 1); assert_eq!(prior.len(), 5544); assert_eq!(digest(&prior), "fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d"); let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")); assert_eq!(prior, include_str!("../../e/fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d")); assert_eq!(reader::read(&root.join("e/fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d"), 5544, "fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d", 65536).unwrap(), prior); next488_epoch = prior; next488_epoch.as_str() } else { current };  let direct1031_physical_current = current; let direct1031_epoch; let current = if digest(current) == "fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d" { if next488_physical_current == current { authenticate("docs/AGENT-SKILL-BUNDLE-V1.json", current, include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"), 5544, "fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d"); } else { authenticate("e/fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d", current, include_str!("../../e/fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d"), 5544, "fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d"); } let mut prior = current.to_owned(); replace(&mut prior, "sha256:a041275d58191e4e29ea94c565e29d604dfd4ab0ca186fa2ab8316f1c9d20de2", "sha256:4ca5fd0773a7c1190068b1dc9f4f51f981fbb6afaf716fce2578e7b304bf9251", 1); replace(&mut prior, "sha256:4f3b1979d609cadf9154d7caa1e0859f04acd83bca89eeb010f64b9600d439a9", "sha256:4e3574bc753d9e3dcca9b2bb3176536b485eeabb44045f9a5e0a52516e870e5a", 1); assert_eq!(prior.len(), 5544); assert_eq!(digest(&prior), "4ec03ff7a164c0f48a37ace16c206de7f096ae84e05c70c7409159beb65cf301"); let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")); assert_eq!(prior, include_str!("../../e/4ec03ff7a164c0f48a37ace16c206de7f096ae84e05c70c7409159beb65cf301")); assert_eq!(reader::read(&root.join("e/4ec03ff7a164c0f48a37ace16c206de7f096ae84e05c70c7409159beb65cf301"), 5544, "4ec03ff7a164c0f48a37ace16c206de7f096ae84e05c70c7409159beb65cf301", 65536).unwrap(), prior); direct1031_epoch = prior; direct1031_epoch.as_str() } else { current };
    // Source-only structural1012 epoch; fresh production must confirm these bytes.
    let boundary1012_physical_current = current;
    let boundary1012_epoch;
    let current = if digest(current) == "4ec03ff7a164c0f48a37ace16c206de7f096ae84e05c70c7409159beb65cf301" {
        if direct1031_physical_current == current { authenticate("docs/AGENT-SKILL-BUNDLE-V1.json", current,
            include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"), 5544,
            "4ec03ff7a164c0f48a37ace16c206de7f096ae84e05c70c7409159beb65cf301"); } else { assert_eq!(digest(direct1031_physical_current), "fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d"); assert_eq!(current, include_str!("../../e/4ec03ff7a164c0f48a37ace16c206de7f096ae84e05c70c7409159beb65cf301")); }
        let mut prior = current.to_owned();
        replace(&mut prior, "sha256:4ca5fd0773a7c1190068b1dc9f4f51f981fbb6afaf716fce2578e7b304bf9251", "sha256:d3fbde442377a3e9f72b4f05d96e33e8ceba46523a5f52a88c9e5044c3396ca4", 1);
        replace(&mut prior, "sha256:4e3574bc753d9e3dcca9b2bb3176536b485eeabb44045f9a5e0a52516e870e5a", "sha256:479123c567e380c66cc6618e88ed5b1ef8e430558e341a2cae95c6edf2ab86a7", 1);
        assert_eq!(prior.len(), 5544);
        assert_eq!(digest(&prior), "efc915af4b2eb7e3d0746f7abe48b380a8db6862ae1f72016a75d4fc55db97d9");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(prior, include_str!("../../e/efc915af4b2eb7e3d0746f7abe48b380a8db6862ae1f72016a75d4fc55db97d9"));
        assert_eq!(reader::read(&root.join("e/efc915af4b2eb7e3d0746f7abe48b380a8db6862ae1f72016a75d4fc55db97d9"), 5544,
            "efc915af4b2eb7e3d0746f7abe48b380a8db6862ae1f72016a75d4fc55db97d9", 65536).unwrap(), prior);
        boundary1012_epoch = prior;
        boundary1012_epoch.as_str()
    } else { current };
    // Source-only prospective1011 epoch; fresh production must confirm these bytes.
    let combined1011_physical_current = current;
    let combined1011_epoch;
    let current = if digest(current) == "efc915af4b2eb7e3d0746f7abe48b380a8db6862ae1f72016a75d4fc55db97d9" {
        if boundary1012_physical_current == current {
        authenticate("docs/AGENT-SKILL-BUNDLE-V1.json", current,
            include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"), 5544,
            "efc915af4b2eb7e3d0746f7abe48b380a8db6862ae1f72016a75d4fc55db97d9");
        } else {
            assert_eq!(digest(boundary1012_physical_current), "4ec03ff7a164c0f48a37ace16c206de7f096ae84e05c70c7409159beb65cf301");
            assert_eq!(current, include_str!("../../e/efc915af4b2eb7e3d0746f7abe48b380a8db6862ae1f72016a75d4fc55db97d9"));
        }
        let mut prior = current.to_owned();
        replace(&mut prior, "sha256:d3fbde442377a3e9f72b4f05d96e33e8ceba46523a5f52a88c9e5044c3396ca4", "sha256:f961a332a6051ebb1f1600aa714a50efedf8769b5447daf73b141a5cfa1170d4", 1);
        replace(&mut prior, "sha256:479123c567e380c66cc6618e88ed5b1ef8e430558e341a2cae95c6edf2ab86a7", "sha256:0fb7be8e37a7c95c33fb3b0af0f3bba7caf73981635901f3444753fb60650419", 1);
        assert_eq!(prior.len(), 5544);
        assert_eq!(digest(&prior), "b0a4f70186e44c1866f1e606c795bd4b2afe868c1839a9ad3f875ae4792ade15");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(prior, include_str!("../../e/b0a4f70186e44c1866f1e606c795bd4b2afe868c1839a9ad3f875ae4792ade15"));
        assert_eq!(reader::read(&root.join("e/b0a4f70186e44c1866f1e606c795bd4b2afe868c1839a9ad3f875ae4792ade15"), 5544,
            "b0a4f70186e44c1866f1e606c795bd4b2afe868c1839a9ad3f875ae4792ade15", 65536).unwrap(), prior);
        combined1011_epoch = prior;
        combined1011_epoch.as_str()
    } else { current };
    // Source-only prospective epoch: the fresh public producer must confirm these bytes.
    let next_physical_current = current;
    let next_epoch;
    let current = if digest(current) == "b0a4f70186e44c1866f1e606c795bd4b2afe868c1839a9ad3f875ae4792ade15" {
        if combined1011_physical_current == current {
        authenticate("docs/AGENT-SKILL-BUNDLE-V1.json", current,
            include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"), 5544,
            "b0a4f70186e44c1866f1e606c795bd4b2afe868c1839a9ad3f875ae4792ade15");
        } else {
            assert_eq!(digest(combined1011_physical_current), "efc915af4b2eb7e3d0746f7abe48b380a8db6862ae1f72016a75d4fc55db97d9");
            assert_eq!(current, include_str!("../../e/b0a4f70186e44c1866f1e606c795bd4b2afe868c1839a9ad3f875ae4792ade15"));
        }
        let mut prior = current.to_owned();
        replace(&mut prior, "sha256:f961a332a6051ebb1f1600aa714a50efedf8769b5447daf73b141a5cfa1170d4", "sha256:6d5fd3ef7155ad7c3c5df13a49e7f2176feeafc6e238e177d87fc16d0a31fc8f", 1);
        replace(&mut prior, "sha256:0fb7be8e37a7c95c33fb3b0af0f3bba7caf73981635901f3444753fb60650419", "sha256:ca7d65ae3e549d3465749be1d6deda0cf4a1a7c21d21d7b6804fd37842526e5a", 1);
        assert_eq!(prior.len(), 5544);
        assert_eq!(digest(&prior), "0ce960e3b7ff74c435bc3d43126ab85a09c43e7eed125dbd9922d85bae89621b");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(reader::read(&root.join("e/0ce960e3b7ff74c435bc3d43126ab85a09c43e7eed125dbd9922d85bae89621b"), 5544,
            "0ce960e3b7ff74c435bc3d43126ab85a09c43e7eed125dbd9922d85bae89621b", 65536).unwrap(), prior);
        next_epoch = prior;
        next_epoch.as_str()
    } else { current };
    // One complete repair epoch before every unchanged historical GUIDE inverse.
    let repair_physical_current = current;
    let repair_epoch;
    let current = if digest(current) == "0ce960e3b7ff74c435bc3d43126ab85a09c43e7eed125dbd9922d85bae89621b" {
        if next_physical_current == current {
        authenticate("docs/AGENT-SKILL-BUNDLE-V1.json", current,
            include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"), 5544,
            "0ce960e3b7ff74c435bc3d43126ab85a09c43e7eed125dbd9922d85bae89621b");
        } else {
            assert_eq!(digest(next_physical_current), "b0a4f70186e44c1866f1e606c795bd4b2afe868c1839a9ad3f875ae4792ade15");
            assert_eq!(current, include_str!("../../e/0ce960e3b7ff74c435bc3d43126ab85a09c43e7eed125dbd9922d85bae89621b"));
        }
        let mut prior = current.to_owned();
        replace(&mut prior, "sha256:6d5fd3ef7155ad7c3c5df13a49e7f2176feeafc6e238e177d87fc16d0a31fc8f", "sha256:9832e06cd6bd1cbe55d25009498f577f2cd1796843279b6f579d78f25bfb1a55", 1);
        replace(&mut prior, "sha256:ca7d65ae3e549d3465749be1d6deda0cf4a1a7c21d21d7b6804fd37842526e5a", "sha256:86c97a10aa779d1496a8f733ea651247af93704a7b18c9459be6e1dc7658c400", 1);
        assert_eq!(prior.len(), 5544);
        assert_eq!(digest(&prior), "f4b75c854f4c04a841816bcad6c5972b330b794de644b547ca1ced533ac2345c");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(reader::read(&root.join("e/f4b75c854f4c04a841816bcad6c5972b330b794de644b547ca1ced533ac2345c"), 5544,
            "f4b75c854f4c04a841816bcad6c5972b330b794de644b547ca1ced533ac2345c", 65536).unwrap(), prior);
        repair_epoch = prior;
        repair_epoch.as_str()
    } else { current };
    let physical_current = current;
    // Prospective source epoch only; fresh public production must confirm these bytes.
    let incoming_b7_guide_epoch;
    let current = if digest(current)
        == "f4b75c854f4c04a841816bcad6c5972b330b794de644b547ca1ced533ac2345c"
    {
        if repair_physical_current == current {
        authenticate(
            "docs/AGENT-SKILL-BUNDLE-V1.json",
            current,
            include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"),
            5544,
            "f4b75c854f4c04a841816bcad6c5972b330b794de644b547ca1ced533ac2345c",
        );
        } else {
            assert_eq!(digest(repair_physical_current), "0ce960e3b7ff74c435bc3d43126ab85a09c43e7eed125dbd9922d85bae89621b");
            assert_eq!(current, include_str!("../../e/f4b75c854f4c04a841816bcad6c5972b330b794de644b547ca1ced533ac2345c"));
        }
        let mut prior = current.to_owned();
        replace(
            &mut prior,
            "sha256:9832e06cd6bd1cbe55d25009498f577f2cd1796843279b6f579d78f25bfb1a55",
            "sha256:46b6167a2937d423dea03a3e20d222f1a23942f8c83bf6a6a480d9473e5205f1",
            1,
        );
        replace(
            &mut prior,
            "sha256:86c97a10aa779d1496a8f733ea651247af93704a7b18c9459be6e1dc7658c400",
            "sha256:e12ce170f16c346621a997c7437721048d23b61746d67e71dde626033d5b8752",
            1,
        );
        assert_eq!(prior.len(), 5544);
        assert_eq!(digest(&prior), "41ff139de9ebc3d6b0b96f29811246df36051422db5bbc5715717e416599b71e");
        incoming_b7_guide_epoch = prior;
        incoming_b7_guide_epoch.as_str()
    } else { current };
    let predicted_guide_epoch;
    let current = if digest(current)
        == "41ff139de9ebc3d6b0b96f29811246df36051422db5bbc5715717e416599b71e"
    {
        if physical_current == current {
        authenticate(
            "docs/AGENT-SKILL-BUNDLE-V1.json",
            current,
            include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"),
            5544,
            "41ff139de9ebc3d6b0b96f29811246df36051422db5bbc5715717e416599b71e",
        );
        } else {
            assert_eq!(physical_current.len(), 5544);
            assert_eq!(digest(physical_current), "f4b75c854f4c04a841816bcad6c5972b330b794de644b547ca1ced533ac2345c");
            assert_eq!(current.len(), 5544);
            assert_eq!(digest(current), "41ff139de9ebc3d6b0b96f29811246df36051422db5bbc5715717e416599b71e");
        }
        let mut prior = current.to_owned();
        replace(
            &mut prior,
            "sha256:46b6167a2937d423dea03a3e20d222f1a23942f8c83bf6a6a480d9473e5205f1",
            "sha256:4d03ba63891a822aa638558f9d5dd58fcb6cec3e51cc70b742dc1845476abf73",
            1,
        );
        replace(
            &mut prior,
            "sha256:e12ce170f16c346621a997c7437721048d23b61746d67e71dde626033d5b8752",
            "sha256:7422df629f3232df98c2b2afeb908aceab7bcf63400495fbdd1f1d9928950922",
            1,
        );
        assert_eq!(prior.len(), 5544);
        assert_eq!(
            digest(&prior),
            "cfcee456219f766e1d939c7a8c90a38c2e42bce79644a3990323719491ae648a"
        );
        predicted_guide_epoch = prior;
        predicted_guide_epoch.as_str()
    } else {
        current
    };
    // Authenticate the actual public epoch before constructing a named historical input.
    let guide_epoch;
    let current = if digest(current)
        == "cfcee456219f766e1d939c7a8c90a38c2e42bce79644a3990323719491ae648a"
    {
        if physical_current == current {
            authenticate(
                "docs/AGENT-SKILL-BUNDLE-V1.json",
                current,
                include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"),
                5544,
                "cfcee456219f766e1d939c7a8c90a38c2e42bce79644a3990323719491ae648a",
            );
        } else {
            assert_eq!(physical_current.len(), 5544);
            if digest(physical_current) == "f4b75c854f4c04a841816bcad6c5972b330b794de644b547ca1ced533ac2345c" {
                assert_eq!(physical_current.len(), 5544);
            } else {
            assert_eq!(
                digest(physical_current),
                "41ff139de9ebc3d6b0b96f29811246df36051422db5bbc5715717e416599b71e"
            );
            }
            assert_eq!(current.len(), 5544);
            assert_eq!(
                digest(current),
                "cfcee456219f766e1d939c7a8c90a38c2e42bce79644a3990323719491ae648a"
            );
        }
        let mut prior = current.to_owned();
        replace(
            &mut prior,
            "sha256:4d03ba63891a822aa638558f9d5dd58fcb6cec3e51cc70b742dc1845476abf73",
            "sha256:91f9c6c07e143a11d4b9c837395c775f143d438d163f8474aa7356998125fb07",
            1,
        );
        replace(
            &mut prior,
            "sha256:7422df629f3232df98c2b2afeb908aceab7bcf63400495fbdd1f1d9928950922",
            "sha256:68bf89cdc667b569778de5905df8ebd4c4cf4d1a5f76774b734fa2d74d15cfde",
            1,
        );
        assert_eq!(prior.len(), 5544);
        assert_eq!(
            digest(&prior),
            "29ad234d3e02654130c56d0facfa568d8113d67fb9779ede8fd87a4aad04127d"
        );
        guide_epoch = prior;
        guide_epoch.as_str()
    } else {
        current
    };
    let projected_current;
    let current =
        if digest(current) == "29ad234d3e02654130c56d0facfa568d8113d67fb9779ede8fd87a4aad04127d" {
            if physical_current == current {
                authenticate(
                    "docs/AGENT-SKILL-BUNDLE-V1.json",
                    current,
                    include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"),
                    5544,
                    "29ad234d3e02654130c56d0facfa568d8113d67fb9779ede8fd87a4aad04127d",
                );
            } else {
                // The physical epoch was authenticated before its exact two-field inverse.
                assert_eq!(physical_current.len(), 5544);
                if digest(physical_current) == "f4b75c854f4c04a841816bcad6c5972b330b794de644b547ca1ced533ac2345c" {
                    assert_eq!(physical_current.len(), 5544);
                } else if digest(physical_current)
                    == "41ff139de9ebc3d6b0b96f29811246df36051422db5bbc5715717e416599b71e"
                {
                    assert_eq!(physical_current.len(), 5544);
                } else {
                    assert_eq!(
                        digest(physical_current),
                        "cfcee456219f766e1d939c7a8c90a38c2e42bce79644a3990323719491ae648a"
                    );
                }
                assert_eq!(current.len(), 5544);
                assert_eq!(
                    digest(current),
                    "29ad234d3e02654130c56d0facfa568d8113d67fb9779ede8fd87a4aad04127d"
                );
            }
            // Named historical INPUT view; the public producer must confirm this epoch.
            let mut prior = current.to_owned();
            replace(
                &mut prior,
                "sha256:91f9c6c07e143a11d4b9c837395c775f143d438d163f8474aa7356998125fb07",
                "sha256:698207edde124e92d551bea1eaad72f317aeaa20b912d711226fa943254af497",
                1,
            );
            replace(
                &mut prior,
                "sha256:68bf89cdc667b569778de5905df8ebd4c4cf4d1a5f76774b734fa2d74d15cfde",
                "sha256:24c7df3eb5fbb4cbc37b2b2f791d3bc60ada5ae75b8bb374cb75eb182c50e72d",
                1,
            );
            assert_eq!(prior.len(), 5544);
            assert_eq!(
                digest(&prior),
                "95f46ebd3476911f26ab508995fc436f958a62a77a348cf0672dbbd2aec00f0e"
            );
            projected_current = prior;
            projected_current.as_str()
        } else {
            authenticate(
                "docs/AGENT-SKILL-BUNDLE-V1.json",
                current,
                include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"),
                5544,
                "95f46ebd3476911f26ab508995fc436f958a62a77a348cf0672dbbd2aec00f0e",
            );
            current
        };
    // Explicit historical INPUT view only; physical/current bytes above stay fresh.
    let mut prior_bundle = current.to_owned();
    replace(
        &mut prior_bundle,
        "sha256:698207edde124e92d551bea1eaad72f317aeaa20b912d711226fa943254af497",
        "sha256:a628a4b1184603fceb6a85f63e8e44176cb7a253db701c03440f5f3420cd72e3",
        1,
    );
    replace(
        &mut prior_bundle,
        "sha256:24c7df3eb5fbb4cbc37b2b2f791d3bc60ada5ae75b8bb374cb75eb182c50e72d",
        "sha256:4a38fc585ee4e86c00aabc5f4c11c2760d809c7e0dcbae9c6ea5938a26e3039b",
        1,
    );
    assert_eq!(prior_bundle.len(), 5544);
    assert_eq!(
        digest(&prior_bundle),
        "d4099b09009b4a1fcd2afffabee894ab4adf6d0608e416c95d0d6615db561270"
    );
    let current = prior_bundle.as_str();
    let receipt = include_str!("../../docs/evidence/boxed-path-states-v1/catalog-receipt.json");
    authenticate(
        "docs/evidence/boxed-path-states-v1/catalog-receipt.json",
        receipt,
        receipt,
        receipt.len(),
        "bb2d89942ce7b5ad759a787bcb1a0c5a0f1def93075fb70966089d30d1fdc279",
    );
    let mut before = current.to_owned();
    // Restore the entire authenticated source-cut seed bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:4a38fc585ee4e86c00aabc5f4c11c2760d809c7e0dcbae9c6ea5938a26e3039b",
        "sha256:982c5fb36e2e246a0e11290f170181b44984a918028a4efe5dccdddfbdb60dca",
        1,
    );
    replace(
        &mut before,
        "sha256:a628a4b1184603fceb6a85f63e8e44176cb7a253db701c03440f5f3420cd72e3",
        "sha256:5a10694f4e1872ce61d0e2c6b16e7c9c7685a91bc65016855dae42f2ae59af5f",
        1,
    );
    assert_eq!(
        digest(&before),
        "708991a555c4d082f6dfe12670238ab1dff76267ca841fc318e9106de0a4b5a4"
    );
    // Restore the entire authenticated source-cut seed bundle before its unchanged old stages.
    assert_eq!(
        digest(&before),
        "708991a555c4d082f6dfe12670238ab1dff76267ca841fc318e9106de0a4b5a4"
    );
    // Restore the entire authenticated Git89df seed bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:982c5fb36e2e246a0e11290f170181b44984a918028a4efe5dccdddfbdb60dca",
        "sha256:d4fbca5ca5434cb96703b7836a455433e574e8b4b49753e6d626e15d54279f75",
        1,
    );
    replace(
        &mut before,
        "sha256:5a10694f4e1872ce61d0e2c6b16e7c9c7685a91bc65016855dae42f2ae59af5f",
        "sha256:d71f8b7129ad920130746efb90c50f1d01193564dc877714dba88570f4eb0751",
        1,
    );
    assert_eq!(
        digest(&before),
        "b89cb859f6fdf0a95f783a26145331fbc45f47ee45dd18c048337f8c08f3493f"
    );
    // Restore the entire authenticated Git89df seed bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:d4fbca5ca5434cb96703b7836a455433e574e8b4b49753e6d626e15d54279f75",
        "sha256:8fb19f233cda3a16e72d4f7eadc1704a919cff1b26f8c7f5c09c097c14b375b7",
        1,
    );
    replace(
        &mut before,
        "sha256:d71f8b7129ad920130746efb90c50f1d01193564dc877714dba88570f4eb0751",
        "sha256:b79a3992d9e09d405e0a40cb918a4966f5495aef408ab045e345b34aaca9f7da",
        1,
    );
    assert_eq!(
        digest(&before),
        "ac6508eb6f2512eeb79c548d60e92f92cfb95c01e4d79a7b76e2e3971ca15524"
    );
    // Restore the entire authenticated Git89df seed bundle before its unchanged old stages.
    assert_eq!(
        digest(&before),
        "ac6508eb6f2512eeb79c548d60e92f92cfb95c01e4d79a7b76e2e3971ca15524"
    );
    // Restore the entire authenticated Git3e seed bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:8fb19f233cda3a16e72d4f7eadc1704a919cff1b26f8c7f5c09c097c14b375b7",
        "sha256:fcc27b3427068e199c2c7b34dd3bcbcdfabb73c92c96c475f3e860aa7ded9d4d",
        1,
    );
    replace(
        &mut before,
        "sha256:b79a3992d9e09d405e0a40cb918a4966f5495aef408ab045e345b34aaca9f7da",
        "sha256:f74d1c326025cdcbce2ee7775ad58fe5fb6f902744d6f07bb349beab65748d5a",
        1,
    );
    assert_eq!(
        digest(&before),
        "9f49e2011de221c6da0ed781cf6c04ea2176b1b1972bddd42ffbdc48e2f8ed71"
    );
    // Restore the entire authenticated Git5d seed bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:fcc27b3427068e199c2c7b34dd3bcbcdfabb73c92c96c475f3e860aa7ded9d4d",
        "sha256:af6e6094624826672ef9c1101fa970d419253f699ff043e9d70cc294056b660b",
        1,
    );
    replace(
        &mut before,
        "sha256:f74d1c326025cdcbce2ee7775ad58fe5fb6f902744d6f07bb349beab65748d5a",
        "sha256:bb992492efa1a070766040e0adb0943c032fd051ec132d5444a1e1f478fb5342",
        1,
    );
    assert_eq!(
        digest(&before),
        "a140b4074a80af47f3615831ac01354a851f0789b9f03265526f2d457ffc05a9"
    );
    // Restore the entire authenticated 2837 bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:af6e6094624826672ef9c1101fa970d419253f699ff043e9d70cc294056b660b",
        "sha256:cb997dd38feea65c0ba677503350005b9d9d3dee885c6812947cdc364018eb4d",
        1,
    );
    replace(
        &mut before,
        "sha256:bb992492efa1a070766040e0adb0943c032fd051ec132d5444a1e1f478fb5342",
        "sha256:7413596845b7d232f8ff8c7b9c3d9318acd4b7b5324d6deb64d8e0f8b891c86e",
        1,
    );
    assert_eq!(
        digest(&before),
        "f4031b6b357499122748e9d0c5de0d3641f45855566ed899ff600347688210c8"
    );
    // Restore the entire authenticated 2837 bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:cb997dd38feea65c0ba677503350005b9d9d3dee885c6812947cdc364018eb4d",
        "sha256:525efa443c2b2a1f97b64debeac080e3a82b092ed510feb416798f29ad58ed48",
        1,
    );
    replace(
        &mut before,
        "sha256:7413596845b7d232f8ff8c7b9c3d9318acd4b7b5324d6deb64d8e0f8b891c86e",
        "sha256:bb7816577dde8feaf180dfa647195c9abd4a1dfac194ddfa55c9b7ae3e22b134",
        1,
    );
    assert_eq!(
        digest(&before),
        "77cc2280493407e6f3a1c4247f778cceaf09be14f1421f6f23787f8c4f0dabf7"
    );
    // Restore the entire authenticated 2837 bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:525efa443c2b2a1f97b64debeac080e3a82b092ed510feb416798f29ad58ed48",
        "sha256:d9abead8fcb85521d6ca6183d62f885ba3b85558f81384c960b0db01b948add8",
        1,
    );
    replace(
        &mut before,
        "sha256:bb7816577dde8feaf180dfa647195c9abd4a1dfac194ddfa55c9b7ae3e22b134",
        "sha256:7f4788567710a86259ceb3621963db6ca499ad6ac5323d1b4497f313b52c635b",
        1,
    );
    assert_eq!(
        digest(&before),
        "12698588bbf09117e96fe8f8f62f74b1290f37fc7d8d0e6e20dfe54f8bc02c65"
    );
    // Restore the entire authenticated 56fb bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:d9abead8fcb85521d6ca6183d62f885ba3b85558f81384c960b0db01b948add8",
        "sha256:e894c9102dfb2fc1b29d48ee64265786dd18ec9491417cea9725aaac3a40d96f",
        1,
    );
    replace(
        &mut before,
        "sha256:7f4788567710a86259ceb3621963db6ca499ad6ac5323d1b4497f313b52c635b",
        "sha256:19cb70ce2a043430d6fd71b60ff203ce2cf26390ed4ca01339c8259efb7d36ed",
        1,
    );
    assert_eq!(
        digest(&before),
        "51c95c8f2849633d9abfd66812f57429d8ff9c72b2d9882af01179bdbd9862dc"
    );
    replace(
        &mut before,
        "sha256:19cb70ce2a043430d6fd71b60ff203ce2cf26390ed4ca01339c8259efb7d36ed",
        "sha256:023c0d3ea1823196a2f5d44e3b10a862df939f834e98d7a164b3245af340a73c",
        1,
    );
    replace(
        &mut before,
        "sha256:e894c9102dfb2fc1b29d48ee64265786dd18ec9491417cea9725aaac3a40d96f",
        "sha256:479c7de1c209061d5706e405ffea92358aef690d02a536dd1f9162c13c977e85",
        1,
    );
    replace(&mut before, "\"code_count\":1517", "\"code_count\":1513", 1);
    replace(
        &mut before,
        "sha256:bad33c1869a184361663b0a9f4b5d46a6fb9a52433c94d66289c8456ced75b67",
        "sha256:cb620f216f1fe93bad2c50db7e07448679497e73857a0104c499f3d22238ea3c",
        1,
    );
    assert_eq!(
        digest(&before),
        "bb1d335ef40cff2cf81d08ddcf0943b168d04721ee1e7de1b3d809d3b9600d2f"
    );
    if digest(direct1031_physical_current) == "fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d" { assert_eq!({ if next488_physical_current == direct1031_physical_current { reader::read(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/AGENT-SKILL-BUNDLE-V1.json"), 5544, "fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d", 65536).unwrap() } else { assert_eq!(reader::read(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/AGENT-SKILL-BUNDLE-V1.json"), 5544, "282c7d4eba939d34b7a6fcf94cc3c45d9dcfbba0f7075668f7f98080962e8d03", 65536).unwrap(), next488_physical_current); reader::read(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("e/fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d"), 5544, "fb8d1f3d35e195eaf39a22a6cc1e3e6f1f03ab90e497c885d782e32809564a6d", 65536).unwrap() } }, direct1031_physical_current); } else { assert_eq!(
        if digest(boundary1012_physical_current) == "4ec03ff7a164c0f48a37ace16c206de7f096ae84e05c70c7409159beb65cf301" {
            reader::read(&std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("docs/AGENT-SKILL-BUNDLE-V1.json"), 5544,
                "4ec03ff7a164c0f48a37ace16c206de7f096ae84e05c70c7409159beb65cf301", 65536).unwrap()
        } else {
            if digest(combined1011_physical_current) == "efc915af4b2eb7e3d0746f7abe48b380a8db6862ae1f72016a75d4fc55db97d9" {
            reader::read(&std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("docs/AGENT-SKILL-BUNDLE-V1.json"), 5544,
                "efc915af4b2eb7e3d0746f7abe48b380a8db6862ae1f72016a75d4fc55db97d9", 65536).unwrap()
        } else {
            std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/AGENT-SKILL-BUNDLE-V1.json")
            )
            .unwrap()
        }
        },
        boundary1012_physical_current,
    ); }
    before
}
