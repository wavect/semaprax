//! Fixed incoming a3 source metadata. No arbitrary path or compiler authority.
#[derive(Clone, Copy)]
pub(crate) enum Fixture { Compact7, Receivers12, LegacyNominal18, PrefixCertified3, SelectedSession19 }
pub(super) struct Spec { pub(super) path: &'static str, pub(super) length: usize, pub(super) sha256: [u8; 32] }
impl Fixture { pub(super) fn spec(self) -> Spec { match self {
Self::Compact7 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/maina3-independent-custody-20261016/compact-inverse7.json"), length: 60728, sha256: [67, 62, 228, 145, 74, 239, 210, 245, 150, 169, 59, 40, 88, 88, 218, 70, 208, 67, 235, 241, 150, 111, 65, 48, 68, 9, 228, 36, 60, 133, 79, 61] },
Self::LegacyNominal18 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/maina3-independent-custody-20261016/legacy-nominal-inverse18.json"), length: 897602, sha256: [22, 222, 216, 85, 111, 148, 96, 102, 26, 64, 86, 177, 216, 184, 163, 161, 19, 202, 92, 73, 91, 158, 254, 237, 149, 246, 164, 4, 208, 74, 0, 89] },
Self::PrefixCertified3 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/maina3-independent-custody-20261016/prefix-certified-inverse3.json"), length: 1056800, sha256: [56, 41, 42, 41, 152, 147, 210, 2, 100, 102, 68, 23, 78, 152, 93, 106, 99, 232, 126, 150, 213, 235, 147, 161, 168, 239, 93, 235, 92, 217, 226, 61] },
Self::Receivers12 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/maina3-independent-custody-20261016/receiver-inverse12.json"), length: 87333, sha256: [217, 134, 104, 74, 157, 27, 149, 227, 26, 59, 41, 248, 36, 192, 243, 37, 91, 74, 203, 44, 48, 101, 100, 214, 161, 38, 11, 217, 67, 126, 234, 155] },
Self::SelectedSession19 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/maina3-independent-custody-20261016/selected-session-inverse19.json"), length: 2040968, sha256: [123, 18, 246, 176, 128, 106, 138, 141, 170, 112, 137, 252, 56, 95, 18, 45, 236, 124, 123, 234, 173, 231, 151, 225, 67, 202, 25, 248, 7, 72, 134, 255] },
}}}
// Final Input/normalizer definitions are bound once all batch source is quiet.
