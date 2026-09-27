//! Minimal streaming PNG writer (stored/uncompressed deflate). Shared by the
//! build script (texture previews) and the game (`--shot` screenshots).
//! No allocation: pixels are pulled row by row and bytes pushed to `out`.

fn crc_update(mut crc: u32, bytes: &[u8]) -> u32 {
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { 0xedb8_8320 ^ (crc >> 1) } else { crc >> 1 };
        }
    }
    crc
}

struct Chunk<'a, F: FnMut(&[u8])> {
    out: &'a mut F,
    crc: u32,
}

impl<F: FnMut(&[u8])> Chunk<'_, F> {
    fn put(&mut self, b: &[u8]) {
        self.crc = crc_update(self.crc, b);
        (self.out)(b);
    }
}

/// Writes a `w` x `h` RGB PNG. `rgb(x, y)` returns each pixel.
pub fn write_png(w: usize, h: usize, rgb: &mut dyn FnMut(usize, usize) -> [u8; 3], out: &mut dyn FnMut(&[u8])) {
    let mut out = out;
    out(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = [0u8; 13];
    ihdr[0..4].copy_from_slice(&(w as u32).to_be_bytes());
    ihdr[4..8].copy_from_slice(&(h as u32).to_be_bytes());
    ihdr[8] = 8; // bit depth
    ihdr[9] = 2; // RGB
    out(&13u32.to_be_bytes());
    let mut c = Chunk { out: &mut out, crc: 0xffff_ffff };
    c.put(b"IHDR");
    c.put(&ihdr);
    let crc = !c.crc;
    out(&crc.to_be_bytes());

    let raw = h * (1 + w * 3);
    let blocks = raw.div_ceil(65535).max(1);
    let idat_len = 2 + blocks * 5 + raw + 4;
    out(&(idat_len as u32).to_be_bytes());
    let mut c = Chunk { out: &mut out, crc: 0xffff_ffff };
    c.put(b"IDAT");
    c.put(&[0x78, 0x01]);
    let (mut a, mut b) = (1u32, 0u32);
    let mut left_in_block = 0usize;
    let mut remaining = raw;
    let mut emit = |c: &mut Chunk<_>, byte: u8, left: &mut usize, rem: &mut usize| {
        if *left == 0 {
            let n = (*rem).min(65535);
            let last = if n == *rem { 1 } else { 0 };
            c.put(&[last, n as u8, (n >> 8) as u8, !n as u8, (!n >> 8) as u8]);
            *left = n;
        }
        c.put(&[byte]);
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
        *left -= 1;
        *rem -= 1;
    };
    for y in 0..h {
        emit(&mut c, 0, &mut left_in_block, &mut remaining);
        for x in 0..w {
            let p = rgb(x, y);
            for v in p {
                emit(&mut c, v, &mut left_in_block, &mut remaining);
            }
        }
    }
    c.put(&((b << 16) | a).to_be_bytes());
    let crc = !c.crc;
    out(&crc.to_be_bytes());
    out(&0u32.to_be_bytes());
    let mut c = Chunk { out: &mut out, crc: 0xffff_ffff };
    c.put(b"IEND");
    let crc = !c.crc;
    out(&crc.to_be_bytes());
}
