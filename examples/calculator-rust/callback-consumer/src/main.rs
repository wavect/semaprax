use core::num::NonZeroU32;
use semaprax_generated_native_rust_sdk::{
    NativeRustSdk, NativeRustSdkCallError, NativeRustSdkImportResult, NativeRustSdkImports,
    NativeRustSdkStatusClass,
};

struct Host {
    mode: u8,
}

impl NativeRustSdkImports for Host {
    fn spx_calculator_dot_callback_dot_adjust(
        &mut self,
        value: i64,
    ) -> NativeRustSdkImportResult<i64> {
        match self.mode {
            0 => NativeRustSdkImportResult::Success(value + 1),
            1 => NativeRustSdkImportResult::Status {
                code: NonZeroU32::new(7).unwrap(),
                class: NativeRustSdkStatusClass::Import,
                retryable: true,
            },
            2 => NativeRustSdkImportResult::HostFailure,
            3 => panic!("caught callback panic"),
            _ => NativeRustSdkImportResult::Status {
                code: NonZeroU32::new(7).unwrap(),
                class: NativeRustSdkStatusClass::Semantic,
                retryable: false,
            },
        }
    }
}

fn main() {
    assert!(NativeRustSdk::new(Host { mode: 0 }, &["unexpected.capability"]).is_err());
    let mut calculator =
        NativeRustSdk::new(Host { mode: 0 }, &[]).expect("admit generated callback SDK");
    assert_eq!(
        calculator.spx_calculator_dot_callback_dot_apply(19, 22),
        Ok(42)
    );
    let mut status = NativeRustSdk::new(Host { mode: 1 }, &[]).unwrap();
    assert_eq!(
        status.spx_calculator_dot_callback_dot_apply(19, 22),
        Err(NativeRustSdkCallError::Semantic {
            domain_id: "calculator.callback.v1",
            code: NonZeroU32::new(7).unwrap(),
            class: NativeRustSdkStatusClass::Import,
            retryable: true,
        })
    );
    let mut failed = NativeRustSdk::new(Host { mode: 2 }, &[]).unwrap();
    assert_eq!(
        failed.spx_calculator_dot_callback_dot_apply(19, 22),
        Err(NativeRustSdkCallError::HostFailed)
    );
    std::panic::set_hook(Box::new(|_| {}));
    let mut panicked = NativeRustSdk::new(Host { mode: 3 }, &[]).unwrap();
    assert_eq!(
        panicked.spx_calculator_dot_callback_dot_apply(19, 22),
        Err(NativeRustSdkCallError::HostPanicked)
    );
    let mut wrong_class = NativeRustSdk::new(Host { mode: 4 }, &[]).unwrap();
    assert_eq!(
        wrong_class.spx_calculator_dot_callback_dot_apply(19, 22),
        Err(NativeRustSdkCallError::AdapterRejected)
    );
    println!("42");
}
