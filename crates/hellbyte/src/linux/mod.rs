//! Linux platform layer: no libc, just system calls.

#[cfg(raw_start)]
pub mod arch;
#[cfg(not(raw_start))]
pub mod arch_libc;
pub mod nr;

pub mod app;
pub mod fb;
pub mod term;
pub mod x11;

#[cfg(raw_start)]
use arch::syscall6;
#[cfg(not(raw_start))]
use arch_libc::syscall6;

// ------------------------------------------------------------------ raw wrappers

#[inline(always)]
pub fn sys(n: usize, a: usize, b: usize, c: usize) -> isize {
    unsafe { syscall6(n, a, b, c, 0, 0, 0) }
}

#[inline(always)]
pub fn sys6(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    unsafe { syscall6(n, a, b, c, d, e, f) }
}

/// Some targets (C-SKY) lower LLVM's trap to a call to C's `abort`.
#[cfg(raw_start)]
#[unsafe(no_mangle)]
pub extern "C" fn abort() -> ! {
    exit(134)
}

pub fn exit(code: i32) -> ! {
    sys(nr::EXIT_GROUP, code as usize, 0, 0);
    loop {}
}

pub fn write(fd: i32, buf: &[u8]) -> isize {
    sys(nr::WRITE, fd as usize, buf.as_ptr() as usize, buf.len())
}

/// Write the whole buffer, retrying on short writes and EINTR/EAGAIN.
pub fn write_all(fd: i32, mut buf: &[u8]) -> bool {
    while !buf.is_empty() {
        let n = write(fd, buf);
        if n > 0 {
            buf = &buf[n as usize..];
        } else if n == -nr::EINTR || n == -nr::EAGAIN {
            sleep_us(500);
        } else {
            return false;
        }
    }
    true
}

pub fn read(fd: i32, buf: &mut [u8]) -> isize {
    sys(nr::READ, fd as usize, buf.as_mut_ptr() as usize, buf.len())
}

pub fn close(fd: i32) {
    sys(nr::CLOSE, fd as usize, 0, 0);
}

pub fn ioctl(fd: i32, req: usize, arg: usize) -> isize {
    sys(nr::IOCTL, fd as usize, req, arg)
}

/// Open a NUL-terminated path. Returns the fd or a negative errno.
pub fn open(path: &[u8], flags: usize, mode: usize) -> i32 {
    debug_assert!(path.last() == Some(&0));
    sys6(nr::OPENAT, nr::AT_FDCWD as usize, path.as_ptr() as usize, flags, mode, 0, 0) as i32
}

pub fn mkdir(path: &[u8], mode: usize) -> isize {
    sys(nr::MKDIRAT, nr::AT_FDCWD as usize, path.as_ptr() as usize, mode)
}

pub fn mmap_shared(fd: i32, len: usize, prot: usize) -> *mut u8 {
    let r = if nr::MMAP_VIA_STRUCT {
        let args = [0usize, len, prot, nr::MAP_SHARED, fd as usize, 0];
        sys(nr::MMAP, args.as_ptr() as usize, 0, 0)
    } else {
        sys6(nr::MMAP, 0, len, prot, nr::MAP_SHARED, fd as usize, 0)
    };
    // Errors come back as small negative numbers.
    if (-4096..0).contains(&r) { core::ptr::null_mut() } else { r as *mut u8 }
}

pub fn socket(domain: usize, ty: usize) -> i32 {
    sys(nr::SOCKET, domain, ty, 0) as i32
}

pub fn connect(fd: i32, addr: &[u8]) -> isize {
    sys(nr::CONNECT, fd as usize, addr.as_ptr() as usize, addr.len())
}

/// Bytes waiting to be read on a socket/tty without blocking.
pub fn bytes_available(fd: i32) -> usize {
    let mut n: i32 = 0;
    let r = ioctl(fd, nr::FIONREAD, &mut n as *mut i32 as usize);
    if r < 0 || n < 0 { 0 } else { n as usize }
}

// ------------------------------------------------------------------ time

/// Monotonic clock in microseconds. Tries the 64-bit-time call first on
/// 32-bit targets (the only one available on RV32), then the legacy call.
pub fn now_us() -> u64 {
    if nr::CLOCK_GETTIME64 != 0 {
        let mut ts = [0i64; 2];
        if sys(nr::CLOCK_GETTIME64, nr::CLOCK_MONOTONIC, ts.as_mut_ptr() as usize, 0) == 0 {
            return ts[0] as u64 * 1_000_000 + ts[1] as u64 / 1000;
        }
    }
    if nr::CLOCK_GETTIME != 0 {
        // Native `long` pair: 32-bit on 32-bit targets, 64-bit on 64-bit.
        let mut ts = [0isize; 2];
        if sys(nr::CLOCK_GETTIME, nr::CLOCK_MONOTONIC, ts.as_mut_ptr() as usize, 0) == 0 {
            return ts[0] as u64 * 1_000_000 + ts[1] as u64 / 1000;
        }
    }
    0
}

pub fn sleep_us(us: u64) {
    if nr::CLOCK_NANOSLEEP64 != 0 {
        let ts = [(us / 1_000_000) as i64, ((us % 1_000_000) * 1000) as i64];
        let r = sys6(nr::CLOCK_NANOSLEEP64, nr::CLOCK_MONOTONIC, 0, ts.as_ptr() as usize, 0, 0, 0);
        if r != -nr::ENOSYS {
            return;
        }
    }
    if nr::NANOSLEEP != 0 {
        let ts = [(us / 1_000_000) as isize, ((us % 1_000_000) * 1000) as isize];
        sys(nr::NANOSLEEP, ts.as_ptr() as usize, 0, 0);
    }
}

// ------------------------------------------------------------------ process start

pub struct Env {
    pub argc: usize,
    pub argv: *const *const u8,
    pub envp: *const *const u8,
}

impl Env {
    pub fn arg(&self, i: usize) -> Option<&'static [u8]> {
        if i >= self.argc {
            return None;
        }
        unsafe { Some(cstr(*self.argv.add(i))) }
    }

    /// Look up an environment variable by name (without the `=`).
    pub fn var(&self, name: &[u8]) -> Option<&'static [u8]> {
        let mut p = self.envp;
        unsafe {
            while !(*p).is_null() {
                let kv = cstr(*p);
                if kv.len() > name.len() && kv[name.len()] == b'=' && &kv[..name.len()] == name {
                    return Some(&kv[name.len() + 1..]);
                }
                p = p.add(1);
            }
        }
        None
    }
}

/// # Safety
/// `p` must point to a NUL-terminated string that lives for the whole program.
pub unsafe fn cstr(p: *const u8) -> &'static [u8] {
    let mut n = 0;
    unsafe {
        while *p.add(n) != 0 {
            n += 1;
        }
        core::slice::from_raw_parts(p, n)
    }
}

/// Called from `_start` with the initial stack pointer:
/// `[argc][argv...][NULL][envp...][NULL][auxv...]`.
#[cfg(raw_start)]
pub unsafe extern "C" fn start_rust(sp: *const usize) -> ! {
    let env = unsafe {
        let argc = *sp;
        let argv = sp.add(1) as *const *const u8;
        let envp = argv.add(argc + 1);
        Env { argc, argv, envp }
    };
    let code = crate::main_linux(env);
    exit(code)
}
