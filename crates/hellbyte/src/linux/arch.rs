//! Per-architecture process entry point and raw system call instruction.
//!
//! Every function here is the only place that knows how a given CPU talks to
//! the Linux kernel. `syscall6` always passes six arguments (the kernel ignores
//! the unused ones) and normalises the result so that errors are returned as
//! `-errno` on every architecture, including the ones (MIPS, PowerPC, SPARC)
//! that signal errors out-of-band.

#![allow(clippy::missing_safety_doc)]

use core::arch::{asm, naked_asm};

use super::start_rust;

// ---------------------------------------------------------------- x86_64
#[cfg(target_arch = "x86_64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "xor ebp, ebp",
        "mov rdi, rsp",
        "and rsp, -16",
        "call {m}",
        "ud2",
        m = sym start_rust
    )
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("syscall",
            inlateout("rax") n as isize => r,
            in("rdi") a, in("rsi") b, in("rdx") c, in("r10") d, in("r8") e, in("r9") f,
            lateout("rcx") _, lateout("r11") _,
            options(nostack));
    }
    r
}

// ---------------------------------------------------------------- x86 (i386 .. i686)
#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "xor ebp, ebp",
        "mov eax, esp",
        "and esp, -16",
        "sub esp, 12",
        "push eax",
        "call {m}",
        "ud2",
        m = sym start_rust
    )
}

#[cfg(target_arch = "x86")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    // esi and ebp are reserved by LLVM, so arguments 4..6 travel through
    // memory and are loaded by hand around the trap.
    let rest = [d, e, f];
    let r: isize;
    unsafe {
        asm!(
            "push ebp",
            "push esi",
            "mov esi, [edi]",
            "mov ebp, [edi + 8]",
            "mov edi, [edi + 4]",
            "int 0x80",
            "pop esi",
            "pop ebp",
            inlateout("eax") n as isize => r,
            in("ebx") a, in("ecx") b, in("edx") c,
            inlateout("edi") rest.as_ptr() => _,
        );
    }
    r
}

// ---------------------------------------------------------------- 32-bit ARM (EABI, ARMv4T .. ARMv7)
#[cfg(target_arch = "arm")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "mov fp, #0",
        "mov lr, #0",
        "mov r0, sp",
        "bic sp, sp, #15",
        "bl {m}",
        "0: b 0b",
        m = sym start_rust
    )
}

#[cfg(target_arch = "arm")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("svc 0",
            in("r7") n,
            inlateout("r0") a as isize => r,
            in("r1") b, in("r2") c, in("r3") d, in("r4") e, in("r5") f,
            options(nostack));
    }
    r
}

// ---------------------------------------------------------------- AArch64 (LE and BE)
#[cfg(target_arch = "aarch64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "mov x29, xzr",
        "mov x30, xzr",
        "mov x0, sp",
        "bl {m}",
        "brk #0",
        m = sym start_rust
    )
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("svc 0",
            in("x8") n,
            inlateout("x0") a as isize => r,
            in("x1") b, in("x2") c, in("x3") d, in("x4") e, in("x5") f,
            options(nostack));
    }
    r
}

// ---------------------------------------------------------------- RISC-V (RV32 and RV64)
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        ".option push",
        ".option norelax",
        "lla gp, __global_pointer$",
        ".option pop",
        "mv a0, sp",
        "andi sp, sp, -16",
        "call {m}",
        "unimp",
        m = sym start_rust
    )
}

#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("ecall",
            in("a7") n,
            inlateout("a0") a as isize => r,
            in("a1") b, in("a2") c, in("a3") d, in("a4") e, in("a5") f,
            options(nostack));
    }
    r
}

// ---------------------------------------------------------------- LoongArch64
#[cfg(target_arch = "loongarch64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "move $fp, $zero",
        "move $a0, $sp",
        "bstrins.d $sp, $zero, 3, 0",
        "bl {m}",
        "break 0",
        m = sym start_rust
    )
}

