# Actual scoped Clippy correction

The existing unsigned usize sum used checked_add followed by unwrap_or(usize::MAX). saturating_add returns precisely the same exact sum when representable and usize::MAX on overflow, on every target width. This mechanical spelling change resolves the actual manual_saturating_arithmetic lint. All requested charges, mutation order, refusal selection and limits remain unchanged; no fee is credited for the eliminated temporary Option. Full immediate-before source/inverse retained. No Rust/Clippy execution; fresh scoped recheck required.
