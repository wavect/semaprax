//! Entry point for the private packaged native-desktop callable-v3 fixture.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: The platform packaging gate creates a fresh directory containing
    // only this executable and its exact compiler-generated provider claim.
    unsafe { semaprax_native_host::private_desktop_v3_app_main() }
}