#[cfg(target_arch = "loongarch64")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("syscall 0",
            in("$a7") n,
            inlateout("$a0") a as isize => r,
            in("$a1") b, in("$a2") c, in("$a3") d, in("$a4") e, in("$a5") f,
            lateout("$t0") _, lateout("$t1") _, lateout("$t2") _, lateout("$t3") _,
            lateout("$t4") _, lateout("$t5") _, lateout("$t6") _, lateout("$t7") _, lateout("$t8") _,
            options(nostack));
    }
    r
}

// ---------------------------------------------------------------- MIPS o32 (mips, mipsel, r6)
#[cfg(target_arch = "mips")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        ".set push",
        ".set noreorder",
        "move $4, $29",
        "addiu $29, $29, -32",
        "li $8, -16",
        "and $29, $29, $8",
        "la $25, {m}",
        "jalr $25",
        "nop",
        "break",
        ".set pop",
        m = sym start_rust
    )
}

#[cfg(target_arch = "mips")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    // o32 passes arguments 5 and 6 on the stack, above the 16-byte home area.
    let r: isize;
    let err: usize;
    unsafe {
        asm!(
            "addiu $29, $29, -32",
            "sw $16, 16($29)",
            "sw $17, 20($29)",
            "syscall",
            "addiu $29, $29, 32",
            inlateout("$2") n as isize => r,
            in("$4") a, in("$5") b, in("$6") c,
            inlateout("$7") d => err,
            in("$16") e, in("$17") f,
            lateout("$3") _, lateout("$8") _, lateout("$9") _, lateout("$10") _, lateout("$11") _,
            lateout("$12") _, lateout("$13") _, lateout("$14") _, lateout("$15") _,
            lateout("$24") _, lateout("$25") _,
        );
    }
    if err != 0 { -r } else { r }
}

// ---------------------------------------------------------------- MIPS n64 (mips64, mips64el, r6)
#[cfg(target_arch = "mips64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        ".set push",
        ".set noreorder",
        "move $4, $29",
        "daddiu $29, $29, -32",
        "li $8, -16",
        "and $29, $29, $8",
        "dla $25, {m}",
        "jalr $25",
        "nop",
        "break",
        ".set pop",
        m = sym start_rust
    )
}

#[cfg(target_arch = "mips64")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    let err: usize;
    unsafe {
        asm!("syscall",
            inlateout("$2") n as isize => r,
            in("$4") a, in("$5") b, in("$6") c,
            inlateout("$7") d => err,
            in("$8") e, in("$9") f,
            lateout("$3") _, lateout("$10") _, lateout("$11") _,
            lateout("$12") _, lateout("$13") _, lateout("$14") _, lateout("$15") _,
            lateout("$24") _, lateout("$25") _,
            options(nostack));
    }
    if err != 0 { -r } else { r }
}

// ---------------------------------------------------------------- PowerPC 32-bit
#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "mr 3, 1",
        "clrrwi 1, 1, 4",
        "li 0, 0",
        "stwu 1, -16(1)",
        "stw 0, 0(1)",
        "bl {m}",
        "trap",
        m = sym start_rust
    )
}

// ---------------------------------------------------------------- PowerPC 64-bit, ELFv2 (BE musl and LE)
#[cfg(target_arch = "powerpc64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "bl 1f",
        "1:",
        "mflr 12",
        "addis 2, 12, .TOC.-1b@ha",
        "addi 2, 2, .TOC.-1b@l",
        "mr 3, 1",
        "clrrdi 1, 1, 4",
        "li 0, 0",
        "stdu 0, -64(1)",
        "bl {m}",
        "nop",
        "trap",
        m = sym start_rust
    )
}

#[cfg(any(target_arch = "powerpc", target_arch = "powerpc64"))]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    let cr: usize;
    unsafe {
        asm!("sc", "mfcr {cr}",
            cr = out(reg) cr,
            inlateout("r0") n => _,
            inlateout("r3") a as isize => r,
            inlateout("r4") b => _, inlateout("r5") c => _, inlateout("r6") d => _,
            inlateout("r7") e => _, inlateout("r8") f => _,
            lateout("r9") _, lateout("r10") _, lateout("r11") _, lateout("r12") _,
            lateout("cr0") _, lateout("ctr") _, lateout("xer") _,
            options(nostack));
    }
    // CR0.SO flags an error; r3 then holds a positive errno.
    if cr & 0x1000_0000 != 0 { -r } else { r }
}

