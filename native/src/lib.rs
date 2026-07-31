//! The `para/html` package's native entry crate.
//!
//! The composed dev toolchain aggregates each dev-native dependency's `NOETA_EXTENSIONS` slice and
//! installs the union into the `noeta fmt` formatter registry. This crate is the thin entry point
//! declared by the package manifest's `dev-native = "native"` key; the formatters themselves (the
//! `"html"` and `"css"` body formatters) live in the `noeta-para-html` library crate.
//!
//! Trust-free: a `dev-native` entry contributes only tier-body formatters to the dev toolchain, so a
//! consumer authorizes nothing (`[trust] native` is not required, and there is no prod code here).
pub use noeta_para_html::NOETA_EXTENSIONS;
