//! `UnRAR`'s C++ code calls the registry, process-token and crypto APIs of Windows, which live in `advapi32`; the
//! `unrar_sys` crate does not link it, so every binary using RAR support failed to link on Windows.
fn main() {
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    if windows && std::env::var_os("CARGO_FEATURE_RAR").is_some() {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