// ---------------------------------------------------------------- IBM Z (s390x)
#[cfg(target_arch = "s390x")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "lgr %r2, %r15",
        "aghi %r15, -160",
        "lghi %r0, 0",
        "stg %r0, 0(%r15)",
        "brasl %r14, {m}",
        ".word 0",
        m = sym start_rust
    )
}

#[cfg(target_arch = "s390x")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("svc 0",
            in("r1") n,
            inlateout("r2") a as isize => r,
            in("r3") b, in("r4") c, in("r5") d, in("r6") e, in("r7") f,
            options(nostack));
    }
    r
}

// ---------------------------------------------------------------- SPARC (32-bit v8+) and SPARC64
#[cfg(target_arch = "sparc64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    // The 64-bit ABI biases %sp by 2047; argc sits after the 128-byte window save area.
    naked_asm!(
        "mov %g0, %fp",
        "add %sp, 2175, %o0",
        "call {m}",
        "nop",
        "illtrap 0",
        m = sym start_rust
    )
}

#[cfg(target_arch = "sparc")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "mov %g0, %fp",
        "add %sp, 64, %o0",
        "call {m}",
        "nop",
        "unimp 0",
        m = sym start_rust
    )
}

#[cfg(target_arch = "sparc64")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("mov {n}, %g1",
            "ta 0x6d",
            "bcc,pt %xcc, 1f",
            "nop",
            "sub %g0, %o0, %o0",
            "1:",
            n = in(reg) n,
            inlateout("o0") a as isize => r,
            in("o1") b, in("o2") c, in("o3") d, in("o4") e, in("o5") f,
            options(nostack));
    }
    r
}

#[cfg(target_arch = "sparc")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("mov {n}, %g1",
            "ta 0x10",
            "bcc 1f",
            "nop",
            "sub %g0, %o0, %o0",
            "1:",
            n = in(reg) n,
            inlateout("o0") a as isize => r,
            in("o1") b, in("o2") c, in("o3") d, in("o4") e, in("o5") f,
            options(nostack));
    }
    r
}

// ---------------------------------------------------------------- Motorola 68k
#[cfg(target_arch = "m68k")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "move.l %sp, %a0",
        "move.l %a0, -(%sp)",
        "jsr {m}",
        "illegal",
        m = sym start_rust
    )
}

#[cfg(target_arch = "m68k")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("trap #0",
            inlateout("d0") n as isize => r,
            in("d1") a, in("d2") b, in("d3") c, in("d4") d, in("d5") e, in("a0") f,
            options(nostack));
    }
    r
}

// ---------------------------------------------------------------- C-SKY (abiv2)
#[cfg(target_arch = "csky")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "mov16 a0, sp",
        "jbsr {m}",
        "bkpt",
        m = sym start_rust
    )
}

#[cfg(target_arch = "csky")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("mov16 r13, r7",
            "mov16 r7, r12",
            "trap32 0",
            "mov16 r7, r13",
            in("r12") n,
            out("r13") _,
            inlateout("r0") a as isize => r,
            in("r1") b, in("r2") c, in("r3") d, in("r4") e, in("r5") f,
            options(nostack));
    }
    r
}

// ---------------------------------------------------------------- Qualcomm Hexagon
#[cfg(target_arch = "hexagon")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "r0 = r29",
        "r29 = and(r29, #-8)",
        "call {m}",
        m = sym start_rust
    )
}

#[cfg(target_arch = "hexagon")]
#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("trap0(#1)",
            in("r6") n,
            inlateout("r0") a as isize => r,
            in("r1") b, in("r2") c, in("r3") d, in("r4") e, in("r5") f,
            options(nostack));
    }
    r
}
