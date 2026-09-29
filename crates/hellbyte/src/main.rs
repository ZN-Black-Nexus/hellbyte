//! Hellbyte platform entry points. Each OS module turns raw input/output into
//! calls on the engine; nothing here links a C library on Linux.

#![no_std]
#![no_main]
#![cfg_attr(
    any(
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "sparc",
        target_arch = "sparc64",
        target_arch = "m68k",
        target_arch = "csky",
        target_arch = "hexagon"
    ),
    feature(asm_experimental_arch)
)]

// Pulls in memcpy/memset/... (no libc on any target).
extern crate hellbyte_rt;

mod common;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
fn main_linux(env: linux::Env) -> i32 {
    linux::app::run(env)
}

#[cfg(libc_start)]
#[unsafe(no_mangle)]
pub extern "C" fn main(argc: i32, argv: *const *const u8, envp: *const *const u8) -> i32 {
    main_linux(linux::Env { argc: argc as usize, argv, envp })
}

#[cfg(libc_start)]
#[link(name = "c")]
unsafe extern "C" {}

// The prebuilt core library (used by native `cargo build` dev builds) is
// compiled for unwinding and references these; Hellbyte aborts on panic, so
// they are never called. Release builds optimise the references away.
#[cfg(not(target_os = "windows"))]
#[unsafe(no_mangle)]
pub extern "C" fn rust_eh_personality() {}

#[cfg(not(target_os = "windows"))]
#[unsafe(no_mangle)]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}

/// A crash: put the screen back, say what went wrong and where, exit with 101.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;
    let mut msg: common::Buf<512> = common::Buf::new();
    let _ = write!(msg, "hellbyte crashed: {}", info.message());
    if let Some(l) = info.location() {
        let _ = write!(msg, " (at {}:{})", l.file(), l.line());
    }
    let _ = msg.write_str("\n");
    #[cfg(target_os = "linux")]
    {
        linux::fb::restore_console();
        linux::write_all(2, msg.as_bytes());
        linux::exit(101);
    }
    #[cfg(target_os = "windows")]
    windows::crash(msg.as_bytes());
    #[cfg(target_os = "macos")]
    {
        macos::emergency_restore();
        macos::write_all(2, msg.as_bytes());
        macos::quit(101);
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    loop {}
}
