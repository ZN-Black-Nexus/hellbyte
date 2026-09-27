//! Linux system call numbers and ABI constants, per architecture family.
//!
//! Values cross-checked against the kernel UAPI headers (as mirrored by the
//! `linux-raw-sys` crate). Only what the game needs is listed.

#![allow(dead_code)]

// ------------------------------------------------------------------ syscall numbers

#[cfg(target_arch = "x86_64")]
mod sys {
    pub const READ: usize = 0;
    pub const WRITE: usize = 1;
    pub const OPENAT: usize = 257;
    pub const CLOSE: usize = 3;
    pub const IOCTL: usize = 16;
    pub const MMAP: usize = 9;
    pub const EXIT_GROUP: usize = 231;
    pub const CLOCK_GETTIME: usize = 228;
    pub const CLOCK_GETTIME64: usize = 0;
    pub const NANOSLEEP: usize = 35;
    pub const CLOCK_NANOSLEEP64: usize = 0;
    pub const SOCKET: usize = 41;
    pub const CONNECT: usize = 42;
    pub const MKDIRAT: usize = 258;
}

#[cfg(target_arch = "x86")]
mod sys {
    pub const READ: usize = 3;
    pub const WRITE: usize = 4;
    pub const OPENAT: usize = 295;
    pub const CLOSE: usize = 6;
    pub const IOCTL: usize = 54;
    pub const MMAP: usize = 192; // mmap2
    pub const EXIT_GROUP: usize = 252;
    pub const CLOCK_GETTIME: usize = 265;
    pub const CLOCK_GETTIME64: usize = 403;
    pub const NANOSLEEP: usize = 162;
    pub const CLOCK_NANOSLEEP64: usize = 407;
    pub const SOCKET: usize = 359;
    pub const CONNECT: usize = 362;
    pub const MKDIRAT: usize = 296;
}

#[cfg(target_arch = "arm")]
mod sys {
    pub const READ: usize = 3;
    pub const WRITE: usize = 4;
    pub const OPENAT: usize = 322;
    pub const CLOSE: usize = 6;
    pub const IOCTL: usize = 54;
    pub const MMAP: usize = 192; // mmap2
    pub const EXIT_GROUP: usize = 248;
    pub const CLOCK_GETTIME: usize = 263;
    pub const CLOCK_GETTIME64: usize = 403;
    pub const NANOSLEEP: usize = 162;
    pub const CLOCK_NANOSLEEP64: usize = 407;
    pub const SOCKET: usize = 281;
    pub const CONNECT: usize = 283;
    pub const MKDIRAT: usize = 323;
}

// asm-generic table, 64-bit flavour.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64", target_arch = "loongarch64"))]
mod sys {
    pub const READ: usize = 63;
    pub const WRITE: usize = 64;
    pub const OPENAT: usize = 56;
    pub const CLOSE: usize = 57;
    pub const IOCTL: usize = 29;
    pub const MMAP: usize = 222;
    pub const EXIT_GROUP: usize = 94;
    pub const CLOCK_GETTIME: usize = 113;
    pub const CLOCK_GETTIME64: usize = 0;
    pub const NANOSLEEP: usize = 101;
    pub const CLOCK_NANOSLEEP64: usize = 0;
    pub const SOCKET: usize = 198;
    pub const CONNECT: usize = 203;
    pub const MKDIRAT: usize = 34;
}

// asm-generic table, 32-bit flavour. RV32 only has the 64-bit time calls.
#[cfg(any(target_arch = "riscv32", target_arch = "csky", target_arch = "hexagon"))]
mod sys {
    pub const READ: usize = 63;
    pub const WRITE: usize = 64;
    pub const OPENAT: usize = 56;
    pub const CLOSE: usize = 57;
    pub const IOCTL: usize = 29;
    pub const MMAP: usize = 222; // mmap2
    pub const EXIT_GROUP: usize = 94;
    #[cfg(target_arch = "riscv32")]
    pub const CLOCK_GETTIME: usize = 0;
    #[cfg(not(target_arch = "riscv32"))]
    pub const CLOCK_GETTIME: usize = 113;
    pub const CLOCK_GETTIME64: usize = 403;
    #[cfg(target_arch = "riscv32")]
    pub const NANOSLEEP: usize = 0;
    #[cfg(not(target_arch = "riscv32"))]
    pub const NANOSLEEP: usize = 101;
    pub const CLOCK_NANOSLEEP64: usize = 407;
    pub const SOCKET: usize = 198;
    pub const CONNECT: usize = 203;
    pub const MKDIRAT: usize = 34;
}

