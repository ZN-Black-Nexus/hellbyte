//! Generates, at compile time, everything the game would normally load from
//! data files: trig tables, the palette and light maps, all textures, and
//! every level in `levels/` (compiled to BSP form by `hellbyte-mapc`).
//! Set HELLBYTE_DUMP=<dir> to also write PNG previews of the textures.

#[path = "build/palette.rs"]
mod palette;
#[path = "src/png.rs"]
#[allow(dead_code)]
mod png;
#[path = "build/tables.rs"]
mod tables;
#[path = "build/textures.rs"]
mod textures;

use std::env;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let levels_dir = manifest.join("../../levels");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build");
    println!("cargo:rerun-if-changed={}", levels_dir.display());
    println!("cargo:rerun-if-env-changed=HELLBYTE_DUMP");

    let mut code = String::new();
    tables::emit(&mut code);
    let pal = palette::build();
    palette::emit(&mut code, &pal);

    let set = textures::build();
    let (mut wbin, mut fbin) = (vec![], vec![]);
    textures::emit(&set, &mut code, &mut wbin, &mut fbin);
    fs::write(out_dir.join("walls.bin"), &wbin).unwrap();
    fs::write(out_dir.join("flats.bin"), &fbin).unwrap();
    code.push_str("pub static WALL_PIXELS: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/walls.bin\"));\n");
    code.push_str("pub static FLAT_PIXELS: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/flats.bin\"));\n");

    // ---- levels
    let wall = |n: &str| set.walls.iter().position(|(w, _)| w == n).map(|i| (i + 1) as u8);
    let flat = |n: &str| set.flats.iter().position(|(f, _)| f == n).map(|i| i as u8);
    let sky_flat = flat("SKY").unwrap();
    let tex = hellbyte_mapc::Textures { wall: &wall, flat: &flat, sky_flat };
    let mut files: Vec<PathBuf> = fs::read_dir(&levels_dir)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "lvl")).collect())
        .unwrap_or_default();
    files.sort();
    let mut maps = String::new();
    let mut idents = vec![];
    let mut report = String::new();
    let mut max = [1usize; 4];
    for f in &files {
        let src = fs::read_to_string(f).unwrap();
        match hellbyte_mapc::compile(&src, &tex) {
            Ok(l) => {
                maps.push_str(&l.code);
                writeln!(report, "{}", l.stats).unwrap();
                for (m, v) in max.iter_mut().zip([l.sectors, l.lines, l.sides, l.blocks]) {
                    *m = (*m).max(v);
                }
                idents.push(l.ident);
            }
            Err(e) => panic!("level {}: {}", f.display(), e),
        }
    }
    if idents.is_empty() {
        panic!("no levels found in {}", levels_dir.display());
    }
    write!(maps, "pub static MAPS: &[&MapData] = &[").unwrap();
    for i in &idents {
        write!(maps, "&MAP_{i},").unwrap();
    }
    maps.push_str("];\n");
    fs::write(
        out_dir.join("limits.rs"),
        format!(
            "pub const MAX_SECTORS: usize = {};\npub const MAX_LINES: usize = {};\npub const MAX_SIDES: usize = {};\npub const MAX_BLOCKS: usize = {};\n",
            max[0], max[1], max[2], max[3]
        ),
    )
    .unwrap();
    fs::write(out_dir.join("generated.rs"), code).unwrap();
    fs::write(out_dir.join("maps.rs"), maps).unwrap();
    fs::write(out_dir.join("levels.txt"), report).unwrap();

    if let Ok(dir) = env::var("HELLBYTE_DUMP") {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir).unwrap();
        let sheet = |items: &[(String, textures::Img)], name: &str| {
            let cols = 8;
            let cell = 132;
            let rows = items.len().div_ceil(cols);
            let (w, h) = (cols * cell, rows * cell);
            let mut buf = vec![[40u8, 40, 48]; w * h];
            for (i, (_, im)) in items.iter().enumerate() {
                let (ox, oy) = ((i % cols) * cell + 2, (i / cols) * cell + 2);
                for y in 0..im.h.min(128) {
                    for x in 0..im.w.min(128) {
                        buf[(oy + y) * w + ox + x] = pal[im.px[y * im.w + x] as usize];
                    }
                }
            }
            let mut bytes = vec![];
            png::write_png(w, h, &mut |x, y| buf[y * w + x], &mut |b| bytes.extend_from_slice(b));
            fs::write(dir.join(name), bytes).unwrap();
        };
        sheet(&set.walls, "walls.png");
        sheet(&set.flats, "flats.png");
        let mut bytes = vec![];
        png::write_png(16 * 16, 16 * 16, &mut |x, y| pal[(y / 16) * 16 + x / 16], &mut |b| bytes.extend_from_slice(b));
        fs::write(dir.join("palette.png"), bytes).unwrap();
    }
}
