//! Literal whole-wire pins for the two CI-native builder layouts.
pub(crate) fn unit_wire_pin(previous: &str) -> &'static str {
    let (macos, portable) = match previous {
        "sha256:1a8aa6c12947c96d1adb1ff01bf0df47b44ae3da6a7a7c00de294008e1ff3f3d" => (
            "sha256:0667599bf492ae9859b1e916be0ca6b921f33501814a9dc07ca3909ceb86ed6d",
            "sha256:55c93db0af71880fb640b7684aa21d43dc3f5d9a2dbf393b3391e9d6467536ba",
        ),
        "sha256:19b5d32b46a89d5db591037d56d4e56051827eefda2a4efca086e2cc9e353822" => (
            "sha256:7aab5e8bf468c79c4f5c5ba98b44a93903b2216d23664f99f621258b5ed111bc",
            "sha256:0582592e9570c9e8198d5454516677e76429df20b4eb288130d9fc67070716b2",
        ),
        "sha256:fb442feaa785f66b280e98e019e6b84ae0626d76643ed2d7fe3f4675f36a703b" => (
            "sha256:44422011d00c82de9fa8f406d62d9de2bfdff4266faf5de6853d94c945763150",
            "sha256:36ee151090b93a7e4e668120bd7d7c2126caa0c2ff10135e6c5011e65bc2f082",
        ),
        "sha256:8b916f85723b506089076716d00ba18c5dead37fc990bec42a7a82bad7935d7b" => (
            "sha256:1ae7e640893213ee381bab90712952c2540ca00da5c8832038060e3bbb746734",
            "sha256:ed8c85d677b9252832b6acb5ab9d54377cf4bdbd4c94f75a1a6c29104b60e074",
        ),
        "sha256:6168a7d5ca2d0d76828f9a94f5b36e283d3a2eb4a855c8b32210ae80fa887a03" => (
            "sha256:afd5f20ed9e0712b9c8c05821a62afb4cd7d816771ba3502a56a4be50ce6c4f3",
            "sha256:c17632b3b6c8def49caef0150dfd2621455abc2be9baf8e84acb9f5b3dc87f14",
        ),
        "sha256:6b4aff6f4a51fe0e8dd384e48ed8047e7fe730a48611e94caf70553e38f0c768" => (
            "sha256:72eafff3bfb7ac949f10a98a69271cb08d0d1405666c4561eed0326b6f85b75a",
            "sha256:0db2a5420fe0e0ebaa4db06a16142504df48bcf0c9990f7fda9d8b55ef433dd7",
        ),
        "sha256:c3a7834f9a9c41b7000cd4707ede5aa438bc88e30d88f807e5e63a2cfdfa37f9" => (
            "sha256:ac52431535b9e798f8cd4ba3ebb5321de879b954ba17083659a4688704b42b7c",
            "sha256:b6ef2b450daa6c9a09e730e28a3079976f00fa9b37b2a6abe26e17fa91f81138",
        ),
        "sha256:8c16f86e0a668ef523f2a4d4d3cc7a74d803ab8102726baf117e4d7a6e84f7f1" => (
            "sha256:86838335db234630c96956470914827d9c5288506bf26c5542b66a1dcbc2e448",
            "sha256:d675be31765bba093f04c3b0a89216a4ad9e1472bfffa63574b003d2e94f78bb",
        ),
        "sha256:9faff8393c8f2911cc59c09109a031d6cefbb33ec1b4d0beeaa945a2f731f034" => (
            "sha256:c88b7b4dc4aa997c075bbca02afe27e8c2be7c0063188654957639609e68e78f",
            "sha256:6d2fa7c230113135205b36e9fa1bf873e0020ff061b22795d21dad70060d65c7",
        ),
        "sha256:7bcd3cbd739609fee3784a07643e3d94c06c068c4e35c6881ec1bc99c796b52f" => (
            "sha256:533dff2b0494adf066bc4b4db55bd5356544d35e3ebdd8e3276a443f51dcefd4",
            "sha256:2b21276016d0af2f15503b82d42a8ec9e462590597fed808d2273f7131ccc049",
        ),
        _ => panic!("unknown whole-wire fixture pin"),
    };
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        macos
    } else {
        portable
    }
}