#[cfg(target_arch = "mips")]
mod sys {
    pub const READ: usize = 4003;
    pub const WRITE: usize = 4004;
    pub const OPENAT: usize = 4288;
    pub const CLOSE: usize = 4006;
    pub const IOCTL: usize = 4054;
    pub const MMAP: usize = 4210; // mmap2
    pub const EXIT_GROUP: usize = 4246;
    pub const CLOCK_GETTIME: usize = 4263;
    pub const CLOCK_GETTIME64: usize = 4403;
    pub const NANOSLEEP: usize = 4166;
    pub const CLOCK_NANOSLEEP64: usize = 4407;
    pub const SOCKET: usize = 4183;
    pub const CONNECT: usize = 4170;
    pub const MKDIRAT: usize = 4289;
}

#[cfg(target_arch = "mips64")]
mod sys {
    pub const READ: usize = 5000;
    pub const WRITE: usize = 5001;
    pub const OPENAT: usize = 5247;
    pub const CLOSE: usize = 5003;
    pub const IOCTL: usize = 5015;
    pub const MMAP: usize = 5009;
    pub const EXIT_GROUP: usize = 5205;
    pub const CLOCK_GETTIME: usize = 5222;
    pub const CLOCK_GETTIME64: usize = 0;
    pub const NANOSLEEP: usize = 5034;
    pub const CLOCK_NANOSLEEP64: usize = 0;
    pub const SOCKET: usize = 5040;
    pub const CONNECT: usize = 5041;
    pub const MKDIRAT: usize = 5248;
}

#[cfg(any(target_arch = "powerpc", target_arch = "powerpc64"))]
mod sys {
    pub const READ: usize = 3;
    pub const WRITE: usize = 4;
    pub const OPENAT: usize = 286;
    pub const CLOSE: usize = 6;
    pub const IOCTL: usize = 54;
    #[cfg(target_arch = "powerpc")]
    pub const MMAP: usize = 192; // mmap2
    #[cfg(target_arch = "powerpc64")]
    pub const MMAP: usize = 90;
    pub const EXIT_GROUP: usize = 234;
    pub const CLOCK_GETTIME: usize = 246;
    #[cfg(target_arch = "powerpc")]
    pub const CLOCK_GETTIME64: usize = 403;
    #[cfg(target_arch = "powerpc64")]
    pub const CLOCK_GETTIME64: usize = 0;
    pub const NANOSLEEP: usize = 162;
    #[cfg(target_arch = "powerpc")]
    pub const CLOCK_NANOSLEEP64: usize = 407;
    #[cfg(target_arch = "powerpc64")]
    pub const CLOCK_NANOSLEEP64: usize = 0;
    pub const SOCKET: usize = 326;
    pub const CONNECT: usize = 328;
    pub const MKDIRAT: usize = 287;
}

#[cfg(target_arch = "s390x")]
mod sys {
    pub const READ: usize = 3;
    pub const WRITE: usize = 4;
    pub const OPENAT: usize = 288;
    pub const CLOSE: usize = 6;
    pub const IOCTL: usize = 54;
    pub const MMAP: usize = 90; // old_mmap: takes a pointer to six longs
    pub const EXIT_GROUP: usize = 248;
    pub const CLOCK_GETTIME: usize = 260;
    pub const CLOCK_GETTIME64: usize = 0;
    pub const NANOSLEEP: usize = 162;
    pub const CLOCK_NANOSLEEP64: usize = 0;
    pub const SOCKET: usize = 359;
    pub const CONNECT: usize = 362;
    pub const MKDIRAT: usize = 289;
}

