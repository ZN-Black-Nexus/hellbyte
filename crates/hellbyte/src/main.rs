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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    #[cfg(target_os = "linux")]
    {
        linux::fb::restore_console();
        linux::write_all(2, b"hellbyte: internal error (panic)\n");
        linux::exit(101);
    }
    #[cfg(target_os = "windows")]
    unsafe {
        windows::sys::ExitProcess(101)
    }
    #[cfg(target_os = "macos")]
    {
        macos::emergency_restore();
        macos::write_all(2, b"hellbyte: internal error (panic)\n");
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    loop {}
}
