# Rust calculator consumer

This standalone example has two phases. The setup package calls the public
SEMAPRAX Native Rust SDK builder for `../calculator.spx`. The consumer package
then depends only on the generated `semaprax-generated-native-rust-sdk` package at
`../generated-sdk`; it has no source or workspace dependency on the SEMAPRAX
compiler.

The hosted public-SDK gate copies this directory to a fresh temporary
directory, builds `generated-sdk` there, and runs the consumer with Cargo in
locked offline mode. The repository intentionally does not contain generated
SDK artifacts.

The same gate builds `callback-sdk` from `callback.spx` and runs the separate
callback consumer. Its host implements only the stable-ID-derived callback
method and returns the generated closed import-result type.
