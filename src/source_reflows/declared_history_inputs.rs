//! Exact current-to-declared history tuples; no compiler or billing authority.
use sha2::{Digest, Sha256};
use std::path::{Component, Path};
use std::sync::OnceLock;
#[path = "../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
const CAP: usize = 2_097_152;
pub(crate) fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
const PARTS: [(&str, &str, usize, &str); 7] = [
    (
        "docs/evidence/declared-history-inputs-20261105/part-0.json",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/declared-history-inputs-20261105/part-0.json"
        )),
        873093,
        "435f6a7b3d6f0c41540411e98bf2bc6c97e920f6631b2555c969ad27e9ccfcc6",
    ),
    (
        "docs/evidence/declared-history-inputs-20261105/part-1.json",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/declared-history-inputs-20261105/part-1.json"
        )),
        870404,
        "04d8f9db890668933d8436e981ce9a4840ecce9428ec863a166ae65f46a23fb5",
    ),
    (
        "docs/evidence/declared-history-inputs-20261105/part-2.json",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/declared-history-inputs-20261105/part-2.json"
        )),
        837591,
        "2451f1573a35253db78dd2eefdb47af8c5774ec880e97e0e0fe824e2dd2703a8",
    ),
    (
        "docs/evidence/declared-history-inputs-20261105/part-3.json",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/declared-history-inputs-20261105/part-3.json"
        )),
        831295,
        "a0836c7b26b8a695035801c9ef42b0e935d7a4b9b78e469a32f2d10974f08fcf",
    ),
    (
        "docs/evidence/declared-history-inputs-20261105/part-4.json",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/declared-history-inputs-20261105/part-4.json"
        )),
        327356,
        "1b2f920b9220ce29e6709de284d80b2bb8c08cfc369765264dc946c5da7f1494",
    ),
    (
        "docs/evidence/declared-history-inputs-20261105/part-5.json",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/declared-history-inputs-20261105/part-5.json"
        )),
        15987,
        "bfcaf0d7314c4933a709007b77855c66c7d6e4513c92e19974a559e762336d63",
    ),
    (
        "docs/evidence/declared-history-inputs-20261105/part-6.json",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/declared-history-inputs-20261105/part-6.json"
        )),
        196882,
        "69e53ec225a48a8b279af2a76a91dfa8fb308f1af2acbccf4f298c1a20670110",
    ),
];
static CACHE: [OnceLock<serde_json::Value>; 7] = [const { OnceLock::new() }; 7];
pub(crate) fn records(part: usize) -> &'static serde_json::Value {
    let (path, compiled, length, sha) = PARTS[part];
    let fresh = reader::read(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(path),
        length,
        sha,
        CAP,
    )
    .unwrap();
    assert_eq!(
        fresh, compiled,
        "whole fresh/compiled declared-history table"
    );
    CACHE[part].get_or_init(|| serde_json::from_str(compiled).unwrap())
}
pub(crate) fn slot(source: &str, expected: &str) -> Option<(usize, usize)> {
    match (source,expected) {
("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "42c45b3a526462ada8ab195a1a031ed890ded88a1022c8af17f1a63760a976b3" ) => Some((0,0)),
("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "509e64bcddcc9fb49d850b195487b0cdc0723e4c0d535eaae3931b7f4cdcab73" ) => Some((0,1)),
("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "70b707a2915c51abb76a72aeb084835c943c8a969c906a1b12eb287e7fa6beca" ) => Some((0,2)),
("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "754b81a88c3b698f7098524bd76cf0726d189632c44cf9027e6724e314cd7d53" ) => Some((0,3)),
("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "d764e8d6fbd373a2dbb40953baabf688fb4459562760fd5e1545cb9b73b9e6b0" ) => Some((0,4)),
("src/byte_ops.rs", "22e3f98ab80d320bed149c19de4ab1394f0dca7b55be3b7080f2745a50186ade" ) => Some((0,5)),
("src/cleanup_plan/renewal_profile.rs", "36323cd8461cb885b7bd6511a2738d29d709ba8d2a4278e83e38f2e3b2767d70" ) => Some((0,6)),
("src/cleanup_plan/renewal_profile.rs", "bde7beca5c39e461272e062a2c9137ad75a95cff150e230d0adbe4f4f0f7bf2b" ) => Some((0,7)),
("src/cleanup_plan/renewal_profile/funded.rs", "076c0150da2329e18d1238ccae4088f69f465f5d2fb4aba0c7529bce546bb356" ) => Some((0,8)),
("src/cleanup_plan/renewal_profile/funded.rs", "852de60c94bcf8d34061bb51e91068a5d2f8f7a475f2ef95b2dc2901d41ee184" ) => Some((0,9)),
("src/cleanup_plan/replay.rs", "6db5b0ed361258023e3d7cad058c3de6d737b416b67e98e7113ecb0a993c86c1" ) => Some((0,10)),
("src/cleanup_plan/replay.rs", "cf20e9132b5b94602777d4b1d70495da2c8411181a9f642919f6a0d4aa8c12c8" ) => Some((0,11)),
("src/cleanup_plan/replay.rs", "e02105834f829503b1e1a6645067f7b0784fed133890c6f7b4553346ba011f28" ) => Some((0,12)),
("src/cleanup_plan/replay.rs", "f55a89e71e01d7254fd13ae57b9fbe104a08bddc1a3f080140b6d70213c1544d" ) => Some((0,13)),
("src/cleanup_plan/replay/branch_clone.rs", "e0c30462e1576af2a9b48f9bdc55a64ce606510864752932fc8b280a3c77b1d4" ) => Some((0,14)),
("src/cleanup_plan/replay/branch_clone/tests.rs", "1ceebb30aefeb84f0b3be9a2ff916c247a8017ecbe1a33d58f36bd556094c757" ) => Some((0,15)),
("src/cleanup_plan/replay/branch_clone/tests.rs", "28c1f3469009b7307fb8ecaa36c383bbbf270ccc2ce88f1d0adefb823b55080e" ) => Some((0,16)),
("src/cleanup_plan/replay/branch_clone/tests.rs", "48956a2d192fdbc73cbfa58cb9e57f3e4380a4ff601728ded0d53c2bbf9db8ca" ) => Some((1,0)),
("src/cleanup_plan/replay/branch_clone/tests.rs", "6509ab480c69cfe00583d9d640f73726f5184f728b16dcde2d7d35b42fe15496" ) => Some((1,1)),
("src/cleanup_plan/replay/continuation_storage.rs", "4afe0bddb6402b7708b59f02963f954b7c2c9ade69c99a7803a95cdd23b745a8" ) => Some((1,2)),
("src/cleanup_plan/replay/continuation_storage/tests.rs", "ef071c7def7542ab0365cdd9566779a4b37270365bf6c147df7e87122debbdc2" ) => Some((1,3)),
("src/cleanup_plan/replay/expression_facts/arena.rs", "2eae0ecc943b9114099a7e87c7b2acf565c0f6454cb02f396ee30e981a109488" ) => Some((1,4)),
("src/cleanup_plan/replay/expression_facts/arena.rs", "c075f4e758cd36ab40328df263bbd8b4836c8012abdcc6c4fd23fd95efb13a59" ) => Some((1,5)),
("src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs", "fdb672ec67714810ba990a306ec8b282ce7c628d627b272b322d21e51293da34" ) => Some((1,6)),
("src/cleanup_plan/replay/expression_facts/arena/prefix.rs", "5b41b54ec6cf5e584e1845ab4e823ce752012ded7d3b9c485a3eb4fb181a74ae" ) => Some((1,7)),
("src/cleanup_plan/replay/expression_facts/arena/prefix.rs", "af5646f90884f31ab2a52f32a98a90a0a1d2710c36e1b0e04838f4a27ac98365" ) => Some((1,8)),
("src/cleanup_plan/replay/expression_facts/arena/prefix.rs", "dd227ffdfe6bccda89dbce95c99f3ed7d6106d54a4be3f89de319311e8a1d1b7" ) => Some((1,9)),
("src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs", "3de11b7505c11098ea01952832745ce8a2818a4a2c9a06932678bb783081c9b4" ) => Some((1,10)),
("src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs", "cf3a63ba83b16a9308def7f749b37f30e9287cf34ee0bb2845baf4c26fca78d8" ) => Some((1,11)),
("src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs", "37dbfa2677e03beab295d5f96fbb3a4f9a4a84908721021b566113156dbc43f0" ) => Some((1,12)),
("src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs", "9f9c495e16fe9af39e773c6ef5413bc8eff8363b21d5771620f7bd91eec746b1" ) => Some((1,13)),
("src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs", "371f112bb250011df5930334be7d3bd4f4ff82e10a8471469d981ca0ba094a87" ) => Some((1,14)),
("src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs", "58d578202f5276792d67035830b908a97408d3c702d015e89ce316becf13bc17" ) => Some((1,15)),
("src/cleanup_plan/replay/expression_facts/receiving/helpers.rs", "0c17a23ade80a8a8cd9f69e0f2ee79afa19df2996444ad86a204ea710a15d049" ) => Some((1,16)),
("src/cleanup_plan/replay/expression_facts/receiving/helpers.rs", "601b4fc2980f98019a1491988449050c3c4f54359466c88acc46e030245930f6" ) => Some((1,17)),
("src/cleanup_plan/replay/expression_facts/receiving/helpers.rs", "d8283639362ab72ba61cfa137d83bb02389deb9dd67599f627a662e0d55ddf16" ) => Some((1,18)),
("src/cleanup_plan/replay/expression_facts/receiving/helpers/matching.rs", "29a301abb545c42ffd46c2f8b017d24743deaa6ac702ca9b919a529e29b32910" ) => Some((1,19)),
("src/cleanup_plan/replay/expression_facts/receiving/helpers/matching.rs", "a965e2955383a9a2a8f3d6b67ce264008ee221f12d0f1dcc23ef8f5127971465" ) => Some((1,20)),
("src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/tests.rs", "8aa70c2958a9f8e31ff2b281c7d44bc7f2ef5d69439c86b64c79ea7072850cac" ) => Some((1,21)),
("src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs", "1d31e1bf8377e7a1d7dd1d5a8844faa974cefa6cf94bd3029283647f70d2a51c" ) => Some((1,22)),
("src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs", "f5bf554985629d8bd03343ea98b22c83cd042e77176a7fffff79b090b43cc330" ) => Some((1,23)),
("src/cleanup_plan/replay/expression_facts/receiving/remaining.rs", "545c82627829c38bdc380b7e7c9023c8cba599af9cfd6e0fcd235b494c1e4b45" ) => Some((1,24)),
("src/cleanup_plan/replay/expression_facts/receiving/remaining.rs", "eff95ac901df52b4012ad9058f12e0189eb57997690e2f2e3bab47ae0cbd0b2a" ) => Some((1,25)),
("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "7739c1324f612967d28bb4eb6fb87cdaa5354d743b4435d396d6f148c1354b31" ) => Some((1,26)),
("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "912a91d979144205750367d55e7aa9b74a4b3f4a1afc8472c010cf26af2677e8" ) => Some((1,27)),
("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "993310a1f8247923519f59099a7a5deaae6566972345a71b8f4c41ce1482d120" ) => Some((1,28)),
("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "b55c833abd41243a9e3bca06a953ae48f0e9cc3b82a7709ddf3ddb823c3eff07" ) => Some((1,29)),
("src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs", "8edeff9193bd5e4bf0e0026f532aeeead3fb152225bad78a618f21e43301d9b8" ) => Some((1,30)),
("src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs", "a372db755ee90873f7ddf58bdab6f091589c2cd63b6e9dc6991f6eb0d4499ddd" ) => Some((1,31)),
("src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs", "f6c3b1bf1b3d2e3bd3ca4e8278df6eb3238892acc12c74d358c25cc4b3cb12db" ) => Some((1,32)),
("src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs", "2d11e36011a6ff396fb6d1d2b4c7c31bf2298d696979bb2fa5a13ee2043818a5" ) => Some((1,33)),
("src/cleanup_plan/replay/factored.rs", "0214a3a244d8e26e50d7413a3570eede88b10b013784c0efc130ad2a8ef2ad72" ) => Some((1,34)),
("src/cleanup_plan/replay/factored.rs", "343eda3712d0c3b224742d72c34838a327e0c69449c8a94fd7eb81dc5e0134a6" ) => Some((1,35)),
("src/cleanup_plan/replay/factored.rs", "d3808da9519009c1a7b223a91a635bc94cdeb80a2c8bc40b22bcfc8b1f2f918c" ) => Some((1,36)),
("src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs", "c9c42329ff2e2ee3802de78f0da6484b42a7c58ae6636532024c4e955b743033" ) => Some((1,37)),
("src/cleanup_plan/replay/factored/oracle_hook.rs", "0126d25150ca4b96f11995293fc9043d23fc8fff2008485e9560abe5c85cb7ae" ) => Some((1,38)),
("src/cleanup_plan/replay/factored/oracle_hook.rs", "a0cf6a6604b1909859e78ecfe95f780ba1600072ae6ca9e259d2943c7dbebfa7" ) => Some((1,39)),
("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "4d534b5433078d8d65ec9f09c7f763614a177cd743aa02465de64c023e509173" ) => Some((1,40)),
("src/cleanup_plan/replay/skeleton_work.rs", "087f973c5c08945976d8e0ebb4fc7efc8a98e9d01da0229ee7e1305654424f3b" ) => Some((1,41)),
("src/cleanup_plan/replay/skeleton_work.rs", "78712ffd0063c945b7053258f62e608cefdbbd892b2348f99d54fee9c8ad577f" ) => Some((1,42)),
("src/cleanup_plan/replay/skeleton_work.rs", "916b817cb82b9e47eaacab364e94503b0e6ed0028d64c522ec4ccbb11a60c455" ) => Some((1,43)),
("src/hir.rs", "78d497e9da9a2e0866e42ef48a2898aa67b525e9f9112a4c2872385d48b4514e" ) => Some((1,44)),
("src/hir.rs", "e3aaf1c870be500fa794ec24d393f8dc94fcd996990abf388222ce079f4e0b04" ) => Some((1,45)),
("src/hir/expr_nodes.rs", "be3f28e10fc840d3c1397ccff1573a7940bd933931eee4b99a41bf3c63b15c90" ) => Some((1,46)),
("src/hir/expr_nodes.rs", "e2291267786bdec4371b969031209b4e77aa6876f4564048ad1c7561b004b73f" ) => Some((1,47)),
("src/hir/function_value.rs", "25b6ed470ac8b334e509e44bd2934caa87ab9b018d08816bdcef425aa2e21c3c" ) => Some((1,48)),
("src/hir/iterator_loop.rs", "6051dfc2627fcdba382fc5edec7c17dac32172d4a0f52f22458beb7bde14fb79" ) => Some((1,49)),
("src/hir/iterator_loop/renewal.rs", "18c1e353e54705458a511f8be6c23ed5484391d0e6db674666bcb93100a0ae2d" ) => Some((1,50)),
("src/hir/iterator_loop/renewal.rs", "83636c9a9e903ce7856ccb6e84a3957bd1df28193aa4ef9908500c42c4073149" ) => Some((1,51)),
("src/hir/lexical_exit.rs", "246cb0df50653502b3cf26ab9348e6db14508dad14c870852a22aab95db1e05d" ) => Some((1,52)),
("src/hir/lexical_exit.rs", "b13b08c925580c8cbcfba2b73e578fcff7573ff64c3f62f615cfba06d11bc7bf" ) => Some((1,53)),
("src/hir/lexical_exit/owner_funded.rs", "8f01359ece098fe361fad2ec7005ac55b6303ee5f94aa0aae3d06899e08a7164" ) => Some((1,54)),
("src/hir/validation.rs", "564b21a368d3199b4395675ee9eec7e384a485509c558ee8c366c57588377332" ) => Some((1,55)),
("src/hir/validation.rs", "85ae66ee35b346b0b65fb2c7490601216c49ac858aaae3345a03cbe08691447f" ) => Some((1,56)),
("src/hir/validation.rs", "f89e882a3d8ed796b8b8b864247e4e7eb19631d6299cd0dd94ddcc6b5ca17eba" ) => Some((2,0)),
("src/literal_format.rs", "73eb425701fe579bc9eae82d73c5dab9f364ab85db0e2cbb4de7f472afe5968e" ) => Some((2,1)),
("src/literal_format/funded.rs", "9f2f03f9fe83a556e94151c3a96b7d5bc72416d14912e025708566c0a320c8af" ) => Some((2,2)),
("src/literal_format/funded.rs", "c80bf82ed4cb60a58038c05229725b154b858a3afa56935000df8a5651c1b8a7" ) => Some((2,3)),
("src/loop_calls/byte_writer.rs", "3d99d7a80a99912b3aba134781cfedaea49e8db8774e136fb15044652797a544" ) => Some((2,4)),
("src/loop_calls/byte_writer.rs", "9d9227d8d51508bf478aa83d9c76ee2d16a40ea8bda2413b239da92bf87fa863" ) => Some((2,5)),
("src/loop_calls/byte_writer/resolved.rs", "03212b0a28826fbb9d8c1521962d057fcdd504500ebcc3a0a1301d8bda8f9106" ) => Some((2,6)),
("src/loop_calls/byte_writer/resolved.rs", "a357e24edfcba7588e983f59e997c3186f24841f0aab818aa8059fd650829d2e" ) => Some((2,7)),
("src/loop_calls/byte_writer/resolved.rs", "df9e8311daff58f93801e4b381b08941cc2692158b0dbb44bfe98836c68d0498" ) => Some((2,8)),
("src/loop_calls/byte_writer/source.rs", "6d78124797c1af7280919705207a8da235a7a1160d71f25379bf24cf99f2467c" ) => Some((2,9)),
("src/source_reflows/incoming67107_semantic_receivers.rs", "5b14ab93dba6fe594204817ac76eca64a4b7e1fd537177a3d3ac81f87e387c4a" ) => Some((2,10)),
("src/source_reflows/incoming67107_semantic_receivers.rs", "6c5d74b4423ae089bdb8864c3b0210bad4b160b8a6e003819a5553d7ab891e43" ) => Some((2,11)),
("src/source_reflows/incoming67107_semantic_receivers.rs", "de536dea276a4e1801a8df06f7c45cbed916b1b861e7c7859a2f040ee37759c8" ) => Some((2,12)),
("src/source_reflows/semantic_receivers.rs", "29582410736a63c8b42c4b29a26cf6b268a4f510d6bd5b2259d55eba9bcad181" ) => Some((2,13)),
("src/source_reflows/semantic_receivers.rs", "361eed66147b91cae6e0f4fe4f0f3446575f918424a23ed27374b51424199c2f" ) => Some((2,14)),
("src/source_reflows/semantic_receivers.rs", "88d2914bfe2a711b7cae92275ddf8e6c311dcdbc9b90024bd2f032fa357ab32a" ) => Some((2,15)),
("src/source_reflows/semantic_receivers.rs", "a620377ce8bcfe1ff7d44654ddc5ebf45cfecb2743e3ad4248220260a4dc58c7" ) => Some((2,16)),
("src/source_reflows/semantic_receivers.rs", "ba2ddbf8486039ba24fb0f36a90dea66910e931cbb71115b4d2eab114906430b" ) => Some((2,17)),
("src/source_verify/declaration.rs", "1ee6c27c7e20e6578261de0f58d0019762080e33f5631685081a8318485420d8" ) => Some((2,18)),
("src/source_verify/declaration.rs", "70619a1c9924119a6cd6a19e0512dc8c62f4126259e291bf1e466ac4c3f09be6" ) => Some((2,19)),
("src/source_verify/declaration.rs", "f6687bf16ba0ae287c79b96ab888de22e41d4ec7cecf010c81cdd6545018fc23" ) => Some((2,20)),
("src/string_ops/conditions.rs", "75eb3ac3e51c19cbab6332ae439dc33e0e7f5b5e496effae25fc6bfc4ec41560" ) => Some((2,21)),
("src/string_ops/conditions/funded.rs", "fa8cd466ca164dc0639e8670d2c61586943ee07adb5ea4aea2affcb4451e6f08" ) => Some((2,22)),
("src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs", "b54a1e1ef7192aed60c821f228e40955b5fd60b8755d4428e2f87f72eb6c41b2" ) => Some((2,23)),
("src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs", "bf59d79e6bcd513cf071c52112ded222e4561c785389f2723684d072c15d0fc2" ) => Some((2,24)),
("src/workspace_graph/identity_resolution.rs", "9668150574b0286f0eb965f3f96e47312135bd4484bf60a6b1fbe07387d2f93e" ) => Some((2,25)),
("tests/hir_module_boundaries.rs", "055a8f7ddd8b9f5b2d12122154de053a0a6823b75c298580cd4770628c4e8851" ) => Some((2,26)),
("tests/hir_module_boundaries.rs", "378d33aba7c844e66352f2eb7dc2c25b5e2f3d7101e7b331ba5bceb359a5d0fe" ) => Some((2,27)),
("tests/hir_module_boundaries.rs", "39fe4a52afaa8c025239eb27e793f314ffec7a4ed33a6526fb3f4dfb2bd924ac" ) => Some((3,0)),
("tests/hir_module_boundaries.rs", "46cc885fc3bcade09e1ad43aa6c382e9a3e96ea6e182abeb3ec39519082d45d3" ) => Some((3,1)),
("tests/hir_module_boundaries.rs", "d6e8ad51661a3f8727e9f654bd7e7f2687ffa6fa13fd9c66900217def541c1cc" ) => Some((3,2)),
("tests/workspace/semantic_graph/source_inventory.rs", "1cd423cdab9473ed591145366c339f9519892e06593d287c28b799f7e359c660" ) => Some((3,3)),
("tests/workspace/semantic_graph/source_inventory.rs", "29a6c3be08485c93251c2c58a1eef2be4de905b21f54865f85356d3a85bdf21b" ) => Some((4,0)),
("tests/workspace/semantic_graph/source_inventory.rs", "b0f9a67f3982c8f98d33e11a071663e41e37842201519a2b0d1061d7050aa967" ) => Some((4,1)),
("tests/workspace/semantic_graph/source_inventory.rs", "f2869b65eff9852441e9d53e39898c0587acda490ce27a0403e5597f26bb93ec" ) => Some((4,2)),
("src/cleanup/inventory_builder.rs", "a00caa48b822a64862c176eb74acf2be1387b59b5af2298577167099c75ec356" ) => Some((5,0)),
("src/cleanup_plan/build.rs", "d2b0c09384cf4aa8c03112e5f96779bd36b938ba51f5ccde0db212df04a694e7" ) => Some((5,1)),
("src/cleanup_plan/replay/expression_facts.rs", "82c15975078a1e841c342440723a2e75ec2a0bfee01366ea8f43c49af0b6dec1" ) => Some((5,2)),
("src/cleanup_plan/replay/physical_clone.rs", "f730ed63dbd193546319a8aa9bdadf15c7cd44c984bbafd96bb9756628695ac3" ) => Some((5,3)),
("src/cleanup_plan/replay/scalar_replay/records.rs", "d0a7ecff13b07ac0c5197fb5d590de2a734c45c4ebd02ab66eb8bcb1479ad09b" ) => Some((5,4)),
("src/cleanup_plan/replay/selected_entry/tests/paid.rs", "8b29defc8cd06d9ed215ad1c615821b24d1e01b843dc640ed45a152b64940dc6" ) => Some((5,5)),
("src/cleanup_plan/replay/selected_owner/paid_before_children.rs", "c1d2ee47fd0ce510eec0d84f648b877ea539fdea5b4420aa41c06b4bf0a95fd1" ) => Some((5,6)),
("src/format.rs", "a0afc1d0c8a137742ae524f57e1e2fc585c2cbff808c932e1bd1e0081b358a6e" ) => Some((5,7)),
("src/loan_plan/funded_flat/tests_before.rs", "3df0efd03e2a08467722689870682ed3fe33de13289d5ceee1a407187ebc5366" ) => Some((5,8)),
("src/workspace_graph/constructor_failure/typed.rs", "b07ecf1da889cf25e1b9296bf320070d49c58af316efe6036dce534fccd73fb7" ) => Some((5,9)),
("src/cleanup_plan/replay/selected_context/child/tests/source.rs", "37ab096e743107558d25938fccf62cc5e3636bd10ad7ac751d6884d7378b989a" ) => Some((5,10)),
("src/cleanup.rs", "7481c128ef581ef9f19fea61cf26faec4ded5461ea01c63da0c4b346f218719e" ) => Some((6,0)),
("src/cleanup/inventory_builder.rs", "488fe089e7ecaed678bb421ed0d8793302a930bc6c1c6674df37c54ed742f6c5" ) => Some((6,1)),
("src/cleanup_plan/renewal_profile.rs", "46f3c87a7d1bd762ea86895c8dada6d980331f1e6cf13e63820a49c254219008" ) => Some((6,2)),
("src/cleanup_plan/renewal_profile/child_refusal.rs", "fbd2da5b2009357bf8a1ff35f0bb4792e89c5dd41efde6afff740f41f0db4f42" ) => Some((6,3)),
("src/cleanup_plan/renewal_profile/selected.rs", "9e786d9ee1ea0710c8c56b8d22839f4d83731bf33e10dc6c38f17a008ef5f870" ) => Some((6,4)),
("src/cleanup_plan/renewal_profile/selected_inline.rs", "3234fe03a54657e882829f0bb28f8ff64c39a60b5d169ef4a1eb95e734ce7744" ) => Some((6,5)),
("src/cleanup_plan/renewal_profile/selected_inline/append.rs", "b072c182cb4b4bc5078143c57bf22107792497f2d2f3e67613a26c3f8b4739b1" ) => Some((6,6)),
("src/cleanup_plan/replay/selected_context.rs", "f391b7b290a737dd283020ac713e7045b138968832d03dde27e913351ab7940a" ) => Some((6,7)),
("src/cleanup_plan/replay/selected_context/child.rs", "1710f802f9b90889de12bbfc6bd5ad713e02d67a605f3c9ada8413580973fe7e" ) => Some((6,8)),
("src/cleanup_plan/replay/selected_context/child/tests.rs", "c0fff9d41c28830186a0fd30844355cf1fc1da9528fb165131ff3a0e5fc3c860" ) => Some((6,9)),
("src/cleanup_plan/replay/selected_context/child/tests/source.rs", "9dd840b0fabba16d5964a3880ebea1c2283412cb83db8455acc31b8d8cee7145" ) => Some((6,10)),
("src/cleanup_plan/replay/selected_context/paid.rs", "c1cc9bc652c03d6050b35a9b33083ed230046f36e4573ce341200b401e7c2173" ) => Some((6,11)),
("src/cleanup_plan/replay/selected_context/paid_before_children.rs", "b952c81a627eb3e5b1617922fd39ef8e54bce78e0df000fe57604be054bf2d3f" ) => Some((6,12)),
("src/cleanup_plan/replay/selected_owner/paid.rs", "e984f475bdce585143155529a2a22f07f4ad3b926cae328623b49c94c0ec8b53" ) => Some((6,13)),
("src/cleanup_plan/replay/selected_profile.rs", "01c374590bc4a77cd30e04a3c9e0a9e0e02467257ee4a51608a017a0c866f2ff" ) => Some((6,14)),
("src/cleanup_plan/replay/selected_profile/paid.rs", "221fd08f97e69e82edb47318c57adb57b1b43e66b661a4259aad94f6eda99f9e" ) => Some((6,15)),
("src/cleanup_plan/replay/selected_profile/paid_before_children.rs", "625b05295bc3bc1ab00b0d2ce86eef7f13a29b36f851ad090bb8d4b586421570" ) => Some((6,16)),
("src/cleanup_plan/replay/selected_profile/paid_children.rs", "9b959ccb9a045733478f24d14927c41f79af6a50a3d2c1c6382ef4753e223216" ) => Some((6,17)),
("src/cleanup_plan/replay/selected_profile/paid_children/tests.rs", "eef330e8e938e3b890ff0cad984903cd58fb030d4e312b445c286df4d300b5ca" ) => Some((6,18)),
("src/cleanup_plan/replay/semantic_refusal.rs", "fc1a6f41a225e0936ede78ff9923eb0eea1e93ce5c8e37fbde9709e6ee78c823" ) => Some((6,19)),
("src/hir/lexical_exit.rs", "b99458953d0fdbe9d93c5ea35881cf8628bd402d759fb6f705007ca6e861c89b" ) => Some((6,20)),
("src/hir/lexical_exit/latch_inline.rs", "c79f9ea1a8653de79d8118645e0054e9c936783b3972f8fdc81b8749cc2df7fa" ) => Some((6,21)),
("src/hir/lexical_exit/selected_child.rs", "c082d210432372df2e1124fe0147580963621284d32c0f0b5f9531ad52fd78e9" ) => Some((6,22)),
("src/hir/static_text.rs", "b8e938ecb551c85962ca1c94bb9a94a46bea08911143d8059df5695ffb143fd0" ) => Some((6,23)),
("src/loop_calls/byte_writer.rs", "104f71d5a46ea1188b7182f88505e5de68a441ca1c6e8b8025c9cf1969dc806f" ) => Some((6,24)),
("src/loop_calls/byte_writer/resolved.rs", "0ab9fa73794e264a5701323deb73baa2763fadc5a2574c2f55c3812e6db26908" ) => Some((6,25)),
("src/loop_calls/byte_writer/resolved/selected_inline.rs", "92e0068c436fc7157eab5df8bf54af86cd77cc5f651f77efebf07ef370ff9a1a" ) => Some((6,26)),
("tests/workspace/semantic_graph/source_inventory.rs", "4a1920f6abdff90036fe9afabe687c091b08a38d4f26a28e2e7c7feb678cbd14" ) => Some((6,27)),
_ => None, }
}
pub(crate) fn compiled(source: &str) -> &'static str {
    match source {
"crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
"src/byte_ops.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/byte_ops.rs")),
"src/cleanup.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup.rs")),
"src/cleanup/inventory_builder.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup/inventory_builder.rs")),
"src/cleanup_plan/build.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/build.rs")),
"src/cleanup_plan/renewal_profile.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile.rs")),
"src/cleanup_plan/renewal_profile/child_refusal.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile/child_refusal.rs")),
"src/cleanup_plan/renewal_profile/funded.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile/funded.rs")),
"src/cleanup_plan/renewal_profile/selected.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile/selected.rs")),
"src/cleanup_plan/renewal_profile/selected_inline.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile/selected_inline.rs")),
"src/cleanup_plan/renewal_profile/selected_inline/append.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/renewal_profile/selected_inline/append.rs")),
"src/cleanup_plan/replay.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs")),
"src/cleanup_plan/replay/branch_clone.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/branch_clone.rs")),
"src/cleanup_plan/replay/branch_clone/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/branch_clone/tests.rs")),
"src/cleanup_plan/replay/continuation_storage.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/continuation_storage.rs")),
"src/cleanup_plan/replay/continuation_storage/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/continuation_storage/tests.rs")),
"src/cleanup_plan/replay/expression_facts.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts.rs")),
"src/cleanup_plan/replay/expression_facts/arena.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena.rs")),
"src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs")),
"src/cleanup_plan/replay/expression_facts/arena/prefix.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena/prefix.rs")),
"src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs")),
"src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs")),
"src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs")),
"src/cleanup_plan/replay/expression_facts/receiving/helpers.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/helpers.rs")),
"src/cleanup_plan/replay/expression_facts/receiving/helpers/matching.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/helpers/matching.rs")),
"src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/tests.rs")),
"src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs")),
"src/cleanup_plan/replay/expression_facts/receiving/remaining.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/remaining.rs")),
"src/cleanup_plan/replay/expression_facts/receiving/retained.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")),
"src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs")),
"src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs")),
"src/cleanup_plan/replay/factored.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored.rs")),
"src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs")),
"src/cleanup_plan/replay/factored/oracle_hook.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/oracle_hook.rs")),
"src/cleanup_plan/replay/live_oracle/source_custody/b.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")),
"src/cleanup_plan/replay/physical_clone.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/physical_clone.rs")),
"src/cleanup_plan/replay/scalar_replay/records.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/scalar_replay/records.rs")),
"src/cleanup_plan/replay/selected_context.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context.rs")),
"src/cleanup_plan/replay/selected_context/child.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/child.rs")),
"src/cleanup_plan/replay/selected_context/child/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/child/tests.rs")),
"src/cleanup_plan/replay/selected_context/child/tests/source.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/child/tests/source.rs")),
"src/cleanup_plan/replay/selected_context/paid.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/paid.rs")),
"src/cleanup_plan/replay/selected_context/paid_before_children.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/paid_before_children.rs")),
"src/cleanup_plan/replay/selected_entry/tests/paid.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests/paid.rs")),
"src/cleanup_plan/replay/selected_owner/paid.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_owner/paid.rs")),
"src/cleanup_plan/replay/selected_owner/paid_before_children.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_owner/paid_before_children.rs")),
"src/cleanup_plan/replay/selected_profile.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile.rs")),
"src/cleanup_plan/replay/selected_profile/paid.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile/paid.rs")),
"src/cleanup_plan/replay/selected_profile/paid_before_children.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile/paid_before_children.rs")),
"src/cleanup_plan/replay/selected_profile/paid_children.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile/paid_children.rs")),
"src/cleanup_plan/replay/selected_profile/paid_children/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile/paid_children/tests.rs")),
"src/cleanup_plan/replay/semantic_refusal.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/semantic_refusal.rs")),
"src/cleanup_plan/replay/skeleton_work.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work.rs")),
"src/format.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/format.rs")),
"src/hir.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir.rs")),
"src/hir/expr_nodes.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/expr_nodes.rs")),
"src/hir/function_value.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/function_value.rs")),
"src/hir/iterator_loop.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/iterator_loop.rs")),
"src/hir/iterator_loop/renewal.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/iterator_loop/renewal.rs")),
"src/hir/lexical_exit.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/lexical_exit.rs")),
"src/hir/lexical_exit/latch_inline.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/lexical_exit/latch_inline.rs")),
"src/hir/lexical_exit/owner_funded.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/lexical_exit/owner_funded.rs")),
"src/hir/lexical_exit/selected_child.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/lexical_exit/selected_child.rs")),
"src/hir/static_text.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/static_text.rs")),
"src/hir/validation.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation.rs")),
"src/literal_format.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/literal_format.rs")),
"src/literal_format/funded.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/literal_format/funded.rs")),
"src/loan_plan/funded_flat/tests_before.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/funded_flat/tests_before.rs")),
"src/loop_calls/byte_writer.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/loop_calls/byte_writer.rs")),
"src/loop_calls/byte_writer/resolved.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/loop_calls/byte_writer/resolved.rs")),
"src/loop_calls/byte_writer/resolved/selected_inline.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/loop_calls/byte_writer/resolved/selected_inline.rs")),
"src/loop_calls/byte_writer/source.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/loop_calls/byte_writer/source.rs")),
"src/source_reflows/incoming67107_semantic_receivers.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/incoming67107_semantic_receivers.rs")),
"src/source_reflows/semantic_receivers.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows/semantic_receivers.rs")),
"src/source_verify/declaration.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/declaration.rs")),
"src/string_ops/conditions.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/string_ops/conditions.rs")),
"src/string_ops/conditions/funded.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/string_ops/conditions/funded.rs")),
"src/workspace_graph/constructor_failure/typed.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/constructor_failure/typed.rs")),
"src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs")),
"src/workspace_graph/identity_resolution.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/identity_resolution.rs")),
"tests/hir_module_boundaries.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")),
"tests/workspace/semantic_graph/source_inventory.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")),
_ => panic!("unknown declared current source"), }
}
pub(crate) fn restore(source: &str, row: &serde_json::Value) -> String {
    assert_eq!(row["source"], source);
    let current = compiled(source);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path: &str, version: &str| {
        reader::read(
            &root.join(path),
            row[format!("{version}_bytes")].as_u64().unwrap() as usize,
            row[format!("{version}_sha256")].as_str().unwrap(),
            CAP,
        )
        .unwrap()
    };
    assert_eq!(current.len() as u64, row["current_bytes"]);
    assert_eq!(digest(current), row["current_sha256"]);
    assert_eq!(
        read(source, "current"),
        current,
        "whole fresh/compiled physical owner"
    );
    assert_eq!(
        read(row["current_archive"].as_str().unwrap(), "current"),
        current
    );
    let mut before = current.to_owned();
    for edit in row["inverse"].as_array().unwrap().iter().rev() {
        let at = edit[0].as_u64().unwrap() as usize;
        let after = edit[1].as_str().unwrap();
        assert_eq!(&before[at..at + after.len()], after);
        before.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
    }
    assert_eq!(before.len() as u64, row["before_bytes"]);
    assert_eq!(digest(&before), row["before_sha256"]);
    assert_eq!(
        read(row["before_archive"].as_str().unwrap(), "before"),
        before
    );
    before
}
pub(crate) fn expected_value(source: &str, expected: &str, supplied: &str) -> Option<String> {
    let (part, index) = slot(source, expected)?;
    if supplied != compiled(source) {
        return None;
    }
    Some(restore(source, &records(part)[index]))
}
pub(crate) fn expected_file(path: &Path, length: usize, expected: &str) -> Option<String> {
    let relative = path.strip_prefix(env!("CARGO_MANIFEST_DIR")).ok()?;
    if !relative
        .components()
        .all(|p| matches!(p, Component::Normal(_)))
    {
        return None;
    }
    let source = relative
        .iter()
        .map(|part| part.to_str())
        .collect::<Option<Vec<_>>>()?
        .join("/");
    let (part, index) = slot(&source, expected)?;
    let row = &records(part)[index];
    if row["before_bytes"].as_u64() != Some(length as u64) {
        return None;
    }
    Some(restore(&source, row))
}
#[test]
fn every_declared_tuple_keeps_complete_current_and_old_inputs_and_refuses_foreign_requests() {
    reader::controls();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut names = std::collections::BTreeSet::new();
    for part in 0..PARTS.len() {
        for row in records(part).as_array().unwrap() {
            let source = row["source"].as_str().unwrap();
            let expected = row["before_sha256"].as_str().unwrap();
            assert!(names.insert((source, expected)));
            let length = row["before_bytes"].as_u64().unwrap() as usize;
            let current = compiled(source);
            let before = restore(source, row);
            assert_eq!(
                expected_value(source, expected, current).as_deref(),
                Some(before.as_str())
            );
            assert_eq!(
                expected_file(&root.join(source), length, expected).as_deref(),
                Some(before.as_str())
            );
            assert!(expected_file(&root.join(source), length + 1, expected).is_none());
            assert!(expected_value(source, &"0".repeat(64), current).is_none());
            assert!(expected_value("foreign", expected, current).is_none());
            for wrong in [
                String::new(),
                current[..current.len() - 1].to_owned(),
                format!("{current}{current}"),
                format!("!{}", &current[1..]),
            ] {
                assert!(expected_value(source, expected, &wrong).is_none());
            }
        }
    }
}
