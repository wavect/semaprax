//! Unsafe Windows filesystem authority quarantine for Project Revision Store v1.
//!
//! The safe surface exposes opaque held handles and authenticated facts only.
//! It deliberately has no path-returning, deletion, replacement, recovery, or
//! ambient-root API.

#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::*;