#[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
mod sys {
    pub const READ: usize = 3;
    pub const WRITE: usize = 4;
    pub const OPENAT: usize = 284;
    pub const CLOSE: usize = 6;
    pub const IOCTL: usize = 54;
    #[cfg(target_arch = "sparc")]
    pub const MMAP: usize = 56; // mmap2
    #[cfg(target_arch = "sparc64")]
    pub const MMAP: usize = 71;
    pub const EXIT_GROUP: usize = 188;
    pub const CLOCK_GETTIME: usize = 257;
    #[cfg(target_arch = "sparc")]
    pub const CLOCK_GETTIME64: usize = 403;
    #[cfg(target_arch = "sparc64")]
    pub const CLOCK_GETTIME64: usize = 0;
    pub const NANOSLEEP: usize = 249;
    #[cfg(target_arch = "sparc")]
    pub const CLOCK_NANOSLEEP64: usize = 407;
    #[cfg(target_arch = "sparc64")]
    pub const CLOCK_NANOSLEEP64: usize = 0;
    pub const SOCKET: usize = 97;
    pub const CONNECT: usize = 98;
    pub const MKDIRAT: usize = 285;
}

#[cfg(target_arch = "m68k")]
mod sys {
    pub const READ: usize = 3;
    pub const WRITE: usize = 4;
    pub const OPENAT: usize = 288;
    pub const CLOSE: usize = 6;
    pub const IOCTL: usize = 54;
    pub const MMAP: usize = 192; // mmap2
    pub const EXIT_GROUP: usize = 247;
    pub const CLOCK_GETTIME: usize = 260;
    pub const CLOCK_GETTIME64: usize = 403;
    pub const NANOSLEEP: usize = 162;
    pub const CLOCK_NANOSLEEP64: usize = 407;
    pub const SOCKET: usize = 356;
    pub const CONNECT: usize = 359;
    pub const MKDIRAT: usize = 289;
}

pub use sys::*;

/// Whether `MMAP` takes its six arguments through a pointer (s390x old_mmap).
pub const MMAP_VIA_STRUCT: bool = cfg!(target_arch = "s390x");

// ------------------------------------------------------------------ ABI constants

#[cfg(any(target_arch = "mips", target_arch = "mips64"))]
mod abi {
    pub const O_NONBLOCK: usize = 0x80;
    pub const O_CREAT: usize = 0x100;
    pub const O_TRUNC: usize = 0x200;
    pub const SOCK_STREAM: usize = 2;
    pub const ENOSYS: isize = 89;
    pub const TCGETS: usize = 0x540d;
    pub const TCSETS: usize = 0x540e;
    pub const TIOCGWINSZ: usize = 0x4008_7468;
    pub const FIONREAD: usize = 0x467f;
    pub const EVIOCGRAB: usize = 0x8004_4590;
}

#[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
mod abi {
    pub const O_NONBLOCK: usize = 0x4000;
    pub const O_CREAT: usize = 0x200;
    pub const O_TRUNC: usize = 0x400;
    pub const SOCK_STREAM: usize = 1;
    pub const ENOSYS: isize = 90;
    pub const TCGETS: usize = 0x4024_5408;
    pub const TCSETS: usize = 0x8024_5409;
    pub const TIOCGWINSZ: usize = 0x4008_7468;
    pub const FIONREAD: usize = 0x4004_667f;
    pub const EVIOCGRAB: usize = 0x8004_4590;
}

#[cfg(any(target_arch = "powerpc", target_arch = "powerpc64"))]
mod abi {
    pub const O_NONBLOCK: usize = 0x800;
    pub const O_CREAT: usize = 0x40;
    pub const O_TRUNC: usize = 0x200;
    pub const SOCK_STREAM: usize = 1;
    pub const ENOSYS: isize = 38;
    pub const TCGETS: usize = 0x402c_7413;
    pub const TCSETS: usize = 0x802c_7414;
    pub const TIOCGWINSZ: usize = 0x4008_7468;
    pub const FIONREAD: usize = 0x4004_667f;
    pub const EVIOCGRAB: usize = 0x8004_4590;
}

