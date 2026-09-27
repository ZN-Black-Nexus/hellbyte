//! Platform-independent frontend helpers: argument parsing and small utils.

pub mod args;
pub mod script;
pub mod tui;

/// Fixed-capacity byte string (paths, messages) — no heap anywhere.
pub struct Buf<const N: usize> {
    pub b: [u8; N],
    pub len: usize,
}

impl<const N: usize> Buf<N> {
    pub const fn new() -> Self {
        Buf { b: [0; N], len: 0 }
    }
    pub fn push(&mut self, s: &[u8]) -> &mut Self {
        for &c in s {
            if self.len + 1 < N {
                self.b[self.len] = c;
                self.len += 1;
            }
        }
        self
    }
    pub fn num(&mut self, v: u64) -> &mut Self {
        let mut t = [0u8; 20];
        let mut i = 20;
        let mut n = v;
        loop {
            i -= 1;
            t[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        self.push(&t[i..])
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.b[..self.len]
    }
    /// NUL-terminated view for system calls.
    pub fn cstr(&mut self) -> &[u8] {
        self.b[self.len.min(N - 1)] = 0;
        &self.b[..self.len + 1]
    }
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

pub fn parse_u32(s: &[u8]) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let mut v: u32 = 0;
    for &c in s {
        if !c.is_ascii_digit() {
            return None;
        }
        v = v.checked_mul(10)?.checked_add((c - b'0') as u32)?;
    }
    Some(v)
}

/// Parse "WxH".
pub fn parse_size(s: &[u8]) -> Option<(usize, usize)> {
    let x = s.iter().position(|&c| c == b'x' || c == b'X')?;
    Some((parse_u32(&s[..x])? as usize, parse_u32(&s[x + 1..])? as usize))
}
