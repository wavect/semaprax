//! Stable local source used to check rustdoc extraction and stable signatures.

mod implementation {
    pub struct ReExported(pub u8);

    impl ReExported {
        pub fn contains(&self, needle: &str) -> bool {
            self.0.to_string().contains(needle)
        }
    }
}

pub use implementation::ReExported;

macro_rules! publish_type {
    ($name:ident) => {
        pub struct $name;

        impl $name {
            pub fn answer(&self) -> u8 {
                42
            }
        }
    };
}

publish_type!(MacroGenerated);

#[cfg(feature = "fixture-selected")]
pub fn cfg_selected(value: u8) -> u8 {
    value
}

#[cfg(not(feature = "fixture-selected"))]
pub fn cfg_unselected(value: u8) -> u8 {
    value
}

pub trait Measures {
    type Output;

    fn measure(&self) -> Self::Output;
}

impl Measures for ReExported {
    type Output = u8;

    fn measure(&self) -> Self::Output {
        self.0
    }
}

mod private_seal {
    pub trait Seal {}
}

pub trait SealedApi: private_seal::Seal {
    type HiddenOutput;
}

pub struct SealedType;

impl private_seal::Seal for SealedType {}

impl SealedApi for SealedType {
    type HiddenOutput = u8;
}

pub fn opaque_output() -> impl Iterator<Item = u8> {
    [1, 2, 3].into_iter()
}

pub fn generic_output<T: Clone>(value: T) -> T {
    value
}
