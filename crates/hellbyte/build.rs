//! Chooses how the program starts on Linux:
//! * `raw_start` - our own `_start`, no libc, static (used whenever rust-lld is the linker:
//!   all release/cross builds).
//! * `libc_start` - a plain `main` started by the system C runtime, so a bare
//!   `cargo run` on a Linux dev box works with the default toolchain.
use std::env;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(raw_start)");
    println!("cargo:rustc-check-cfg=cfg(libc_start)");
    println!("cargo:rerun-if-env-changed=RUSTC_LINKER");
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let linker = env::var("RUSTC_LINKER").unwrap_or_default();
    if os == "macos" {
        // Link against our own libSystem stub (see macos/libSystem.tbd): no SDK needed.
        let dir = std::path::Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("macos");
        println!("cargo:rustc-link-search=native={}", dir.display());
        println!("cargo:rerun-if-changed=macos/libSystem.tbd");
    }
    if os == "linux" {
        // Any configured linker (rust-lld, or a GNU `*-ld` from scripts/linker) means a
        // static, libc-free build; the default `cc` means a native dev build.
        let name = linker.rsplit('/').next().unwrap_or("");
        if name.contains("ld") {
            println!("cargo:rustc-cfg=raw_start");
        } else {
            println!("cargo:rustc-cfg=libc_start");
        }
    }
}
