//! Development builds started by the system C runtime (`cargo run` on a Linux
//! host with the default linker). System calls go through libc's `syscall()`
//! so this works on any architecture without our hand-written entry code.
unsafe extern "C" {
    fn syscall(n: isize, ...) -> isize;
    fn __errno_location() -> *mut i32;
}

#[inline(always)]
pub unsafe fn syscall6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    unsafe {
        let r = syscall(n as isize, a, b, c, d, e, f);
        if r == -1 { -(*__errno_location() as isize) } else { r }
    }
}