#[cfg(not(any(
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "sparc",
    target_arch = "sparc64",
    target_arch = "powerpc",
    target_arch = "powerpc64"
)))]
mod abi {
    pub const O_NONBLOCK: usize = 0x800;
    pub const O_CREAT: usize = 0x40;
    pub const O_TRUNC: usize = 0x200;
    pub const SOCK_STREAM: usize = 1;
    pub const ENOSYS: isize = 38;
    pub const TCGETS: usize = 0x5401;
    pub const TCSETS: usize = 0x5402;
    pub const TIOCGWINSZ: usize = 0x5413;
    pub const FIONREAD: usize = 0x541b;
    pub const EVIOCGRAB: usize = 0x4004_4590;
}

pub use abi::*;

/// termios layout: byte offset of `c_cc` and the VMIN / VTIME slots, plus
/// the local/input mode bits we clear to get a raw terminal.
#[cfg(any(target_arch = "powerpc", target_arch = "powerpc64"))]
pub mod termios {
    pub const CC_OFFSET: usize = 16;
    pub const VMIN: usize = 5;
    pub const VTIME: usize = 7;
    pub const ISIG: u32 = 0x80;
    pub const ICANON: u32 = 0x100;
    pub const ECHO: u32 = 0x8;
    pub const IEXTEN: u32 = 0x400;
    pub const IXON: u32 = 0x200;
}

#[cfg(any(target_arch = "mips", target_arch = "mips64"))]
pub mod termios {
    pub const CC_OFFSET: usize = 17;
    pub const VMIN: usize = 4;
    pub const VTIME: usize = 5;
    pub const ISIG: u32 = 0x1;
    pub const ICANON: u32 = 0x2;
    pub const ECHO: u32 = 0x8;
    pub const IEXTEN: u32 = 0x100;
    pub const IXON: u32 = 0x400;
}

#[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
pub mod termios {
    pub const CC_OFFSET: usize = 17;
    pub const VMIN: usize = 4;
    pub const VTIME: usize = 5;
    pub const ISIG: u32 = 0x1;
    pub const ICANON: u32 = 0x2;
    pub const ECHO: u32 = 0x8;
    pub const IEXTEN: u32 = 0x8000;
    pub const IXON: u32 = 0x400;
}

#[cfg(not(any(
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "sparc",
    target_arch = "sparc64",
    target_arch = "powerpc",
    target_arch = "powerpc64"
)))]
pub mod termios {
    pub const CC_OFFSET: usize = 17;
    pub const VMIN: usize = 6;
    pub const VTIME: usize = 5;
    pub const ISIG: u32 = 0x1;
    pub const ICANON: u32 = 0x2;
    pub const ECHO: u32 = 0x8;
    pub const IEXTEN: u32 = 0x8000;
    pub const IXON: u32 = 0x400;
}

// Shared by every architecture.
pub const O_RDONLY: usize = 0;
pub const O_WRONLY: usize = 1;
pub const O_RDWR: usize = 2;
pub const AT_FDCWD: isize = -100;
pub const EAGAIN: isize = 11;
pub const EINTR: isize = 4;
pub const EEXIST: isize = 17;
pub const AF_UNIX: usize = 1;
pub const AF_INET: usize = 2;
pub const PROT_READ: usize = 1;
pub const PROT_WRITE: usize = 2;
pub const MAP_SHARED: usize = 1;
pub const CLOCK_MONOTONIC: usize = 1;
pub const ICRNL: u32 = 0x100;
pub const BRKINT: u32 = 0x2;
pub const INPCK: u32 = 0x10;
pub const ISTRIP: u32 = 0x20;
pub const KDSETMODE: usize = 0x4b3a;
pub const KD_TEXT: usize = 0;
pub const KD_GRAPHICS: usize = 1;
pub const FBIOGET_VSCREENINFO: usize = 0x4600;
pub const FBIOGET_FSCREENINFO: usize = 0x4602;
